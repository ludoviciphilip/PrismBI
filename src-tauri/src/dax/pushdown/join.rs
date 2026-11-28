//! Join planning: builds the two-table `FROM` (base + at most one more,
//! "extra") round 1 supports, choosing the join *direction* from which
//! table hosts the category column, not always from the fact table.
//!
//! This matters for blank-member parity: if the category is a dimension
//! column and we joined `FROM fact LEFT JOIN dim`, a fact row whose FK
//! doesn't match any dimension row would surface as a synthetic
//! `NULL`-keyed group — a "blank member" category the DAX engine's
//! `collect_sorted_entries` never produces for this visual shape (it
//! enumerates the *dimension's own rows*, not the fact's FK values; an
//! orphan fact row simply never gets counted under any category, it isn't
//! shown as one). So instead the category's own table is always the `b0`
//! (driving) side: `FROM dim AS b0 LEFT JOIN fact AS b1 ON ...` — every
//! dimension row is a candidate group (dropped later only if every
//! aggregate on it is NULL, the same rule the DAX path uses), and unmatched
//! fact rows are simply never joined to anything, exactly like today.
//!
//! When the category is on the fact table itself (or there's no category at
//! all — card/kpi/multi-card), `b0` just stays the fact table and a needed
//! dimension (for a filter or a measure's `CALCULATE` filter) becomes `b1`,
//! joined the usual `fact -> dim` direction.
//!
//! Only one `b1` table total is allowed (round 1: one dimension axis per
//! visual, see `dax::pushdown` module doc); a second distinct table rejects.

use super::super::duckdb_load::quote_ident;
use super::super::{DaxModel, DaxRelationshipInput};

pub(super) struct Joiner<'m> {
    model: &'m DaxModel,
    fact_key: String,
    pub(super) fact_duckdb: String,
    base_key: String,
    pub(super) base_duckdb: String,
    extra: Option<(String, String)>,
    join_sql: Option<String>,
    /// v0.11 Frente H (items c/d/e): set by `set_base_table` to the fact-side
    /// join column (`b1.<fk>`) when the category is a dimension column (base
    /// re-based, fact joined in as `b1`). `NULL` there means "no fact row
    /// joined to this dimension row" — the LEFT JOIN produced a row anyway,
    /// so `COUNT(*)` can't tell a real group from a phantom one; this marker
    /// can. `None` when there's no such ambiguity (base stayed the fact
    /// table), in which case callers use a `COUNT(*)`-equivalent instead.
    dim_join_presence: Option<String>,
}

impl<'m> Joiner<'m> {
    pub(super) fn new(model: &'m DaxModel, fact_key: String, fact_duckdb: String) -> Self {
        Joiner {
            model,
            base_key: fact_key.clone(),
            base_duckdb: fact_duckdb.clone(),
            fact_key,
            fact_duckdb,
            extra: None,
            join_sql: None,
            dim_join_presence: None,
        }
    }

    pub(super) fn fact_key_for_measures(&self) -> String {
        self.fact_key.clone()
    }

    /// v0.11 Frente H (items c/d/e): `"1"` when there's no fact join (base
    /// is still the fact table, or no join happened at all) — a plain
    /// `COUNT(CASE WHEN cond THEN 1 END)` already tells "no rows" from
    /// "some rows" correctly in that case. Otherwise the fact-side join
    /// column set by `set_base_table`, which is `NULL` exactly when the
    /// `LEFT JOIN` found no matching fact row for this dimension row.
    pub(super) fn row_presence_sql(&self) -> String {
        self.dim_join_presence.clone().unwrap_or_else(|| "1".to_string())
    }

    pub(super) fn join_sql(&self) -> String {
        self.join_sql.as_ref().map(|j| format!(" {}", j)).unwrap_or_default()
    }

