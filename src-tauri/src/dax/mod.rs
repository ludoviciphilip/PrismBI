pub mod ast;
pub mod calculated;
pub mod duckdb_load;
pub mod evaluator;
pub mod functions;
pub mod lexer;
pub mod parser;
pub mod pushdown;
pub mod query_planner;
/// v0.11 Frente I: N níveis nas COLUNAS da matriz (gancho mínimo em
/// `query_planner.rs`; toda a lógica — árvore, subtotal por nível e a
/// interseção linha×coluna — mora aqui).
pub mod matrix_columns;
#[cfg(test)]
mod tests_duckdb_sync;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Declared or inferred storage type for a model column. Drives how raw JSON
/// cell values are parsed into typed `DaxValue`s at sync time, and how
/// `NativeFilterInput` values (slicers/visual filters) are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Text,
    Integer,
    Decimal,
    Currency,
    Date,
    Boolean,
}

impl ColumnType {
    pub fn from_str_opt(s: Option<&str>) -> Option<ColumnType> {
        match s?.trim().to_lowercase().as_str() {
            "text" => Some(ColumnType::Text),
            "number" => Some(ColumnType::Decimal), // refined to Integer/Decimal by content below
            "currency" => Some(ColumnType::Currency),
            "percentage" => Some(ColumnType::Decimal),
            "date" => Some(ColumnType::Date),
            "boolean" => Some(ColumnType::Boolean),
            _ => None,
        }
    }
}

/// A hashable, typed representation of a `DaxValue` used as a filter-set
/// member. Replaces the old `String`-keyed `canonical_key()` inside
/// `FilterContext` so that comparisons never allocate and so that values of
/// different underlying JSON shape (e.g. text "2024" vs number 2024) don't
/// collide unless DAX itself would consider them equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValueKey {
    Blank,
    Bool(bool),
    /// Normalized numeric value (micros) so Integer/Decimal/Currency/Boolean
    /// values that are numerically equal share one key.
    Num(i64),
    /// DAX serial date*86400 (seconds since 1899-12-30), so a date column
    /// filtered by an ISO string matches a `DateTime` cell and vice versa.
    Date(i64),
    Text(String),
}

pub fn build_filter_context(filters: &[NativeFilterInput], model: &DaxModel) -> FilterContext {
    let mut ctx = FilterContext::default();
    for f in filters {
        let col_type = model
            .get_table(&f.table_id)
            .and_then(|t| t.get_col_idx(&f.column).map(|i| (t, i)))
            .and_then(|(t, i)| t.col_types.get(i).copied());
        // v0.11 Frente H (item h): `between` (range slicers, relative-date
        // windows) needs a [lo, hi] *range* test, not an IN-list of exactly
        // the two endpoints — see `FilterContext::range_filters`.
        if f.operator.as_deref() == Some("between") && f.values.len() >= 2 {
            let a = DaxValue::from_json_as(&f.values[0], col_type).value_key();
            let b = DaxValue::from_json_as(&f.values[1], col_type).value_key();
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            ctx.set_range_filter(&f.table_id, &f.column, lo, hi);
            continue;
        }
        let allowed: HashSet<ValueKey> = f
            .values
            .iter()
            .map(|v| DaxValue::from_json_as(v, col_type).value_key())
            .collect();
        if !allowed.is_empty() {
            ctx.set_column_filter(&f.table_id, &f.column, allowed);
        }
    }
    // §2.3/§3: ALLSELECTED restores this query's starting ("visual") filter
    // state, not the model's unfiltered state (that's ALL/REMOVEFILTERS).
    // Captured once here, before any CALCULATE runs, and carried forward
    // unchanged through every nested CALCULATE/context transition.
    ctx.shadow = Some(Arc::new(ctx.clone()));
    ctx
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NativeFilterInput {
    pub table_id: String,
    pub column: String,
    pub values: Vec<JsonValue>,
    /// v0.11 Frente H (item h): `"in"`/`None` = value-list membership (today's
    /// only behavior); `"between"` = an inclusive range over `values[0]`..`values[1]`
    /// (segmentador "Entre" and relative-date windows, which used to be
    /// flattened into a 2-value IN list in `VisualCard.tsx`, losing the
    /// operator — see `FilterContext::range_filters`).
    #[serde(default)]
    pub operator: Option<String>,
}

/// DAX engine 2.0 typed value. Storage stays row-based (`Vec<Vec<DaxValue>>`)
/// for now; only the value representation changes (see docs/RUST_ENGINE_2.md
/// §2.1). `DateTime` stores a DAX/Excel serial number: whole days since
/// 1899-12-30, fractional part = time of day.
#[derive(Debug, Clone, PartialEq)]
pub enum DaxValue {
    Blank,
    Boolean(bool),
    Integer(i64),
    Decimal(f64),
    /// Scaled by 10_000 (4 decimal places), DAX's native currency precision.
    Currency(i64),
    DateTime(f64),
    Text(String),
}

/// Relative "width" of a numeric kind used to decide the result type of an
/// arithmetic operation between two `DaxValue`s (DAX promotion rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NumKind {
    Integer,
    Decimal,
    Currency,
}

