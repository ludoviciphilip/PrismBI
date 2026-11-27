//! Resolves one y-field (plain column + explicit aggregation, or a simple
//! measure, optionally `CALCULATE`-wrapped) into a SQL aggregate expression.
//! Measures are parsed with the project's own DAX parser (`dax::parser`) and
//! matched against a small set of `Expr` shapes — not a general DAX-to-SQL
//! compiler, see the module doc in `pushdown/mod.rs` for the exact subset.

use super::super::ast::{BinaryOp, Expr};
use super::super::query_planner::resolve_table_and_col;
use super::super::{parser, ColumnType, DaxModel};
use super::join::Joiner;
use super::literal::literal_sql;
use serde_json::Value as JsonValue;

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

pub(super) fn render_agg(call: &AggCall, case_cond: Option<&str>) -> String {
    let col_expr = match (&call.col_sql, case_cond) {
        (Some(c), Some(cond)) => format!("(CASE WHEN {} THEN {} ELSE NULL END)", cond, c),
        (Some(c), None) => c.clone(),
        (None, Some(cond)) => format!("(CASE WHEN {} THEN 1 ELSE NULL END)", cond),
        (None, None) => "*".to_string(),
    };
    match call.kind {
        AggKind::Sum => format!("SUM({})", col_expr),
        AggKind::Count | AggKind::CountA => format!("COUNT({})", col_expr),
        AggKind::CountRows => {
            if col_expr == "*" {
                "COUNT(*)".to_string()
            } else {
                format!("COUNT({})", col_expr)
            }
        }
        AggKind::DistinctCount => format!("COUNT(DISTINCT {})", col_expr),
        AggKind::Min => format!("MIN({})", col_expr),
        AggKind::Max => format!("MAX({})", col_expr),
        AggKind::Average => format!("AVG({})", col_expr),
    }
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
fn translate_calc_filter(model: &DaxModel, joiner: &mut Joiner<'_>, e: &Expr) -> Result<String, String> {
    match e {
        Expr::InList { expr, list } => {
            let Expr::TableColRef { table, column } = expr.as_ref() else {
                return Err("filtro CALCULATE: IN exige Tabela[Coluna] do lado esquerdo".to_string());
            };
            let (col_sql, ct) = table_col_ref(model, joiner, table, column)?;
            let lits: Result<Vec<String>, String> =
                list.iter().map(|le| literal_from_expr(le).ok_or_else(|| "filtro CALCULATE: item de IN não é literal simples".to_string())
                    .and_then(|v| literal_sql(ct, &v))).collect();
            Ok(format!("{} IN ({})", col_sql, lits?.join(", ")))
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
            let (col_sql, ct) = table_col_ref(model, joiner, table, column)?;
            let lit = literal_from_expr(lit_expr)
                .ok_or_else(|| "filtro CALCULATE: lado do literal não é um literal simples".to_string())?;
            Ok(format!("{} {} {}", col_sql, sql_op, literal_sql(ct, &lit)?))
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
    let (col_sql, _ct) = table_col_ref(model, joiner, table, column)?;
    Ok(AggCall { kind, col_sql: Some(col_sql) })
}

/// Resolves one y-field name into a `YAgg`: a measure (optionally
/// `CALCULATE`-wrapped with simple filters) if `clean` names one — exactly
/// like `query_planner::eval_field_opt`, which checks `model.get_measure`
/// *before* treating the name as a plain column, ignoring `plain_agg` in
/// that case — otherwise a plain column aggregated with `plain_agg`.
pub(super) fn resolve_y_agg(
    model: &DaxModel,
    joiner: &mut Joiner<'_>,
    default_table: &str,
    raw: &str,
    plain_agg: &str,
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
                    conds.push(translate_calc_filter(model, joiner, filt)?);
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
    let col_sql = joiner.col_ref(&tbl.name.to_lowercase(), &col)?;
    Ok(YAgg {
        name: clean.to_string(),
        agg_call: AggCall { kind: plain_agg_kind(plain_agg), col_sql: Some(col_sql) },
        case_cond: None,
    })
}
