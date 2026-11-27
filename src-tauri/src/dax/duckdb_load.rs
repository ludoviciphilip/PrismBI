//! Loads a DAX model table directly from DuckDB (all rows, typed), instead
//! of going through the frontend's `PREVIEW_ROW_LIMIT`-capped JSON rows. See
//! `docs/RUST_ENGINE_2.md` §3 step 2.
//!
//! Also tracks, independently from both the DB connection lock and the
//! `DaxModel` lock, the version a table was last loaded at (`LAST_SYNC_VERSIONS`)
//! and a load counter used by tests to assert the cache actually skips
//! reloading unchanged tables (`LOAD_COUNT`).

use super::{functions, ColumnType, DaxValue};
use duckdb::types::ValueRef;
use duckdb::Connection;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static LAST_SYNC_VERSIONS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
thread_local! {
    // Per thread: cargo runs tests in parallel threads, and a process-wide
    // counter made the cache test flaky. Loads run on the calling thread.
    static LOAD_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn last_sync_versions() -> &'static Mutex<HashMap<String, u64>> {
    LAST_SYNC_VERSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_last_sync_version(table_name: &str) -> Option<u64> {
    last_sync_versions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&table_name.to_lowercase())
        .copied()
}

pub fn set_last_sync_version(table_name: &str, version: u64) {
    last_sync_versions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(table_name.to_lowercase(), version);
}

/// Test hook: how many times `load_table_from_duckdb` actually ran a
/// `SELECT *` against DuckDB (as opposed to being skipped by the version
/// cache). Not reset automatically between tests; each test that relies on
/// it should read the counter before and after its own operations.
pub fn load_count_for_test() -> u64 {
    LOAD_COUNT.with(|c| c.get())
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// DAX/Excel serial epoch offset: days between 1899-12-30 (serial 0) and
/// 1970-01-01, matching `functions::SERIAL_EPOCH_OFFSET_DAYS` (private there).
const SERIAL_EPOCH_OFFSET_DAYS: f64 = 25_569.0;

fn duckdb_type_to_column_type(duck_type: &str) -> ColumnType {
    let up = duck_type.to_uppercase();
    if up.contains("BOOL") {
        ColumnType::Boolean
    } else if up.contains("DATE") || up.contains("TIME") {
        ColumnType::Date
    } else if up.contains("DECIMAL")
        || up.contains("DOUBLE")
        || up.contains("FLOAT")
        || up.contains("NUMERIC")
        || up.contains("REAL")
    {
        ColumnType::Decimal
    } else if up.contains("INT") {
        ColumnType::Integer
    } else {
        ColumnType::Text
    }
}

/// Resolves the effective `ColumnType` for a loaded column: an explicit
/// frontend-declared type (`column_types[i]`, same vocabulary as the JSON
/// path: text/number/date/boolean/currency/percentage) wins; otherwise falls
/// back to the structural DuckDB type. `number`/`percentage` are refined to
/// Integer when the DuckDB column itself is an integral type (no need to
/// rescan values the way the JSON path does, since DuckDB already knows).
fn effective_column_type(declared: Option<&str>, base: ColumnType) -> ColumnType {
    match declared.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("text") => ColumnType::Text,
        Some("currency") => ColumnType::Currency,
        Some("boolean") => ColumnType::Boolean,
        Some("date") => ColumnType::Date,
        Some("number") | Some("percentage") => {
            if matches!(base, ColumnType::Integer) {
                ColumnType::Integer
            } else {
                ColumnType::Decimal
            }
        }
        _ => base,
    }
}

fn int_to_dax(i: i64, ct: ColumnType) -> DaxValue {
    match ct {
        ColumnType::Currency => DaxValue::Currency(i.saturating_mul(10_000)),
        ColumnType::Decimal => DaxValue::Decimal(i as f64),
        ColumnType::Text => DaxValue::Text(i.to_string()),
        ColumnType::Boolean => DaxValue::Boolean(i != 0),
        ColumnType::Date => DaxValue::DateTime(i as f64),
        ColumnType::Integer => DaxValue::Integer(i),
    }
}