impl DaxValue {
    /// Smart numeric constructor: integral finite values become `Integer`,
    /// everything else (fractional, NaN, Infinity) becomes `Decimal`. Used by
    /// scalar functions that don't have a stronger type opinion (ROUND, ABS,
    /// literals, ...).
    pub fn number_smart(n: f64) -> DaxValue {
        if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
            DaxValue::Integer(n as i64)
        } else {
            DaxValue::Decimal(n)
        }
    }

    pub fn is_blank(&self) -> bool {
        matches!(self, DaxValue::Blank)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            DaxValue::Integer(_) | DaxValue::Decimal(_) | DaxValue::Currency(_) | DaxValue::Boolean(_) | DaxValue::Blank
        )
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DaxValue::Blank => Some(0.0),
            DaxValue::Integer(n) => Some(*n as f64),
            DaxValue::Decimal(n) => Some(*n),
            DaxValue::Currency(c) => Some(*c as f64 / 10_000.0),
            DaxValue::DateTime(s) => Some(*s),
            DaxValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            DaxValue::Text(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        }
    }

    /// Date value (DAX serial, days since 1899-12-30) for either a native
    /// `DateTime` or a `Text` cell holding an ISO date/datetime string. Lets
    /// date arithmetic work uniformly whether a column was synced with an
    /// explicit `date` type or still holds legacy ISO text.
    pub fn as_date_serial(&self) -> Option<f64> {
        match self {
            DaxValue::DateTime(s) => Some(*s),
            DaxValue::Text(s) => functions::parse_iso_to_serial(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            DaxValue::Blank => false,
            DaxValue::Boolean(b) => *b,
            DaxValue::Integer(n) => *n != 0,
            DaxValue::Decimal(n) => *n != 0.0,
            DaxValue::Currency(c) => *c != 0,
            DaxValue::DateTime(s) => *s != 0.0,
            DaxValue::Text(s) => {
                let l = s.trim().to_lowercase();
                !l.is_empty() && l != "false" && l != "0" && l != "não" && l != "nao"
            }
        }
    }

    pub fn as_display_string(&self) -> String {
        match self {
            DaxValue::Blank => String::new(),
            DaxValue::Integer(n) => n.to_string(),
            DaxValue::Decimal(n) => {
                if (n.fract()).abs() < 1e-12 && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            DaxValue::Currency(c) => {
                let v = *c as f64 / 10_000.0;
                if (v.fract()).abs() < 1e-12 {
                    format!("{}", v as i64)
                } else {
                    format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            DaxValue::DateTime(s) => functions::serial_to_iso(*s),
            DaxValue::Text(s) => s.clone(),
            DaxValue::Boolean(b) => b.to_string(),
        }
    }

    /// String-keyed dedup key (BTreeMap/HashSet grouping, LOOKUPVALUE,
    /// SUMMARIZE row identity, ...). Not used for `FilterContext` membership
    /// anymore (see `value_key`), kept for all the places that merely need a
    /// stable, order-friendly identity string for a cell.
    pub fn canonical_key(&self) -> String {
        match self {
            DaxValue::Blank => "__BLANK__".to_string(),
            DaxValue::Integer(n) => format!("n:{}", n),
            DaxValue::Decimal(n) => {
                if (n.fract()).abs() < 1e-9 {
                    format!("n:{}", n.round() as i64)
                } else {
                    format!("n:{:.6}", n)
                }
            }
            DaxValue::Currency(c) => {
                let v = *c as f64 / 10_000.0;
                if (v.fract()).abs() < 1e-9 {
                    format!("n:{}", v.round() as i64)
                } else {
                    format!("n:{:.6}", v)
                }
            }
            DaxValue::DateTime(s) => format!("t:{}", functions::serial_to_iso(*s).get(..10).unwrap_or("").to_string()),
            DaxValue::Boolean(b) => format!("b:{}", b),
            DaxValue::Text(s) => {
                let t = s.trim();
                if let Ok(n) = t.parse::<f64>() {
                    if n.is_finite() {
                        return DaxValue::number_smart(n).canonical_key();
                    }
                }
                if t.is_ascii()
                    && t.len() >= 19
                    && (t.as_bytes()[10] == b'T' || t.as_bytes()[10] == b' ')
                    && t[11..].starts_with("00:00:00")
                {
                    let date_part = &t[..10];
                    if date_part.bytes().enumerate().all(|(i, b)| {
                        if i == 4 || i == 7 {
                            b == b'-'
                        } else {
                            b.is_ascii_digit()
                        }
                    }) {
                        return format!("t:{}", date_part);
                    }
                }
                format!("t:{}", t.to_lowercase())
            }
        }
    }

    /// Typed, hashable filter-set key. Numeric kinds (Integer/Decimal/
    /// Currency/Boolean) normalize to the same `Num` bucket so `2024` and
    /// `2024.0` and a `$2024.00` currency all match one slicer selection.
    /// Dates normalize to `Date` regardless of whether the cell is a native
    /// `DateTime` or legacy ISO `Text`. A `Text` cell that doesn't parse as a
    /// number or a date becomes a lowercase `Text` key.
    pub fn value_key(&self) -> ValueKey {
        match self {
            DaxValue::Blank => ValueKey::Blank,
            DaxValue::Boolean(b) => ValueKey::Bool(*b),
            DaxValue::Integer(n) => ValueKey::Num(n.saturating_mul(1_000_000)),
            DaxValue::Decimal(n) => ValueKey::Num((n * 1_000_000.0).round() as i64),
            DaxValue::Currency(c) => ValueKey::Num(((*c as f64 / 10_000.0) * 1_000_000.0).round() as i64),
            DaxValue::DateTime(s) => ValueKey::Date((*s * 86_400.0).round() as i64),
            // Estrito: texto só casa com texto (comparação sem diferenciar
            // maiúsculas, como no DAX). A conversão de valores vindos do JSON
            // (slicers, filtros de visual) é feita pelo tipo da coluna em
            // `build_filter_context`, não adivinhando aqui.
            DaxValue::Text(s) => {
                let t = s.trim();
                if t.is_empty() {
                    ValueKey::Blank
                } else {
                    ValueKey::Text(t.to_lowercase())
                }
            }
        }
    }

    fn num_kind(&self) -> NumKind {
        match self {
            DaxValue::Currency(_) => NumKind::Currency,
            DaxValue::Decimal(_) | DaxValue::DateTime(_) => NumKind::Decimal,
            DaxValue::Integer(_) | DaxValue::Boolean(_) | DaxValue::Blank => NumKind::Integer,
            DaxValue::Text(s) => match s.trim().replace(',', ".").parse::<f64>() {
                Ok(n) if n.fract() == 0.0 && n.is_finite() => NumKind::Integer,
                _ => NumKind::Decimal,
            },
        }
    }

    fn currency_scaled(&self) -> i64 {
        match self {
            DaxValue::Currency(c) => *c,
            DaxValue::Integer(n) => n.saturating_mul(10_000),
            _ => (self.as_f64().unwrap_or(0.0) * 10_000.0).round() as i64,
        }
    }

    /// `+` with DAX type promotion: Integer+Integer stays Integer, anything
    /// touching Currency stays Currency (exact when both sides are already
    /// Currency/Integer), anything touching Decimal (or non-integral Text)
    /// promotes to Decimal. BLANK coerces to 0 when paired with a non-blank
    /// value, and BLANK + BLANK = BLANK.
    pub fn dax_add(&self, other: &DaxValue) -> DaxValue {
        if self.is_blank() && other.is_blank() {
            return DaxValue::Blank;
        }
        match self.num_kind().max(other.num_kind()) {
            NumKind::Currency => DaxValue::Currency(self.currency_scaled() + other.currency_scaled()),
            NumKind::Decimal => DaxValue::Decimal(self.as_f64().unwrap_or(0.0) + other.as_f64().unwrap_or(0.0)),
            NumKind::Integer => DaxValue::Integer(self.as_f64().unwrap_or(0.0).round() as i64 + other.as_f64().unwrap_or(0.0).round() as i64),
        }
    }

    pub fn dax_sub(&self, other: &DaxValue) -> DaxValue {
        if self.is_blank() && other.is_blank() {
            return DaxValue::Blank;
        }
        match self.num_kind().max(other.num_kind()) {
            NumKind::Currency => DaxValue::Currency(self.currency_scaled() - other.currency_scaled()),
            NumKind::Decimal => DaxValue::Decimal(self.as_f64().unwrap_or(0.0) - other.as_f64().unwrap_or(0.0)),
            NumKind::Integer => DaxValue::Integer(self.as_f64().unwrap_or(0.0).round() as i64 - other.as_f64().unwrap_or(0.0).round() as i64),
        }
    }

    /// `*` with promotion: Currency*Integer (either order) stays Currency.
    pub fn dax_mul(&self, other: &DaxValue) -> DaxValue {
        match self.num_kind().max(other.num_kind()) {
            NumKind::Currency => {
                let a = self.as_f64().unwrap_or(0.0);
                let b = other.as_f64().unwrap_or(0.0);
                DaxValue::Currency((a * b * 10_000.0).round() as i64)
            }
            NumKind::Decimal => DaxValue::Decimal(self.as_f64().unwrap_or(0.0) * other.as_f64().unwrap_or(0.0)),
            NumKind::Integer => DaxValue::Integer(self.as_f64().unwrap_or(0.0).round() as i64 * other.as_f64().unwrap_or(0.0).round() as i64),
        }
    }

    /// Raw `/` (IEEE754 semantics, can yield Infinity/NaN; `DIVIDE()` handles
    /// the safe BLANK/alternate-result case). Division always yields a
    /// `Decimal` per DAX type rules, even Currency/Currency.
    pub fn dax_div(&self, other: &DaxValue) -> DaxValue {
        DaxValue::Decimal(self.as_f64().unwrap_or(0.0) / other.as_f64().unwrap_or(0.0))
    }

    /// Unary minus, preserving the operand's numeric kind.
    pub fn dax_neg(&self) -> DaxValue {
        match self {
            DaxValue::Integer(n) => DaxValue::Integer(-n),
            DaxValue::Decimal(n) => DaxValue::Decimal(-n),
            DaxValue::Currency(c) => DaxValue::Currency(-c),
            DaxValue::Blank => DaxValue::Blank,
            other => DaxValue::number_smart(-(other.as_f64().unwrap_or(0.0))),
        }
    }

    /// DAX loose equality (`=`): BLANK coerces per operand type (BLANK=0 is
    /// TRUE, BLANK="" is TRUE), numeric kinds compare by value regardless of
    /// representation, Text compares case/trim-insensitively, and a numeric
    /// Text still matches its numeric counterpart ("1" = 1).
    pub fn dax_equals(&self, other: &DaxValue) -> bool {
        match (self, other) {
            (DaxValue::Blank, DaxValue::Blank) => true,
            (DaxValue::Blank, DaxValue::Text(s)) | (DaxValue::Text(s), DaxValue::Blank) => s.trim().is_empty(),
            (DaxValue::Blank, other) | (other, DaxValue::Blank) if other.is_numeric() => {
                other.as_f64().unwrap_or(0.0) == 0.0
            }
            (DaxValue::Boolean(a), DaxValue::Boolean(b)) => a == b,
            (DaxValue::Text(a), DaxValue::Text(b)) => a.trim().eq_ignore_ascii_case(b.trim()),
            _ if self.is_numeric() && other.is_numeric() => {
                (self.as_f64().unwrap_or(0.0) - other.as_f64().unwrap_or(0.0)).abs() < 1e-9
            }
            (DaxValue::DateTime(_), _) | (_, DaxValue::DateTime(_)) => {
                match (self.as_date_serial(), other.as_date_serial()) {
                    (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                    _ => self.canonical_key() == other.canonical_key(),
                }
            }
            _ => self.canonical_key() == other.canonical_key(),
        }
    }

    pub fn strict_equals(&self, other: &DaxValue) -> bool {
        match (self, other) {
            (DaxValue::Blank, DaxValue::Blank) => true,
            (DaxValue::Blank, _) | (_, DaxValue::Blank) => false,
            _ => self.dax_equals(other),
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            DaxValue::Blank => JsonValue::Null,
            DaxValue::Integer(n) => JsonValue::Number((*n).into()),
            DaxValue::Decimal(n) => serde_json::Number::from_f64(*n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            DaxValue::Currency(c) => serde_json::Number::from_f64(*c as f64 / 10_000.0)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            DaxValue::DateTime(s) => {
                if s.is_finite() {
                    JsonValue::String(functions::serial_to_iso(*s))
                } else {
                    JsonValue::Null
                }
            }
            DaxValue::Text(s) => JsonValue::String(s.clone()),
            DaxValue::Boolean(b) => JsonValue::Bool(*b),
        }
    }

    /// Untyped JSON -> DaxValue, with best-effort inference when no column
    /// type is known: integral numbers become `Integer`, fractional numbers
    /// `Decimal`, and strings shaped like `YYYY-MM-DD` or
    /// `YYYY-MM-DDT00:00:00` become `DateTime`. Everything else stays `Text`.
    pub fn from_json(v: &JsonValue) -> Self {
        Self::from_json_as(v, None)
    }

    /// JSON -> DaxValue honoring an explicit (or inferred) column type. When
    /// `col_type` is `None`, falls back to the same inference `from_json`
    /// uses.
    pub fn from_json_as(v: &JsonValue, col_type: Option<ColumnType>) -> Self {
        match v {
            JsonValue::Null => DaxValue::Blank,
            JsonValue::Bool(b) => DaxValue::Boolean(*b),
            JsonValue::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                match col_type {
                    Some(ColumnType::Currency) => DaxValue::Currency((f * 10_000.0).round() as i64),
                    Some(ColumnType::Date) => DaxValue::DateTime(f),
                    Some(ColumnType::Decimal) => DaxValue::Decimal(f),
                    Some(ColumnType::Boolean) => DaxValue::Boolean(f != 0.0),
                    Some(ColumnType::Text) => DaxValue::Text(n.to_string()),
                    Some(ColumnType::Integer) | None => DaxValue::number_smart(f),
                }
            }
            JsonValue::String(s) => {
                if s.is_empty() {
                    return DaxValue::Blank;
                }
                match col_type {
                    Some(ColumnType::Date) => functions::parse_iso_to_serial(s)
                        .map(DaxValue::DateTime)
                        .unwrap_or_else(|| DaxValue::Text(s.clone())),
                    Some(ColumnType::Currency) => s
                        .trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .map(|f| DaxValue::Currency((f * 10_000.0).round() as i64))
                        .unwrap_or_else(|_| DaxValue::Text(s.clone())),
                    Some(ColumnType::Boolean) => {
                        let l = s.trim().to_lowercase();
                        DaxValue::Boolean(l == "true" || l == "1" || l == "sim")
                    }
                    Some(ColumnType::Integer) | Some(ColumnType::Decimal) => s
                        .trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .map(DaxValue::number_smart)
                        .unwrap_or_else(|_| DaxValue::Text(s.clone())),
                    Some(ColumnType::Text) => DaxValue::Text(s.clone()),
                    None => {
                        if is_iso_date_like(s) {
                            if let Some(serial) = functions::parse_iso_to_serial(s) {
                                return DaxValue::DateTime(serial);
                            }
                        }
                        DaxValue::Text(s.clone())
                    }
                }
            }
            other => DaxValue::Text(other.to_string()),
        }
    }
}

/// `YYYY-MM-DD` or `YYYY-MM-DDT00:00:00` (time-of-day all zero) — the shapes
/// the legacy ISO-text date representation always used, so untyped inference
/// treats them as dates without touching real free-text columns.
fn is_iso_date_like(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 10 || !t.is_char_boundary(10) {
        return false;
    }
    let date_part = &t[..10];
    let shape_ok = date_part.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    if !shape_ok {
        return false;
    }
    if t.len() == 10 {
        return functions::parse_iso_ymd(date_part).is_some();
    }
    if t.len() >= 19 && (t.as_bytes()[10] == b'T' || t.as_bytes()[10] == b' ') && t[11..].starts_with("00:00:00") {
        return functions::parse_iso_ymd(date_part).is_some();
    }
    false
}

#[derive(Debug, Clone)]
pub struct DaxTableData {
    pub name: String,
    pub columns: Vec<String>,
    pub col_index: HashMap<String, usize>,
    pub col_types: Vec<ColumnType>,
    pub rows: Vec<Vec<DaxValue>>,
    /// Map of lowercased column name -> sortByColumn name (e.g. "mês" -> "MêsNum").
    pub sort_by_columns: HashMap<String, String>,
    /// True for a materialized calculated table (CALENDAR, CALENDARAUTO,
    /// SUMMARIZE, ADDCOLUMNS, UNION, ..., or a TMDL `calculated` partition).
    /// CALENDARAUTO must skip these when scanning `model.tables` for date
    /// columns (dax.guide/calendarauto: "CALENDARAUTO ignores calculated
    /// tables and calculated columns"). Defaults to `false`; synthetic
    /// query-time tables (VALUES/FILTER/... intermediate results) are never
    /// calculated *model* tables, so they also default to `false`.
    pub is_calculated: bool,
}

impl DaxTableData {
    pub fn from_json_rows(name: &str, columns: Vec<String>, raw_rows: &[serde_json::Map<String, JsonValue>]) -> Self {
        Self::from_json_rows_typed(name, columns, raw_rows, &[])
    }

    /// Builds table data, honoring per-column declared types where given
    /// (`declared_types[i]` aligned with `columns[i]`, `None`/out-of-range
    /// means "infer"). When a column has no declared type, infers one from
    /// its values: all-integral numbers -> Integer, ISO date-shaped strings
    /// -> Date, otherwise falls back to per-cell inference (`DaxValue::from_json`).
    pub fn from_json_rows_typed(
        name: &str,
        columns: Vec<String>,
        raw_rows: &[serde_json::Map<String, JsonValue>],
        declared_types: &[Option<String>],
    ) -> Self {
        let mut col_index = HashMap::new();
        for (idx, c) in columns.iter().enumerate() {
            col_index.insert(c.to_lowercase(), idx);
        }
        let col_types: Vec<ColumnType> = columns
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                let declared = declared_types.get(idx).and_then(|o| o.as_deref());
                ColumnType::from_str_opt(declared)
                    .map(|ct| refine_declared_number_type(ct, declared, c, raw_rows))
                    .unwrap_or_else(|| infer_column_type(c, raw_rows))
            })
            .collect();
        let rows = raw_rows
            .iter()
            .map(|rmap| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| {
                        rmap.get(c)
                            .map(|v| DaxValue::from_json_as(v, Some(col_types[idx])))
                            .unwrap_or(DaxValue::Blank)
                    })
                    .collect()
            })
            .collect();
        Self {
            name: name.to_string(),
            columns,
            col_index,
            col_types,
            rows,
            sort_by_columns: HashMap::new(),
            is_calculated: false,
        }
    }

    pub fn get_col_idx(&self, col_name: &str) -> Option<usize> {
        self.col_index.get(&col_name.to_lowercase()).copied()
    }

    pub fn sort_by_col_idx(&self, col_idx: usize) -> Option<usize> {
        let col_name = self.columns.get(col_idx)?;
        let sb_name = self.sort_by_columns.get(&col_name.to_lowercase())?;
        self.get_col_idx(sb_name)
    }
}

