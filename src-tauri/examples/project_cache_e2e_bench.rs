//! Measures the real `project_store::save_project_cache` /
//! `load_project_cache` path end-to-end (not the raw duckdb_bench numbers)
//! with 10M rows, reporting the number required by docs/V011.md §B meta:
//! "reabrir um projeto com 10M linhas em menos de 1s até o primeiro
//! visual" — here, until `load_project_cache` returns with the table
//! materialized back into the live DuckDB connection (the point at which
//! `sync_model_to_rust`/the DAX model can see full data).
use duckdb::Connection;
use std::env;
use std::fs;
use std::time::Instant;
use vetrobi_lib::project_store::{
    load_project_cache, save_project_cache, TableCacheMeta,
};

fn vm_hwm_kb() -> u64 {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                return rest.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("save");
    let n: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10_000_000);
    let dir = args.get(3).cloned().unwrap_or_else(|| "/tmp/vetrobi_e2e_cache".to_string());

    let meta = TableCacheMeta {
        name: "fact".to_string(),
        columns_signature: "id:number,val:number".to_string(),
        source_signature: "bench://fixed".to_string(),
        row_count: n as usize,
    };

    match mode {
        "save" => {
            let conn = Connection::open_in_memory().expect("open duckdb");
            conn.execute(
                &format!(
                    "CREATE TABLE fact AS SELECT id, id::DOUBLE AS val FROM range({n}) t(id)"
                ),
                [],
            )
            .expect("create synthetic fact table");
            let outcome = save_project_cache(&conn, std::path::Path::new(&dir), &[meta], "0.11.0-e2e")
                .expect("save_project_cache");
            println!(
                "e2e-save,{n},{},{}",
                outcome.elapsed_ms,
                outcome.bytes_on_disk
            );
        }
        "load" => {
            let conn = Connection::open_in_memory().expect("open duckdb");
            let start = Instant::now();
            let outcome = load_project_cache(&conn, std::path::Path::new(&dir), &[meta])
                .expect("load_project_cache");
            let wall_ms = start.elapsed().as_millis();
            let sum: f64 = conn
                .query_row("SELECT SUM(val) FROM fact", [], |r| r.get(0))
                .expect("sum after cache load");
            println!(
                "e2e-load,{n},loaded={},internal_ms={},wall_ms={},rss_kb={},sum={}",
                outcome.loaded,
                outcome.elapsed_ms,
                wall_ms,
                vm_hwm_kb(),
                sum as i64
            );
        }
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
}
