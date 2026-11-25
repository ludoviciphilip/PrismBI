//! `EtlQuery` → DuckDB SQL: a chain of CTEs, one per step, reading from
//! `raw__<query>`.
//!
//! Safety: every identifier goes through [`quote_ident`] and every value
//! through [`lit_str`] / [`lit_scalar`]; nothing coming from the user is ever
//! interpolated raw. Numbers are formatted by Rust from parsed `f64`/`i64`.
//!
//! Hidden columns carried through every CTE:
//! - `__row_idx` (BIGINT): row order. Raw tables materialize it; every step
//!   either keeps it or recomputes it (sort, group, unpivot, joins, append),
//!   and the final SELECT orders by it. This gives `fillDown`/`fillUp`,
//!   `keepTopRows`, `removeDuplicates` (first occurrence) and
//!   `promoteHeaders` (first row) a stable, Power-Query-like order.
//! - `__err__<col>` (BOOLEAN): "this cell is an error". See
//!   [`convert_expr`] / `changeType`: Power Query keeps a failed conversion as
//!   an *error value*, distinct from `null`. DuckDB has no error values, so
//!   `changeType` uses `TRY_CAST`-style conversions (NULL on failure) and sets
//!   `__err__<col> = TRUE` exactly when the source value was **not null, not
//!   an empty/blank string, and the conversion produced NULL**. A genuine
//!   NULL (or "") source gives NULL with the flag FALSE. Excel error cells
//!   (`#N/A`, `#DIV/0!`, …) are loaded as NULL with the flag TRUE.
//!   `removeRowsWithErrors` drops rows whose flags (all, or the listed
//!   columns') are TRUE. Flags follow renames/duplicates/merges and are
//!   dropped by reshaping steps (group, pivot, unpivot, append).

use super::custom_expr;
use super::model::*;
use crate::engine::duckdb_store::execute_select_query;
use duckdb::Connection;
use serde_json::Value as JsonValue;
use std::collections::HashSet;

pub const ROW_IDX: &str = "__row_idx";
pub const ERR_PREFIX: &str = "__err__";
const CTE_PREFIX: &str = "__etl_s";
const MAX_PIVOT_VALUES: usize = 1000;

/// `"name"` with embedded quotes doubled.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn qi(name: &str) -> String {
    quote_ident(name)
}

/// SQL string literal: `'…'` with `'` doubled. NUL bytes (which cannot be
/// embedded in a C string) are spliced in with `chr(0)`.
pub fn lit_str(s: &str) -> String {
    if s.contains('\0') {
        let parts: Vec<String> = s
            .split('\0')
            .map(|p| format!("'{}'", p.replace('\'', "''")))
            .collect();
        format!("({})", parts.join(" || chr(0) || "))
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

/// SQL numeric literal (always parses back to the same value).
pub fn lit_num(n: f64) -> Result<String, String> {
    if !n.is_finite() {
        return Err("número não finito não suportado".into());
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        Ok(format!("{}", n as i64))
    } else {
        Ok(format!("CAST({} AS DOUBLE)", lit_str(&format!("{:e}", n))))
    }
}

pub fn lit_scalar(s: &Scalar) -> Result<String, String> {
    Ok(match s {
        Scalar::Null => "NULL".into(),
        Scalar::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
        Scalar::Number(n) => lit_num(*n)?,
        Scalar::Text(t) => lit_str(t),
    })
}

pub fn raw_table_name(query: &str) -> String {
    format!("raw__{}", query)
}

pub fn flag_name(col: &str) -> String {
    format!("{}{}", ERR_PREFIX, col)
}

pub fn is_hidden(col: &str) -> bool {
    col == ROW_IDX || col.starts_with(ERR_PREFIX)
}

/// Rejects names the engine reserves or that DuckDB cannot carry.
pub fn check_user_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("nome de coluna vazio".into());
    }
    if name.contains('\0') {
        return Err("nome de coluna com caractere NUL".into());
    }
    if is_hidden(name) || name.starts_with(CTE_PREFIX) || name == "__rid" || name == "__src" || name == "__ord" {
        return Err(format!("o nome '{}' é reservado pelo motor ETL", name));
    }
    Ok(())
}

// ------------------------------------------------------------ type helpers

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cat {
    Text,
    Int,
    Float,
    Decimal,
    Date,
    Timestamp,
    Bool,
    Null,
    Other,
}

impl Cat {
    pub fn is_numeric(self) -> bool {
        matches!(self, Cat::Int | Cat::Float | Cat::Decimal)
    }
}

pub fn cat_of(duck_ty: &str) -> Cat {
    let t = duck_ty.trim().to_uppercase();
    match t.as_str() {
        "VARCHAR" | "TEXT" | "STRING" => Cat::Text,
        "BIGINT" | "INTEGER" | "SMALLINT" | "TINYINT" | "HUGEINT" | "UBIGINT" | "UINTEGER" | "USMALLINT"
        | "UTINYINT" | "UHUGEINT" | "INT" | "INT8" | "INT4" => Cat::Int,
        "DOUBLE" | "FLOAT" | "REAL" => Cat::Float,
        "DATE" => Cat::Date,
        "BOOLEAN" | "BOOL" => Cat::Bool,
        "\"NULL\"" | "NULL" => Cat::Null,
        _ if t.starts_with("DECIMAL") || t.starts_with("NUMERIC") => Cat::Decimal,
        _ if t.starts_with("TIMESTAMP") || t == "DATETIME" => Cat::Timestamp,
        _ if t.starts_with("VARCHAR") => Cat::Text,
        _ => Cat::Other,
    }
}

/// Contract type name shown by the preview (`{name, type}`).
pub fn etl_type_name(duck_ty: &str) -> &'static str {
    let t = duck_ty.to_uppercase();
    match cat_of(duck_ty) {
        Cat::Text => "text",
        Cat::Int => "integer",
        Cat::Float => "decimal",
        Cat::Decimal => {
            if t.replace(' ', "").ends_with(",4)") {
                "currency"
            } else {
                "decimal"
            }
        }
        Cat::Date => "date",
        Cat::Timestamp => "datetime",
        Cat::Bool => "boolean",
        Cat::Null | Cat::Other => "text",
    }
}

fn scalar_cat(s: &Scalar) -> Cat {
    match s {
        Scalar::Null => Cat::Null,
        Scalar::Bool(_) => Cat::Bool,
        Scalar::Number(_) => Cat::Float,
        Scalar::Text(_) => Cat::Text,
    }
}