/// `'number'` is ambiguous between Integer/Decimal in the frontend's type
/// vocabulary; refine it from the column's own data the same way inference
/// would, so an explicitly-declared `number` column with only integral
/// values still ends up `Integer` (matters for `+`/`-` promotion and
/// `to_json` shape).
fn refine_declared_number_type(
    declared: ColumnType,
    raw: Option<&str>,
    col_name: &str,
    raw_rows: &[serde_json::Map<String, JsonValue>],
) -> ColumnType {
    if raw != Some("number") || declared != ColumnType::Decimal {
        return declared;
    }
    let mut all_integral = true;
    let mut any = false;
    for r in raw_rows {
        if let Some(JsonValue::Number(n)) = r.get(col_name) {
            any = true;
            if n.as_f64().map(|f| f.fract() != 0.0).unwrap_or(true) {
                all_integral = false;
                break;
            }
        }
    }
    if any && all_integral {
        ColumnType::Integer
    } else {
        ColumnType::Decimal
    }
}

fn infer_column_type(col_name: &str, raw_rows: &[serde_json::Map<String, JsonValue>]) -> ColumnType {
    let mut saw_number = false;
    let mut saw_string = false;
    let mut saw_other = false;
    let mut all_integral = true;
    let mut all_iso_date = true;
    for r in raw_rows {
        match r.get(col_name) {
            None | Some(JsonValue::Null) => {}
            Some(JsonValue::Number(n)) => {
                saw_number = true;
                if n.as_f64().map(|f| f.fract() != 0.0).unwrap_or(true) {
                    all_integral = false;
                }
            }
            Some(JsonValue::String(s)) => {
                saw_string = true;
                if !is_iso_date_like(s) {
                    all_iso_date = false;
                }
            }
            _ => saw_other = true,
        }
    }
    if saw_number && !saw_string && !saw_other {
        if all_integral {
            ColumnType::Integer
        } else {
            ColumnType::Decimal
        }
    } else if saw_string && !saw_number && !saw_other && all_iso_date {
        ColumnType::Date
    } else {
        ColumnType::Text // per-cell DaxValue::from_json still infers Integer/Decimal/Date individually
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxRelationshipInput {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub cross_filter_direction: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxMeasureInput {
    pub name: String,
    pub table_name: String,
    pub formula: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxTableInput {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Map<String, JsonValue>>,
    /// Per-column data type, aligned by index with `columns`. Optional: a
    /// shorter vec, missing entries (`None`) or the whole field being
    /// absent (old frontend payloads) fall back to type inference. Values:
    /// `"text" | "number" | "date" | "boolean" | "currency" | "percentage"`.
    #[serde(default)]
    pub column_types: Vec<Option<String>>,
    /// When set, this table's data comes from the DuckDB table of this name
    /// (ALL rows, not the frontend's `PREVIEW_ROW_LIMIT`-capped preview).
    /// `rows` is then ignored (the frontend sends `[]`). Tables without a
    /// backing DuckDB table (sample data, manual entry, inline REST, etc.)
    /// keep using `rows`. See `docs/RUST_ENGINE_2.md` §3 step 2.
    #[serde(default)]
    pub duckdb_table: Option<String>,
    /// Optional column name -> sortByColumn name metadata for chronological/custom ordering.
    #[serde(default)]
    pub sort_by_columns: HashMap<String, String>,
    /// True for a calculated table (TMDL `calculated` partition / "Nova
    /// tabela" with a DAX table expression). `rows`/`columns` are ignored;
    /// the table is materialized from `expression` by
    /// `calculated::materialize_calculated_tables`. CALENDARAUTO must skip
    /// these tables when scanning for date columns (dax.guide/calendarauto).
    #[serde(default)]
    pub is_calculated: bool,
    /// DAX table expression for a calculated table (CALENDAR, CALENDARAUTO,
    /// SUMMARIZE, SUMMARIZECOLUMNS, ADDCOLUMNS, UNION, FILTER, DISTINCT,
    /// DATATABLE, ...). Only meaningful when `is_calculated` is true.
    #[serde(default)]
    pub expression: Option<String>,
    /// Calculated columns on this table: lowercased column name -> DAX row
    /// expression (RELATED/RELATEDTABLE/EARLIER in scope). Values for these
    /// columns in `rows` are ignored; they're computed per-row by
    /// `calculated::materialize_calculated_columns`.
    #[serde(default)]
    pub calculated_columns: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxModelSyncPayload {
    pub tables: Vec<DaxTableInput>,
    pub relationships: Vec<DaxRelationshipInput>,
    pub measures: Vec<DaxMeasureInput>,
}

#[derive(Debug, Clone)]
pub struct RelOverride {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub activate: Option<bool>,
    pub direction: Option<String>,
}

/// §2.3: a filter over a *combination* of columns of one table (FILTER over
/// several columns, SUMMARIZE/VALUES used directly as a filter, multi-column
/// TREATAS). Unlike independent per-column filters — which AND each column's
/// allowed set separately and so let through combinations that were never
/// actually in the source table (e.g. (CategoriaA, SubB) when only
/// (CategoriaA, SubA) and (CategoriaB, SubB) exist) — a `TupleFilter` only
/// keeps rows whose exact tuple of values appears in `tuples`.
#[derive(Debug, Clone)]
pub struct TupleFilter {
    /// Lowercase table name.
    pub table: String,
    /// Lowercase column names, in a fixed order matching each tuple's order.
    pub columns: Vec<String>,
    pub tuples: HashSet<Vec<ValueKey>>,
}

#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    /// table_lower -> col_lower -> allowed value keys
    pub filters: HashMap<String, HashMap<String, HashSet<ValueKey>>>,
    /// v0.11 Frente H (item h): table_lower -> col_lower -> inclusive [lo, hi]
    /// range (segmentador "Entre", relative-date windows). Kept separate from
    /// `filters` because a range can't be expressed as a finite allowed set.
    pub range_filters: HashMap<String, HashMap<String, (ValueKey, ValueKey)>>,
    /// Dynamic USERELATIONSHIP / CROSSFILTER overrides applied inside CALCULATE
    pub rel_overrides: Vec<RelOverride>,
    /// §2.3: multi-column tuple filters (see `TupleFilter`), on top of the
    /// per-column `filters` above.
    pub tuple_filters: Vec<TupleFilter>,
    /// §2.3/§3: the filter state this query started from (the visual's own
    /// filters/slicers, before any CALCULATE), for ALLSELECTED. `None` means
    /// "this context IS the shadow" (set once by `build_filter_context`);
    /// every CALCULATE clones the same `Arc` forward untouched.
    pub shadow: Option<Arc<FilterContext>>,
}

impl FilterContext {
    pub fn set_column_filter(&mut self, table: &str, col: &str, allowed: HashSet<ValueKey>) {
        let t = table.to_lowercase();
        let c = col.to_lowercase();
        self.tuple_filters.retain(|tf| !(tf.table == t && tf.columns.contains(&c)));
        self.filters.entry(t).or_default().insert(c, allowed);
    }

    /// v0.11 Frente H (item h): inclusive range filter (`lo <= value <= hi`),
    /// for the segmentador "Entre" and relative-date windows.
    pub fn set_range_filter(&mut self, table: &str, col: &str, lo: ValueKey, hi: ValueKey) {
        let t = table.to_lowercase();
        let c = col.to_lowercase();
        self.tuple_filters.retain(|tf| !(tf.table == t && tf.columns.contains(&c)));
        self.range_filters.entry(t).or_default().insert(c, (lo, hi));
    }

    /// Replaces (or adds) the tuple filter with this exact `(table, columns)`
    /// signature; any single-column filter already on one of those columns
    /// stays (a tuple filter narrows further, it doesn't replace column filters
    /// on *other* columns).
    pub fn set_tuple_filter(&mut self, table: &str, columns: &[String], tuples: HashSet<Vec<ValueKey>>) {
        let t = table.to_lowercase();
        let cols: Vec<String> = columns.iter().map(|c| c.to_lowercase()).collect();
        self.tuple_filters.retain(|tf| !(tf.table == t && tf.columns == cols));
        self.tuple_filters.push(TupleFilter { table: t, columns: cols, tuples });
    }

    pub fn clear_table(&mut self, table: &str) {
        let t = table.to_lowercase();
        self.filters.remove(&t);
        self.tuple_filters.retain(|tf| tf.table != t);
    }

    pub fn clear_column(&mut self, table: &str, col: &str) {
        let t = table.to_lowercase();
        let c = col.to_lowercase();
        if let Some(cols) = self.filters.get_mut(&t) {
            cols.remove(&c);
        }
        // ALL(col) on a column that participates in a tuple filter drops the
        // whole tuple constraint (losing the other columns' restriction too):
        // a simplification of the DAX marginalization rule, documented in
        // docs/RUST_ENGINE_2.md §2.3.
        self.tuple_filters.retain(|tf| !(tf.table == t && tf.columns.contains(&c)));
    }

    pub fn clear_except(&mut self, table: &str, keep_cols: &[String]) {
        let t = table.to_lowercase();
        let keep: HashSet<String> = keep_cols.iter().map(|s| s.to_lowercase()).collect();
        if let Some(cols) = self.filters.get_mut(&t) {
            cols.retain(|k, _| keep.contains(k));
        }
        self.tuple_filters
            .retain(|tf| tf.table != t || tf.columns.iter().all(|c| keep.contains(c)));
    }

    /// Context ALLSELECTED should restore: this query's starting filter
    /// state (shadow), or `self` when `self` already IS the shadow (no
    /// CALCULATE has run yet since `build_filter_context`).
    pub fn shadow_ctx(&self) -> &FilterContext {
        self.shadow.as_deref().unwrap_or(self)
    }
}

pub struct DaxModel {
    pub tables: HashMap<String, DaxTableData>,
    pub relationships: Vec<DaxRelationshipInput>,
    pub measures: HashMap<String, DaxMeasureInput>,
    /// Chaves de filtro pré-calculadas no sync: tabela -> coluna -> linha.
    /// A propagação de filtros compara milhões de células por consulta; sem
    /// isso cada comparação de texto alocava um `to_lowercase()`.
    key_cache: HashMap<String, Vec<Vec<ValueKey>>>,
    /// DuckDB version each DAX table key was last loaded at (só para
    /// `duckdb_table`-backed tables), so a repeated sync with the same
    /// version can reuse `tables`/`key_cache` instead of reloading.
    duckdb_table_versions: HashMap<String, u64>,
    /// table_key (lowercase) -> set of calculated column names (lowercase)
    /// on that table. CALENDARAUTO (and anything else honoring
    /// "ignores calculated tables and calculated columns") must skip these.
    pub calculated_columns: HashMap<String, HashSet<String>>,
    /// Errors from materializing calculated tables/columns during the last
    /// sync, keyed by `"table"` (table-level, e.g. a dependency cycle) or
    /// `"table.column"` (a calculated column's DAX failed to evaluate).
    /// Surfaced inline by the UI's formula editor (see `src/dax/calculated`).
    pub calculated_errors: HashMap<String, String>,
    /// v0.11 Frente A (pushdown): DAX table key (lowercase) -> the DuckDB
    /// table name that physically backs it (`DaxTableInput::duckdb_table`).
    /// Tables without a DuckDB source (sample data, manual entry...) have no
    /// entry here, which is exactly the signal `pushdown` needs to bail out
    /// to the DAX engine (no physical table to push SQL down to).
    duckdb_table_names: HashMap<String, String>,
}

impl Default for DaxModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DaxModel {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            relationships: Vec::new(),
            measures: HashMap::new(),
            key_cache: HashMap::new(),
            duckdb_table_versions: HashMap::new(),
            calculated_columns: HashMap::new(),
            calculated_errors: HashMap::new(),
            duckdb_table_names: HashMap::new(),
        }
    }

    fn build_key_cache(tbl: &DaxTableData) -> Vec<Vec<ValueKey>> {
        (0..tbl.columns.len())
            .map(|c| tbl.rows.iter().map(|r| r.get(c).map(DaxValue::value_key).unwrap_or(ValueKey::Blank)).collect())
            .collect()
    }

    /// Chaves da coluna `col` da tabela `tkey` (minúscula), do cache quando
    /// ele está consistente com a tabela, senão calculadas na hora.
    fn column_keys<'a>(&'a self, tkey: &str, tbl: &DaxTableData, col: usize) -> std::borrow::Cow<'a, [ValueKey]> {
        match self.key_cache.get(tkey).and_then(|cols| cols.get(col)) {
            Some(keys) if keys.len() == tbl.rows.len() => std::borrow::Cow::Borrowed(keys.as_slice()),
            _ => std::borrow::Cow::Owned(tbl.rows.iter().map(|r| r[col].value_key()).collect()),
        }
    }

    pub fn sync_from_payload(&mut self, payload: DaxModelSyncPayload) {
        self.sync_from_payload_with_duckdb(payload, HashMap::new());
    }

    /// Like `sync_from_payload`, but tables whose `duckdb_table` was
    /// (re)loaded by the caller are passed in `freshly_loaded`
    /// (duckdb-table-name.lowercase() -> (version, data)). Tables with a
    /// `duckdb_table` NOT present in `freshly_loaded` are assumed unchanged
    /// since the last sync and reuse the previously cached `DaxTableData`
    /// when one exists; if none exists yet (first sync, or the DuckDB table
    /// was missing), they fall back to the JSON `rows` sent in the payload,
    /// with a warning on stderr — this is the project-reopen path, since
    /// DuckDB is in-memory and does not yet have the table right after a
    /// project loads (see `open_pbip_project`/`sync_model_to_rust` callers).
    pub fn sync_from_payload_with_duckdb(
        &mut self,
        payload: DaxModelSyncPayload,
        mut freshly_loaded: HashMap<String, (u64, DaxTableData)>,
    ) {
        let mut new_tables = HashMap::new();
        let mut new_key_cache = HashMap::new();
        let mut new_versions = HashMap::new();
        // Calculated tables (TMDL `calculated` partition / "Nova tabela")
        // aren't built from `rows`; they're materialized below, in
        // dependency order, once every non-calculated table is in place.
        let mut calc_table_inputs: Vec<crate::dax::calculated::CalcTableInput> = Vec::new();
        // table_key -> (col_key -> expression), for every table (base or
        // calculated) that has calculated columns.
        let mut calc_column_inputs: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut new_duckdb_names = HashMap::new();
        for t in payload.tables {
            let key = t.name.to_lowercase();
            if let Some(duckdb_name) = &t.duckdb_table {
                new_duckdb_names.insert(key.clone(), duckdb_name.clone());
            }
            if !t.calculated_columns.is_empty() {
                calc_column_inputs.insert(
                    key.clone(),
                    t.calculated_columns.iter().map(|(c, e)| (c.to_lowercase(), e.clone())).collect(),
                );
            }
            if t.is_calculated {
                calc_table_inputs.push(crate::dax::calculated::CalcTableInput {
                    name: t.name.clone(),
                    expression: t.expression.clone().unwrap_or_default(),
                });
                continue;
            }
            let sort_by: HashMap<String, String> = t
                .sort_by_columns
                .iter()
                .map(|(c, sb)| (c.to_lowercase(), sb.clone()))
                .collect();
            if let Some(duckdb_name) = &t.duckdb_table {
                let dkey = duckdb_name.to_lowercase();
                if let Some((version, mut data)) = freshly_loaded.remove(&dkey) {
                    data.sort_by_columns = sort_by;
                    new_key_cache.insert(key.clone(), Self::build_key_cache(&data));
                    new_versions.insert(key.clone(), version);
                    new_tables.insert(key, data);
                    continue;
                }
                if let (Some(mut old_data), Some(&old_version)) =
                    (self.tables.remove(&key), self.duckdb_table_versions.get(&key))
                {
                    old_data.sort_by_columns = sort_by;
                    // old_data is byte-for-byte the same as last sync (version
                    // unchanged), so its previously computed key cache is
                    // still valid; only rebuild if it somehow wasn't cached.
                    let cache = self
                        .key_cache
                        .remove(&key)
                        .unwrap_or_else(|| Self::build_key_cache(&old_data));
                    new_key_cache.insert(key.clone(), cache);
                    new_versions.insert(key.clone(), old_version);
                    new_tables.insert(key, old_data);
                    continue;
                }
                eprintln!(
                    "aviso: tabela DuckDB '{}' (para a tabela do modelo '{}') não pôde ser carregada; \
                     usando as linhas de amostra enviadas pelo frontend como fallback temporário",
                    duckdb_name, t.name
                );
            }
            let mut data = DaxTableData::from_json_rows_typed(&t.name, t.columns.clone(), &t.rows, &t.column_types);
            data.sort_by_columns = sort_by;
            new_key_cache.insert(key.clone(), Self::build_key_cache(&data));
            new_tables.insert(key, data);
        }
        self.tables = new_tables;
        self.key_cache = new_key_cache;
        self.duckdb_table_versions = new_versions;
        self.duckdb_table_names = new_duckdb_names;
        self.relationships = payload.relationships;
        self.measures.clear();
        for m in payload.measures {
            self.measures.insert(m.name.to_lowercase(), m);
        }
        self.calculated_columns = calc_column_inputs
            .iter()
            .map(|(t, cols)| (t.clone(), cols.keys().cloned().collect()))
            .collect();
        self.calculated_errors.clear();
        let table_errors = crate::dax::calculated::materialize_calculated_tables(self, calc_table_inputs);
        self.calculated_errors.extend(table_errors);
        let column_errors = crate::dax::calculated::materialize_calculated_columns(self, &calc_column_inputs);
        self.calculated_errors.extend(column_errors);
    }

    pub fn get_table(&self, name: &str) -> Option<&DaxTableData> {
        self.tables.get(&name.trim().to_lowercase())
    }

    /// Inserts a freshly materialized calculated table (`dax::calculated`)
    /// into the model, rebuilding its filter-key cache so later evaluations
    /// (including dependent calculated tables/columns) see it immediately.
    pub fn insert_calculated_table(&mut self, key: &str, data: DaxTableData) {
        let cache = Self::build_key_cache(&data);
        self.key_cache.insert(key.to_string(), cache);
        self.tables.insert(key.to_string(), data);
    }

    pub fn get_measure(&self, name: &str) -> Option<&DaxMeasureInput> {
        self.measures.get(&name.trim().to_lowercase())
    }

    /// v0.11 Frente A (pushdown): the DuckDB table name backing `dax_table`
    /// (by its DAX model name, any case), if any. `None` means this table
    /// has no physical DuckDB table (sample/manual data) and pushdown must
    /// fall back to the DAX engine.
    pub fn duckdb_table_name(&self, dax_table: &str) -> Option<&str> {
        self.duckdb_table_names
            .get(&dax_table.trim().to_lowercase())
            .map(String::as_str)
    }

    /// §3 item 3 / blank member: true when some active many-to-one relationship
    /// into `dim_table` has fact rows whose (non-blank) FK value doesn't match
    /// any row of the dimension's key column — i.e. VertiPaq would add a blank
    /// row to `dim_table`. VALUES()/ADDCOLUMNS over it must surface that blank
    /// member; ALL/DISTINCT and the physical table itself never do.
    pub fn has_orphan_member(&self, dim_table: &str) -> bool {
        self.relationships.iter().any(|rel| {
            rel.is_active
                && rel.to_table.eq_ignore_ascii_case(dim_table)
                && (|| -> Option<bool> {
                    let dim = self.get_table(dim_table)?;
                    let dim_cidx = dim.get_col_idx(&rel.to_column)?;
                    let fact = self.get_table(&rel.from_table)?;
                    let fact_cidx = fact.get_col_idx(&rel.from_column)?;
                    let dim_keys: HashSet<ValueKey> =
                        dim.rows.iter().map(|r| r[dim_cidx].value_key()).collect();
                    Some(fact.rows.iter().any(|r| {
                        let k = r[fact_cidx].value_key();
                        !matches!(k, ValueKey::Blank) && !dim_keys.contains(&k)
                    }))
                })()
                .unwrap_or(false)
        })
    }

    /// Computes visible row indices for `target_table` under `ctx` by propagating active relationships.
    pub fn get_visible_row_indices(&self, target_table: &str, ctx: &FilterContext) -> Vec<usize> {
        let Some(tbl) = self.get_table(target_table) else {
            return Vec::new();
        };
        if ctx.filters.is_empty() && ctx.tuple_filters.is_empty() && ctx.range_filters.is_empty() {
            return (0..tbl.rows.len()).collect();
        }

        // Start with direct column filters on every table
        let mut table_masks: HashMap<String, Vec<bool>> = HashMap::new();
        for (tkey, tdata) in &self.tables {
            let mut mask = vec![true; tdata.rows.len()];
            if let Some(col_filters) = ctx.filters.get(tkey) {
                for (ckey, allowed) in col_filters {
                    if let Some(cidx) = tdata.get_col_idx(ckey) {
                        let keys = self.column_keys(tkey, tdata, cidx);
                        for (r_idx, k) in keys.iter().enumerate() {
                            if mask[r_idx] && !allowed.contains(k) {
                                mask[r_idx] = false;
                            }
                        }
                    }
                }
            }
            // v0.11 Frente H (item h): inclusive range ("Entre"/relative-date).
            // A BLANK value never satisfies a range (same as DAX: BLANK is
            // excluded from `x >= lo && x <= hi` unless lo/hi themselves
            // coerce it in, which we don't special-case here).
            if let Some(col_ranges) = ctx.range_filters.get(tkey) {
                for (ckey, (lo, hi)) in col_ranges {
                    if let Some(cidx) = tdata.get_col_idx(ckey) {
                        let keys = self.column_keys(tkey, tdata, cidx);
                        for (r_idx, k) in keys.iter().enumerate() {
                            if mask[r_idx] && (matches!(k, ValueKey::Blank) || k < lo || k > hi) {
                                mask[r_idx] = false;
                            }
                        }
                    }
                }
            }
            table_masks.insert(tkey.clone(), mask);
        }

        // §2.3: tuple filters (FILTER over several columns, SUMMARIZE/VALUES used
        // directly as a filter, multi-column TREATAS) keep only rows whose exact
        // tuple of values was in the source table — a per-column AND would let
        // through combinations that never actually occurred together.
        for tf in &ctx.tuple_filters {
            let Some(tdata) = self.tables.get(&tf.table) else { continue };
            let cidxs: Vec<usize> = tf.columns.iter().filter_map(|c| tdata.get_col_idx(c)).collect();
            if cidxs.len() != tf.columns.len() {
                continue;
            }
            if let Some(mask) = table_masks.get_mut(&tf.table) {
                for (r_idx, row) in tdata.rows.iter().enumerate() {
                    if mask[r_idx] {
                        let key: Vec<ValueKey> = cidxs.iter().map(|&ci| row[ci].value_key()).collect();
                        if !tf.tuples.contains(&key) {
                            mask[r_idx] = false;
                        }
                    }
                }
            }
        }

        // Effective relationships after USERELATIONSHIP / CROSSFILTER overrides
        let mut effective_rels: Vec<(usize, DaxRelationshipInput)> = Vec::with_capacity(self.relationships.len());
        let activated_pairs: Vec<(String, String)> = ctx
            .rel_overrides
            .iter()
            .filter(|o| o.activate == Some(true))
            .map(|o| (o.from_table.to_lowercase(), o.to_table.to_lowercase()))
            .collect();

        for (rel_idx, rel) in self.relationships.iter().enumerate() {
            let mut eff = rel.clone();
            let from_l = rel.from_table.to_lowercase();
            let to_l = rel.to_table.to_lowercase();

            // If USERELATIONSHIP activated another relationship between this same table pair, temporarily deactivate this one
            let pair_activated_elsewhere = activated_pairs
                .iter()
                .any(|(a, b)| (a == &from_l && b == &to_l) || (a == &to_l && b == &from_l));
            if pair_activated_elsewhere && eff.is_active {
                eff.is_active = false;
            }

            for ov in &ctx.rel_overrides {
                let fwd = ov.from_table.eq_ignore_ascii_case(&rel.from_table)
                    && ov.from_column.eq_ignore_ascii_case(&rel.from_column)
                    && ov.to_table.eq_ignore_ascii_case(&rel.to_table)
                    && ov.to_column.eq_ignore_ascii_case(&rel.to_column);
                let rev = ov.from_table.eq_ignore_ascii_case(&rel.to_table)
                    && ov.from_column.eq_ignore_ascii_case(&rel.to_column)
                    && ov.to_table.eq_ignore_ascii_case(&rel.from_table)
                    && ov.to_column.eq_ignore_ascii_case(&rel.from_column);
                if fwd || rev {
                    if let Some(act) = ov.activate {
                        eff.is_active = act;
                    }
                    if let Some(ref dir) = ov.direction {
                        if dir.eq_ignore_ascii_case("none") {
                            eff.is_active = false;
                        } else {
                            eff.cross_filter_direction = dir.clone();
                        }
                    }
                }
            }
            if eff.is_active {
                effective_rels.push((rel_idx, eff));
            }
        }

        // Propagate filters along relationships (1/to -> N/from always, and from -> to if bidirectional)
        // Track traversed relationships so a bidirectional edge never bounces propagated filters
        // backward from a sparse table (e.g. monthly opening balances) onto the calendar table.
        let mut queue: VecDeque<String> = ctx
            .filters
            .keys()
            .cloned()
            .chain(ctx.tuple_filters.iter().map(|tf| tf.table.clone()))
            .chain(ctx.range_filters.keys().cloned())
            .collect();
        let mut used_rels: HashSet<usize> = HashSet::new();
        let mut hops = 0usize;
        while let Some(src_key) = queue.pop_front() {
            if hops > 64 {
                break;
            }
            hops += 1;

            for &(rel_idx, ref rel) in &effective_rels {
                if used_rels.contains(&rel_idx) {
                    continue;
                }
                let from_k = rel.from_table.to_lowercase();
                let to_k = rel.to_table.to_lowercase();

                // Dimension (to_table) -> Fact (from_table)
                if src_key == to_k {
                    if let (Some(to_tbl), Some(from_tbl)) =
                        (self.tables.get(&to_k), self.tables.get(&from_k))
                    {
                        if let (Some(to_col), Some(from_col)) = (
                            to_tbl.get_col_idx(&rel.to_column),
                            from_tbl.get_col_idx(&rel.from_column),
                        ) {
                            // Dimensão -> fato pode ser reaplicada sempre que a
                            // dimensão encolher de novo (máscaras só encolhem, então
                            // termina). Só a volta fato -> dimensão é marcada como
                            // usada, para evitar o "bounce-back" bidirecional.
                            let to_mask = &table_masks[&to_k];
                            let to_keys = self.column_keys(&to_k, to_tbl, to_col);
                            let allowed_keys: HashSet<&ValueKey> = to_keys
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| to_mask[*i])
                                .map(|(_, k)| k)
                                .collect();

                            let from_keys = self.column_keys(&from_k, from_tbl, from_col);
                            let from_mask = table_masks.get_mut(&from_k).unwrap();
                            let mut changed = false;
                            for (r_idx, k) in from_keys.iter().enumerate() {
                                if from_mask[r_idx] && !allowed_keys.contains(k) {
                                    from_mask[r_idx] = false;
                                    changed = true;
                                }
                            }
                            if changed {
                                queue.push_back(from_k.clone());
                            }
                        }
                    }
                }

                // Fact (from_table) -> Dimension (to_table) when crossFilterDirection == "both"
                if src_key == from_k && rel.cross_filter_direction.eq_ignore_ascii_case("both") {
                    if let (Some(from_tbl), Some(to_tbl)) =
                        (self.tables.get(&from_k), self.tables.get(&to_k))
                    {
                        if let (Some(from_col), Some(to_col)) = (
                            from_tbl.get_col_idx(&rel.from_column),
                            to_tbl.get_col_idx(&rel.to_column),
                        ) {
                            used_rels.insert(rel_idx);
                            let from_mask = &table_masks[&from_k];
                            let from_keys = self.column_keys(&from_k, from_tbl, from_col);
                            let allowed_keys: HashSet<&ValueKey> = from_keys
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| from_mask[*i])
                                .map(|(_, k)| k)
                                .collect();

                            let to_keys = self.column_keys(&to_k, to_tbl, to_col);
                            let to_mask = table_masks.get_mut(&to_k).unwrap();
                            let mut changed = false;
                            for (r_idx, k) in to_keys.iter().enumerate() {
                                if to_mask[r_idx] && !allowed_keys.contains(k) {
                                    to_mask[r_idx] = false;
                                    changed = true;
                                }
                            }
                            if changed {
                                queue.push_back(to_k);
                            }
                        }
                    }
                }
            }
        }

        let target_mask = &table_masks[&target_table.to_lowercase()];
        target_mask
            .iter()
            .enumerate()
            .filter_map(|(idx, &vis)| if vis { Some(idx) } else { None })
            .collect()
    }

    pub fn evaluate_dax(&self, expression: &str, ctx: &FilterContext) -> Result<DaxValue, String> {
        // v0.11 Frente G: a fresh top-level evaluation starts a fresh window
        // partition cache (`evaluator::window`'s `WINDOW_PARTITION_CACHE`) —
        // see its doc comment. This keeps the cache's lifetime scoped to
        // exactly one query/measure evaluation, never leaking a cached
        // partition into an unrelated later evaluation.
        evaluator::window::clear_window_partition_cache();
        let ast = parser::parse_dax(expression)?;
        evaluator::evaluate_ast(&ast, self, ctx, &[], &HashMap::new())
    }
}

