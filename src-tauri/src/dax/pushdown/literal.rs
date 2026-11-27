//! Converts a JSON filter/literal value into a SQL literal, for the column
//! types pushdown supports (`Text`/`Integer`/`Decimal`/`Boolean`). `Date`
//! and `Currency` reject: the DAX/Excel serial<->SQL DATE conversion and the
//! ×10 000 currency scale aren't needed for round 1's "SUM by category"
//! target and are easy to get subtly wrong, so they fall back to the DAX
//! engine instead.

use super::super::ColumnType;
use serde_json::Value as JsonValue;

pub(super) fn literal_sql(ct: ColumnType, v: &JsonValue) -> Result<String, String> {
    match ct {
        ColumnType::Text => match v {
            JsonValue::String(s) => Ok(format!("'{}'", s.replace('\'', "''"))),
            JsonValue::Number(n) => Ok(format!("'{}'", n)),
            JsonValue::Bool(b) => Ok(format!("'{}'", b)),
            _ => Err("literal de texto inválido para pushdown".to_string()),
        },
        ColumnType::Boolean => match v {
            JsonValue::Bool(b) => Ok(if *b { "TRUE".to_string() } else { "FALSE".to_string() }),
            JsonValue::Number(n) => Ok(if n.as_f64().unwrap_or(0.0) != 0.0 { "TRUE".to_string() } else { "FALSE".to_string() }),
            JsonValue::String(s) => {
                let l = s.trim().to_lowercase();
                Ok(if l == "true" || l == "1" || l == "sim" { "TRUE".to_string() } else { "FALSE".to_string() })
            }
            _ => Err("literal booleano inválido para pushdown".to_string()),
        },
        ColumnType::Integer => v
            .as_f64()
            .map(|f| (f.round() as i64).to_string())
            .ok_or_else(|| "literal inteiro inválido para pushdown".to_string()),
        ColumnType::Decimal => v
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| format!("{}", f))
            .ok_or_else(|| "literal decimal inválido para pushdown".to_string()),
        ColumnType::Date | ColumnType::Currency => {
            Err("filtro/literal em coluna Date/Currency não suportado no pushdown (round 1)".to_string())
        }
    }
}

/// v0.11 Frente H (item a): the raw text of a JSON literal, the same
/// coercions `literal_sql`'s `Text` branch accepts (string/number/bool),
/// without SQL-quoting — used to build a case/trim-normalized comparison.
fn text_literal_raw(v: &JsonValue) -> Result<String, String> {
    match v {
        JsonValue::String(s) => Ok(s.clone()),
        JsonValue::Number(n) => Ok(n.to_string()),
        JsonValue::Bool(b) => Ok(b.to_string()),
        _ => Err("literal de texto inválido para pushdown".to_string()),
    }
}

/// v0.11 Frente H (item a): `DaxValue::value_key`'s `Text` case folds via
/// `trim().to_lowercase()` and treats an empty/whitespace-only string as
/// BLANK (`dax::mod.rs` comment: "Estrito: texto só casa com texto,
/// comparação sem diferenciar maiúsculas, como no DAX"). SQL `=`/`IN` on a
/// raw column is case-sensitive, so the motor and the pushdown disagree the
/// moment two rows differ only by case — this wraps both sides in the same
/// `LOWER(TRIM(...))` fold, with `NULL`/`''` collapsed to `''` so a genuine
/// BLANK matches an empty-string literal (and vice versa) like `dax_equals`.
pub(super) fn text_norm_sql(col_sql: &str) -> String {
    format!("LOWER(TRIM(COALESCE({}, '')))", col_sql)
}

/// Same fold as `text_norm_sql`, applied to a JSON filter value, quoted as a
/// SQL string literal.
pub(super) fn text_norm_literal(v: &JsonValue) -> Result<String, String> {
    let raw = text_literal_raw(v)?;
    Ok(format!("'{}'", raw.trim().to_lowercase().replace('\'', "''")))
}

