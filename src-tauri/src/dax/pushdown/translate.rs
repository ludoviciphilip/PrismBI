//! Resolves one y-field (plain column + explicit aggregation, or a simple
//! measure, optionally `CALCULATE`-wrapped) into a SQL aggregate expression.
//! Measures are parsed with the project's own DAX parser (`dax::parser`) and
//! matched against a small set of `Expr` shapes — not a general DAX-to-SQL
//! compiler, see the module doc in `pushdown/mod.rs` for the exact subset.

use super::super::ast::{BinaryOp, Expr};
use super::super::query_planner::resolve_table_and_col;
use super::super::{parser, ColumnType, DaxModel};
use super::join::Joiner;
use super::literal::{eq_cond, neq_cond, ord_cond};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub(super) enum AggKind {
    Sum,
    Count,
    CountA,
    CountRows,
    DistinctCount,
    Min,
    Max,
    Average,
}

fn agg_kind_from_name(name: &str) -> Option<AggKind> {
    match name.to_uppercase().as_str() {
        "SUM" => Some(AggKind::Sum),
        "COUNT" => Some(AggKind::Count),
        "COUNTA" => Some(AggKind::CountA),
        "COUNTROWS" => Some(AggKind::CountRows),
        "DISTINCTCOUNT" => Some(AggKind::DistinctCount),
        "MIN" => Some(AggKind::Min),
        "MAX" => Some(AggKind::Max),
        "AVERAGE" | "AVG" => Some(AggKind::Average),
        _ => None,
    }
}

/// Same mapping `query_planner::eval_field_opt` uses for a plain
/// column + `aggregation` string pair (not a measure): "COUNT" and
/// "DISTINCTCOUNT" are special-cased, everything else (including values the
/// real engine doesn't special-case, like "COUNTROWS") falls through to its
/// default numeric branch. Mirrored bit-for-bit so pushdown agrees with the
/// DAX path even on these edge inputs, not just the documented aggregations.
fn plain_agg_kind(agg: &str) -> AggKind {
    match agg.to_uppercase().as_str() {
        "COUNT" => AggKind::Count,
        "DISTINCTCOUNT" => AggKind::DistinctCount,
        "AVG" | "AVERAGE" => AggKind::Average,
        "MIN" => AggKind::Min,
        "MAX" => AggKind::Max,
        _ => AggKind::Sum,
    }
}

pub(super) struct AggCall {
    kind: AggKind,
    /// `None` only for `COUNTROWS` without a `CASE WHEN` wrapper (-> `COUNT(*)`).
    col_sql: Option<String>,
}

pub(super) struct YAgg {
    pub(super) name: String,
    pub(super) agg_call: AggCall,
    pub(super) case_cond: Option<String>,
}