fn float_to_dax(f: f64, ct: ColumnType) -> DaxValue {
    match ct {
        ColumnType::Currency => DaxValue::Currency((f * 10_000.0).round() as i64),
        ColumnType::Integer => DaxValue::number_smart(f),
        ColumnType::Text => DaxValue::Text(f.to_string()),
        ColumnType::Boolean => DaxValue::Boolean(f != 0.0),
        ColumnType::Date => DaxValue::DateTime(f),
        ColumnType::Decimal => DaxValue::Decimal(f),
    }
}

fn text_to_dax(s: &str, ct: ColumnType) -> DaxValue {
    match ct {
        ColumnType::Date => functions::parse_iso_to_serial(s)
            .map(DaxValue::DateTime)
            .unwrap_or_else(|| DaxValue::Text(s.to_string())),
        ColumnType::Currency => s
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map(|f| DaxValue::Currency((f * 10_000.0).round() as i64))
            .unwrap_or_else(|_| DaxValue::Text(s.to_string())),
        ColumnType::Boolean => {
            let l = s.trim().to_lowercase();
            DaxValue::Boolean(l == "true" || l == "1" || l == "sim")
        }
        ColumnType::Integer | ColumnType::Decimal => s
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map(DaxValue::number_smart)
            .unwrap_or_else(|_| DaxValue::Text(s.to_string())),
        ColumnType::Text => DaxValue::Text(s.to_string()),
    }
}

fn value_ref_to_dax(vref: ValueRef<'_>, ct: ColumnType) -> DaxValue {
    match vref {
        ValueRef::Null => DaxValue::Blank,
        ValueRef::Boolean(b) => {
            if ct == ColumnType::Text {
                DaxValue::Text(b.to_string())
            } else {
                DaxValue::Boolean(b)
            }
        }
        ValueRef::TinyInt(i) => int_to_dax(i as i64, ct),
        ValueRef::SmallInt(i) => int_to_dax(i as i64, ct),
        ValueRef::Int(i) => int_to_dax(i as i64, ct),
        ValueRef::BigInt(i) => int_to_dax(i, ct),
        ValueRef::HugeInt(i) => int_to_dax(i64::try_from(i).unwrap_or(0), ct),
        ValueRef::UTinyInt(u) => int_to_dax(u as i64, ct),
        ValueRef::USmallInt(u) => int_to_dax(u as i64, ct),
        ValueRef::UInt(u) => int_to_dax(u as i64, ct),
        ValueRef::UBigInt(u) => int_to_dax(u as i64, ct),
        ValueRef::Float(f) => float_to_dax(f as f64, ct),
        ValueRef::Double(d) => float_to_dax(d, ct),
        ValueRef::Decimal(d) => {
            let s = d.to_string();
            float_to_dax(s.parse::<f64>().unwrap_or(0.0), ct)
        }
        ValueRef::Text(bytes) => text_to_dax(&String::from_utf8_lossy(bytes), ct),
        ValueRef::Blob(bytes) => DaxValue::Text(format!("<blob {} bytes>", bytes.len())),
        ValueRef::Date32(days) => DaxValue::DateTime(days as f64 + SERIAL_EPOCH_OFFSET_DAYS),
        ValueRef::Timestamp(_, micros) => {
            DaxValue::DateTime(micros as f64 / 86_400_000_000.0 + SERIAL_EPOCH_OFFSET_DAYS)
        }
        _ => DaxValue::Blank,
    }
}

