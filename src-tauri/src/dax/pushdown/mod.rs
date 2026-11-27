//! v0.11 Frente A: SQL pushdown for `query_planner::execute_visual_query`.
//!
//! ## Why
//! `duckdb_load::load_table_from_duckdb` pulls *every row* of every DuckDB
//! table into the `DaxModel` (`Vec<Vec<DaxValue>>`), and `query_planner`
//! aggregates in Rust by scanning `get_visible_row_indices` per category.
//! That's O(rows) per visual and keeps the whole table resident in RAM —
//! fine at thousands of rows, not at 10M (`docs/V011.md` Frente A: SUM by
//! category over 10M rows in < 200ms, memory that doesn't grow with N).
//!
//! DuckDB already has the rows, columnar, with real aggregation and join
//! operators. For the *subset* of visual requests whose semantics map
//! directly onto `SELECT ... FROM fact LEFT JOIN dim ... WHERE ... GROUP BY
//! ...`, this module generates that SQL and lets DuckDB do the work instead
//! of ever materializing the fact table in the DAX model's row format.
//!
//! ## Design
//! `try_pushdown` is the single entry point, called from
//! `query_planner::execute_visual_query_with_conn` *before* the normal
//! DAX-engine path. It returns `Ok(result)` on success (with
//! `result.engine == "duckdb"`) or `Err(reason)` the moment the request (or
//! any measure/filter inside it) uses something outside the supported
//! subset — the caller then falls through to the unchanged DAX engine and
//! logs `reason` to stderr. **Nothing here ever approximates**: a shape we
//! don't fully understand is a hard reject, never a partial/best-effort SQL
//! query. That is what the differential test in `tests_differential.rs`
//! exists to police.
//!
//! Supported subset (round 1):
//! - Visual shapes: card/kpi/gauge (single value), multi-card and any
//!   x-axis-less visual with only y-fields (same gate `query_planner` itself
//!   uses for its "multi-card" branch), and single-category grouped charts
//!   (one `x_axis_field`, no `legend_field`). `category_levels`,
//!   `matrix_rows`, `drill_filters` and the `slicer` visual stay on the DAX
//!   path (drill/levels/matrix need per-node subtotals of *non-additive*
//!   measures, which isn't a single GROUP BY; slicer already short-circuits
//!   cheaply in Rust).
//! - Grouping: a single dimension column reached from the request's base
//!   table by **at most one hop** across an **active**, **non-bidirectional**
//!   relationship (`cross_filter_direction != "both"`), or a column on the
//!   base table itself. Blank/orphan members (`LEFT JOIN`, unmatched or NULL
//!   key) are kept and labelled "Sem dados", matching
//!   `query_planner::collect_sorted_entries(..., keep_blank_as_sem_dados: true)`.
//!   Sort order is `(col IS NULL) DESC, col ASC` — i.e. blank first, then
//!   ascending — which matches `ValueKey`'s derived `Ord` (`Blank` sorts
//!   before every other variant) for a single-typed column. Columns with a
//!   `sortByColumn` override, or of `Date`/`Currency` type, reject (round 1
//!   doesn't special-case fiscal date ordering or the ×10 000 currency
//!   scale in SQL).
//! - Values: a plain fact column with `aggregation` in
//!   {SUM, COUNT, COUNTA, COUNTROWS, DISTINCTCOUNT, MIN, MAX, AVERAGE/AVG},
//!   or a *simple* measure whose formula is exactly one of those aggregation
//!   calls, optionally wrapped in `CALCULATE(<agg>, <simple filter>, ...)`
//!   where each filter is `Table[Col] = literal`, a comparison
//!   (`<`,`<=`,`>`,`>=`,`<>`) against a literal, or `Table[Col] IN {...}`.
//!   CALCULATE's filters are translated as a `CASE WHEN <cond> THEN col END`
//!   wrapped around *that one* aggregate (not a shared `WHERE`), so several
//!   y-fields with different CALCULATE filters in the same visual still get
//!   each its own, correctly isolated, filter — exactly like DAX semantics
//!   (CALCULATE only restricts its own expression, not sibling measures).
//! - Filters: `NativeVisualQueryRequest::filters` (value-list / IN) on any
//!   column reachable the same way as the grouping column.
//! - Row literal types: `Text`, `Integer`, `Decimal`, `Boolean` only (no
//!   `Date`/`Currency` — the DAX/Excel serial<->SQL DATE conversion and the
//!   ×10 000 currency scale are both real but not needed for the
//!   "SUM by category" scale target, so round 1 just rejects and falls back).
//!
//! Everything else (CALCULATE with a measure filter, FILTER(), legend axis,
//! matrix/levels, `USERELATIONSHIP`, inactive or bidirectional relationships,
//! Date/Currency grouping or filtering, measures referencing other measures,
//! ...) rejects with a specific reason and the DAX engine runs unchanged —
//! this is intentionally a thin, growable slice, not a general DAX-to-SQL
//! compiler.
//!
//! ## Flag
//! `VETROBI_PUSHDOWN=0` disables this module entirely (see
//! `pushdown_enabled`); every other value (including unset) enables it.