pub(super) fn render_agg(call: &AggCall, case_cond: Option<&str>, presence_sql: &str) -> String {
    let col_expr = match (&call.col_sql, case_cond) {
        (Some(c), Some(cond)) => format!("(CASE WHEN {} THEN {} ELSE NULL END)", cond, c),
        (Some(c), None) => c.clone(),
        (None, Some(cond)) => format!("(CASE WHEN {} THEN 1 ELSE NULL END)", cond),
        (None, None) => "*".to_string(),
    };
    let base = match call.kind {
        AggKind::Sum => return format!("SUM({})", col_expr),
        AggKind::Min => return format!("MIN({})", col_expr),
        AggKind::Max => return format!("MAX({})", col_expr),
        AggKind::Average => return format!("AVG({})", col_expr),
        AggKind::Count | AggKind::CountA => format!("COUNT({})", col_expr),
        AggKind::CountRows => {
            if col_expr == "*" {
                "COUNT(*)".to_string()
            } else {
                format!("COUNT({})", col_expr)
            }
        }
        AggKind::DistinctCount => format!("COUNT(DISTINCT {})", col_expr),
    };
    // v0.11 Frente H (items c/d): COUNT/COUNTA/COUNTROWS/DISTINCTCOUNT all
    // return BLANK, not 0, when there are no rows to count (dax.guide:
    // "If the table has no rows, COUNTROWS returns BLANK, not 0" — same
    // remark on COUNT/DISTINCTCOUNT); plain SQL `COUNT` always returns an
    // integer 0 on an empty/unmatched group, so every Count-family render
    // gets wrapped in this presence check. `presence_sql` disambiguates a
    // real zero-row group from a `LEFT JOIN` phantom row (see
    // `Joiner::row_presence_sql`).
    let presence_cond = case_cond.unwrap_or("TRUE");
    let presence_count = format!("COUNT(CASE WHEN {} THEN {} ELSE NULL END)", presence_cond, presence_sql);
    if matches!(call.kind, AggKind::DistinctCount) {
        if let Some(raw_col) = &call.col_sql {
            // v0.11 Frente H (item e): DISTINCTCOUNT counts a stored BLANK
            // as one of its distinct values (dax.guide), but SQL's
            // `COUNT(DISTINCT col)` silently drops NULLs — add back 1 when
            // at least one *present* (post-filter, post-join) row has NULL
            // for this column.
            let blank_exists = format!(
                "MAX(CASE WHEN ({cond}) AND ({presence}) IS NOT NULL AND {col} IS NULL THEN 1 ELSE 0 END)",
                cond = presence_cond,
                presence = presence_sql,
                col = raw_col
            );
            return format!("(CASE WHEN {presence_count} = 0 THEN NULL ELSE {base} + {blank_exists} END)");
        }
    }
    format!("(CASE WHEN {presence_count} = 0 THEN NULL ELSE {base} END)")
}

fn literal_from_expr(e: &Expr) -> Option<JsonValue> {
    match e {
        Expr::Number(n) => Some(JsonValue::from(*n)),
        Expr::Text(s) => Some(JsonValue::String(s.clone())),
        Expr::Boolean(b) => Some(JsonValue::Bool(*b)),
        Expr::Unary { op: super::super::ast::UnaryOp::Neg, expr } => {
            literal_from_expr(expr).and_then(|v| v.as_f64()).map(|f| JsonValue::from(-f))
        }
        _ => None,
    }
}

fn table_col_ref(
    model: &DaxModel,
    joiner: &mut Joiner<'_>,
    table: &str,
    column: &str,
) -> Result<(String, ColumnType), String> {
    let tbl = model
        .get_table(table)
        .ok_or_else(|| format!("tabela '{}' (referenciada na medida) não encontrada", table))?;
    let cidx = tbl
        .get_col_idx(column)
        .ok_or_else(|| format!("coluna '{}.{}' (referenciada na medida) não encontrada", table, column))?;
    let ct = tbl.col_types[cidx];
    let col_sql = joiner.col_ref(&tbl.name.to_lowercase(), column)?;
    Ok((col_sql, ct))
}

