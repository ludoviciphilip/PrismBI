pub mod dax;
pub mod engine;
pub mod pbi;
pub mod updater;

use dax::{build_filter_context, DaxModel, DaxModelSyncPayload, NativeFilterInput};
use engine::{DuckDbStore, EngineInfo, ImportedTablePayload, SqlQueryResult, TableSummary};
use pbi::pbip_writer::ExportPbipPayload;
use pbi::PbiProjectSummary;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub db: DuckDbStore,
    pub dax_model: Mutex<DaxModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDaxEvalResult {
    pub value: JsonValue,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshPbipResult {
    pub loaded_tables: Vec<ImportedTablePayload>,
    pub warnings: Vec<String>,
}

#[tauri::command]
fn get_engine_info(state: State<'_, AppState>) -> Result<EngineInfo, String> {
    state.db.get_engine_info()
}

#[tauri::command]
fn import_csv_native(
    state: State<'_, AppState>,
    path: String,
    table_name: Option<String>,
) -> Result<ImportedTablePayload, String> {
    state.db.import_csv(&PathBuf::from(path), table_name.as_deref())
}

#[tauri::command]
fn import_parquet_native(
    state: State<'_, AppState>,
    path: String,
    table_name: Option<String>,
) -> Result<ImportedTablePayload, String> {
    state.db.import_parquet(&PathBuf::from(path), table_name.as_deref())
}

#[tauri::command]
fn import_json_native(
    state: State<'_, AppState>,
    path: String,
    table_name: Option<String>,
) -> Result<ImportedTablePayload, String> {
    state.db.import_json_file(&PathBuf::from(path), table_name.as_deref())
}

#[tauri::command]
fn import_xlsx_native(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<ImportedTablePayload>, String> {
    state.db.import_xlsx(&PathBuf::from(path))
}

#[tauri::command]
fn import_xlsx_sheet_native(
    state: State<'_, AppState>,
    path: String,
    sheet_name: Option<String>,
    table_name: Option<String>,
) -> Result<ImportedTablePayload, String> {
    state.db.import_xlsx_sheet(
        &PathBuf::from(path),
        sheet_name.as_deref(),
        table_name.as_deref(),
    )
}

#[tauri::command]
fn refresh_table_native(
    state: State<'_, AppState>,
    request: pbi::m_loader::RefreshTableRequest,
) -> Result<ImportedTablePayload, String> {
    pbi::m_loader::refresh_single_table(&request, &state.db)
}

#[tauri::command]
fn refresh_pbip_tables(
    state: State<'_, AppState>,
    path: String,
) -> Result<RefreshPbipResult, String> {
    let summary = pbi::pbip::open_pbip_project(&PathBuf::from(path), Some(&state.db))?;
    Ok(RefreshPbipResult {
        loaded_tables: summary.loaded_tables,
        warnings: summary.warnings,
    })
}

#[tauri::command]
fn query_duckdb(state: State<'_, AppState>, sql: String) -> Result<SqlQueryResult, String> {
    state.db.query_sql(&sql)
}

#[tauri::command]
fn list_duckdb_tables(state: State<'_, AppState>) -> Result<Vec<TableSummary>, String> {
    state.db.list_tables()
}

#[tauri::command]
fn get_table_preview(
    state: State<'_, AppState>,
    table_name: String,
    limit: Option<usize>,
) -> Result<SqlQueryResult, String> {
    state.db.get_table_preview(&table_name, limit.unwrap_or(500))
}

#[tauri::command]
fn open_pbip_project(
    state: State<'_, AppState>,
    path: String,
) -> Result<PbiProjectSummary, String> {
    let summary = pbi::pbip::open_pbip_project(&PathBuf::from(path), Some(&state.db))?;
    // Sync loaded tables, relationships, and measures into native Rust DaxModel
    if !summary.loaded_tables.is_empty() {
        let sync_payload = DaxModelSyncPayload {
            tables: summary
                .loaded_tables
                .iter()
                .map(|t| dax::DaxTableInput {
                    name: t.table_name.clone(),
                    columns: t.columns.iter().map(|c| c.name.clone()).collect(),
                    rows: t.rows.clone(),
                })
                .collect(),
            relationships: summary
                .relationships
                .iter()
                .map(|r| dax::DaxRelationshipInput {
                    from_table: r.from_table.clone(),
                    from_column: r.from_column.clone(),
                    to_table: r.to_table.clone(),
                    to_column: r.to_column.clone(),
                    cross_filter_direction: r.cross_filter_direction.clone(),
                    is_active: r.is_active,
                })
                .collect(),
            measures: summary
                .tables
                .iter()
                .flat_map(|t| t.measures.iter())
                .map(|m| dax::DaxMeasureInput {
                    name: m.name.clone(),
                    table_name: m.table_name.clone(),
                    formula: m.expression.clone(),
                })
                .collect(),
        };
        if let Ok(mut guard) = state.dax_model.lock() {
            guard.sync_from_payload(sync_payload);
        }
    }
    Ok(summary)
}

#[tauri::command]
fn open_pbix_file(path: String) -> Result<PbiProjectSummary, String> {
    pbi::pbix::open_pbix_file(&PathBuf::from(path))
}

#[tauri::command]
fn save_pbip_project(target_dir: String, payload: ExportPbipPayload) -> Result<String, String> {
    pbi::pbip_writer::save_pbip_project(&PathBuf::from(target_dir), &payload)
}

#[tauri::command]
fn sync_model_to_rust(
    state: State<'_, AppState>,
    payload: DaxModelSyncPayload,
) -> Result<usize, String> {
    let count = payload.tables.len();
    let mut guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    guard.sync_from_payload(payload);
    Ok(count)
}

#[tauri::command]
fn evaluate_dax_native(
    state: State<'_, AppState>,
    expression: String,
    filters: Option<Vec<NativeFilterInput>>,
) -> Result<NativeDaxEvalResult, String> {
    let ctx = build_filter_context(&filters.unwrap_or_default());
    let guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    let val = guard.evaluate_dax(&expression, &ctx)?;
    Ok(NativeDaxEvalResult {
        display: val.as_display_string(),
        value: val.to_json(),
    })
}

#[tauri::command]
fn evaluate_dax_batch_native(
    state: State<'_, AppState>,
    items: Vec<(String, String, Vec<NativeFilterInput>)>,
) -> Result<HashMap<String, JsonValue>, String> {
    let guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    let mut out = HashMap::with_capacity(items.len());
    for (key, expr, flist) in items {
        let ctx = build_filter_context(&flist);
        let val = guard.evaluate_dax(&expr, &ctx).unwrap_or(dax::DaxValue::Blank);
        out.insert(key, val.to_json());
    }
    Ok(out)
}

#[tauri::command]
fn query_visual_native(
    state: State<'_, AppState>,
    request: dax::query_planner::NativeVisualQueryRequest,
) -> Result<dax::query_planner::NativeVisualQueryResult, String> {
    let guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    dax::query_planner::execute_visual_query(&guard, &request)
}

#[tauri::command]
fn check_for_updates(current_version: String) -> Result<updater::UpdateCheckResult, String> {
    updater::check_for_updates(&current_version)
}

#[tauri::command]
fn download_and_install_update(
    download_url: String,
    asset_name: String,
    expected_sha256: Option<String>,
) -> Result<updater::UpdateInstallResult, String> {
    updater::download_and_install_update(&download_url, &asset_name, expected_sha256.as_deref())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = DuckDbStore::new_in_memory().expect("Falha ao inicializar DuckDB nativo");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db,
            dax_model: Mutex::new(DaxModel::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_engine_info,
            import_csv_native,
            import_parquet_native,
            import_json_native,
            import_xlsx_native,
            import_xlsx_sheet_native,
            refresh_table_native,
            refresh_pbip_tables,
            query_duckdb,
            list_duckdb_tables,
            get_table_preview,
            open_pbip_project,
            open_pbix_file,
            save_pbip_project,
            sync_model_to_rust,
            evaluate_dax_native,
            evaluate_dax_batch_native,
            query_visual_native,
            check_for_updates,
            download_and_install_update,
        ])
        .run(tauri::generate_context!())
        .expect("Erro ao executar PrismBI");
}