#[cfg(test)]
mod type_rule_tests {
    use super::*;

    #[test]
    fn integer_plus_integer_stays_integer() {
        assert_eq!(DaxValue::Integer(2).dax_add(&DaxValue::Integer(3)), DaxValue::Integer(5));
    }

    #[test]
    fn division_always_decimal() {
        assert_eq!(DaxValue::Integer(10).dax_div(&DaxValue::Integer(4)), DaxValue::Decimal(2.5));
        match DaxValue::Currency(50_000).dax_div(&DaxValue::Currency(20_000)) {
            DaxValue::Decimal(v) => assert!((v - 2.5).abs() < 1e-9),
            other => panic!("expected Decimal, got {:?}", other),
        }
    }

    #[test]
    fn currency_times_integer_stays_currency_and_rounds_to_4_decimals() {
        // $10.3333 * 3 = $30.9999, stored scaled by 10_000 -> exact cents-ish precision
        let price = DaxValue::Currency(103_333); // 10.3333
        let qty = DaxValue::Integer(3);
        match price.dax_mul(&qty) {
            DaxValue::Currency(c) => assert_eq!(c, 309_999),
            other => panic!("expected Currency, got {:?}", other),
        }
    }

    #[test]
    fn decimal_poisons_the_result() {
        assert_eq!(DaxValue::Integer(1).dax_add(&DaxValue::Decimal(0.5)), DaxValue::Decimal(1.5));
    }