mod join;
mod literal;
mod translate;

#[cfg(test)]
mod tests_differential;

#[cfg(test)]
#[path = "bench.rs"]
mod bench_tests;

use super::query_planner::{
    resolve_table_and_col, NativeVisualQueryRequest, NativeVisualQueryResult, NativeVisualSeries,
};
use super::{ColumnType, DaxModel, NativeFilterInput};
use duckdb::Connection;
use join::Joiner;
use std::time::Instant;
use translate::{render_agg, resolve_y_agg, YAgg};

/// Reads `VETROBI_PUSHDOWN` once per call (cheap env lookup; call sites are
/// per-visual-query, not per-row). `"0"` disables pushdown; anything else
/// (including unset) leaves it on.
pub fn pushdown_enabled() -> bool {
    std::env::var("VETROBI_PUSHDOWN").map(|v| v != "0").unwrap_or(true)
}

fn elapsed_ms(t0: Instant) -> f64 {
    (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0
}

/// True for visual shapes `query_planner::execute_visual_query` treats as
/// "no grouping, just aggregate(s) over the filtered table" (its branches 1
/// and 2). Mirrored here so pushdown and the DAX path agree on which shape a
/// request is before either one looks at `x_axis_field`/`category_levels`.
fn is_single_value_shape(req: &NativeVisualQueryRequest) -> bool {
    matches!(req.visual_type.as_str(), "card" | "kpi" | "gauge")
        || req.visual_type == "multi-card"
        || (req.x_axis_field.is_none()
            && !req.y_axis_fields.is_empty()
            && !matches!(
                req.visual_type.as_str(),
                "slicer" | "table" | "action-button" | "textbox"
            ))
}

pub fn try_pushdown(
    model: &DaxModel,
    conn: &Connection,
    req: &NativeVisualQueryRequest,
    agg: &str,
    t0: Instant,
) -> Result<NativeVisualQueryResult, String> {
    if !req.drill_filters.is_empty() {
        return Err("drill_filters não suportado no pushdown (round 1)".to_string());
    }
    if !req.category_levels.is_empty() || !req.matrix_rows.is_empty() {
        return Err("category_levels/matrix_rows não suportados no pushdown (round 1, subtotais não-aditivos por nó)".to_string());
    }
    if req.legend_field.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
        return Err("legend_field (2ª dimensão) não suportado no pushdown (round 1)".to_string());
    }
    if matches!(req.visual_type.as_str(), "slicer" | "action-button" | "textbox") {
        return Err(format!("visual_type '{}' fica no caminho DAX", req.visual_type));
    }

    let fact_key = model
        .get_table(&req.table_id)
        .ok_or_else(|| format!("tabela base '{}' não encontrada no modelo", req.table_id))?
        .name
        .to_lowercase();
    let fact_duckdb = model
        .duckdb_table_name(&fact_key)
        .ok_or_else(|| format!("tabela base '{}' sem tabela DuckDB (dados de amostra?)", fact_key))?
        .to_string();
    let mut joiner = Joiner::new(model, fact_key, fact_duckdb);

    if is_single_value_shape(req) {
        let mut wheres: Vec<String> = Vec::new();
        for f in &req.filters {
            if let Some(cond) = translate_value_filter(model, &mut joiner, f)? {
                wheres.push(cond);
            }
        }
        return execute_single_value(model, conn, req, agg, &mut joiner, &wheres, t0);
    }

    let raw_cat = req.x_axis_field.as_deref().unwrap_or("");
    if raw_cat.trim().is_empty() {
        return Err("sem x_axis_field e não é card/kpi/multi-card".to_string());
    }
    let (cat_tbl_name, cat_col) = resolve_table_and_col(model, &req.table_id, raw_cat);
    let cat_tbl = model
        .get_table(&cat_tbl_name)
        .ok_or_else(|| format!("tabela de categoria '{}' não encontrada", cat_tbl_name))?;
    let cat_cidx = cat_tbl
        .get_col_idx(&cat_col)
        .ok_or_else(|| format!("coluna de categoria '{}.{}' não encontrada", cat_tbl_name, cat_col))?;
    let cat_ct = cat_tbl.col_types[cat_cidx];
    if matches!(cat_ct, ColumnType::Date | ColumnType::Currency) {
        return Err("agrupar por Date/Currency não suportado no pushdown (round 1)".to_string());
    }
    if cat_tbl.sort_by_columns.contains_key(&cat_col.to_lowercase()) {
        return Err("coluna de categoria com sortByColumn não suportado no pushdown (round 1)".to_string());
    }
    // Re-base the FROM onto the category's own table *before* resolving
    // filters/measures, so they land on the right side (`b0`/`b1`) — see
    // `join::Joiner::set_base_table` for why the join direction must follow
    // the category, not always the fact table.
    joiner.set_base_table(&cat_tbl.name.to_lowercase())?;
    let cat_col_sql = joiner.col_ref(&cat_tbl.name.to_lowercase(), &cat_col)?;

    let mut wheres: Vec<String> = Vec::new();
    for f in &req.filters {
        if let Some(cond) = translate_value_filter(model, &mut joiner, f)? {
            wheres.push(cond);
        }
    }

    execute_grouped(model, conn, req, agg, &mut joiner, &wheres, &cat_col_sql, cat_ct, t0)
}