/// Loads every row of `table_name` from `conn` into a `DaxTableData`.
/// `declared_types` is aligned by index with the resulting column order
/// (DuckDB's own column order), same vocabulary as `DaxTableInput::column_types`.
/// Errors clearly (does not panic) when the table doesn't exist, so the
/// caller can fall back to the JSON preview rows.
pub fn load_table_from_duckdb(
    conn: &Connection,
    table_name: &str,
    declared_types: &[Option<String>],
) -> Result<super::DaxTableData, String> {
    let describe_sql = format!("DESCRIBE {}", quote_ident(table_name));
    let mut stmt = conn.prepare(&describe_sql).map_err(|e| {
        format!(
            "Tabela DuckDB '{}' não encontrada para sincronizar o modelo: {}",
            table_name, e
        )
    })?;
    let schema: Vec<(String, String)> = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let duck_type: String = row.get(1)?;
            Ok((name, duck_type))
        })
        .map_err(|e| format!("Erro ao ler schema de '{}': {}", table_name, e))?
        .filter_map(Result::ok)
        .collect();

    if schema.is_empty() {
        return Err(format!(
            "Tabela DuckDB '{}' não existe ou não tem colunas (pode não ter sido recarregada ainda nesta sessão)",
            table_name
        ));
    }

    let col_names: Vec<String> = schema.iter().map(|(n, _)| n.clone()).collect();
    let col_types: Vec<ColumnType> = schema
        .iter()
        .enumerate()
        .map(|(idx, (_, duck_type))| {
            let base = duckdb_type_to_column_type(duck_type);
            let declared = declared_types.get(idx).and_then(|o| o.as_deref());
            effective_column_type(declared, base)
        })
        .collect();

    let select_sql = format!("SELECT * FROM {}", quote_ident(table_name));
    let mut stmt = conn
        .prepare(&select_sql)
        .map_err(|e| format!("Erro ao preparar leitura de '{}': {}", table_name, e))?;
    let mut rows_cursor = stmt
        .query([])
        .map_err(|e| format!("Erro ao executar leitura de '{}': {}", table_name, e))?;

    let mut rows: Vec<Vec<DaxValue>> = Vec::new();
    while let Some(row) = rows_cursor
        .next()
        .map_err(|e| format!("Erro ao iterar linhas de '{}': {}", table_name, e))?
    {
        let mut r = Vec::with_capacity(col_names.len());
        for idx in 0..col_names.len() {
            let v = match row.get_ref(idx) {
                Ok(vref) => value_ref_to_dax(vref, col_types[idx]),
                Err(_) => DaxValue::Blank,
            };
            r.push(v);
        }
        rows.push(r);
    }

    let mut col_index = HashMap::new();
    for (idx, c) in col_names.iter().enumerate() {
        col_index.insert(c.to_lowercase(), idx);
    }

    LOAD_COUNT.with(|c| c.set(c.get() + 1));

    Ok(super::DaxTableData {
        name: table_name.to_string(),
        columns: col_names,
        col_index,
        col_types,
        rows,
        sort_by_columns: HashMap::new(),
    })
}

/// Step 1 of `sync_model_to_rust`: loads every DuckDB-backed table whose
/// DuckDB version changed since the last sync (unchanged ones are skipped and
/// reused from the DaxModel cache). Keys are lowercase DuckDB table names.
pub fn load_changed_tables(
    conn: &Connection,
    tables: &[super::DaxTableInput],
) -> HashMap<String, (u64, super::DaxTableData)> {
    let mut out = HashMap::new();
    for t in tables {
        let Some(duckdb_name) = &t.duckdb_table else { continue };
        let current_version = crate::engine::duckdb_store::get_table_version(duckdb_name);
        if get_last_sync_version(duckdb_name) == Some(current_version) {
            continue; // unchanged since the last sync: DaxModel reuses its cache.
        }
        match load_table_from_duckdb(conn, duckdb_name, &t.column_types) {
            Ok(data) => {
                set_last_sync_version(duckdb_name, current_version);
                out.insert(duckdb_name.to_lowercase(), (current_version, data));
            }
            Err(e) => {
                // Left out: sync_from_payload_with_duckdb falls back to the
                // cached table (if any) or the JSON rows.
                eprintln!("aviso: não foi possível carregar a tabela DuckDB '{}': {}", duckdb_name, e);
            }
        }
    }
    out
}