/// Translates one CALCULATE filter argument: `Table[Col] = lit`,
/// `Table[Col] <op> lit` (`<>`,`<`,`<=`,`>`,`>=`), or `Table[Col] IN {...}`.
/// Anything else (nested FILTER, measure references, multi-column
/// conditions, AND/OR of several columns...) rejects — see module doc.
///
/// `outer_filter_cols` (v0.11 Frente H, item f): the (table, column) pairs
/// the *visual's own* filters already apply via the shared `WHERE` clause.
/// `CALCULATE`'s filter argument *replaces* the context filter on the same
/// column (DAX: a column filter inside CALCULATE overrides, never
/// intersects with, an outer filter on that exact column) — but pushdown's
/// `WHERE` is shared by every series in the visual, so a `CASE WHEN`
/// *narrowing* it further would wrongly AND the two together instead of
/// replacing. Rather than rebuild the whole query per-series to express the
/// override, this rejects the pushdown outright the moment a CALCULATE
/// filter column collides with an outer filter column, falling back to the
/// (always correct) DAX engine for that visual.
fn translate_calc_filter(
    model: &DaxModel,
    joiner: &mut Joiner<'_>,
    e: &Expr,
    outer_filter_cols: &HashSet<(String, String)>,
) -> Result<String, String> {
    let check_no_override = |table: &str, column: &str| -> Result<(), String> {
        let key = (table.to_lowercase(), column.to_lowercase());
        if outer_filter_cols.contains(&key) {
            Err(format!(
                "filtro CALCULATE em '{}[{}]' colide com um filtro do próprio visual na mesma coluna \
                 (CALCULATE substitui o filtro de contexto, o WHERE compartilhado do pushdown intersectaria) \
                 — não suportado no pushdown (round 2)",
                table, column
            ))
        } else {
            Ok(())
        }
    };
    match e {
        Expr::InList { expr, list } => {
            let Expr::TableColRef { table, column } = expr.as_ref() else {
                return Err("filtro CALCULATE: IN exige Tabela[Coluna] do lado esquerdo".to_string());
            };
            check_no_override(table, column)?;
            let (col_sql, ct) = table_col_ref(model, joiner, table, column)?;
            let lits: Result<Vec<JsonValue>, String> = list
                .iter()
                .map(|le| literal_from_expr(le).ok_or_else(|| "filtro CALCULATE: item de IN não é literal simples".to_string()))
                .collect();
            super::literal::in_cond(ct, &col_sql, &lits?)
        }
        Expr::Binary { op, left, right } => {
            let sql_op = match op {
                BinaryOp::Eq | BinaryOp::StrictEq => "=",
                BinaryOp::Neq => "<>",
                BinaryOp::Lt => "<",
                BinaryOp::Lte => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Gte => ">=",
                _ => return Err("filtro CALCULATE: operador não suportado no pushdown".to_string()),
            };
            let (col_ref_expr, lit_expr) = match (left.as_ref(), right.as_ref()) {
                (Expr::TableColRef { .. }, _) => (left.as_ref(), right.as_ref()),
                (_, Expr::TableColRef { .. }) => (right.as_ref(), left.as_ref()),
                _ => return Err("filtro CALCULATE: esperava Tabela[Coluna] em um dos lados".to_string()),
            };
            let Expr::TableColRef { table, column } = col_ref_expr else { unreachable!() };
            check_no_override(table, column)?;
            let (col_sql, ct) = table_col_ref(model, joiner, table, column)?;
            let lit = literal_from_expr(lit_expr)
                .ok_or_else(|| "filtro CALCULATE: lado do literal não é um literal simples".to_string())?;
            match sql_op {
                "=" => eq_cond(ct, &col_sql, &lit),
                "<>" => neq_cond(ct, &col_sql, &lit),
                other => ord_cond(ct, &col_sql, other, &lit),
            }
        }
        _ => Err("filtro CALCULATE: forma não suportada no pushdown (round 1)".to_string()),
    }
}

/// Extracts the `AggCall` from an aggregation `Expr::Call` (`SUM(T[C])`,
/// `COUNTROWS(T)`, ...).
fn extract_agg_call(model: &DaxModel, joiner: &mut Joiner<'_>, fact_key: &str, e: &Expr) -> Result<AggCall, String> {
    let Expr::Call { name, args } = e else {
        return Err("medida: esperava uma chamada de agregação simples".to_string());
    };
    let kind = agg_kind_from_name(name).ok_or_else(|| format!("medida: agregação '{}' não suportada no pushdown", name))?;
    if matches!(kind, AggKind::CountRows) {
        let Some(Expr::Ident(tbl_name)) = args.first() else {
            return Err("COUNTROWS: esperava um nome de tabela".to_string());
        };
        let tbl_key = model
            .get_table(tbl_name)
            .ok_or_else(|| format!("COUNTROWS: tabela '{}' não encontrada", tbl_name))?
            .name
            .to_lowercase();
        if tbl_key != fact_key {
            return Err("COUNTROWS de uma tabela diferente da base do visual não suportado no pushdown (round 1)".to_string());
        }
        return Ok(AggCall { kind, col_sql: None });
    }
    let Some(Expr::TableColRef { table, column }) = args.first() else {
        return Err(format!("{}: esperava Tabela[Coluna]", name));
    };
    let tbl_key = model
        .get_table(table)
        .ok_or_else(|| format!("{}: tabela '{}' não encontrada", name, table))?
        .name
        .to_lowercase();
    if tbl_key != fact_key {
        return Err(format!(
            "{}: agregação de coluna de dimensão '{}[{}]' não suportada no pushdown \
             (multiplicaria pelo número de linhas do fato via o JOIN)",
            name, table, column
        ));
    }
    let (col_sql, _ct) = table_col_ref(model, joiner, table, column)?;
    Ok(AggCall { kind, col_sql: Some(col_sql) })
}

