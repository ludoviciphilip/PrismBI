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