    #[test]
    fn blank_arithmetic_stays_blank_when_both_sides_or_unary_are_blank() {
        assert_eq!(DaxValue::Blank.dax_add(&DaxValue::Blank), DaxValue::Blank);
        assert_eq!(DaxValue::Blank.dax_sub(&DaxValue::Blank), DaxValue::Blank);
        assert_eq!(DaxValue::Blank.dax_neg(), DaxValue::Blank);
        assert_eq!(DaxValue::Blank.dax_add(&DaxValue::Integer(1)), DaxValue::Integer(1));
    }

    #[test]
    fn blank_coercion_matches_dax() {
        // BLANK + 1 = 1
        assert_eq!(DaxValue::Blank.dax_add(&DaxValue::Integer(1)), DaxValue::Integer(1));
        // BLANK & "a" = "a"
        assert_eq!(format!("{}{}", DaxValue::Blank.as_display_string(), "a"), "a");
        // BLANK = 0 is TRUE (loose equality)
        assert!(DaxValue::Blank.dax_equals(&DaxValue::Integer(0)));
        // BLANK == 0 is FALSE (strict equality never matches BLANK)
        assert!(!DaxValue::Blank.strict_equals(&DaxValue::Integer(0)));
    }

    #[test]
    fn datetime_arithmetic_and_serial_roundtrip() {
        let d = functions::parse_iso_to_serial("2024-03-01").unwrap();
        let dt = DaxValue::DateTime(d);
        // DateTime + 1 -> next day
        let next = dt.as_date_serial().unwrap() + 1.0;
        assert_eq!(functions::serial_to_iso(next), "2024-03-02");
        // DateTime - DateTime -> Decimal days
        let d2 = functions::parse_iso_to_serial("2024-03-10").unwrap();
        let days = d2 - d;
        assert!((days - 9.0).abs() < 1e-9);
        // Round trip through JSON stays an ISO date (time = 0)
        assert_eq!(dt.to_json(), JsonValue::String("2024-03-01".to_string()));
    }