fn cats_compatible(col: Cat, val: Cat) -> bool {
    match (col, val) {
        (_, Cat::Null) | (Cat::Null, _) | (Cat::Other, _) => true,
        (a, b) if a.is_numeric() && b.is_numeric() => true,
        (a, b) => a == b,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateOrder {
    Dmy,
    Mdy,
    Iso,
}

#[derive(Debug, Clone, Copy)]
pub struct Culture {
    dec: char,
    group: Option<char>,
    order: DateOrder,
}

const INVARIANT: Culture = Culture {
    dec: '.',
    group: Some(','),
    order: DateOrder::Iso,
};

/// Static culture table (never the OS/ICU locale).
pub fn culture_info(c: Option<&str>) -> Result<Culture, String> {
    let Some(c) = c else { return Ok(INVARIANT) };
    let lc = c.trim().to_lowercase();
    let lang = lc.split(['-', '_']).next().unwrap_or("");
    Ok(match (lang, lc.as_str()) {
        (_, "en-us") | ("en", "en") => Culture { dec: '.', group: Some(','), order: DateOrder::Mdy },
        ("en", _) => Culture { dec: '.', group: Some(','), order: DateOrder::Dmy },
        ("pt" | "es" | "it" | "de" | "nl" | "id" | "da" | "tr" | "ro" | "el", _) => {
            Culture { dec: ',', group: Some('.'), order: DateOrder::Dmy }
        }
        ("fr" | "ru" | "pl" | "cs" | "sv" | "fi" | "nb" | "no" | "uk" | "hu" | "sk", _) => {
            Culture { dec: ',', group: Some(' '), order: DateOrder::Dmy }
        }
        ("", _) => INVARIANT,
        _ => return Err(format!("cultura '{}' não suportada", c)),
    })
}

/// Text → DOUBLE honouring the culture's separators; NULL when not a number.
fn text_to_double(t: &str, culture: Culture, typed_text: bool) -> String {
    let mut s = format!("replace(replace(trim({}), ' ', ''), chr(160), '')", t);
    if let Some(g) = culture.group {
        if g != ' ' {
            s = format!("replace({}, {}, '')", s, lit_str(&g.to_string()));
        }
    }
    if culture.dec != '.' {
        s = format!("replace({}, {}, '.')", s, lit_str(&culture.dec.to_string()));
    }
    let cultured = format!("TRY_CAST({} AS DOUBLE)", s);
    if typed_text && culture.dec != '.' {
        // Excel/inline raw tables encode typed number cells canonically
        // ("1234.5"); those must not be re-read with the culture's separators.
        format!(
            "CASE WHEN regexp_full_match(trim({t}), '-?[0-9]+(\\.[0-9]+)?([eE][-+]?[0-9]+)?') THEN TRY_CAST(trim({t}) AS DOUBLE) ELSE {c} END",
            t = t,
            c = cultured
        )
    } else {
        cultured
    }
}

fn date_formats(order: DateOrder, with_time: bool) -> Vec<&'static str> {
    let mut v = match order {
        DateOrder::Dmy => vec!["%d/%m/%Y", "%d-%m-%Y", "%d.%m.%Y", "%d/%m/%y"],
        DateOrder::Mdy => vec!["%m/%d/%Y", "%m-%d-%Y", "%m/%d/%y"],
        DateOrder::Iso => vec!["%Y/%m/%d", "%d/%m/%Y"],
    };
    if with_time {
        let base: Vec<&'static str> = match order {
            DateOrder::Dmy => vec!["%d/%m/%Y %H:%M:%S", "%d/%m/%Y %H:%M"],
            DateOrder::Mdy => vec!["%m/%d/%Y %H:%M:%S", "%m/%d/%Y %I:%M:%S %p", "%m/%d/%Y %H:%M"],
            DateOrder::Iso => vec!["%Y/%m/%d %H:%M:%S"],
        };
        let mut out = base;
        out.append(&mut v);
        v = out;
    }
    v
}

fn text_to_timestamp(t: &str, culture: Culture) -> String {
    let mut parts = vec![format!("TRY_CAST(trim({}) AS TIMESTAMP)", t)];
    for f in date_formats(culture.order, true) {
        parts.push(format!("try_strptime(trim({}), {})", t, lit_str(f)));
    }
    format!("COALESCE({})", parts.join(", "))
}

fn text_to_date(t: &str, culture: Culture, typed_text: bool) -> String {
    let mut parts = vec![
        format!("TRY_CAST(trim({}) AS DATE)", t),
        format!("CAST(TRY_CAST(trim({}) AS TIMESTAMP) AS DATE)", t),
    ];
    for f in date_formats(culture.order, false) {
        parts.push(format!("CAST(try_strptime(trim({}), {}) AS DATE)", t, lit_str(f)));
    }
    if typed_text {
        // Canonical number cell read as an Excel serial date.
        parts.push(serial_to_date(&format!(
            "(CASE WHEN regexp_full_match(trim({t}), '-?[0-9]+(\\.[0-9]+)?') THEN TRY_CAST(trim({t}) AS DOUBLE) END)",
            t = t
        )));
    }
    format!("COALESCE({})", parts.join(", "))
}

fn serial_to_date(x: &str) -> String {
    format!(
        "(CASE WHEN abs({x}) < 2958466 THEN CAST(DATE '1899-12-30' + CAST(floor({x}) AS INTEGER) AS DATE) END)",
        x = x
    )
}

fn serial_to_timestamp(x: &str) -> String {
    format!(
        "(CASE WHEN abs({x}) < 2958466 THEN TIMESTAMP '1899-12-30 00:00:00' + to_microseconds(CAST(round({x} * 86400000000) AS BIGINT)) END)",
        x = x
    )
}

fn number_to_text(x: &str, culture: Culture) -> String {
    let s = format!(
        "(CASE WHEN isfinite(CAST({x} AS DOUBLE)) AND CAST({x} AS DOUBLE) = trunc(CAST({x} AS DOUBLE)) AND abs(CAST({x} AS DOUBLE)) < 1e15 THEN CAST(CAST(CAST({x} AS DOUBLE) AS BIGINT) AS VARCHAR) ELSE CAST(CAST({x} AS DOUBLE) AS VARCHAR) END)",
        x = x
    );
    if culture.dec != '.' {
        format!("replace({}, '.', {})", s, lit_str(&culture.dec.to_string()))
    } else {
        s
    }
}

/// Any value → text, as `Text.From(x, culture)` would.
pub fn to_text(x: &str, cat: Cat, culture: Culture) -> String {
    match cat {
        Cat::Text => x.to_string(),
        Cat::Int => format!("CAST({} AS VARCHAR)", x),
        Cat::Float | Cat::Decimal => number_to_text(x, culture),
        Cat::Date => match culture.order {
            DateOrder::Dmy => format!("strftime({}, '%d/%m/%Y')", x),
            DateOrder::Mdy => format!("strftime({}, '%-m/%-d/%Y')", x),
            DateOrder::Iso => format!("strftime({}, '%Y-%m-%d')", x),
        },
        Cat::Timestamp => match culture.order {
            DateOrder::Dmy => format!("strftime({}, '%d/%m/%Y %H:%M:%S')", x),
            DateOrder::Mdy => format!("strftime({}, '%-m/%-d/%Y %-I:%M:%S %p')", x),
            DateOrder::Iso => format!("strftime({}, '%Y-%m-%d %H:%M:%S')", x),
        },
        Cat::Bool => format!("(CASE WHEN {x} THEN 'true' WHEN NOT {x} THEN 'false' END)", x = x),
        Cat::Null => "CAST(NULL AS VARCHAR)".into(),
        Cat::Other => format!("CAST({} AS VARCHAR)", x),
    }
}

/// Conversion used by `changeType` (and `type` arguments): yields NULL when
/// the value cannot be converted, never raises. `typed_text`: the text came
/// from an Excel/inline raw table where typed cells are encoded canonically.
pub fn convert_expr(x: &str, cat: Cat, target: EtlType, culture: Culture, typed_text: bool) -> String {
    let to_double = |x: &str, cat: Cat| -> String {
        match cat {
            Cat::Text => text_to_double(x, culture, typed_text),
            Cat::Int | Cat::Float | Cat::Decimal => format!("CAST({} AS DOUBLE)", x),
            Cat::Bool => format!("(CASE WHEN {x} THEN 1.0 WHEN NOT {x} THEN 0.0 END)", x = x),
            Cat::Date => format!("CAST(({} - DATE '1899-12-30') AS DOUBLE)", x),
            Cat::Timestamp => format!("(epoch({}) / 86400.0 + 25569.0)", x),
            Cat::Null => "CAST(NULL AS DOUBLE)".into(),
            Cat::Other => format!("TRY_CAST({} AS DOUBLE)", x),
        }
    };
    let body = match target {
        EtlType::Text => return to_text(x, cat, culture),
        EtlType::Integer => match cat {
            Cat::Int => format!("CAST({} AS BIGINT)", x),
            _ => format!("TRY_CAST(round_even({}, 0) AS BIGINT)", to_double(x, cat)),
        },
        EtlType::Decimal => to_double(x, cat),
        EtlType::Currency => format!("TRY_CAST(round_even({}, 4) AS DECIMAL(19,4))", to_double(x, cat)),
        EtlType::Date => match cat {
            Cat::Date => x.to_string(),
            Cat::Timestamp => format!("CAST({} AS DATE)", x),
            Cat::Text => text_to_date(x, culture, typed_text),
            Cat::Int | Cat::Float | Cat::Decimal => serial_to_date(&format!("CAST({} AS DOUBLE)", x)),
            Cat::Null => "CAST(NULL AS DATE)".into(),
            Cat::Bool | Cat::Other => format!("TRY_CAST({} AS DATE)", x),
        },
        EtlType::Datetime => match cat {
            Cat::Timestamp => format!("CAST({} AS TIMESTAMP)", x),
            Cat::Date => format!("CAST({} AS TIMESTAMP)", x),
            Cat::Text => text_to_timestamp(x, culture),
            Cat::Int | Cat::Float | Cat::Decimal => serial_to_timestamp(&format!("CAST({} AS DOUBLE)", x)),
            Cat::Null => "CAST(NULL AS TIMESTAMP)".into(),
            Cat::Bool | Cat::Other => format!("TRY_CAST({} AS TIMESTAMP)", x),
        },
        EtlType::Boolean => match cat {
            Cat::Bool => x.to_string(),
            Cat::Text => format!(
                "(CASE lower(trim({x})) WHEN 'true' THEN TRUE WHEN 'false' THEN FALSE END)",
                x = x
            ),
            Cat::Int | Cat::Float | Cat::Decimal => format!("(CAST({} AS DOUBLE) <> 0)", x),
            Cat::Null => "CAST(NULL AS BOOLEAN)".into(),
            _ => format!("TRY_CAST({} AS BOOLEAN)", x),
        },
    };
    if cat == Cat::Text {
        // "" / blank text converts to null (not an error) for non-text targets.
        format!("(CASE WHEN trim({x}) = '' THEN NULL ELSE {b} END)", x = x, b = body)
    } else {
        body
    }
}

