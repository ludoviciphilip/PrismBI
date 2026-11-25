pub mod ast;
pub mod evaluator;
pub mod functions;
pub mod lexer;
pub mod parser;
pub mod query_planner;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};

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
        let allowed: HashSet<ValueKey> = f
            .values
            .iter()
            .map(|v| DaxValue::from_json_as(v, col_type).value_key())
            .collect();
        if !allowed.is_empty() {
            ctx.set_column_filter(&f.table_id, &f.column, allowed);
        }
    }
    ctx
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeFilterInput {
    pub table_id: String,
    pub column: String,
    pub values: Vec<JsonValue>,
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
            DaxValue::Text(s) => {
                let t = s.trim();
                if t.is_empty() {
                    return ValueKey::Blank;
                }
                if let Ok(n) = t.replace(',', ".").parse::<f64>() {
                    if n.is_finite() {
                        return ValueKey::Num((n * 1_000_000.0).round() as i64);
                    }
                }
                if let Some(serial) = functions::parse_iso_to_serial(t) {
                    return ValueKey::Date((serial * 86_400.0).round() as i64);
                }
                ValueKey::Text(t.to_lowercase())
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
    /// promotes to Decimal. BLANK coerces to 0.
    pub fn dax_add(&self, other: &DaxValue) -> DaxValue {
        match self.num_kind().max(other.num_kind()) {
            NumKind::Currency => DaxValue::Currency(self.currency_scaled() + other.currency_scaled()),
            NumKind::Decimal => DaxValue::Decimal(self.as_f64().unwrap_or(0.0) + other.as_f64().unwrap_or(0.0)),
            NumKind::Integer => DaxValue::Integer(self.as_f64().unwrap_or(0.0).round() as i64 + other.as_f64().unwrap_or(0.0).round() as i64),
        }
    }

    pub fn dax_sub(&self, other: &DaxValue) -> DaxValue {
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
            DaxValue::Blank => DaxValue::Integer(0),
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
        }
    }

    pub fn get_col_idx(&self, col_name: &str) -> Option<usize> {
        self.col_index.get(&col_name.to_lowercase()).copied()
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    /// table_lower -> col_lower -> allowed value keys
    pub filters: HashMap<String, HashMap<String, HashSet<ValueKey>>>,
    /// Dynamic USERELATIONSHIP / CROSSFILTER overrides applied inside CALCULATE
    pub rel_overrides: Vec<RelOverride>,
}

impl FilterContext {
    pub fn set_column_filter(&mut self, table: &str, col: &str, allowed: HashSet<ValueKey>) {
        self.filters
            .entry(table.to_lowercase())
            .or_default()
            .insert(col.to_lowercase(), allowed);
    }

    pub fn clear_table(&mut self, table: &str) {
        self.filters.remove(&table.to_lowercase());
    }

    pub fn clear_column(&mut self, table: &str, col: &str) {
        if let Some(cols) = self.filters.get_mut(&table.to_lowercase()) {
            cols.remove(&col.to_lowercase());
        }
    }

    pub fn clear_except(&mut self, table: &str, keep_cols: &[String]) {
        let keep: HashSet<String> = keep_cols.iter().map(|s| s.to_lowercase()).collect();
        if let Some(cols) = self.filters.get_mut(&table.to_lowercase()) {
            cols.retain(|k, _| keep.contains(k));
        }
    }
}

pub struct DaxModel {
    pub tables: HashMap<String, DaxTableData>,
    pub relationships: Vec<DaxRelationshipInput>,
    pub measures: HashMap<String, DaxMeasureInput>,
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
        }
    }

    pub fn sync_from_payload(&mut self, payload: DaxModelSyncPayload) {
        self.tables.clear();
        for t in payload.tables {
            let key = t.name.to_lowercase();
            self.tables.insert(
                key,
                DaxTableData::from_json_rows_typed(&t.name, t.columns, &t.rows, &t.column_types),
            );
        }
        self.relationships = payload.relationships;
        self.measures.clear();
        for m in payload.measures {
            self.measures.insert(m.name.to_lowercase(), m);
        }
    }

    pub fn get_table(&self, name: &str) -> Option<&DaxTableData> {
        self.tables.get(&name.trim().to_lowercase())
    }

    pub fn get_measure(&self, name: &str) -> Option<&DaxMeasureInput> {
        self.measures.get(&name.trim().to_lowercase())
    }

    /// Computes visible row indices for `target_table` under `ctx` by propagating active relationships.
    pub fn get_visible_row_indices(&self, target_table: &str, ctx: &FilterContext) -> Vec<usize> {
        let Some(tbl) = self.get_table(target_table) else {
            return Vec::new();
        };
        if ctx.filters.is_empty() {
            return (0..tbl.rows.len()).collect();
        }

        // Start with direct column filters on every table
        let mut table_masks: HashMap<String, Vec<bool>> = HashMap::new();
        for (tkey, tdata) in &self.tables {
            let mut mask = vec![true; tdata.rows.len()];
            if let Some(col_filters) = ctx.filters.get(tkey) {
                for (ckey, allowed) in col_filters {
                    if let Some(cidx) = tdata.get_col_idx(ckey) {
                        for (r_idx, row) in tdata.rows.iter().enumerate() {
                            if mask[r_idx] && !allowed.contains(&row[cidx].value_key()) {
                                mask[r_idx] = false;
                            }
                        }
                    }
                }
            }
            table_masks.insert(tkey.clone(), mask);
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
        let mut queue: VecDeque<String> = ctx.filters.keys().cloned().collect();
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
                            let allowed_keys: HashSet<ValueKey> = to_tbl
                                .rows
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| to_mask[*i])
                                .map(|(_, r)| r[to_col].value_key())
                                .collect();

                            let from_mask = table_masks.get_mut(&from_k).unwrap();
                            let mut changed = false;
                            for (r_idx, row) in from_tbl.rows.iter().enumerate() {
                                if from_mask[r_idx] && !allowed_keys.contains(&row[from_col].value_key()) {
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
                            let allowed_keys: HashSet<ValueKey> = from_tbl
                                .rows
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| from_mask[*i])
                                .map(|(_, r)| r[from_col].value_key())
                                .collect();

                            let to_mask = table_masks.get_mut(&to_k).unwrap();
                            let mut changed = false;
                            for (r_idx, row) in to_tbl.rows.iter().enumerate() {
                                if to_mask[r_idx] && !allowed_keys.contains(&row[to_col].value_key()) {
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
        assert_eq!(DaxValue::Text("2024".into()).value_key(), DaxValue::Integer(2024).value_key());
        // Text "2024" (a year) must NOT collide with a date whose serial happens to differ
        assert_ne!(DaxValue::Text("2024".into()).value_key(), DaxValue::DateTime(2024.0).value_key());
        let a = DaxValue::Text("2024-03-01".into()).value_key();
        let b = DaxValue::DateTime(functions::parse_iso_to_serial("2024-03-01").unwrap()).value_key();
        assert_eq!(a, b);
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
                DaxTableInput {
                    name: "d_Categoria".into(),
                    columns: vec!["CatID".into(), "Nome".into()],
                    rows: rows(&["CatID", "Nome"], &[&[json!(1), json!("A")], &[json!(2), json!("B")]]),
                    column_types: vec![],
                },
                DaxTableInput {
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
                },
                DaxTableInput {
                    name: "f_Vendas".into(),
                    columns: vec!["ProdID".into(), "Valor".into()],
                    rows: rows(
                        &["ProdID", "Valor"],
                        &[&[json!(10), json!(1)], &[json!(11), json!(10)], &[json!(12), json!(100)]],
                    ),
                    column_types: vec![],
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
                    NativeFilterInput { table_id: "d_Categoria".into(), column: "Nome".into(), values: vec![json!("A")] },
                    NativeFilterInput { table_id: "d_Produto".into(), column: "Cor".into(), values: vec![json!("azul")] },
                ],
                &model,
            );
            // Só o produto 10 é categoria A e azul.
            let v = model.evaluate_dax("SUM(f_Vendas[Valor])", &ctx).unwrap();
            assert_eq!(v, DaxValue::Integer(1));
        }
    }
}
