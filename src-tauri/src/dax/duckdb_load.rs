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

pub(crate) fn quote_ident(ident: &str) -> String {
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

pub(crate) fn value_ref_to_dax(vref: ValueRef<'_>, ct: ColumnType) -> DaxValue {
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

    Ok(super::DaxTableData { is_calculated: false, 
        name: table_name.to_string(),
        columns: col_names,
        col_index,
        col_types,
        rows,
        sort_by_columns: HashMap::new(),
    })
}

/// v0.11 Frente G (Rodada 2, item 4): batch-reads `table_name` through
/// DuckDB's Arrow interface (`Statement::query_arrow`, column-at-a-time
/// `RecordBatch`es) instead of `Statement::query` + per-cell `row.get_ref`.
/// Still produces the SAME `DaxTableData`/`DaxValue` shape as
/// `load_table_from_duckdb` (no storage change, see `docs/RUST_ENGINE_2.md`
/// §5.4/§3 item 2 — that bigger migration is out of scope here); this is
/// only about how fast the cells get copied out of DuckDB.
///
/// Falls back to the row-by-row loader for ANY column whose Arrow
/// `DataType` isn't one of the common ones DuckDB actually emits for our
/// supported DAX column types — never half-converts a table.
pub fn load_table_from_duckdb_arrow(
    conn: &Connection,
    table_name: &str,
    declared_types: &[Option<String>],
) -> Result<super::DaxTableData, String> {
    use duckdb::arrow::array::{
        Array, BooleanArray, Date32Array, Decimal128Array, Float32Array, Float64Array, Int16Array, Int32Array,
        Int64Array, Int8Array, LargeStringArray, StringArray, TimestampMicrosecondArray, UInt16Array, UInt32Array,
        UInt64Array, UInt8Array,
    };
    use duckdb::arrow::datatypes::{DataType, TimeUnit};

    let select_sql = format!("SELECT * FROM {}", quote_ident(table_name));
    let mut stmt = conn
        .prepare(&select_sql)
        .map_err(|e| format!("Erro ao preparar leitura (arrow) de '{}': {}", table_name, e))?;
    let arrow_iter = stmt
        .query_arrow([])
        .map_err(|e| format!("Erro ao executar leitura (arrow) de '{}': {}", table_name, e))?;
    let schema = arrow_iter.get_schema();

    let col_names: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
    if col_names.is_empty() {
        return Err(format!(
            "Tabela DuckDB '{}' não existe ou não tem colunas (pode não ter sido recarregada ainda nesta sessão)",
            table_name
        ));
    }
    let col_types: Vec<ColumnType> = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(idx, f)| {
            let base = match f.data_type() {
                DataType::Boolean => ColumnType::Boolean,
                DataType::Date32 | DataType::Date64 | DataType::Timestamp(_, _) => ColumnType::Date,
                DataType::Float16 | DataType::Float32 | DataType::Float64 | DataType::Decimal128(_, _) => {
                    ColumnType::Decimal
                }
                DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64 => ColumnType::Integer,
                _ => ColumnType::Text,
            };
            let declared = declared_types.get(idx).and_then(|o| o.as_deref());
            effective_column_type(declared, base)
        })
        .collect();

    let mut rows: Vec<Vec<DaxValue>> = Vec::new();
    for batch_res in arrow_iter {
        let batch = batch_res;
        let n = batch.num_rows();
        let n_cols = batch.num_columns();
        // One column extracted to a typed slice/array per batch, reused
        // across all rows of the batch — this is the whole point: avoid
        // one dynamic-dispatch `get_ref` FFI-ish call per CELL.
        let cols: Vec<&dyn Array> = (0..n_cols).map(|c| batch.column(c).as_ref()).collect();
        let base_row_len = rows.len();
        rows.resize_with(base_row_len + n, Vec::new);
        for c in 0..n_cols {
            let arr = cols[c];
            let ct = col_types[c];
            macro_rules! fill_int {
                ($arrty:ty) => {{
                    let a = arr.as_any().downcast_ref::<$arrty>().ok_or_else(|| {
                        format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name)
                    })?;
                    for r in 0..n {
                        rows[base_row_len + r].push(if a.is_null(r) { DaxValue::Blank } else { int_to_dax(a.value(r) as i64, ct) });
                    }
                }};
            }
            macro_rules! fill_float {
                ($arrty:ty) => {{
                    let a = arr.as_any().downcast_ref::<$arrty>().ok_or_else(|| {
                        format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name)
                    })?;
                    for r in 0..n {
                        rows[base_row_len + r]
                            .push(if a.is_null(r) { DaxValue::Blank } else { float_to_dax(a.value(r) as f64, ct) });
                    }
                }};
            }
            match arr.data_type() {
                DataType::Int8 => fill_int!(Int8Array),
                DataType::Int16 => fill_int!(Int16Array),
                DataType::Int32 => fill_int!(Int32Array),
                DataType::Int64 => fill_int!(Int64Array),
                DataType::UInt8 => fill_int!(UInt8Array),
                DataType::UInt16 => fill_int!(UInt16Array),
                DataType::UInt32 => fill_int!(UInt32Array),
                DataType::UInt64 => fill_int!(UInt64Array),
                DataType::Float32 => fill_float!(Float32Array),
                DataType::Float64 => fill_float!(Float64Array),
                DataType::Boolean => {
                    let a = arr
                        .as_any()
                        .downcast_ref::<BooleanArray>()
                        .ok_or_else(|| format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name))?;
                    for r in 0..n {
                        rows[base_row_len + r].push(if a.is_null(r) {
                            DaxValue::Blank
                        } else if ct == ColumnType::Text {
                            DaxValue::Text(a.value(r).to_string())
                        } else {
                            DaxValue::Boolean(a.value(r))
                        });
                    }
                }
                DataType::Utf8 => {
                    let a = arr
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .ok_or_else(|| format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name))?;
                    for r in 0..n {
                        rows[base_row_len + r]
                            .push(if a.is_null(r) { DaxValue::Blank } else { text_to_dax(a.value(r), ct) });
                    }
                }
                DataType::LargeUtf8 => {
                    let a = arr.as_any().downcast_ref::<LargeStringArray>().ok_or_else(|| {
                        format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name)
                    })?;
                    for r in 0..n {
                        rows[base_row_len + r]
                            .push(if a.is_null(r) { DaxValue::Blank } else { text_to_dax(a.value(r), ct) });
                    }
                }
                DataType::Date32 => {
                    let a = arr
                        .as_any()
                        .downcast_ref::<Date32Array>()
                        .ok_or_else(|| format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name))?;
                    for r in 0..n {
                        rows[base_row_len + r].push(if a.is_null(r) {
                            DaxValue::Blank
                        } else {
                            DaxValue::DateTime(a.value(r) as f64 + SERIAL_EPOCH_OFFSET_DAYS)
                        });
                    }
                }
                DataType::Timestamp(TimeUnit::Microsecond, _) => {
                    let a = arr.as_any().downcast_ref::<TimestampMicrosecondArray>().ok_or_else(|| {
                        format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name)
                    })?;
                    for r in 0..n {
                        rows[base_row_len + r].push(if a.is_null(r) {
                            DaxValue::Blank
                        } else {
                            DaxValue::DateTime(a.value(r) as f64 / 86_400_000_000.0 + SERIAL_EPOCH_OFFSET_DAYS)
                        });
                    }
                }
                DataType::Decimal128(_, scale) => {
                    let a = arr.as_any().downcast_ref::<Decimal128Array>().ok_or_else(|| {
                        format!("coluna {} de '{}': tipo Arrow inesperado", col_names[c], table_name)
                    })?;
                    let div = 10f64.powi(*scale as i32);
                    for r in 0..n {
                        rows[base_row_len + r].push(if a.is_null(r) {
                            DaxValue::Blank
                        } else {
                            float_to_dax(a.value(r) as f64 / div, ct)
                        });
                    }
                }
                other => {
                    return Err(format!(
                        "coluna {} de '{}': tipo Arrow {:?} sem conversão rápida (fallback para leitura linha a linha)",
                        col_names[c], table_name, other
                    ));
                }
            }
        }
    }

    let mut col_index = HashMap::new();
    for (idx, c) in col_names.iter().enumerate() {
        col_index.insert(c.to_lowercase(), idx);
    }

    LOAD_COUNT.with(|c| c.set(c.get() + 1));

    Ok(super::DaxTableData {
        is_calculated: false,
        name: table_name.to_string(),
        columns: col_names,
        col_index,
        col_types,
        rows,
        sort_by_columns: HashMap::new(),
    })
}

