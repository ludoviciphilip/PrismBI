//! The five Tauri commands of the v0.9 ETL contract plus their testable
//! implementations (which only need a `DuckDbStore`).

use super::m_from::{query_from_m, FromMResult};
use super::m_to::query_to_m;
use super::model::{EtlQuery, EtlSource};
use super::raw::{load_raw, visible_columns, EtlColumn, RawLoadResult};
use super::sql::{clean_err, compile, quote_ident, Compiled, ROW_IDX};
use crate::engine::duckdb_store::{build_table_payload_pub, execute_select_query, DuckDbStore, ImportedTablePayload};
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::path::{Path, PathBuf};
use tauri::State;

pub const DEFAULT_PREVIEW_LIMIT: usize = 1000;
pub const MAX_PREVIEW_LIMIT: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedStep {
    pub index: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub columns: Vec<EtlColumn>,
    pub rows: Vec<Map<String, JsonValue>>,
    pub row_count: usize,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_step: Option<FailedStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FromMResponse {
    pub query: EtlQuery,
    pub warnings: Vec<String>,
}

fn lock(db: &DuckDbStore) -> Result<std::sync::MutexGuard<'_, duckdb::Connection>, String> {
    db.connection().lock().map_err(|_| "Lock DuckDB corrompido".to_string())
}

pub fn etl_load_raw_impl(
    db: &DuckDbStore,
    query: &str,
    source: &EtlSource,
    pbip_root: Option<&Path>,
) -> Result<RawLoadResult, String> {
    let conn = lock(db)?;
    load_raw(&conn, query, source, pbip_root)
}

fn select_visible(c: &Compiled) -> String {
    let cols: Vec<String> = c.chain.visible().iter().map(|c| quote_ident(&c.name)).collect();
    if cols.is_empty() {
        // DuckDB cannot select zero columns; the caller treats this as "no columns".
        "1 AS \"__none\"".into()
    } else {
        cols.join(", ")
    }
}

fn run_preview(conn: &duckdb::Connection, c: &Compiled, limit: usize) -> Result<(Vec<Map<String, JsonValue>>, usize), String> {
    let rows_sql = c.chain.select(&select_visible(c), &format!("ORDER BY {} LIMIT {}", quote_ident(ROW_IDX), limit));
    let mut res = execute_select_query(conn, &rows_sql).map_err(|e| clean_err(&e))?;
    if c.chain.visible().is_empty() {
        for r in &mut res.rows {
            r.clear();
        }
    }
    let count: i64 = conn
        .query_row(&c.chain.select("count(*)", ""), [], |r| r.get(0))
        .map_err(|e| clean_err(&e.to_string()))?;
    Ok((res.rows, count.max(0) as usize))
}

/// `uptoStep`: inclusive step index (negative = only the source). On a
/// failing step the data of the previous step is returned with `failedStep`.
pub fn etl_preview_impl(
    db: &DuckDbStore,
    query: &EtlQuery,
    upto_step: Option<i64>,
    limit: Option<usize>,
) -> Result<PreviewResult, String> {
    let conn = lock(db)?;
    ensure_raw(&conn, query)?;
    let limit = limit.unwrap_or(DEFAULT_PREVIEW_LIMIT).clamp(1, MAX_PREVIEW_LIMIT);
    let upto: Option<usize> = match upto_step {
        Some(u) if u < 0 => None,
        Some(u) => Some(u as usize),
        None => None,
    };
    let mut q = query.clone();
    if matches!(upto_step, Some(u) if u < 0) {
        q.steps.clear();
    }
    let mut compiled = compile(&conn, &q, upto)?;
    let mut failed = compiled.failed.clone();
    let mut data = run_preview(&conn, &compiled, limit);
    // Runtime failure (compiles, but execution errors): find the first prefix that fails.
    if data.is_err() && compiled.applied > 0 {
        let mut last_ok: Option<(Compiled, (Vec<Map<String, JsonValue>>, usize))> = None;
        for k in 0..compiled.applied {
            let c = compile(&conn, &q, Some(k)).map(|mut c| {
                c.applied = c.applied.min(k + 1);
                c
            })?;
            match run_preview(&conn, &c, limit) {
                Ok(d) => last_ok = Some((c, d)),
                Err(e) => {
                    let name = q.steps.get(k).map(|s| s.name.clone()).unwrap_or_default();
                    failed = Some((k, format!("Etapa '{}': {}", name, e)));
                    break;
                }
            }
        }
        match last_ok {
            Some((c, d)) => {
                compiled = c;
                data = Ok(d);
            }
            None => {
                let mut src_only = q.clone();
                src_only.steps.clear();
                compiled = compile(&conn, &src_only, None)?;
                data = run_preview(&conn, &compiled, limit);
            }
        }
    }
    let (rows, row_count) = data?;
    Ok(PreviewResult {
        columns: visible_columns(&compiled.chain.schema),
        rows,
        row_count,
        warnings: compiled.warnings.clone(),
        failed_step: failed.map(|(index, message)| FailedStep { index, message }),
    })
}