/// "was this conversion an error?" (see module docs).
fn conversion_failed(x: &str, cat: Cat, converted: &str) -> String {
    if cat == Cat::Text {
        format!(
            "({x} IS NOT NULL AND trim({x}) <> '' AND ({c}) IS NULL)",
            x = x,
            c = converted
        )
    } else {
        format!("({x} IS NOT NULL AND ({c}) IS NULL)", x = x, c = converted)
    }
}

/// `Text.Proper`: upper-case letters that follow a non-alphanumeric character.
pub fn proper_sql(x: &str) -> String {
    format!(
        "(CASE WHEN {x} IS NULL THEN NULL ELSE array_to_string(list_transform(range(1, length({x}) + 1), lambda __pq_i: CASE WHEN __pq_i = 1 OR NOT regexp_matches(substr({x}, __pq_i - 1, 1), '[\\p{{L}}\\p{{N}}''’]') THEN upper(substr({x}, __pq_i, 1)) ELSE lower(substr({x}, __pq_i, 1)) END), '') END)",
        x = x
    )
}

/// `Text.Trim`: strips all leading/trailing whitespace (not only spaces).
pub fn trim_ws_sql(x: &str) -> String {
    format!("regexp_replace({}, '^\\s+|\\s+$', '', 'g')", x)
}

/// `Text.Clean`: removes control characters.
pub fn clean_sql(x: &str) -> String {
    format!("regexp_replace({}, '[\\x00-\\x1F\\x7F]', '', 'g')", x)
}

// ------------------------------------------------------------ chain

#[derive(Debug, Clone, PartialEq)]
pub struct ColInfo {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct Chain {
    pub ctes: Vec<String>,
    pub schema: Vec<ColInfo>,
}

impl Chain {
    pub fn last_cte(&self) -> String {
        qi(&format!("{}{}", CTE_PREFIX, self.ctes.len() - 1))
    }

    fn with_clause(&self) -> String {
        let parts: Vec<String> = self
            .ctes
            .iter()
            .enumerate()
            .map(|(i, body)| format!("{} AS ({})", qi(&format!("{}{}", CTE_PREFIX, i)), body))
            .collect();
        format!("WITH {}", parts.join(",\n"))
    }

    /// `WITH … SELECT <cols> FROM <last> <tail>`
    pub fn select(&self, cols: &str, tail: &str) -> String {
        format!("{}\nSELECT {} FROM {} {}", self.with_clause(), cols, self.last_cte(), tail)
    }

    pub fn visible(&self) -> Vec<&ColInfo> {
        self.schema.iter().filter(|c| !is_hidden(&c.name)).collect()
    }

    pub fn visible_names(&self) -> Vec<String> {
        self.visible().iter().map(|c| c.name.clone()).collect()
    }

    pub fn col(&self, name: &str) -> Option<&ColInfo> {
        self.schema.iter().find(|c| c.name == name)
    }

    fn has_flag(&self, col: &str) -> bool {
        self.col(&flag_name(col)).is_some()
    }

    fn require(&self, name: &str) -> Result<&ColInfo, String> {
        if is_hidden(name) {
            return Err(format!("A coluna '{}' da tabela não foi encontrada.", name));
        }
        self.col(name)
            .ok_or_else(|| format!("A coluna '{}' da tabela não foi encontrada.", name))
    }

    /// Columns (visible + flags) as a select list, `__row_idx` first.
    fn passthrough(&self) -> Vec<String> {
        let mut out = vec![qi(ROW_IDX)];
        out.extend(self.schema.iter().filter(|c| c.name != ROW_IDX).map(|c| qi(&c.name)));
        out
    }

    fn flags(&self) -> Vec<String> {
        self.schema
            .iter()
            .filter(|c| c.name.starts_with(ERR_PREFIX))
            .map(|c| c.name.clone())
            .collect()
    }
}

pub fn describe(conn: &Connection, sql: &str) -> Result<Vec<ColInfo>, String> {
    let mut stmt = conn
        .prepare(&format!("DESCRIBE {}", sql))
        .map_err(|e| clean_err(&e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ColInfo {
                name: r.get::<_, String>(0)?,
                ty: r.get::<_, String>(1)?,
            })
        })
        .map_err(|e| clean_err(&e.to_string()))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| clean_err(&e.to_string()))
}

/// DuckDB error text without internal CTE names.
pub fn clean_err(e: &str) -> String {
    let mut s = e.to_string();
    if let Some(i) = s.find("\n\nLINE") {
        s.truncate(i);
    }
    s
}

fn check_unique_ci(names: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for n in names {
        if !seen.insert(n.to_lowercase()) {
            return Err(format!("A coluna '{}' já existe na tabela.", n));
        }
    }
    Ok(())
}

pub struct CompileCtx<'a> {
    pub conn: &'a Connection,
    /// Raw table holds canonically-encoded typed cells (Excel / inline).
    pub typed_text: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Compiled {
    pub chain: Chain,
    /// Number of steps compiled successfully (prefix length).
    pub applied: usize,
    pub failed: Option<(usize, String)>,
    pub warnings: Vec<String>,
}

/// Compiles `raw__<query>` + steps `[0..=upto]` (all if None). Errors only
/// when the raw table itself is missing; step failures are reported in
/// `failed` with the chain cut before the failing step.
pub fn compile(conn: &Connection, q: &EtlQuery, upto: Option<usize>) -> Result<Compiled, String> {
    let raw = raw_table_name(&q.name);
    let base = format!("SELECT * FROM {}", qi(&raw));
    let schema = describe(conn, &base).map_err(|e| {
        format!(
            "Tabela bruta '{}' não carregada (chame etl_load_raw): {}",
            raw, e
        )
    })?;
    let mut chain = Chain {
        ctes: vec![base],
        schema,
    };
    let mut ctx = CompileCtx {
        conn,
        typed_text: matches!(q.source.kind, SourceKind::Excel | SourceKind::Inline),
        warnings: Vec::new(),
    };
    let mut applied = 0;
    let mut failed = None;
    let last = match upto {
        Some(u) => u.min(q.steps.len().saturating_sub(1)),
        None => q.steps.len().saturating_sub(1),
    };
    if !q.steps.is_empty() {
        for (i, step) in q.steps.iter().enumerate().take(last + 1) {
            let res = compile_step(&mut ctx, &chain, &step.kind).and_then(|body| {
                let mut next = chain.clone();
                next.ctes.push(body);
                let schema = describe(conn, &next.select("*", ""))?;
                next.schema = schema;
                Ok(next)
            });
            match res {
                Ok(next) => {
                    chain = next;
                    applied += 1;
                }
                Err(e) => {
                    let label = if step.name.is_empty() { step.kind.op_name().to_string() } else { step.name.clone() };
                    failed = Some((i, format!("Etapa '{}': {}", label, e)));
                    break;
                }
            }
        }
    }
    Ok(Compiled {
        chain,
        applied,
        failed,
        warnings: ctx.warnings,
    })
}