    #[test]
    fn datetime_with_time_renders_iso_datetime_in_json() {
        let serial = functions::parse_iso_to_serial("2024-03-01T13:30:00").unwrap();
        let dt = DaxValue::DateTime(serial);
        assert_eq!(dt.to_json(), JsonValue::String("2024-03-01T13:30:00".to_string()));
    }

    #[test]
    fn value_key_unifies_numeric_representations_for_filters() {
        assert_eq!(DaxValue::Integer(2024).value_key(), DaxValue::Decimal(2024.0).value_key());
        // Texto "2024" não é o número 2024 (era o bug do canonical_key).
        assert_ne!(DaxValue::Text("2024".into()).value_key(), DaxValue::Integer(2024).value_key());
        assert_ne!(DaxValue::Text("2024".into()).value_key(), DaxValue::DateTime(2024.0).value_key());
        assert_eq!(DaxValue::Text(" Ana ".into()).value_key(), DaxValue::Text("ana".into()).value_key());
        // Valores de filtro chegam como JSON e são convertidos pelo tipo da coluna.
        let a = DaxValue::from_json_as(&JsonValue::from("2024-03-01"), Some(ColumnType::Date)).value_key();
        let b = DaxValue::DateTime(functions::parse_iso_to_serial("2024-03-01").unwrap()).value_key();
        assert_eq!(a, b);
        let n = DaxValue::from_json_as(&JsonValue::from("2024"), Some(ColumnType::Integer)).value_key();
        assert_eq!(n, DaxValue::Integer(2024).value_key());
    }