/// Loads `raw__<query>` on demand if it was never loaded (best effort, no pbipRoot).
fn ensure_raw(conn: &duckdb::Connection, q: &EtlQuery) -> Result<(), String> {
    let raw = super::sql::raw_table_name(&q.name);
    if super::sql::describe(conn, &format!("SELECT * FROM {}", quote_ident(&raw))).is_ok() {
        return Ok(());
    }
    load_raw(conn, &q.name, &q.source, None).map(|_| ())
}

/// Materializes the full query into `target_table` (CREATE OR REPLACE).
/// Returns `Ok(None)` when the result has no columns (e.g. a measures host table).
pub fn etl_apply_inner(db: &DuckDbStore, query: &EtlQuery, target_table: &str) -> Result<Option<ImportedTablePayload>, String> {
    let conn = lock(db)?;
    ensure_raw(&conn, query)?;
    let compiled = compile(&conn, query, None)?;
    if let Some((i, msg)) = &compiled.failed {
        return Err(format!("Consulta '{}', etapa {}: {}", query.name, i + 1, msg));
    }
    super::sql::check_user_name(target_table).map_err(|e| format!("tabela de destino inválida: {}", e))?;
    if compiled.chain.visible().is_empty() {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", quote_ident(target_table)))
            .map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let body = compiled
        .chain
        .select(&select_visible(&compiled), &format!("ORDER BY {}", quote_ident(ROW_IDX)));
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} AS {}", quote_ident(target_table), body))
        .map_err(|e| format!("Erro ao aplicar a consulta '{}': {}", query.name, clean_err(&e.to_string())))?;
    let mut payload = build_table_payload_pub(&conn, target_table, &query.name)?;
    payload.table_name = target_table.to_string();
    payload.original_name = query.name.clone();
    payload.source_path = query.source.path.clone();
    payload.sheet_name = query.source.sheet.clone();
    Ok(Some(payload))
}

pub fn etl_apply_impl(db: &DuckDbStore, query: &EtlQuery, target_table: &str) -> Result<ImportedTablePayload, String> {
    etl_apply_inner(db, query, target_table)?
        .ok_or_else(|| format!("A consulta '{}' não produz nenhuma coluna.", query.name))
}

pub fn etl_to_m_impl(query: &EtlQuery) -> Result<String, String> {
    query_to_m(query)
}

pub fn etl_from_m_impl(name: &str, m: &str) -> Result<FromMResponse, String> {
    let FromMResult { query, warnings } = query_from_m(name, m)?;
    Ok(FromMResponse { query, warnings })
}

// ------------------------------------------------------------ Tauri layer

#[tauri::command]
pub fn etl_load_raw(
    state: State<'_, AppState>,
    query: String,
    source: EtlSource,
    pbip_root: Option<String>,
) -> Result<RawLoadResult, String> {
    let root = pbip_root.filter(|s| !s.trim().is_empty()).map(PathBuf::from);
    etl_load_raw_impl(&state.db, &query, &source, root.as_deref())
}

#[tauri::command]
pub fn etl_preview(
    state: State<'_, AppState>,
    query: EtlQuery,
    upto_step: Option<i64>,
    limit: Option<usize>,
) -> Result<PreviewResult, String> {
    etl_preview_impl(&state.db, &query, upto_step, limit)
}

#[tauri::command]
pub fn etl_apply(state: State<'_, AppState>, query: EtlQuery, target_table: String) -> Result<ImportedTablePayload, String> {
    etl_apply_impl(&state.db, &query, &target_table)
}

#[tauri::command]
pub fn etl_to_m(query: EtlQuery) -> Result<String, String> {
    etl_to_m_impl(&query)
}

#[tauri::command]
pub fn etl_from_m(name: String, m: String) -> Result<FromMResponse, String> {
    etl_from_m_impl(&name, &m)
}