/// Translates one `NativeFilterInput` (value-list / IN filter) into a SQL
/// condition, joining its table onto the fact table if needed. `None` means
/// "no values" — `build_filter_context` treats an empty value list as "no
/// filter at all" (see `dax::build_filter_context`), not "match nothing", so
/// pushdown must do the same instead of emitting `IN ()`.
fn translate_value_filter(
    model: &DaxModel,
    joiner: &mut Joiner<'_>,
    f: &NativeFilterInput,
) -> Result<Option<String>, String> {
    if f.values.is_empty() {
        return Ok(None);
    }
    let tbl = model
        .get_table(&f.table_id)
        .ok_or_else(|| format!("filtro em tabela desconhecida '{}'", f.table_id))?;
    let cidx = tbl
        .get_col_idx(&f.column)
        .ok_or_else(|| format!("filtro em coluna desconhecida '{}.{}'", f.table_id, f.column))?;
    let ct = tbl.col_types[cidx];
    let col_ref = joiner.col_ref(&tbl.name.to_lowercase(), &f.column)?;
    let lits: Result<Vec<String>, String> = f.values.iter().map(|v| literal::literal_sql(ct, v)).collect();
    let lits = lits?;
    Ok(Some(format!("{} IN ({})", col_ref, lits.join(", "))))
}