    /// Re-bases the driving (`b0`) table onto `table_key` when it differs
    /// from the fact table — call this once, before any `col_ref`, as soon
    /// as the category's table is known. A no-op when the category is on
    /// the fact table itself.
    pub(super) fn set_base_table(&mut self, table_key: &str) -> Result<(), String> {
        if table_key == self.fact_key {
            return Ok(());
        }
        if self.extra.is_some() {
            return Err("pushdown round 1: categoria em uma 2ª tabela depois de um filtro já ter fixado outra".to_string());
        }
        let rel = self.find_rel(&self.fact_key, table_key).ok_or_else(|| {
            format!(
                "sem relacionamento ativo e unidirecional de '{}' para '{}' (categoria)",
                self.fact_key, table_key
            )
        })?;
        let dim_duckdb = self
            .model
            .duckdb_table_name(table_key)
            .ok_or_else(|| format!("tabela '{}' sem tabela DuckDB", table_key))?
            .to_string();
        // v0.11 Frente H (item b): `IS NOT DISTINCT FROM` instead of `=` so a
        // genuinely BLANK (NULL) FK matches a genuinely BLANK dimension key,
        // the same way the DAX evaluator's relationship propagation does
        // (`ValueKey::Blank` is a normal, matchable key there, not excluded).
        self.join_sql = Some(format!(
            "LEFT JOIN {} AS b1 ON b1.{} IS NOT DISTINCT FROM b0.{}",
            quote_ident(&self.fact_duckdb),
            quote_ident(&rel.from_column),
            quote_ident(&rel.to_column),
        ));
        self.base_key = table_key.to_string();
        self.base_duckdb = dim_duckdb;
        self.extra = Some((self.fact_key.clone(), "b1".to_string()));
        self.dim_join_presence = Some(format!("b1.{}", quote_ident(&rel.from_column)));
        Ok(())
    }

    /// SQL for `table_key.column`, resolving which alias (`b0`/`b1`) hosts
    /// it, joining the second table in on first use when the base is still
    /// the fact table (fact -> dimension direction).
    pub(super) fn col_ref(&mut self, table_key: &str, column: &str) -> Result<String, String> {
        if table_key == self.base_key {
            return Ok(format!("b0.{}", quote_ident(column)));
        }
        if let Some((ek, ea)) = self.extra.clone() {
            if table_key == ek {
                return Ok(format!("{}.{}", ea, quote_ident(column)));
            }
            return Err(format!(
                "tabela '{}' não suportada no pushdown junto de '{}'/'{}' (round 1 só junta 2 tabelas)",
                table_key, self.base_key, ek
            ));
        }
        if self.base_key != self.fact_key {
            // Base was re-based to a dimension (category); the fact table is
            // already the reserved `extra` slot, so any other table has no
            // room left.
            return Err(format!("tabela '{}' não alcançável no pushdown (round 1)", table_key));
        }
        let rel = self.find_rel(&self.fact_key, table_key).ok_or_else(|| {
            format!("sem relacionamento ativo e unidirecional de '{}' para '{}'", self.fact_key, table_key)
        })?;
        let dim_duckdb = self
            .model
            .duckdb_table_name(table_key)
            .ok_or_else(|| format!("tabela '{}' sem tabela DuckDB", table_key))?
            .to_string();
        self.join_sql = Some(format!(
            "LEFT JOIN {} AS b1 ON b0.{} IS NOT DISTINCT FROM b1.{}",
            quote_ident(&dim_duckdb),
            quote_ident(&rel.from_column),
            quote_ident(&rel.to_column),
        ));
        self.extra = Some((table_key.to_string(), "b1".to_string()));
        Ok(format!("b1.{}", quote_ident(column)))
    }

    /// The single active, non-bidirectional relationship from `from_key` to
    /// `to_key`. Inactive (`is_active == false`) and bidirectional
    /// (`cross_filter_direction == "both"`) relationships are deliberately
    /// excluded, not just skipped: the Frente A differential test requires
    /// both to fall back to the DAX engine rather than being approximated
    /// as a plain one-way join.
    fn find_rel(&self, from_key: &str, to_key: &str) -> Option<&'m DaxRelationshipInput> {
        self.model.relationships.iter().find(|r| {
            r.is_active
                && !r.cross_filter_direction.eq_ignore_ascii_case("both")
                && r.from_table.to_lowercase() == from_key
                && r.to_table.to_lowercase() == to_key
        })
    }
}
