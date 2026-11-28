//! v0.11 Frente G (Rodada 2): release-only bench for the star-schema
//! vectorized fast path (`try_vectorized_star_group_agg`), the sibling of
//! `dax::pushdown::bench` but for a REAL star model (fact + dimension,
//! category on the dimension) instead of a single flat table. Run via:
//! `cargo test --release --lib -- --ignored --nocapture bench_star`
//! (same `flock`/`systemd-run` wrapper as every other v0.11 bench).
//!
//! Always goes through `execute_visual_query` (no DuckDB connection), so
//! whichever code path is live in the compiled binary is exercised exactly
//! as production does: the star fast path when it exists and fires, the
//! pre-existing per-category `get_visible_row_indices` loop otherwise. That
//! makes this file usable UNCHANGED for both the "antes" and "depois"
//! release binaries described in `docs/RUST_ENGINE_2.md` §5 Rodada 2 — only
//! `query_planner.rs` itself differs between the two builds.
#![cfg(test)]

use super::super::duckdb_load;
use super::super::{DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use super::{execute_visual_query, NativeVisualQueryRequest};
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

const N_CATEGORIES: i64 = 1000;

/// Fact(FatoID, CatID, Valor) with N rows, FK uniformly spread over
/// `N_CATEGORIES` dimension rows, plus Dim(CatID, Categoria) with exactly
/// `N_CATEGORIES` rows — the shape the task brief asks for: "fato com FK
/// para uma dimensão de 1.000 categorias". `SUM(Valor) = N(N+1)/2 / 3`
/// (closed form, same shape as `pushdown::bench`'s synthetic data) checked
/// on every run as a correctness guard, not just a speed number.
fn seed_star_schema(conn: &Connection, n: i64) {
    conn.execute_batch(&format!(
        "CREATE OR REPLACE TABLE dim_cat AS \
         SELECT i AS CatID, 'Categoria ' || i AS Categoria \
         FROM range(0, {n_cat}) AS t(i);
         CREATE OR REPLACE TABLE fact_star AS \
         SELECT i AS FatoID, (i % {n_cat}) AS CatID, CAST(i AS DOUBLE) / 3.0 AS Valor \
         FROM range(1, {n_rows}) AS t(i)",
        n_cat = N_CATEGORIES,
        n_rows = n + 1,
    ))
    .expect("seed synthetic star schema");
}

fn sync_payload() -> DaxModelSyncPayload {
    DaxModelSyncPayload {
        tables: vec![
            DaxTableInput {
                name: "dim_cat".into(),
                columns: vec!["CatID".into(), "Categoria".into()],
                rows: vec![],
                column_types: vec![Some("number".into()), Some("text".into())],
                duckdb_table: Some("dim_cat".into()),
                sort_by_columns: Default::default(),
                ..Default::default()
            },
            DaxTableInput {
                name: "fact_star".into(),
                columns: vec!["FatoID".into(), "CatID".into(), "Valor".into()],
                rows: vec![],
                column_types: vec![Some("number".into()), Some("number".into()), Some("number".into())],
                duckdb_table: Some("fact_star".into()),
                sort_by_columns: Default::default(),
                ..Default::default()
            },
        ],
        relationships: vec![DaxRelationshipInput {
            from_table: "fact_star".into(),
            from_column: "CatID".into(),
            to_table: "dim_cat".into(),
            to_column: "CatID".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        measures: vec![],
    }
}

fn model_for_dax(conn: &Connection) -> DaxModel {
    let dim_types = [Some("number".to_string()), Some("text".to_string())];
    let fact_types = [Some("number".to_string()), Some("number".to_string()), Some("number".to_string())];
    let dim_data = duckdb_load::load_table_from_duckdb(conn, "dim_cat", &dim_types).expect("load dim_cat");
    let fact_data = duckdb_load::load_table_from_duckdb(conn, "fact_star", &fact_types).expect("load fact_star");
    let mut freshly = HashMap::new();
    freshly.insert("dim_cat".to_string(), (1u64, dim_data));
    freshly.insert("fact_star".to_string(), (1u64, fact_data));
    let mut model = DaxModel::new();
    model.sync_from_payload_with_duckdb(sync_payload(), freshly);
    model
}

fn sum_req() -> NativeVisualQueryRequest {
    NativeVisualQueryRequest {
        visual_id: "bench_star".into(),
        visual_type: "column".into(),
        table_id: "fact_star".into(),
        x_axis_field: Some("dim_cat.Categoria".into()),
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
    seed_star_schema(&conn, n);
    let expected_total = closed_form_total(n);
    let req = sum_req();

    let t_load = Instant::now();
    let model = model_for_dax(&conn);
    let load_elapsed = t_load.elapsed();
    let hwm_after_load = vm_hwm_kb();

    let t_query = Instant::now();
    let res = execute_visual_query(&model, &req).expect("DAX path");
    let query_elapsed = t_query.elapsed();
    let hwm_after_query = vm_hwm_kb();

    let total: f64 = res.series[0].data.iter().filter_map(|v| *v).sum();
    assert!(
        (total - expected_total).abs() < 1.0,
        "{label}: total {total} != closed form {expected_total}"
    );
    assert_eq!(res.categories.len(), N_CATEGORIES as usize, "{label}: expected {N_CATEGORIES} categories");

    println!(
        "{label}: N={n}, categorias={}\n  load: {load_elapsed:?}, VmHWM -> {hwm_after_load:?} KB\n  query: {query_elapsed:?}, VmHWM -> {hwm_after_query:?} KB (engine={})",
        res.categories.len(),
        res.engine,
    );
}

#[test]
#[ignore]
fn bench_star_1_million_rows() {
    run_bench(1_000_000, "bench_star_1_million_rows");
}

#[test]
#[ignore]
fn bench_star_10_million_rows() {
    run_bench(10_000_000, "bench_star_10_million_rows");
}