fn where_clause(wheres: &[String]) -> String {
    if wheres.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", wheres.join(" AND "))
    }
}

/// Card/KPI/gauge (one y-field, no grouping) and multi-card / bare-measures
/// visuals (N y-fields, no grouping, one row each): a single `SELECT
/// agg0, agg1, ... FROM fact <joins> WHERE ...` with no `GROUP BY`, which
/// DuckDB always answers with exactly one row (NULL aggregates on no
/// matching rows), matching `eval_field_value`'s `unwrap_or(0.0)`.
fn execute_single_value(
    model: &DaxModel,
    conn: &Connection,
    req: &NativeVisualQueryRequest,
    agg: &str,
    joiner: &mut Joiner<'_>,
    wheres: &[String],
    t0: Instant,
) -> Result<NativeVisualQueryResult, String> {
    let is_card = matches!(req.visual_type.as_str(), "card" | "kpi" | "gauge");
    if is_card {
        let y_field = req.y_axis_fields.first().map(String::as_str).unwrap_or("");
        if y_field.trim().is_empty() {
            return Ok(NativeVisualQueryResult {
                categories: vec![y_field.to_string()],
                series: vec![NativeVisualSeries { name: y_field.to_string(), data: vec![Some(0.0)] }],
                summary_value: Some(0.0),
                query_time_ms: elapsed_ms(t0),
                engine: "duckdb".to_string(),
                category_keys: Vec::new(),
                matrix: None,
            });
        }
    }

    if req.y_axis_fields.is_empty() {
        return Err("multi-card/card sem y_axis_fields".to_string());
    }
    let mut yaggs: Vec<YAgg> = Vec::with_capacity(req.y_axis_fields.len());
    for raw in &req.y_axis_fields {
        yaggs.push(resolve_y_agg(model, joiner, &req.table_id, raw, agg)?);
    }

    let select: Vec<String> = yaggs
        .iter()
        .map(|y| format!("CAST({} AS DOUBLE)", render_agg(&y.agg_call, y.case_cond.as_deref())))
        .collect();
    let sql = format!(
        "SELECT {} FROM {} AS b0{}{}",
        select.join(", "),
        super::duckdb_load::quote_ident(&joiner.base_duckdb),
        joiner.join_sql(),
        where_clause(wheres)
    );

    let vals: Vec<Option<f64>> = conn
        .query_row(&sql, [], |row| {
            let mut out = Vec::with_capacity(yaggs.len());
            for i in 0..yaggs.len() {
                out.push(row.get::<_, Option<f64>>(i)?);
            }
            Ok(out)
        })
        .map_err(|e| format!("erro ao executar pushdown single-value: {} | SQL: {}", e, sql))?;

    let data: Vec<Option<f64>> = vals.into_iter().map(|v| Some(v.unwrap_or(0.0))).collect();
    let summary = data.first().copied().flatten();
    Ok(NativeVisualQueryResult {
        categories: yaggs.iter().map(|y| y.name.clone()).collect(),
        series: vec![NativeVisualSeries {
            name: if is_card { yaggs[0].name.clone() } else { "Valores".to_string() },
            data,
        }],
        summary_value: summary,
        query_time_ms: elapsed_ms(t0),
        engine: "duckdb".to_string(),
        category_keys: Vec::new(),
        matrix: None,
    })
}