/// Resolves one y-field name into a `YAgg`: a measure (optionally
/// `CALCULATE`-wrapped with simple filters) if `clean` names one — exactly
/// like `query_planner::eval_field_opt`, which checks `model.get_measure`
/// *before* treating the name as a plain column, ignoring `plain_agg` in
/// that case — otherwise a plain column aggregated with `plain_agg`.
///
/// `outer_filter_cols`: see `translate_calc_filter` (item f).
pub(super) fn resolve_y_agg(
    model: &DaxModel,
    joiner: &mut Joiner<'_>,
    default_table: &str,
    raw: &str,
    plain_agg: &str,
    outer_filter_cols: &HashSet<(String, String)>,
) -> Result<YAgg, String> {
    let clean = raw.trim().trim_start_matches('Σ').trim();
    if let Some(measure) = model.get_measure(clean) {
        let ast = parser::parse_dax(&measure.formula).map_err(|e| format!("medida '{}': erro de parse ({})", clean, e))?;
        let fact_key = joiner.fact_key_for_measures();
        let (inner, case_cond) = match &ast {
            Expr::Call { name, args } if name.eq_ignore_ascii_case("CALCULATE") => {
                let Some(inner) = args.first() else {
                    return Err(format!("medida '{}': CALCULATE sem expressão", clean));
                };
                let mut conds = Vec::with_capacity(args.len().saturating_sub(1));
                for filt in &args[1..] {
                    conds.push(translate_calc_filter(model, joiner, filt, outer_filter_cols)?);
                }
                (inner, if conds.is_empty() { None } else { Some(conds.join(" AND ")) })
            }
            _ => (&ast, None),
        };
        let agg_call = extract_agg_call(model, joiner, &fact_key, inner)?;
        return Ok(YAgg { name: clean.to_string(), agg_call, case_cond });
    }

    let (tbl_name, col) = resolve_table_and_col(model, default_table, clean);
    let tbl = model
        .get_table(&tbl_name)
        .ok_or_else(|| format!("campo '{}': tabela '{}' não encontrada", clean, tbl_name))?;
    if tbl.get_col_idx(&col).is_none() {
        return Err(format!("campo '{}': coluna '{}' não encontrada em '{}'", clean, col, tbl_name));
    }
    // v0.11 Frente H (item g): aggregating a column that lives on a
    // dimension table (not the fact) while that dimension is joined to the
    // fact (grouped-by-dimension shape, or a dimension reached via a
    // CALCULATE-filter join) would fan out: each dimension row is
    // physically repeated once per matching fact row by the `LEFT JOIN`,
    // so `SUM`/`AVERAGE`/`COUNT`/`DISTINCTCOUNT` over it would multiply by
    // the fact row count instead of reading the dimension's own single
    // value — `resolve_y_agg` in the bug report. The DAX engine never
    // fans out a dimension column this way (it has no join to begin with),
    // so this is a hard reject rather than an approximation.
    if tbl.name.to_lowercase() != joiner.fact_key_for_measures() {
        return Err(format!(
            "agregação de coluna de dimensão '{}[{}]' não suportada no pushdown \
             (multiplicaria pelo número de linhas do fato via o JOIN)",
            tbl_name, col
        ));
    }
    let col_sql = joiner.col_ref(&tbl.name.to_lowercase(), &col)?;
    Ok(YAgg {
        name: clean.to_string(),
        agg_call: AggCall { kind: plain_agg_kind(plain_agg), col_sql: Some(col_sql) },
        case_cond: None,
    })
}
