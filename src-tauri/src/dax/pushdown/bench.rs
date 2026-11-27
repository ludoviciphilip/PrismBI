//! v0.11 Frente A: release-only benches for 1M/10M synthetic rows (`docs/V011.md`
//! Frente A target: SUM by category over 10M rows in < 200ms, memory that
//! doesn't grow with N). Run explicitly, in release, via:
//! `cargo test --release --lib -- --ignored --nocapture bench_pushdown`
//! (the usual systemd-run + flock wrapper from `docs/V011.md`/`CONTRIBUTING.md`).
//!
//! Each bench measures **both** paths on the same synthetic DuckDB table —
//! pushdown (SQL aggregation, DAX model holds zero rows) and the DAX engine
//! loading every row into the model (`duckdb_load::load_table_from_duckdb`)
//! then aggregating in Rust — so the numbers in the PR report are a real
//! before/after, not just a pushdown number in isolation. Both are checked
//! against the closed-form total `SUM(i/3.0, i=1..N) = (N*(N+1)/2) / 3.0`
//! (`docs/V011.md`: "use dados sintéticos... com totais em forma fechada").
//!
//! Note: the DAX table name here is deliberately the *same* string as the
//! DuckDB table name ("big"). `query_planner::resolve_table_and_col` (and
//! `collect_groups`) return `DaxTableData::name` — which `duckdb_load`
//! stamps with the *DuckDB* table name, not the DAX display name — as the
//! table to look up in `DaxModel.tables` (keyed by the DAX name). The two
//! only coincide by naming convention today; a project where a table's
//! DAX/display name differs from its backing DuckDB table name would hit a
//! silent "0 categories" on the plain DAX path too, pushdown or not. Found
//! while writing this bench; it's a pre-existing `query_planner.rs` quirk,
//! out of scope for this round (not something pushdown introduced), so
//! flagged here rather than fixed, and this bench just avoids it by naming
//! both the same.

use super::super::query_planner::{execute_visual_query, NativeVisualQueryRequest};
use super::super::{duckdb_load, DaxModel, DaxModelSyncPayload, DaxTableInput};
use super::try_pushdown;
use duckdb::Connection;
use std::collections::HashMap;
use std::time::Instant;

fn vm_hwm_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("VmHWM:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|n| n.parse().ok())
    })
}

fn seed_big_table(conn: &Connection, n: i64) {
    conn.execute_batch(&format!(
        "CREATE OR REPLACE TABLE big AS \
         SELECT (i % 100) AS Categoria, CAST(i AS DOUBLE) / 3.0 AS Valor \
         FROM range(1, {} ) AS t(i)",
        n + 1
    ))
    .expect("seed synthetic fact table");
}

fn sync_payload() -> DaxModelSyncPayload {
    DaxModelSyncPayload {
        tables: vec![DaxTableInput {
            name: "big".into(),
            columns: vec!["Categoria".into(), "Valor".into()],
            rows: vec![],
            column_types: vec![Some("number".into()), Some("number".into())],
            duckdb_table: Some("big".into()),
            sort_by_columns: Default::default(),
            ..Default::default()
        }],
        relationships: vec![],
        measures: vec![],
    }
}

/// DAX model whose `big` table is pushdown-ready but holds **zero rows** —
/// exactly what happens in production once a table syncs with a
/// `duckdb_table` set but pushdown never needs `load_table_from_duckdb` to
/// run. This is the "memory doesn't grow with N" half of the bench.
fn model_for_pushdown() -> DaxModel {
    let mut model = DaxModel::new();
    // No freshly_loaded entry: falls back to the (empty) JSON rows (with a
    // stderr warning) — by design, this is exactly the "never materialize
    // the table" path pushdown relies on.
    model.sync_from_payload_with_duckdb(sync_payload(), HashMap::new());
    model
}

/// The pre-pushdown path: every row of `big` loaded into the model, like
/// `sync_model_to_rust` does today.
fn model_for_dax(conn: &Connection) -> DaxModel {
    let data = duckdb_load::load_table_from_duckdb(conn, "big", &[Some("number".into()), Some("number".into())])
        .expect("load big table for DAX path");
    let mut freshly = HashMap::new();
    freshly.insert("big".to_string(), (1u64, data));
    let mut model = DaxModel::new();
    model.sync_from_payload_with_duckdb(sync_payload(), freshly);
    model
}

fn sum_req() -> NativeVisualQueryRequest {
    NativeVisualQueryRequest {
        visual_id: "bench".into(),
        visual_type: "column".into(),
        table_id: "big".into(),
        x_axis_field: Some("Categoria".into()),
        legend_field: None,
        y_axis_fields: vec!["Valor".into()],
        aggregation: Some("SUM".into()),
        filters: vec![],
        ..Default::default()
    }
}

fn closed_form_total(n: i64) -> f64 {
    (n as f64) * (n as f64 + 1.0) / 2.0 / 3.0
}

fn run_bench(n: i64, label: &str) {
    let conn = Connection::open_in_memory().expect("open duckdb");
    seed_big_table(&conn, n);
    let expected_total = closed_form_total(n);
    let req = sum_req();

    // Pushdown: DAX model never materializes a single row of `big`.
    let model_pd = model_for_pushdown();
    let hwm_before_pd = vm_hwm_kb();
    let t0 = Instant::now();
    let pd = try_pushdown(&model_pd, &conn, &req, "SUM", Instant::now()).expect("pushdown must accept SUM by category");
    let elapsed_pd = t0.elapsed();
    let hwm_after_pd = vm_hwm_kb();
    let total_pd: f64 = pd.series[0].data.iter().filter_map(|v| *v).sum();
    assert_eq!(pd.categories.len(), 100);
    assert!((total_pd - expected_total).abs() < 1.0, "{}: pushdown total {} != closed form {}", label, total_pd, expected_total);

    // DAX engine: loads all N rows into the model first (today's behavior).
    let t1 = Instant::now();
    let model_dax = model_for_dax(&conn);
    let load_elapsed = t1.elapsed();
    let hwm_after_load = vm_hwm_kb();
    let t2 = Instant::now();
    let dax = execute_visual_query(&model_dax, &req).expect("DAX path");
    let elapsed_dax = t2.elapsed();
    let hwm_after_dax = vm_hwm_kb();
    let total_dax: f64 = dax.series[0].data.iter().filter_map(|v| *v).sum();
    assert!((total_dax - expected_total).abs() < 1.0, "{}: DAX total {} != closed form {}", label, total_dax, expected_total);
    assert_eq!(dax.categories.len(), pd.categories.len());

    println!(
        "{label}: N={n}\n  pushdown:  {elapsed_pd:?} (query only), VmHWM {hwm_before_pd:?} KB -> {hwm_after_pd:?} KB\n  DAX load:  {load_elapsed:?}, VmHWM -> {hwm_after_load:?} KB\n  DAX query: {elapsed_dax:?}, VmHWM -> {hwm_after_dax:?} KB (load+query total {:?})",
        load_elapsed + elapsed_dax,
    );
}

#[test]
#[ignore]
fn bench_pushdown_vs_dax_1_million_rows() {
    run_bench(1_000_000, "bench_pushdown_vs_dax_1_million_rows");
}

#[test]
#[ignore]
fn bench_pushdown_vs_dax_10_million_rows() {
    run_bench(10_000_000, "bench_pushdown_vs_dax_10_million_rows");
}