/// Single-category grouped chart: `SELECT cat, agg0, agg1, ... FROM <base>
/// LEFT JOIN <other> ... WHERE ... GROUP BY cat ORDER BY (cat IS NULL)
/// DESC, cat ASC`. `base`/`joins` already reflect the category's own table
/// as the driving side (`join::Joiner::set_base_table`, called by
/// `try_pushdown` before this). Rows where every y-value is NULL are
/// dropped after fetch, matching `query_planner`'s "any value non-blank"
/// keep rule; a text category's empty string is folded into the same
/// `NULL`/"Sem dados" bucket as a true blank, matching
/// `DaxValue::as_display_string`.
#[allow(clippy::too_many_arguments)]
fn execute_grouped(
    model: &DaxModel,
    conn: &Connection,
    req: &NativeVisualQueryRequest,
    agg: &str,
    joiner: &mut Joiner<'_>,
    wheres: &[String],
    cat_col_sql: &str,
    cat_ct: ColumnType,
    t0: Instant,
) -> Result<NativeVisualQueryResult, String> {
    // Empty text ("") is blank for DAX display purposes too; normalize it to
    // NULL so it groups with genuine NULLs under "Sem dados" like
    // `DaxValue::as_display_string().is_empty()` does in `query_planner`.
    let cat_expr = if cat_ct == ColumnType::Text {
        format!("NULLIF({}, '')", cat_col_sql)
    } else {
        cat_col_sql.to_string()
    };

    let y_names: Vec<String> = req
        .y_axis_fields
        .iter()
        .map(|y| y.trim().trim_start_matches('Σ').trim().to_string())
        .collect();
    if y_names.is_empty() {
        return Err("visual agrupado sem y_axis_fields".to_string());
    }
    let mut yaggs: Vec<YAgg> = Vec::with_capacity(y_names.len());
    for raw in &y_names {
        yaggs.push(resolve_y_agg(model, joiner, &req.table_id, raw, agg)?);
    }

    let y_selects: Vec<String> = yaggs
        .iter()
        .map(|y| format!("CAST({} AS DOUBLE)", render_agg(&y.agg_call, y.case_cond.as_deref())))
        .collect();
    let sql = format!(
        "SELECT {cat} AS cat, {ys} FROM {base} AS b0{joins}{wh} GROUP BY {cat} ORDER BY ({cat} IS NULL) DESC, {cat} ASC",
        cat = cat_expr,
        ys = y_selects.join(", "),
        base = super::duckdb_load::quote_ident(&joiner.base_duckdb),
        joins = joiner.join_sql(),
        wh = where_clause(wheres),
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("erro ao preparar pushdown agrupado: {} | SQL: {}", e, sql))?;
    let mut rows_cursor = stmt
        .query([])
        .map_err(|e| format!("erro ao executar pushdown agrupado: {} | SQL: {}", e, sql))?;

    let mut categories: Vec<String> = Vec::new();
    let mut grid: Vec<Vec<Option<f64>>> = Vec::new();
    while let Some(row) = rows_cursor
        .next()
        .map_err(|e| format!("erro ao iterar pushdown agrupado: {}", e))?
    {
        let cat_vref = row
            .get_ref(0)
            .map_err(|e| format!("erro lendo categoria do pushdown: {}", e))?;
        let label = match cat_vref {
            duckdb::types::ValueRef::Null => "Sem dados".to_string(),
            other => {
                let disp = super::duckdb_load::value_ref_to_dax(other, cat_ct).as_display_string();
                if disp.is_empty() {
                    "Sem dados".to_string()
                } else {
                    disp
                }
            }
        };
        let mut vals: Vec<Option<f64>> = Vec::with_capacity(yaggs.len());
        for i in 0..yaggs.len() {
            vals.push(
                row.get::<_, Option<f64>>(i + 1)
                    .map_err(|e| format!("erro lendo valor do pushdown: {}", e))?,
            );
        }
        if vals.iter().any(Option::is_some) {
            categories.push(label);
            grid.push(vals);
        }
    }

    let series: Vec<NativeVisualSeries> = yaggs
        .iter()
        .enumerate()
        .map(|(i, y)| NativeVisualSeries {
            name: y.name.clone(),
            data: grid.iter().map(|r| r[i]).collect(),
        })
        .collect();

    Ok(NativeVisualQueryResult {
        categories,
        series,
        summary_value: None,
        query_time_ms: elapsed_ms(t0),
        engine: "duckdb".to_string(),
        category_keys: Vec::new(),
        matrix: None,
    })
}