fn sel(items: &[String], from: &str, tail: &str) -> String {
    format!("SELECT {} FROM {} {}", items.join(", "), from, tail)
}

fn cond_sql(chain: &Chain, c: &Cond) -> Result<String, String> {
    Ok(match c {
        Cond::And { and } | Cond::Or { or: and } => {
            if and.is_empty() {
                return Err("condição vazia".into());
            }
            let op = if matches!(c, Cond::And { .. }) { " AND " } else { " OR " };
            let parts = and.iter().map(|x| cond_sql(chain, x)).collect::<Result<Vec<_>, _>>()?;
            format!("({})", parts.join(op))
        }
        Cond::Not { not } => format!("(NOT {})", cond_sql(chain, not)?),
        Cond::Compare(CompareCond { column, op }) => {
            let info = chain.require(column)?;
            let cat = cat_of(&info.ty);
            let col = qi(column);
            // Three-valued like M: relational comparisons with null are null
            // (rows dropped by the outer COALESCE(.., FALSE)), `=`/`<>` treat
            // null as a value (null = null is true).
            let rel = |sym: &str, v: &Scalar| -> Result<String, String> {
                if v.is_null() {
                    return Ok("CAST(NULL AS BOOLEAN)".into());
                }
                if !cats_compatible(cat, scalar_cat(v)) {
                    return Err(format!(
                        "não é possível comparar a coluna '{}' ({}) com {}",
                        column,
                        etl_type_name(&info.ty),
                        lit_scalar(v)?
                    ));
                }
                Ok(format!("({} {} {})", col, sym, lit_scalar(v)?))
            };
            let text_fn = |f: &str, v: &Scalar| -> Result<String, String> {
                let Scalar::Text(t) = v else {
                    return Err(format!("o filtro de texto em '{}' exige um valor de texto", column));
                };
                if !matches!(cat, Cat::Text | Cat::Null | Cat::Other) {
                    return Err(format!("o filtro de texto exige uma coluna de texto ('{}')", column));
                }
                Ok(format!("{}({}, {})", f, col, lit_str(t)))
            };
            let in_list = |values: &[Scalar]| -> Result<String, String> {
                let mut lits = Vec::new();
                let mut has_null = false;
                for v in values {
                    if v.is_null() {
                        has_null = true;
                    } else if cats_compatible(cat, scalar_cat(v)) {
                        lits.push(lit_scalar(v)?);
                    }
                }
                let mut s = if lits.is_empty() {
                    "FALSE".to_string()
                } else {
                    format!("COALESCE({} IN ({}), FALSE)", col, lits.join(", "))
                };
                if has_null {
                    s = format!("({} OR {} IS NULL)", s, col);
                }
                Ok(format!("({})", s))
            };
            match op {
                CondOp::Eq { value } => {
                    if value.is_null() {
                        format!("({} IS NULL)", col)
                    } else if cats_compatible(cat, scalar_cat(value)) {
                        format!("({} IS NOT DISTINCT FROM {})", col, lit_scalar(value)?)
                    } else {
                        "FALSE".into()
                    }
                }
                CondOp::Neq { value } => {
                    if value.is_null() {
                        format!("({} IS NOT NULL)", col)
                    } else if cats_compatible(cat, scalar_cat(value)) {
                        format!("({} IS DISTINCT FROM {})", col, lit_scalar(value)?)
                    } else {
                        "TRUE".into()
                    }
                }
                CondOp::Gt { value } => rel(">", value)?,
                CondOp::Gte { value } => rel(">=", value)?,
                CondOp::Lt { value } => rel("<", value)?,
                CondOp::Lte { value } => rel("<=", value)?,
                CondOp::Contains { value } => text_fn("contains", value)?,
                CondOp::NotContains { value } => format!("(NOT {})", text_fn("contains", value)?),
                CondOp::StartsWith { value } => text_fn("prefix", value)?,
                CondOp::EndsWith { value } => text_fn("suffix", value)?,
                CondOp::IsNull => format!("({} IS NULL)", col),
                CondOp::IsNotNull => format!("({} IS NOT NULL)", col),
                CondOp::IsBlank => {
                    if cat == Cat::Text {
                        format!("({c} IS NULL OR {c} = '')", c = col)
                    } else {
                        format!("({} IS NULL)", col)
                    }
                }
                CondOp::IsNotBlank => {
                    if cat == Cat::Text {
                        format!("({c} IS NOT NULL AND {c} <> '')", c = col)
                    } else {
                        format!("({} IS NOT NULL)", col)
                    }
                }
                CondOp::In { values } => in_list(values)?,
                CondOp::NotIn { values } => format!("(NOT {})", in_list(values)?),
            }
        }
    })
}

fn agg_sql(op: AggOp, col: Option<&str>, filter: &str, all_visible: &[String]) -> Result<String, String> {
    let c = || col.map(qi).ok_or_else(|| "agregação sem coluna".to_string());
    Ok(match op {
        AggOp::Sum => format!("sum({}){}", c()?, filter),
        AggOp::Avg => format!("avg({}){}", c()?, filter),
        AggOp::Min => format!("min({}){}", c()?, filter),
        AggOp::Max => format!("max({}){}", c()?, filter),
        AggOp::Median => format!("median({}){}", c()?, filter),
        AggOp::Count => match col {
            Some(cn) => format!("count({}){}", qi(cn), filter),
            None => format!("count(*){}", filter),
        },
        AggOp::CountRows => format!("count(*){}", filter),
        AggOp::CountDistinct => match col {
            // List.Count(List.Distinct(..)) counts null as one distinct value.
            Some(cn) => format!(
                "(count(DISTINCT {c}){f} + CASE WHEN bool_or({c} IS NULL){f} THEN 1 ELSE 0 END)",
                c = qi(cn),
                f = filter
            ),
            None => {
                if all_visible.is_empty() {
                    format!("LEAST(count(*){}, 1)", filter)
                } else {
                    format!(
                        "count(DISTINCT row({})){}",
                        all_visible.iter().map(|n| qi(n)).collect::<Vec<_>>().join(", "),
                        filter
                    )
                }
            }
        },
    })
}

fn value_or_column_sql(chain: &Chain, v: &ValueOrColumn) -> Result<(String, Cat), String> {
    Ok(match v {
        ValueOrColumn::Column { column } => {
            let info = chain.require(column)?;
            (qi(column), cat_of(&info.ty))
        }
        ValueOrColumn::Value(s) => (lit_scalar(s)?, scalar_cat(s)),
    })
}

