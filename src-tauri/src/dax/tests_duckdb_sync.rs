//! Tests for the DuckDB-backed DAX table loader (`duckdb_load`): the full
//! point is that a model table synced via `duckdbTable` reflects ALL rows,
//! not the frontend's 5000-row preview cap. See docs/RUST_ENGINE_2.md §3.

use super::duckdb_load::{
    get_last_sync_version, load_count_for_test, load_table_from_duckdb, set_last_sync_version,
};
use super::{ColumnType, DaxModel, DaxModelSyncPayload, DaxTableInput, DaxValue};
use duckdb::Connection;

fn open_mem() -> Connection {
    Connection::open_in_memory().expect("open duckdb in-memory")
}

/// 20,000-row table synced with `duckdbTable`: SUM/COUNTROWS must reflect
/// every row, not just the first 5000 (the old JSON-preview bug).
#[test]
fn sum_and_count_over_20000_rows_via_duckdb_sync() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE OR REPLACE TABLE fVendas AS \
         SELECT i AS Id, (i % 10) AS Categoria, CAST(i AS DOUBLE) AS Valor \
         FROM range(1, 20001) AS t(i)",
    )
    .expect("seed 20k rows");

    let mut model = DaxModel::new();
    let payload = DaxModelSyncPayload {
        tables: vec![DaxTableInput {
            name: "fVendas".into(),
            columns: vec!["Id".into(), "Categoria".into(), "Valor".into()],
            rows: vec![],
            column_types: vec![],
            duckdb_table: Some("fVendas".into()),
        }],
        relationships: vec![],
        measures: vec![],
    };
    let conn_lock = conn;
    let data = load_table_from_duckdb(&conn_lock, "fVendas", &[]).expect("load full table");
    assert_eq!(data.rows.len(), 20_000, "all 20k rows must load, not just the 5000-row preview");

    let mut loaded = std::collections::HashMap::new();
    loaded.insert("fvendas".to_string(), (1u64, data));
    model.sync_from_payload_with_duckdb(payload, loaded);

    let tbl = model.get_table("fVendas").expect("table synced");
    assert_eq!(tbl.rows.len(), 20_000);

    let valor_idx = tbl.get_col_idx("Valor").unwrap();
    let sum: f64 = tbl
        .rows
        .iter()
        .map(|r| match &r[valor_idx] {
            DaxValue::Decimal(f) => *f,
            DaxValue::Integer(i) => *i as f64,
            _ => 0.0,
        })
        .sum();
    // SUM(1..20000) = 20000*20001/2 = 200,010,000
    assert_eq!(sum as i64, 200_010_000);
    assert_eq!(tbl.rows.len() as i64, 20_000);
}

/// Type mapping for every DuckDB type the importers can produce.
#[test]
fn duckdb_type_mapping_covers_importer_types() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE OR REPLACE TABLE t AS SELECT \
            CAST(1 AS TINYINT) AS c_tinyint, \
            CAST(2 AS SMALLINT) AS c_smallint, \
            CAST(3 AS INTEGER) AS c_int, \
            CAST(4 AS BIGINT) AS c_bigint, \
            CAST(5 AS HUGEINT) AS c_hugeint, \
            CAST(1.5 AS FLOAT) AS c_float, \
            CAST(2.5 AS DOUBLE) AS c_double, \
            CAST(3.25 AS DECIMAL(10,2)) AS c_decimal, \
            TRUE AS c_bool, \
            DATE '2024-03-15' AS c_date, \
            TIMESTAMP '2024-03-15 12:30:00' AS c_timestamp, \
            'hello' AS c_text",
    )
    .expect("seed typed row");

    let data = load_table_from_duckdb(&conn, "t", &[]).expect("load typed table");
    assert_eq!(data.rows.len(), 1);
    let row = &data.rows[0];
    let col = |name: &str| row[data.get_col_idx(name).unwrap()].clone();

    assert!(matches!(col("c_tinyint"), DaxValue::Integer(1)));
    assert!(matches!(col("c_smallint"), DaxValue::Integer(2)));
    assert!(matches!(col("c_int"), DaxValue::Integer(3)));
    assert!(matches!(col("c_bigint"), DaxValue::Integer(4)));
    assert!(matches!(col("c_hugeint"), DaxValue::Integer(5)));
    assert!(matches!(col("c_float"), DaxValue::Decimal(f) if (f - 1.5).abs() < 0.01));
    assert!(matches!(col("c_double"), DaxValue::Decimal(f) if (f - 2.5).abs() < 0.0001));
    assert!(matches!(col("c_decimal"), DaxValue::Decimal(f) if (f - 3.25).abs() < 0.0001));
    assert!(matches!(col("c_bool"), DaxValue::Boolean(true)));
    assert!(matches!(col("c_date"), DaxValue::DateTime(_)));
    assert!(matches!(col("c_timestamp"), DaxValue::DateTime(_)));
    assert!(matches!(col("c_text"), DaxValue::Text(s) if s == "hello"));

    // Declared "currency" overrides the structural DOUBLE -> Decimal mapping.
    let col_names = &data.columns;
    let decl: Vec<Option<String>> = col_names
        .iter()
        .map(|c| if c == "c_double" { Some("currency".to_string()) } else { None })
        .collect();
    let data2 = load_table_from_duckdb(&conn, "t", &decl).expect("load with declared currency");
    let v = &data2.rows[0][data2.get_col_idx("c_double").unwrap()];
    assert!(matches!(v, DaxValue::Currency(c) if *c == 25_000), "2.5 * 10_000 = 25_000 micros");
    let _ = ColumnType::Currency; // keep import used even if matches! above suffices
}