    #[test]
    fn currency_from_json_rounds_to_4_decimals() {
        let v = DaxValue::from_json_as(&JsonValue::from(10.123456), Some(ColumnType::Currency));
        match v {
            DaxValue::Currency(c) => assert_eq!(c, 101_235), // rounds 10.123456 -> 10.1235
            other => panic!("expected Currency, got {:?}", other),
        }
    }

    #[test]
    fn column_type_inference_from_sync_payload() {
        let raw_rows = vec![
            serde_json::json!({"Qtd": 1, "Valor": 10.5, "Data": "2024-01-01"}).as_object().unwrap().clone(),
            serde_json::json!({"Qtd": 2, "Valor": 20.0, "Data": "2024-01-02"}).as_object().unwrap().clone(),
        ];
        let tbl = DaxTableData::from_json_rows_typed(
            "t",
            vec!["Qtd".into(), "Valor".into(), "Data".into()],
            &raw_rows,
            &[],
        );
        assert!(matches!(tbl.rows[0][tbl.get_col_idx("Qtd").unwrap()], DaxValue::Integer(1)));
        assert!(matches!(tbl.rows[0][tbl.get_col_idx("Valor").unwrap()], DaxValue::Decimal(_)));
        assert!(matches!(tbl.rows[0][tbl.get_col_idx("Data").unwrap()], DaxValue::DateTime(_)));
    }
}