fn compile_step(ctx: &mut CompileCtx<'_>, chain: &Chain, kind: &StepKind) -> Result<String, String> {
    let p = chain.last_cte();
    let vis = chain.visible_names();
    Ok(match kind {
        StepKind::OpaqueM { .. } => {
            return Err("passo em M não suportado pelo motor (opaqueM); edite-o no Editor Avançado".into())
        }

        StepKind::PromoteHeaders => {
            if vis.is_empty() {
                return Ok(sel(&chain.passthrough(), &p, ""));
            }
            let probe_cols: Vec<String> = chain
                .visible()
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "{} AS {}",
                        to_text(&qi(&c.name), cat_of(&c.ty), INVARIANT),
                        qi(&format!("h{}", i))
                    )
                })
                .collect();
            let probe = chain.select(&probe_cols.join(", "), &format!("ORDER BY {} LIMIT 1", qi(ROW_IDX)));
            let res = execute_select_query(ctx.conn, &probe).map_err(|e| clean_err(&e))?;
            let Some(first) = res.rows.first() else {
                return Ok(sel(&chain.passthrough(), &p, ""));
            };
            let mut used: HashSet<String> = HashSet::new();
            let mut new_names = Vec::new();
            for (i, old) in vis.iter().enumerate() {
                // Null/empty or reserved header values keep the current name.
                let h = match first.get(&format!("h{}", i)) {
                    Some(JsonValue::String(s)) if check_user_name(s).is_ok() => s.clone(),
                    _ => old.clone(),
                };
                let mut name = h.clone();
                let mut k = 1;
                while used.contains(&name.to_lowercase()) || check_user_name(&name).is_err() {
                    name = format!("{}_{}", h, k);
                    k += 1;
                }
                used.insert(name.to_lowercase());
                new_names.push(name);
            }
            let mut items = vec![qi(ROW_IDX)];
            for (old, new) in vis.iter().zip(&new_names) {
                items.push(format!("{} AS {}", qi(old), qi(new)));
                if chain.has_flag(old) {
                    items.push(format!("{} AS {}", qi(&flag_name(old)), qi(&flag_name(new))));
                }
            }
            sel(
                &items,
                &p,
                &format!("WHERE {r} > (SELECT min({r}) FROM {p})", r = qi(ROW_IDX), p = p),
            )
        }

        StepKind::ChangeType { columns, culture } => {
            let cul = match culture_info(culture.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    ctx.warnings.push(format!("{}; usando cultura invariante", e));
                    INVARIANT
                }
            };
            let mut seen = HashSet::new();
            for spec in columns {
                chain.require(&spec.column)?;
                if !seen.insert(spec.column.clone()) {
                    return Err(format!("coluna '{}' repetida na alteração de tipo", spec.column));
                }
            }
            let mut items = vec![qi(ROW_IDX)];
            let mut new_flags: Vec<String> = Vec::new();
            for c in chain.visible() {
                match columns.iter().find(|s| s.column == c.name) {
                    Some(spec) => {
                        let cat = cat_of(&c.ty);
                        let x = qi(&c.name);
                        let conv = convert_expr(&x, cat, spec.ty, cul, ctx.typed_text);
                        items.push(format!("{} AS {}", conv, x));
                        let failed = if spec.ty == EtlType::Text {
                            "FALSE".to_string()
                        } else {
                            conversion_failed(&x, cat, &conv)
                        };
                        let old = if chain.has_flag(&c.name) {
                            format!("COALESCE({}, FALSE)", qi(&flag_name(&c.name)))
                        } else {
                            "FALSE".to_string()
                        };
                        new_flags.push(format!("({} OR {}) AS {}", old, failed, qi(&flag_name(&c.name))));
                    }
                    None => items.push(qi(&c.name)),
                }
            }
            for f in chain.flags() {
                let col = &f[ERR_PREFIX.len()..];
                if !columns.iter().any(|s| s.column == col) {
                    items.push(qi(&f));
                }
            }
            items.extend(new_flags);
            sel(&items, &p, "")
        }

        StepKind::RenameColumns { renames } => {
            let mut map: Vec<(String, String)> = Vec::new();
            for (old, new) in renames {
                chain.require(old)?;
                check_user_name(new)?;
                if map.iter().any(|(o, _)| o == old) {
                    return Err(format!("coluna '{}' renomeada duas vezes", old));
                }
                map.push((old.clone(), new.clone()));
            }
            let final_names: Vec<String> = vis
                .iter()
                .map(|n| map.iter().find(|(o, _)| o == n).map(|(_, nn)| nn.clone()).unwrap_or(n.clone()))
                .collect();
            check_unique_ci(&final_names)?;
            let mut items = vec![qi(ROW_IDX)];
            for (old, new) in vis.iter().zip(&final_names) {
                items.push(format!("{} AS {}", qi(old), qi(new)));
                if chain.has_flag(old) {
                    items.push(format!("{} AS {}", qi(&flag_name(old)), qi(&flag_name(new))));
                }
            }
            sel(&items, &p, "")
        }

        StepKind::RemoveColumns { columns } => {
            for c in columns {
                chain.require(c)?;
            }
            let mut items = vec![qi(ROW_IDX)];
            for n in &vis {
                if !columns.contains(n) {
                    items.push(qi(n));
                    if chain.has_flag(n) {
                        items.push(qi(&flag_name(n)));
                    }
                }
            }
            sel(&items, &p, "")
        }

        StepKind::SelectColumns { columns } => {
            check_unique_ci(columns)?;
            let mut items = vec![qi(ROW_IDX)];
            for c in columns {
                chain.require(c)?;
                items.push(qi(c));
                if chain.has_flag(c) {
                    items.push(qi(&flag_name(c)));
                }
            }
            sel(&items, &p, "")
        }

        StepKind::DuplicateColumn { column, new_column } => {
            chain.require(column)?;
            check_user_name(new_column)?;
            let mut all = vis.clone();
            all.push(new_column.clone());
            check_unique_ci(&all)?;
            let mut items = chain.passthrough();
            items.push(format!("{} AS {}", qi(column), qi(new_column)));
            if chain.has_flag(column) {
                items.push(format!("{} AS {}", qi(&flag_name(column)), qi(&flag_name(new_column))));
            }
            sel(&items, &p, "")
        }

        StepKind::FilterRows { condition } => sel(
            &chain.passthrough(),
            &p,
            &format!("WHERE COALESCE({}, FALSE)", cond_sql(chain, condition)?),
        ),

        StepKind::RemoveBlankRows => {
            if vis.is_empty() {
                return Ok(sel(&chain.passthrough(), &p, ""));
            }
            let blank: Vec<String> = chain
                .visible()
                .iter()
                .map(|c| {
                    let x = qi(&c.name);
                    if cat_of(&c.ty) == Cat::Text {
                        format!("({x} IS NULL OR {x} = '')", x = x)
                    } else {
                        format!("({} IS NULL)", x)
                    }
                })
                .collect();
            sel(&chain.passthrough(), &p, &format!("WHERE NOT ({})", blank.join(" AND ")))
        }

        StepKind::RemoveRowsWithErrors { columns } => {
            let flags: Vec<String> = match columns {
                Some(cols) => {
                    let mut f = Vec::new();
                    for c in cols {
                        chain.require(c)?;
                        if chain.has_flag(c) {
                            f.push(flag_name(c));
                        }
                    }
                    f
                }
                None => vis.iter().filter(|c| chain.has_flag(c)).map(|c| flag_name(c)).collect(),
            };
            if flags.is_empty() {
                sel(&chain.passthrough(), &p, "")
            } else {
                let conds: Vec<String> = flags.iter().map(|f| format!("NOT COALESCE({}, FALSE)", qi(f))).collect();
                sel(&chain.passthrough(), &p, &format!("WHERE {}", conds.join(" AND ")))
            }
        }

        StepKind::KeepTopRows { count }
        | StepKind::RemoveTopRows { count }
        | StepKind::KeepBottomRows { count }
        | StepKind::RemoveBottomRows { count } => {
            let (dir, cmp) = match kind {
                StepKind::KeepTopRows { .. } => ("ASC", "<="),
                StepKind::RemoveTopRows { .. } => ("ASC", ">"),
                StepKind::KeepBottomRows { .. } => ("DESC", "<="),
                _ => ("DESC", ">"),
            };
            sel(
                &chain.passthrough(),
                &p,
                &format!("QUALIFY row_number() OVER (ORDER BY {} {}) {} {}", qi(ROW_IDX), dir, cmp, count),
            )
        }

        StepKind::RemoveDuplicates { columns } => {
            let cols = match columns {
                Some(c) => {
                    for x in c {
                        chain.require(x)?;
                    }
                    c.clone()
                }
                None => vis.clone(),
            };
            if cols.is_empty() {
                return Ok(sel(&chain.passthrough(), &p, ""));
            }
            sel(
                &chain.passthrough(),
                &p,
                &format!(
                    "QUALIFY row_number() OVER (PARTITION BY {} ORDER BY {}) = 1",
                    cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", "),
                    qi(ROW_IDX)
                ),
            )
        }

        StepKind::Sort { by } => {
            if by.is_empty() {
                return Ok(sel(&chain.passthrough(), &p, ""));
            }
            let mut keys = Vec::new();
            for k in by {
                chain.require(&k.column)?;
                // M orders null before any value.
                keys.push(if k.descending {
                    format!("{} DESC NULLS LAST", qi(&k.column))
                } else {
                    format!("{} ASC NULLS FIRST", qi(&k.column))
                });
            }
            keys.push(qi(ROW_IDX));
            let mut items = vec![format!("row_number() OVER (ORDER BY {}) AS {}", keys.join(", "), qi(ROW_IDX))];
            items.extend(chain.passthrough().into_iter().skip(1));
            sel(&items, &p, "")
        }

        StepKind::ReplaceValues {
            columns,
            find,
            replace,
            match_entire_cell,
        } => {
            for c in columns {
                chain.require(c)?;
            }
            let mut items = vec![qi(ROW_IDX)];
            for c in chain.visible() {
                if !columns.contains(&c.name) {
                    items.push(qi(&c.name));
                    continue;
                }
                let x = qi(&c.name);
                let cat = cat_of(&c.ty);
                let expr = if *match_entire_cell {
                    let matches = if find.is_null() {
                        Some(format!("{} IS NULL", x))
                    } else if cats_compatible(cat, scalar_cat(find)) {
                        Some(format!("{} IS NOT DISTINCT FROM {}", x, lit_scalar(find)?))
                    } else {
                        None
                    };
                    match matches {
                        None => x.clone(),
                        Some(m) => {
                            if replace.is_null() || cats_compatible(cat, scalar_cat(replace)) {
                                format!("(CASE WHEN {} THEN {} ELSE {} END)", m, lit_scalar(replace)?, x)
                            } else {
                                // Mixed types: the column becomes text (M would hold both).
                                let rep = match replace {
                                    Scalar::Number(n) => lit_str(&super::m_parse::format_m_number(*n)),
                                    Scalar::Bool(b) => lit_str(if *b { "true" } else { "false" }),
                                    other => lit_scalar(other)?,
                                };
                                format!("(CASE WHEN {} THEN {} ELSE {} END)", m, rep, to_text(&x, cat, INVARIANT))
                            }
                        }
                    }
                } else {
                    let (Scalar::Text(f), Scalar::Text(r)) = (find, replace) else {
                        return Err("Replacer.ReplaceText exige valores de texto".into());
                    };
                    if !matches!(cat, Cat::Text | Cat::Null | Cat::Other) {
                        return Err(format!(
                            "Replacer.ReplaceText exige coluna de texto ('{}' é {})",
                            c.name,
                            etl_type_name(&c.ty)
                        ));
                    }
                    if f.is_empty() {
                        x.clone()
                    } else {
                        format!("replace({}, {}, {})", x, lit_str(f), lit_str(r))
                    }
                };
                items.push(format!("{} AS {}", expr, x));
            }
            items.extend(chain.flags().iter().map(|f| qi(f)));
            sel(&items, &p, "")
        }

        StepKind::FillDown { columns } | StepKind::FillUp { columns } => {
            for c in columns {
                chain.require(c)?;
            }
            let down = matches!(kind, StepKind::FillDown { .. });
            let mut items = vec![qi(ROW_IDX)];
            for n in &vis {
                if columns.contains(n) {
                    let w = if down {
                        format!(
                            "last_value({} IGNORE NULLS) OVER (ORDER BY {} ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
                            qi(n),
                            qi(ROW_IDX)
                        )
                    } else {
                        format!(
                            "first_value({} IGNORE NULLS) OVER (ORDER BY {} ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING)",
                            qi(n),
                            qi(ROW_IDX)
                        )
                    };
                    items.push(format!("{} AS {}", w, qi(n)));
                } else {
                    items.push(qi(n));
                }
            }
            items.extend(chain.flags().iter().map(|f| qi(f)));
            sel(&items, &p, "")
        }

        StepKind::SplitColumn {
            column,
            mode,
            delimiter,
            positions,
            length,
            into,
        } => {
            let info = chain.require(column)?;
            if into.is_empty() {
                return Err("a divisão precisa de ao menos uma coluna de saída".into());
            }
            for n in into {
                check_user_name(n)?;
            }
            let mut final_names: Vec<String> = Vec::new();
            for n in &vis {
                if n == column {
                    final_names.extend(into.iter().cloned());
                } else {
                    final_names.push(n.clone());
                }
            }
            check_unique_ci(&final_names)?;
            let s = to_text(&qi(column), cat_of(&info.ty), INVARIANT);
            let parts: Vec<String> = match mode {
                SplitMode::DelimiterEach | SplitMode::DelimiterLeftmost | SplitMode::DelimiterRightmost => {
                    let d = delimiter.as_deref().unwrap_or("");
                    if d.is_empty() {
                        return Err("delimitador vazio".into());
                    }
                    let dl = lit_str(d);
                    match mode {
                        SplitMode::DelimiterEach => (1..=into.len())
                            .map(|i| format!("string_split({}, {})[{}]", s, dl, i))
                            .collect(),
                        SplitMode::DelimiterLeftmost => {
                            let pos = format!("strpos({}, {})", s, dl);
                            let mut v = vec![format!(
                                "(CASE WHEN {pos} > 0 THEN substr({s}, 1, {pos} - 1) ELSE {s} END)",
                                pos = pos,
                                s = s
                            )];
                            if into.len() > 1 {
                                v.push(format!(
                                    "(CASE WHEN {pos} > 0 THEN substr({s}, {pos} + length({d})) END)",
                                    pos = pos,
                                    s = s,
                                    d = dl
                                ));
                            }
                            v.extend((2..into.len()).map(|_| "CAST(NULL AS VARCHAR)".to_string()));
                            v
                        }
                        _ => {
                            // position of the last occurrence (1-based), 0 when absent
                            let rpos = format!(
                                "(CASE WHEN strpos(reverse({s}), reverse({d})) > 0 THEN length({s}) - strpos(reverse({s}), reverse({d})) - length({d}) + 2 ELSE 0 END)",
                                s = s,
                                d = dl
                            );
                            let mut v = vec![format!(
                                "(CASE WHEN {r} > 0 THEN substr({s}, 1, {r} - 1) ELSE {s} END)",
                                r = rpos,
                                s = s
                            )];
                            if into.len() > 1 {
                                v.push(format!(
                                    "(CASE WHEN {r} > 0 THEN substr({s}, {r} + length({d})) END)",
                                    r = rpos,
                                    s = s,
                                    d = dl
                                ));
                            }
                            v.extend((2..into.len()).map(|_| "CAST(NULL AS VARCHAR)".to_string()));
                            v
                        }
                    }
                }
                SplitMode::Positions => {
                    let pos = positions.clone().unwrap_or_default();
                    if pos.is_empty() {
                        return Err("posições de divisão vazias".into());
                    }
                    (0..into.len())
                        .map(|i| match pos.get(i) {
                            None => "CAST(NULL AS VARCHAR)".to_string(),
                            Some(&start) => {
                                let len = pos.get(i + 1).map(|&e| e.saturating_sub(start));
                                let sub = match len {
                                    Some(l) => format!("substr({}, {}, {})", s, start + 1, l),
                                    None => format!("substr({}, {})", s, start + 1),
                                };
                                if start == 0 {
                                    sub
                                } else {
                                    format!("(CASE WHEN length({}) > {} THEN {} END)", s, start, sub)
                                }
                            }
                        })
                        .collect()
                }
                SplitMode::FixedLength => {
                    let l = length.unwrap_or(0);
                    if l == 0 {
                        return Err("comprimento de divisão deve ser maior que zero".into());
                    }
                    (0..into.len() as u64)
                        .map(|i| {
                            let start = i * l;
                            if start == 0 {
                                format!("substr({}, 1, {})", s, l)
                            } else {
                                format!(
                                    "(CASE WHEN length({}) > {} THEN substr({}, {}, {}) END)",
                                    s,
                                    start,
                                    s,
                                    start + 1,
                                    l
                                )
                            }
                        })
                        .collect()
                }
            };
            let mut items = vec![qi(ROW_IDX)];
            for n in &vis {
                if n == column {
                    for (expr, name) in parts.iter().zip(into) {
                        items.push(format!("{} AS {}", expr, qi(name)));
                    }
                } else {
                    items.push(qi(n));
                }
            }
            items.extend(
                chain
                    .flags()
                    .iter()
                    .filter(|f| **f != flag_name(column))
                    .map(|f| qi(f)),
            );
            sel(&items, &p, "")
        }

        StepKind::MergeColumns {
            columns,
            separator,
            new_column,
        } => {
            if columns.is_empty() {
                return Err("nenhuma coluna para mesclar".into());
            }
            check_user_name(new_column)?;
            let mut texts = Vec::new();
            for c in columns {
                let info = chain.require(c)?;
                texts.push(format!("COALESCE({}, '')", to_text(&qi(c), cat_of(&info.ty), INVARIANT)));
            }
            let merged = texts.join(&format!(" || {} || ", lit_str(separator)));
            let first_pos = vis.iter().position(|n| columns.contains(n)).unwrap_or(0);
            let mut final_names = Vec::new();
            let mut items = vec![qi(ROW_IDX)];
            for (i, n) in vis.iter().enumerate() {
                if i == first_pos {
                    items.push(format!("({}) AS {}", merged, qi(new_column)));
                    final_names.push(new_column.clone());
                }
                if !columns.contains(n) {
                    items.push(qi(n));
                    final_names.push(n.clone());
                }
            }
            check_unique_ci(&final_names)?;
            let src_flags: Vec<String> = columns
                .iter()
                .filter(|c| chain.has_flag(c))
                .map(|c| format!("COALESCE({}, FALSE)", qi(&flag_name(c))))
                .collect();
            for f in chain.flags() {
                let col = &f[ERR_PREFIX.len()..];
                if !columns.iter().any(|c| c == col) && col != new_column {
                    items.push(qi(&f));
                }
            }
            if !src_flags.is_empty() {
                items.push(format!("({}) AS {}", src_flags.join(" OR "), qi(&flag_name(new_column))));
            }
            sel(&items, &p, "")
        }

        StepKind::TextTransform { columns, func } => {
            for c in columns {
                chain.require(c)?;
            }
            let mut items = vec![qi(ROW_IDX)];
            for c in chain.visible() {
                if !columns.contains(&c.name) {
                    items.push(qi(&c.name));
                    continue;
                }
                let t = to_text(&qi(&c.name), cat_of(&c.ty), INVARIANT);
                let e = match func {
                    TextFn::Upper => format!("upper({})", t),
                    TextFn::Lower => format!("lower({})", t),
                    TextFn::Proper => proper_sql(&t),
                    TextFn::Trim => trim_ws_sql(&t),
                    TextFn::Clean => clean_sql(&t),
                };
                items.push(format!("{} AS {}", e, qi(&c.name)));
            }
            items.extend(chain.flags().iter().map(|f| qi(f)));
            sel(&items, &p, "")
        }

        StepKind::UnpivotOtherColumns {
            keep,
            attribute_column,
            value_column,
        } => {
            for k in keep {
                chain.require(k)?;
            }
            let keep_ordered: Vec<String> = vis.iter().filter(|n| keep.contains(n)).cloned().collect();
            let others: Vec<String> = vis.iter().filter(|n| !keep.contains(n)).cloned().collect();
            unpivot_sql(chain, &keep_ordered, &others, attribute_column, value_column)?
        }

        StepKind::UnpivotColumns {
            columns,
            attribute_column,
            value_column,
        } => {
            for c in columns {
                chain.require(c)?;
            }
            let keep_ordered: Vec<String> = vis.iter().filter(|n| !columns.contains(n)).cloned().collect();
            let others: Vec<String> = vis.iter().filter(|n| columns.contains(n)).cloned().collect();
            unpivot_sql(chain, &keep_ordered, &others, attribute_column, value_column)?
        }

        StepKind::Pivot {
            column,
            values_column,
            aggregation,
        } => {
            let pinfo = chain.require(column)?;
            chain.require(values_column)?;
            let ptext = to_text(&qi(column), cat_of(&pinfo.ty), INVARIANT);
            let probe = chain.select(
                &format!("{} AS \"v\", min({}) AS \"o\"", ptext, qi(ROW_IDX)),
                &format!(
                    "WHERE {} IS NOT NULL GROUP BY 1 ORDER BY \"o\" LIMIT {}",
                    qi(column),
                    MAX_PIVOT_VALUES + 1
                ),
            );
            let res = execute_select_query(ctx.conn, &probe).map_err(|e| clean_err(&e))?;
            if res.rows.len() > MAX_PIVOT_VALUES {
                return Err(format!("mais de {} valores distintos para dinamizar", MAX_PIVOT_VALUES));
            }
            let values: Vec<String> = res
                .rows
                .iter()
                .filter_map(|r| r.get("v").and_then(|v| v.as_str()).map(str::to_string))
                .collect();
            let keys: Vec<String> = vis
                .iter()
                .filter(|n| *n != column && *n != values_column)
                .cloned()
                .collect();
            let mut all = keys.clone();
            for v in &values {
                check_user_name(v)?;
                all.push(v.clone());
            }
            check_unique_ci(&all)?;
            let mut items = vec![format!("min({}) AS {}", qi(ROW_IDX), qi(ROW_IDX))];
            items.extend(keys.iter().map(|k| qi(k)));
            for v in &values {
                let filter = format!(" FILTER (WHERE {} = {})", ptext, lit_str(v));
                items.push(format!(
                    "{} AS {}",
                    agg_sql(*aggregation, Some(values_column), &filter, &[])?,
                    qi(v)
                ));
            }
            let group = if keys.is_empty() {
                String::new()
            } else {
                format!("GROUP BY {}", keys.iter().map(|k| qi(k)).collect::<Vec<_>>().join(", "))
            };
            sel(&items, &p, &group)
        }

        StepKind::GroupBy { keys, aggregations } => {
            for k in keys {
                chain.require(k)?;
            }
            let mut names = keys.clone();
            let mut items = vec![format!("min({}) AS {}", qi(ROW_IDX), qi(ROW_IDX))];
            items.extend(keys.iter().map(|k| qi(k)));
            for a in aggregations {
                check_user_name(&a.name)?;
                if let Some(c) = &a.column {
                    chain.require(c)?;
                }
                names.push(a.name.clone());
                items.push(format!("{} AS {}", agg_sql(a.op, a.column.as_deref(), "", &vis)?, qi(&a.name)));
            }
            check_unique_ci(&names)?;
            let group = if keys.is_empty() {
                String::new()
            } else {
                format!("GROUP BY {}", keys.iter().map(|k| qi(k)).collect::<Vec<_>>().join(", "))
            };
            sel(&items, &p, &group)
        }

        StepKind::AddConditionalColumn {
            new_column,
            rules,
            otherwise,
            ty,
        } => {
            check_user_name(new_column)?;
            let mut all = vis.clone();
            all.push(new_column.clone());
            check_unique_ci(&all)?;
            if rules.is_empty() {
                return Err("coluna condicional sem regras".into());
            }
            let mut branches = Vec::new();
            for r in rules {
                branches.push((Some(cond_sql(chain, &r.when)?), value_or_column_sql(chain, &r.then)?));
            }
            branches.push((None, value_or_column_sql(chain, otherwise)?));
            let cats: Vec<Cat> = branches.iter().map(|(_, (_, c))| *c).filter(|c| *c != Cat::Null).collect();
            let mixed = cats.windows(2).any(|w| !cats_compatible(w[0], w[1]) || (w[0] == Cat::Text) != (w[1] == Cat::Text));
            let mut case = String::from("CASE");
            let mut else_part = String::new();
            for (cond, (v, c)) in &branches {
                let v = if mixed && *c != Cat::Text && *c != Cat::Null {
                    to_text(v, *c, INVARIANT)
                } else {
                    v.clone()
                };
                match cond {
                    Some(cs) => case.push_str(&format!(" WHEN COALESCE({}, FALSE) THEN {}", cs, v)),
                    None => else_part = format!(" ELSE {} END", v),
                }
            }
            case.push_str(&else_part);
            let result_cat = if mixed { Cat::Text } else { cats.first().copied().unwrap_or(Cat::Null) };
            let expr = match ty {
                Some(t) => convert_expr(&format!("({})", case), result_cat, *t, INVARIANT, false),
                None => case,
            };
            let mut items = chain.passthrough();
            items.push(format!("{} AS {}", expr, qi(new_column)));
            sel(&items, &p, "")
        }

        StepKind::AddCustomColumn {
            new_column,
            expression,
            ty,
        } => {
            check_user_name(new_column)?;
            let mut all = vis.clone();
            all.push(new_column.clone());
            check_unique_ci(&all)?;
            let (expr, cat) = custom_expr::translate(expression, chain)?;
            let expr = match ty {
                Some(t) => convert_expr(&format!("({})", expr), cat, *t, INVARIANT, false),
                None => expr,
            };
            let mut items = chain.passthrough();
            items.push(format!("{} AS {}", expr, qi(new_column)));
            sel(&items, &p, "")
        }

        StepKind::AddIndexColumn { new_column, start, step } => {
            check_user_name(new_column)?;
            let mut all = vis.clone();
            all.push(new_column.clone());
            check_unique_ci(&all)?;
            let mut items = chain.passthrough();
            items.push(format!(
                "(CAST({} AS BIGINT) + CAST({} AS BIGINT) * (row_number() OVER (ORDER BY {}) - 1)) AS {}",
                start,
                step,
                qi(ROW_IDX),
                qi(new_column)
            ));
            sel(&items, &p, "")
        }

        StepKind::MergeQueries {
            right_query,
            left_keys,
            right_keys,
            join_kind,
            expand,
        } => {
            if left_keys.len() != right_keys.len() || left_keys.is_empty() {
                return Err("chaves de mesclagem inválidas".into());
            }
            let right_schema = describe(ctx.conn, &format!("SELECT * FROM {}", qi(right_query))).map_err(|e| {
                format!("A consulta '{}' ainda não foi carregada no modelo: {}", right_query, e)
            })?;
            let rhas = |n: &str| right_schema.iter().any(|c| c.name == n);
            for k in left_keys {
                chain.require(k)?;
            }
            for k in right_keys {
                if !rhas(k) {
                    return Err(format!("A coluna '{}' não existe em '{}'.", k, right_query));
                }
            }
            let mut final_names = vis.clone();
            for e in expand {
                if !rhas(&e.column) {
                    return Err(format!("A coluna '{}' não existe em '{}'.", e.column, right_query));
                }
                check_user_name(&e.as_name)?;
                final_names.push(e.as_name.clone());
            }
            check_unique_ci(&final_names)?;
            let on: Vec<String> = left_keys
                .iter()
                .zip(right_keys)
                .map(|(l, r)| format!("__l.{} IS NOT DISTINCT FROM __r.{}", qi(l), qi(r)))
                .collect();
            let (join, filter) = match join_kind {
                JoinKind::LeftOuter => ("LEFT JOIN", String::new()),
                JoinKind::Inner => ("INNER JOIN", String::new()),
                JoinKind::RightOuter => ("RIGHT JOIN", String::new()),
                JoinKind::FullOuter => ("FULL OUTER JOIN", String::new()),
                JoinKind::LeftAnti => ("LEFT JOIN", "WHERE __r.\"__rid\" IS NULL".to_string()),
                JoinKind::RightAnti => ("RIGHT JOIN", format!("WHERE __l.{} IS NULL", qi(ROW_IDX))),
            };
            let mut items = vec![format!(
                "row_number() OVER (ORDER BY __l.{} NULLS LAST, __r.\"__rid\") AS {}",
                qi(ROW_IDX),
                qi(ROW_IDX)
            )];
            for c in chain.schema.iter().filter(|c| c.name != ROW_IDX) {
                items.push(format!("__l.{}", qi(&c.name)));
            }
            for e in expand {
                items.push(format!("__r.{} AS {}", qi(&e.column), qi(&e.as_name)));
            }
            format!(
                "SELECT {} FROM {} AS __l {} (SELECT rowid AS \"__rid\", * FROM {}) AS __r ON {} {}",
                items.join(", "),
                p,
                join,
                qi(right_query),
                on.join(" AND "),
                filter
            )
        }

        StepKind::AppendQueries { queries } => {
            if queries.is_empty() {
                return Ok(sel(&chain.passthrough(), &p, ""));
            }
            let mut parts = vec![format!(
                "SELECT {}, 0 AS \"__src\", {} AS \"__ord\" FROM {}",
                if vis.is_empty() { "NULL AS \"__empty\"".to_string() } else { vis.iter().map(|n| qi(n)).collect::<Vec<_>>().join(", ") },
                qi(ROW_IDX),
                p
            )];
            for (i, q) in queries.iter().enumerate() {
                let rs = describe(ctx.conn, &format!("SELECT * FROM {}", qi(q)))
                    .map_err(|e| format!("A consulta '{}' ainda não foi carregada no modelo: {}", q, e))?;
                for c in &rs {
                    check_user_name(&c.name)?;
                }
                parts.push(format!(
                    "SELECT *, {} AS \"__src\", rowid AS \"__ord\" FROM {}",
                    i + 1,
                    qi(q)
                ));
            }
            let exclude = if vis.is_empty() {
                "\"__src\", \"__ord\", \"__empty\""
            } else {
                "\"__src\", \"__ord\""
            };
            format!(
                "SELECT row_number() OVER (ORDER BY \"__src\", \"__ord\") AS {}, * EXCLUDE ({}) FROM ({}) AS __u",
                qi(ROW_IDX),
                exclude,
                parts.join(" UNION ALL BY NAME ")
            )
        }
    })
}