/// Tries the Arrow batch loader first (fast path, see
/// `load_table_from_duckdb_arrow`'s docs); any error (an unsupported Arrow
/// type, or a DuckDB-side failure) falls back to the always-correct
/// row-by-row `load_table_from_duckdb`. Never returns a partially-converted
/// table: the Arrow loader either converts the WHOLE table or bails before
/// producing any `DaxTableData` at all.
pub fn load_table_from_duckdb_fast(
    conn: &Connection,
    table_name: &str,
    declared_types: &[Option<String>],
) -> Result<super::DaxTableData, String> {
    match load_table_from_duckdb_arrow(conn, table_name, declared_types) {
        Ok(data) => Ok(data),
        Err(_arrow_err) => load_table_from_duckdb(conn, table_name, declared_types),
    }
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
        match load_table_from_duckdb_fast(conn, duckdb_name, &t.column_types) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    /// v0.11 Frente G (Rodada 2, item 4): `load_table_from_duckdb_arrow`
    /// (and the `load_table_from_duckdb_fast` wrapper around it) must
    /// produce EXACTLY the same `DaxTableData` as the always-correct
    /// row-by-row `load_table_from_duckdb` — same columns, same types, same
    /// `DaxValue` per cell, BLANK included — across every column type this
    /// engine supports: integer, decimal, text, boolean, date, declared
    /// currency/percentage, and NULLs in each.
    #[test]
    fn arrow_loader_matches_row_by_row_loader_for_every_supported_type() {
        let conn = Connection::open_in_memory().expect("open duckdb");
        conn.execute_batch(
            "CREATE TABLE tipos (
                i INTEGER, big BIGINT, f DOUBLE, t VARCHAR, b BOOLEAN, d DATE, cur DOUBLE, pct DOUBLE
             );
             INSERT INTO tipos VALUES
                (1, 10, 1.5, 'a', true, '2026-01-10', 19.99, 0.5),
                (NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
                (-3, -40, -2.25, 'texto com espaço', false, '2026-12-31', 0.0, 1.0);",
        )
        .expect("seed tipos");

        let declared: Vec<Option<String>> = vec![
            Some("number".into()),
            Some("number".into()),
            Some("number".into()),
            Some("text".into()),
            Some("boolean".into()),
            Some("date".into()),
            Some("currency".into()),
            Some("percentage".into()),
        ];

        let row_by_row = load_table_from_duckdb(&conn, "tipos", &declared).expect("row-by-row load");
        let arrow = load_table_from_duckdb_arrow(&conn, "tipos", &declared).expect("arrow load must fire for these plain types");
        let fast = load_table_from_duckdb_fast(&conn, "tipos", &declared).expect("fast wrapper load");

        assert_eq!(row_by_row.columns, arrow.columns);
        assert_eq!(row_by_row.col_types, arrow.col_types);
        assert_eq!(row_by_row.rows, arrow.rows, "arrow loader rows differ from row-by-row loader");
        assert_eq!(row_by_row.rows, fast.rows);
    }

    /// Same comparison, but letting type inference run (no declared types),
    /// matching how most real projects sync tables today.
    #[test]
    fn arrow_loader_matches_row_by_row_loader_with_inferred_types() {
        let conn = Connection::open_in_memory().expect("open duckdb");
        conn.execute_batch(
            "CREATE TABLE infer (CatID INTEGER, Categoria VARCHAR, Valor DOUBLE);
             INSERT INTO infer VALUES (1, 'A', 100.0), (2, NULL, NULL), (3, 'C', -5.5);",
        )
        .expect("seed infer");
        let declared: Vec<Option<String>> = vec![None, None, None];
        let row_by_row = load_table_from_duckdb(&conn, "infer", &declared).expect("row-by-row load");
        let arrow = load_table_from_duckdb_arrow(&conn, "infer", &declared).expect("arrow load");
        assert_eq!(row_by_row.col_types, arrow.col_types);
        assert_eq!(row_by_row.rows, arrow.rows);
    }

    /// 10k rows across a batch boundary (DuckDB's Arrow interface yields
    /// multiple `RecordBatch`es past its internal vector size), checked
    /// with a closed-form sum so a batch-boundary bug (off-by-one row
    /// count, a column re-sliced wrong between batches) would show up as a
    /// wrong total, not just a row-count mismatch.
    #[test]
    fn arrow_loader_matches_row_by_row_loader_across_many_batches() {
        let conn = Connection::open_in_memory().expect("open duckdb");
        conn.execute_batch(
            "CREATE TABLE many AS SELECT i AS Id, CAST(i AS DOUBLE) AS Valor FROM range(1, 10001) AS t(i)",
        )
        .expect("seed many");
        let declared: Vec<Option<String>> = vec![Some("number".into()), Some("number".into())];
        let row_by_row = load_table_from_duckdb(&conn, "many", &declared).expect("row-by-row load");
        let arrow = load_table_from_duckdb_arrow(&conn, "many", &declared).expect("arrow load");
        assert_eq!(row_by_row.rows.len(), 10_000);
        assert_eq!(arrow.rows.len(), 10_000);
        assert_eq!(row_by_row.rows, arrow.rows);
        let sum: f64 = arrow.rows.iter().filter_map(|r| r[1].as_f64()).sum();
        assert_eq!(sum, 10_000.0 * 10_001.0 / 2.0);
    }
}