/// A second sync with the same DuckDB table version must not re-run
/// `SELECT *` (cache hit); a new version (CREATE OR REPLACE through an
/// importer/etl_apply bumps it) must reload. Drives the exact function
/// `sync_model_to_rust` calls.
#[test]
fn version_cache_skips_reload_when_table_unchanged() {
    use crate::dax::duckdb_load::load_changed_tables;
    use crate::engine::duckdb_store::bump_table_version;
    let conn = open_mem();
    // Unique name: the version maps are process-wide and tests run in parallel.
    let name = "cache_probe_tbl_7f3a";
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {name} AS SELECT 1 AS x")).unwrap();
    bump_table_version(name);
    let input = vec![DaxTableInput {
        name: "Cache".into(),
        columns: vec!["x".into()],
        rows: vec![],
        column_types: vec![],
        duckdb_table: Some(name.into()),
    }];

    let before = load_count_for_test();
    let first = load_changed_tables(&conn, &input);
    assert!(first.contains_key(name), "first sync must load the table");
    assert_eq!(load_count_for_test(), before + 1);

    let second = load_changed_tables(&conn, &input);
    assert!(second.is_empty(), "unchanged version must be skipped");
    assert_eq!(load_count_for_test(), before + 1, "no SELECT * on a cache hit");

    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {name} AS SELECT 2 AS x")).unwrap();
    bump_table_version(name);
    let third = load_changed_tables(&conn, &input);
    assert!(third.contains_key(name), "a new version must reload");
    assert_eq!(load_count_for_test(), before + 2);
    let _ = (get_last_sync_version, set_last_sync_version); // still exported for lib.rs-level tooling
}

/// Missing DuckDB table (e.g. right after reopening a project, since DuckDB
/// is in-memory and the table hasn't been recreated yet): the loader errors
/// clearly instead of panicking, and the model falls back to JSON rows.
#[test]
fn missing_duckdb_table_errors_and_model_falls_back_to_json_rows() {
    let conn = open_mem();
    let err = load_table_from_duckdb(&conn, "does_not_exist", &[]).unwrap_err();
    assert!(err.contains("does_not_exist"), "error should name the missing table: {err}");

    let mut model = DaxModel::new();
    let payload = DaxModelSyncPayload {
        tables: vec![DaxTableInput {
            name: "Fallback".into(),
            columns: vec!["A".into()],
            rows: vec![{
                let mut m = serde_json::Map::new();
                m.insert("A".into(), serde_json::json!(7));
                m
            }],
            column_types: vec![],
            duckdb_table: Some("does_not_exist".into()),
        }],
        relationships: vec![],
        measures: vec![],
    };
    // No entry in `freshly_loaded` and no prior cache: falls back to rows.
    model.sync_from_payload_with_duckdb(payload, std::collections::HashMap::new());
    let tbl = model.get_table("Fallback").expect("fallback table present");
    assert_eq!(tbl.rows.len(), 1);
    assert!(matches!(tbl.rows[0][0], DaxValue::Integer(7)));
}

/// Benchmark: sync of a 1,000,000-row DuckDB table. Run explicitly with
/// `cargo test --release --lib -- --ignored --nocapture` (needs --release:
/// debug builds are too slow and this is excluded from the normal suite).
#[test]
#[ignore]
fn bench_sync_1_million_rows() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE OR REPLACE TABLE big AS \
         SELECT i AS Id, (i % 100) AS Categoria, CAST(i AS DOUBLE) / 3.0 AS Valor \
         FROM range(1, 1000001) AS t(i)",
    )
    .expect("seed 1M rows");

    let rss_kb = || -> Option<u64> {
        std::fs::read_to_string("/proc/self/status").ok().and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
    };

    let rss_before = rss_kb();
    let start = std::time::Instant::now();
    let data = load_table_from_duckdb(&conn, "big", &[]).expect("load 1M rows");
    let elapsed = start.elapsed();
    let rss_after = rss_kb();

    assert_eq!(data.rows.len(), 1_000_000);
    println!(
        "bench_sync_1_million_rows: {:?} elapsed, RSS before={:?} KB after={:?} KB",
        elapsed, rss_before, rss_after
    );
}