fn unpivot_sql(
    chain: &Chain,
    keep: &[String],
    cols: &[String],
    attr: &str,
    value: &str,
) -> Result<String, String> {
    check_user_name(attr)?;
    check_user_name(value)?;
    let mut all = keep.to_vec();
    all.push(attr.to_string());
    all.push(value.to_string());
    check_unique_ci(&all)?;
    let p = chain.last_cte();
    let keep_q: Vec<String> = keep.iter().map(|k| qi(k)).collect();
    if cols.is_empty() {
        let mut items = vec![qi(ROW_IDX)];
        items.extend(keep_q);
        items.push(format!("CAST(NULL AS VARCHAR) AS {}", qi(attr)));
        items.push(format!("CAST(NULL AS VARCHAR) AS {}", qi(value)));
        return Ok(sel(&items, &p, "WHERE FALSE"));
    }
    let cats: Vec<(String, Cat, String)> = cols
        .iter()
        .map(|c| {
            let info = chain.col(c).unwrap();
            (c.clone(), cat_of(&info.ty), info.ty.clone())
        })
        .collect();
    let same_type = cats.windows(2).all(|w| w[0].2 == w[1].2);
    let structs: Vec<String> = cats
        .iter()
        .enumerate()
        .map(|(i, (c, cat, _))| {
            let v = if same_type { qi(c) } else { to_text(&qi(c), *cat, INVARIANT) };
            format!("{{'a': {}, 'o': {}, 'v': {}}}", lit_str(c), i, v)
        })
        .collect();
    let mut inner = vec![qi(ROW_IDX)];
    inner.extend(keep_q.iter().cloned());
    inner.push(format!("unnest([{}]) AS \"__u\"", structs.join(", ")));
    let mut outer = vec![format!(
        "row_number() OVER (ORDER BY {}, struct_extract(\"__u\", 'o')) AS {}",
        qi(ROW_IDX),
        qi(ROW_IDX)
    )];
    outer.extend(keep_q);
    outer.push(format!("struct_extract(\"__u\", 'a') AS {}", qi(attr)));
    outer.push(format!("struct_extract(\"__u\", 'v') AS {}", qi(value)));
    Ok(format!(
        "SELECT {} FROM ({}) AS __x WHERE struct_extract(\"__u\", 'v') IS NOT NULL",
        outer.join(", "),
        sel(&inner, &p, "")
    ))
}