/// v0.11 Frente H (items a/f): one `col <op> literal` condition that never
/// silently disagrees with DAX's `dax_equals`/`dax_not_equals` on a BLANK
/// (`NULL`) column value — `Table[Col] = lit` and `Table[Col] <> lit` are
/// the two CALCULATE-filter/IN-filter shapes pushdown supports (see
/// `pushdown::translate::translate_calc_filter`). For `Text`, folds case and
/// blank/'' the same way `text_norm_sql` does; for numeric/boolean types,
/// `BLANK` coerces to the type's zero value, matching `DaxValue::dax_equals`.
pub(super) fn eq_cond(ct: ColumnType, col_sql: &str, v: &JsonValue) -> Result<String, String> {
    match ct {
        ColumnType::Text => Ok(format!("{} = {}", text_norm_sql(col_sql), text_norm_literal(v)?)),
        ColumnType::Boolean => Ok(format!("COALESCE({}, FALSE) = {}", col_sql, literal_sql(ct, v)?)),
        ColumnType::Integer | ColumnType::Decimal => {
            Ok(format!("COALESCE({}, 0) = {}", col_sql, literal_sql(ct, v)?))
        }
        ColumnType::Date | ColumnType::Currency => Err(
            "filtro em coluna Date/Currency não suportado no pushdown (round 1)".to_string(),
        ),
    }
}

/// `NOT (eq_cond)` — safe because `eq_cond` always resolves to TRUE/FALSE
/// (it never leaves a bare `NULL` on either side), so De Morgan holds and
/// this matches `!dax_equals(...)` including the BLANK <> "A" (DAX: TRUE)
/// case that a raw SQL `<>` gets wrong (`NULL <> 'A'` is `NULL`, dropping
/// the row instead of keeping it).
pub(super) fn neq_cond(ct: ColumnType, col_sql: &str, v: &JsonValue) -> Result<String, String> {
    Ok(format!("NOT ({})", eq_cond(ct, col_sql, v)?))
}

/// v0.11 Frente H (item f): `<`/`<=`/`>`/`>=` against a literal. Text
/// ordering under DAX's case-insensitive collation isn't the same as SQL's
/// byte/locale ordering, so this rejects on `Text` rather than risk a silent
/// mismatch; numeric/boolean BLANK coerces to 0, matching comparisons in the
/// DAX evaluator (`dax_equals`/`compare_dax_values`).
pub(super) fn ord_cond(ct: ColumnType, col_sql: &str, sql_op: &str, v: &JsonValue) -> Result<String, String> {
    match ct {
        ColumnType::Boolean | ColumnType::Integer | ColumnType::Decimal => {
            Ok(format!("COALESCE({}, 0) {} {}", col_sql, sql_op, literal_sql(ct, v)?))
        }
        ColumnType::Text => Err("comparação ordinal (<,<=,>,>=) em coluna de texto não suportada no pushdown".to_string()),
        ColumnType::Date | ColumnType::Currency => Err(
            "filtro em coluna Date/Currency não suportado no pushdown (round 1)".to_string(),
        ),
    }
}

/// v0.11 Frente H (item a): `Table[Col] IN {...}`, blank/case-aware exactly
/// like `eq_cond` (one `=` per value would also work but `IN` keeps the SQL
/// short for the common slicer/visual-filter case).
pub(super) fn in_cond(ct: ColumnType, col_sql: &str, values: &[JsonValue]) -> Result<String, String> {
    match ct {
        ColumnType::Text => {
            let lits: Result<Vec<String>, String> = values.iter().map(text_norm_literal).collect();
            Ok(format!("{} IN ({})", text_norm_sql(col_sql), lits?.join(", ")))
        }
        ColumnType::Boolean | ColumnType::Integer | ColumnType::Decimal => {
            let default = if ct == ColumnType::Boolean { "FALSE" } else { "0" };
            let lits: Result<Vec<String>, String> = values.iter().map(|v| literal_sql(ct, v)).collect();
            Ok(format!("COALESCE({}, {}) IN ({})", col_sql, default, lits?.join(", ")))
        }
        ColumnType::Date | ColumnType::Currency => Err(
            "filtro/literal em coluna Date/Currency não suportado no pushdown (round 1)".to_string(),
        ),
    }
}
