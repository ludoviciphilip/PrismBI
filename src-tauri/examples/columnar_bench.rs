//! Standalone bench for v0.11 Frente B: compares the two candidate
//! columnar cache formats (Parquet+zstd per table vs a single attached
//! `.duckdb` file) on synthetic data, in isolated processes so each
//! measurement gets a clean RSS peak (`VmHWM`).
//!
//! Usage: columnar_bench <mode> <n_rows> <path>
//!   mode: save-parquet | save-duckdb | load-parquet | load-duckdb
//!
//! Numbers are reported as a single CSV line to stdout:
//!   mode,n_rows,elapsed_ms,bytes_on_disk,vmhwm_kb[,checksum]
//!
//! Synthetic "fact" table: id BIGINT, cat VARCHAR, val DOUBLE, qty INT,
//! with val = id (as f64) so SUM(val) has the closed form N(N+1)/2 — used
//! to confirm round-trip correctness, not just timing.
use duckdb::Connection;
use std::env;
use std::fs;
use std::time::Instant;

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

fn dir_size_bytes(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if p.is_file() {
        return fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    }
    if p.is_dir() {
        let mut total = 0u64;
        for entry in walkdir_simple(p) {
            if let Ok(m) = fs::metadata(&entry) {
                total += m.len();
            }
        }
        return total;
    }
    0
}

fn walkdir_simple(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walkdir_simple(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: columnar_bench <mode> <n_rows> <path>");
        std::process::exit(2);
    }
    let mode = args[1].as_str();
    let n: i64 = args[2].parse().expect("n_rows must be an integer");
    let path = args[3].as_str();

    match mode {
        "save-parquet" => save_parquet(n, path),
        "save-duckdb" => save_duckdb(n, path),
        "load-parquet" => load_parquet(path),
        "load-duckdb" => load_duckdb(path),
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
}

fn make_fact(conn: &Connection, n: i64) {
    conn.execute(
        &format!(
            "CREATE OR REPLACE TABLE fact AS \
             SELECT id, 'cat_' || (id % 50) AS cat, id::DOUBLE AS val, (id % 7)::INTEGER AS qty \
             FROM range({n}) t(id)"
        ),
        [],
    )
    .expect("create synthetic fact table");
}

fn save_parquet(n: i64, path: &str) {
    let _ = fs::remove_file(path);
    let conn = Connection::open_in_memory().expect("open duckdb");
    make_fact(&conn, n);

    let start = Instant::now();
    let tmp = format!("{path}.tmp");
    conn.execute(
        &format!(
            "COPY fact TO '{}' (FORMAT PARQUET, COMPRESSION ZSTD)",
            tmp.replace('\'', "''")
        ),
        [],
    )
    .expect("copy to parquet");
    fs::rename(&tmp, path).expect("atomic rename parquet");
    let elapsed = start.elapsed().as_millis();

    let bytes = dir_size_bytes(path);
    println!("save-parquet,{n},{elapsed},{bytes},{}", vm_hwm_kb());
}

fn save_duckdb(n: i64, path: &str) {
    let _ = fs::remove_file(path);
    let conn = Connection::open_in_memory().expect("open duckdb");
    make_fact(&conn, n);

    let start = Instant::now();
    let tmp = format!("{path}.tmp");
    let _ = fs::remove_file(&tmp);
    conn.execute(&format!("ATTACH '{}' AS cache", tmp.replace('\'', "''")), [])
        .expect("attach cache db");
    conn.execute("CREATE TABLE cache.fact AS SELECT * FROM fact", [])
        .expect("copy into attached db");
    conn.execute("DETACH cache", []).expect("detach cache db");
    fs::rename(&tmp, path).expect("atomic rename duckdb file");
    let elapsed = start.elapsed().as_millis();

    let bytes = dir_size_bytes(path);
    println!("save-duckdb,{n},{elapsed},{bytes},{}", vm_hwm_kb());
}

fn load_parquet(path: &str) {
    let conn = Connection::open_in_memory().expect("open duckdb");
    let start = Instant::now();
    conn.execute(
        &format!(
            "CREATE OR REPLACE TABLE fact AS SELECT * FROM read_parquet('{}')",
            path.replace('\'', "''")
        ),
        [],
    )
    .expect("load from parquet");
    let sum: f64 = conn
        .query_row("SELECT SUM(val) FROM fact", [], |r| r.get(0))
        .expect("sum");
    let elapsed = start.elapsed().as_millis();
    println!(
        "load-parquet,0,{elapsed},0,{},{}",
        vm_hwm_kb(),
        sum as i64
    );
}

fn load_duckdb(path: &str) {
    let conn = Connection::open_in_memory().expect("open duckdb");
    let start = Instant::now();
    conn.execute(
        &format!("ATTACH '{}' AS cache (READ_ONLY)", path.replace('\'', "''")),
        [],
    )
    .expect("attach cache db read-only");
    conn.execute("CREATE TABLE fact AS SELECT * FROM cache.fact", [])
        .expect("copy from attached db");
    let sum: f64 = conn
        .query_row("SELECT SUM(val) FROM fact", [], |r| r.get(0))
        .expect("sum");
    let elapsed = start.elapsed().as_millis();
    println!(
        "load-duckdb,0,{elapsed},0,{},{}",
        vm_hwm_kb(),
        sum as i64
    );
}