#[cfg(test)]
mod propagation_tests {
    use super::*;
    use serde_json::json;

    fn rows(cols: &[&str], data: &[&[serde_json::Value]]) -> Vec<serde_json::Map<String, JsonValue>> {
        data.iter()
            .map(|r| cols.iter().zip(r.iter()).map(|(c, v)| (c.to_string(), v.clone())).collect())
            .collect()
    }

    /// Floco de neve: f_Vendas -> d_Produto -> d_Categoria. Com filtro em
    /// d_Categoria E em d_Produto, a ordem de processamento vinha da iteração
    /// de HashMap (semente aleatória): se d_Produto saía primeiro, a aresta
    /// Produto->Vendas era marcada como usada e o filtro de Categoria, ao chegar
    /// depois em d_Produto, nunca descia até a fato.
    fn snowflake() -> DaxModel {
        let mut m = DaxModel::new();
        m.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput { duckdb_table: None,
                    name: "d_Categoria".into(),
                    columns: vec!["CatID".into(), "Nome".into()],
                    rows: rows(&["CatID", "Nome"], &[&[json!(1), json!("A")], &[json!(2), json!("B")]]),
                    column_types: vec![],
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
                DaxTableInput { duckdb_table: None,
                    name: "d_Produto".into(),
                    columns: vec!["ProdID".into(), "CatID".into(), "Cor".into()],
                    rows: rows(
                        &["ProdID", "CatID", "Cor"],
                        &[
                            &[json!(10), json!(1), json!("azul")],
                            &[json!(11), json!(2), json!("azul")],
                            &[json!(12), json!(1), json!("verde")],
                        ],
                    ),
                    column_types: vec![],
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
                DaxTableInput { duckdb_table: None,
                    name: "f_Vendas".into(),
                    columns: vec!["ProdID".into(), "Valor".into()],
                    rows: rows(
                        &["ProdID", "Valor"],
                        &[&[json!(10), json!(1)], &[json!(11), json!(10)], &[json!(12), json!(100)]],
                    ),
                    column_types: vec![],
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "f_Vendas".into(),
                    from_column: "ProdID".into(),
                    to_table: "d_Produto".into(),
                    to_column: "ProdID".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "d_Produto".into(),
                    from_column: "CatID".into(),
                    to_table: "d_Categoria".into(),
                    to_column: "CatID".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
            ],
            measures: vec![],
        });
        m
    }

    #[test]
    fn snowflake_propagation_is_order_independent() {
        for _ in 0..64 {
            let model = snowflake();
            let ctx = build_filter_context(
                &[
                    NativeFilterInput { table_id: "d_Categoria".into(), column: "Nome".into(), values: vec![json!("A")], ..Default::default() },
                    NativeFilterInput { table_id: "d_Produto".into(), column: "Cor".into(), values: vec![json!("azul")], ..Default::default() },
                ],
                &model,
            );
            // Só o produto 10 é categoria A e azul.
            let v = model.evaluate_dax("SUM(f_Vendas[Valor])", &ctx).unwrap();
            assert_eq!(v, DaxValue::Integer(1));
        }
    }
}

#[cfg(test)]
mod bench_tests {
    use super::*;
    use serde_json::json;
    use std::time::Instant;

    /// 200k-row fact with 3 dimensions, SUM filtered by 2 dimension columns.
    /// Not a true before/after (the untyped `DaxValue::Number`/`String`-keyed
    /// filter representation this PR replaces no longer exists on this
    /// branch to compare against), but gives a concrete number for the typed
    /// `DaxValue` + `ValueKey` path. Run with:
    /// `cargo test --lib bench_200k -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_200k_fact_sum_with_two_dimension_filters() {
        const DIM_ROWS: i64 = 1000;
        const FACT_ROWS: i64 = 200_000;

        let dim_rows = |col: &str| -> Vec<serde_json::Map<String, JsonValue>> {
            (0..DIM_ROWS)
                .map(|i| {
                    let mut m = serde_json::Map::new();
                    m.insert("Id".into(), json!(i));
                    m.insert(col.into(), json!(format!("{}-{}", col, i % 50)));
                    m
                })
                .collect()
        };

        let fact_rows: Vec<serde_json::Map<String, JsonValue>> = (0..FACT_ROWS)
            .map(|i| {
                let mut m = serde_json::Map::new();
                m.insert("Dim1Id".into(), json!(i % DIM_ROWS));
                m.insert("Dim2Id".into(), json!((i * 7) % DIM_ROWS));
                m.insert("Dim3Id".into(), json!((i * 13) % DIM_ROWS));
                m.insert("Valor".into(), json!((i % 1000) as f64 * 1.5));
                m
            })
            .collect();

        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput { duckdb_table: None, name: "Dim1".into(), columns: vec!["Id".into(), "Grupo1".into()], rows: dim_rows("Grupo1"), column_types: vec![], sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new()  },
                DaxTableInput { duckdb_table: None, name: "Dim2".into(), columns: vec!["Id".into(), "Grupo2".into()], rows: dim_rows("Grupo2"), column_types: vec![], sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new()  },
                DaxTableInput { duckdb_table: None, name: "Dim3".into(), columns: vec!["Id".into(), "Grupo3".into()], rows: dim_rows("Grupo3"), column_types: vec![], sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new()  },
                DaxTableInput { duckdb_table: None,
                    name: "Fato".into(),
                    columns: vec!["Dim1Id".into(), "Dim2Id".into(), "Dim3Id".into(), "Valor".into()],
                    rows: fact_rows,
                    column_types: vec![],
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
            ],
            relationships: vec![
                DaxRelationshipInput { from_table: "Fato".into(), from_column: "Dim1Id".into(), to_table: "Dim1".into(), to_column: "Id".into(), cross_filter_direction: "single".into(), is_active: true },
                DaxRelationshipInput { from_table: "Fato".into(), from_column: "Dim2Id".into(), to_table: "Dim2".into(), to_column: "Id".into(), cross_filter_direction: "single".into(), is_active: true },
                DaxRelationshipInput { from_table: "Fato".into(), from_column: "Dim3Id".into(), to_table: "Dim3".into(), to_column: "Id".into(), cross_filter_direction: "single".into(), is_active: true },
            ],
            measures: vec![],
        });

        let ctx = build_filter_context(
            &[
                NativeFilterInput { table_id: "Dim1".into(), column: "Grupo1".into(), values: vec![json!("Grupo1-7")], ..Default::default() },
                NativeFilterInput { table_id: "Dim2".into(), column: "Grupo2".into(), values: vec![json!("Grupo2-49")], ..Default::default() }, // i%50==7 => (7i)%50==49: resultado não vazio
            ],
            &model,
        );

        const ITERS: u32 = 20;
        let t0 = Instant::now();
        let mut last = DaxValue::Blank;
        for _ in 0..ITERS {
            last = model.evaluate_dax("SUM(Fato[Valor])", &ctx).unwrap();
        }
        let elapsed = t0.elapsed();
        println!(
            "bench_200k: {} rows fato, 3 dims, SUM + 2 filters: {:.3} ms/query (avg of {}) -> {:?}",
            FACT_ROWS,
            elapsed.as_secs_f64() * 1000.0 / ITERS as f64,
            ITERS,
            last
        );
        assert!(!last.is_blank(), "o benchmark deve medir um filtro com linhas visíveis");
    }
}
