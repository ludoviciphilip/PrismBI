pub mod connectors;
pub mod dax;
pub mod engine;
pub mod etl;
pub mod migrate_legacy;
pub mod pbi;
pub mod updater;

// Coverage-only test modules for v0.10 round 1 (see docs/TEST_COVERAGE.md).
// These live outside src/dax/**, src/pbi/** and updater.rs (other fronts'
// territory) and test only through the crate's pub/pub(crate) API.
#[cfg(test)]
mod tests_cov_dax;
#[cfg(test)]
mod tests_cov_pbi;
#[cfg(test)]
mod tests_cov_updater;

use connectors::{
    CredentialStore, ConnectionManager, ConnectorConfig, DbConnectionInfo, DbSchemaInfo,
    DbTableInfo, NativeKeyringStore, PreviewResult,
};
use connectors::manager::ImportSource;
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
    /// Pacote de atualização já verificado (assinatura + SHA). O frontend só
    /// pode pedir para instalar este, nunca uma URL arbitrária.
    pub pending_update: Mutex<Option<updater::PendingUpdate>>,
    /// Conexões de banco de dados ativas (ATTACH), ver `connectors::manager`.
    pub connections: ConnectionManager,
    /// Credenciais (senhas de conexão + chave de IA), nunca enviadas ao
    /// frontend; ver `connectors::keyring_store`.
    pub credentials: Box<dyn CredentialStore>,
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
                .map(|t| {
                    let sort_by_columns = summary
                        .tables
                        .iter()
                        .find(|tm| tm.name.eq_ignore_ascii_case(&t.table_name))
                        .map(|tm| {
                            tm.columns
                                .iter()
                                .filter_map(|c| c.sort_by_column.as_ref().map(|sb| (c.name.clone(), sb.clone())))
                                .collect::<HashMap<String, String>>()
                        })
                        .unwrap_or_default();
                    dax::DaxTableInput {
                        name: t.table_name.clone(),
                        columns: t.columns.iter().map(|c| c.name.clone()).collect(),
                        rows: t.rows.clone(),
                        column_types: vec![],
                        duckdb_table: Some(t.table_name.clone()),
                        sort_by_columns, is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                    }
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

    // Step 1: lock only the DB connection, load (or decide to reuse) every
    // duckdb_table-backed table, then release the lock before ever touching
    // dax_model. Never hold both locks at once, and never acquire them in
    // the reverse order anywhere else (see docs/RUST_ENGINE_2.md §3 step 2).
    let freshly_loaded: HashMap<String, (u64, dax::DaxTableData)>;
    {
        let conn_guard = state
            .db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        freshly_loaded = dax::duckdb_load::load_changed_tables(&conn_guard, &payload.tables);
        drop(conn_guard);
    }

    // Step 2: now lock dax_model (db lock is already released).
    let mut guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    guard.sync_from_payload_with_duckdb(payload, freshly_loaded);
    Ok(count)
}

#[tauri::command]
fn evaluate_dax_native(
    state: State<'_, AppState>,
    expression: String,
    filters: Option<Vec<NativeFilterInput>>,
) -> Result<NativeDaxEvalResult, String> {
    let guard = state
        .dax_model
        .lock()
        .map_err(|_| "Lock DaxModel corrompido".to_string())?;
    let ctx = build_filter_context(&filters.unwrap_or_default(), &guard);
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
        let ctx = build_filter_context(&flist, &guard);
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
fn check_for_updates(
    state: State<'_, AppState>,
    current_version: String,
) -> Result<updater::UpdateCheckResult, String> {
    let (result, pending) = updater::check_for_updates(&current_version)?;
    *state
        .pending_update
        .lock()
        .map_err(|_| "Lock do updater corrompido".to_string())? = pending;
    Ok(result)
}

#[tauri::command]
fn download_and_install_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<updater::UpdateInstallResult, String> {
    let pending = state
        .pending_update
        .lock()
        .map_err(|_| "Lock do updater corrompido".to_string())?
        .clone()
        .ok_or_else(|| {
            "Nenhuma atualização verificada pendente. Verifique atualizações novamente.".to_string()
        })?;
    let result = updater::download_and_install_update(&pending)?;
    // No Windows, o `.msi` precisa que o app saia para o instalador
    // substituir os arquivos em uso; dá tempo do frontend mostrar a
    // mensagem antes de fechar.
    if cfg!(windows) && result.installed && result.needs_restart {
        let handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(800));
            handle.exit(0);
        });
    }
    Ok(result)
}

// ---------------------------------------------------------------------
// Conectores nativos de banco de dados (v0.8.0) — ver connectors::manager
// ---------------------------------------------------------------------

#[tauri::command]
fn db_connect(
    state: State<'_, AppState>,
    config: ConnectorConfig,
    persist_credential_key: Option<String>,
) -> Result<DbConnectionInfo, String> {
    let password = config.extract_password();
    let info = state.connections.connect(&state.db, config)?;
    if let (Some(key), Some(pw)) = (persist_credential_key, password) {
        if !pw.is_empty() {
            state.credentials.set(&key, &pw)?;
        }
    }
    Ok(info)
}

#[tauri::command]
fn db_list_schemas(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<DbSchemaInfo>, String> {
    state.connections.list_schemas(&state.db, &connection_id)
}

#[tauri::command]
fn db_list_tables(
    state: State<'_, AppState>,
    connection_id: String,
    schema: Option<String>,
) -> Result<Vec<DbTableInfo>, String> {
    state
        .connections
        .list_tables(&state.db, &connection_id, schema.as_deref())
}

#[tauri::command]
fn db_preview_table(
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
    table: String,
    limit: Option<usize>,
) -> Result<PreviewResult, String> {
    state.connections.preview_table(
        &state.db,
        &connection_id,
        &schema,
        &table,
        limit.unwrap_or(500),
    )
}

#[tauri::command]
fn db_query_preview(
    state: State<'_, AppState>,
    connection_id: String,
    sql: String,
    limit: Option<usize>,
) -> Result<PreviewResult, String> {
    state
        .connections
        .query_preview(&state.db, &connection_id, &sql, limit.unwrap_or(500))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mode")]
enum DbImportSourcePayload {
    Table { schema: String, table: String },
    Sql { sql: String },
}

#[tauri::command]
fn db_import_table(
    state: State<'_, AppState>,
    connection_id: String,
    source: DbImportSourcePayload,
    target_table_name: String,
) -> Result<ImportedTablePayload, String> {
    let source = match source {
        DbImportSourcePayload::Table { schema, table } => ImportSource::Table { schema, table },
        DbImportSourcePayload::Sql { sql } => ImportSource::Sql(sql),
    };
    state
        .connections
        .import_table(&state.db, &connection_id, source, &target_table_name)
}

#[tauri::command]
fn db_disconnect(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    state.connections.disconnect(&state.db, &connection_id)
}

/// Usado pelo botão "Atualizar Dados" para tabelas de origem `db`:
/// reconecta efemeramente usando a senha guardada no keyring (identificada
/// por `credential_key`, nunca pelo `connection_id` de sessão, que não é
/// estável entre execuções do app), reimporta e desconecta em seguida.
#[tauri::command]
fn db_refresh_import(
    state: State<'_, AppState>,
    config: ConnectorConfig,
    credential_key: Option<String>,
    source: DbImportSourcePayload,
    target_table_name: String,
) -> Result<ImportedTablePayload, String> {
    let password = match &credential_key {
        Some(key) => state.credentials.get(key)?,
        None => None,
    };
    let full_config = config.with_password(password);
    let info = state.connections.connect(&state.db, full_config)?;
    let source = match source {
        DbImportSourcePayload::Table { schema, table } => ImportSource::Table { schema, table },
        DbImportSourcePayload::Sql { sql } => ImportSource::Sql(sql),
    };
    let result = state.connections.import_table(
        &state.db,
        &info.connection_id,
        source,
        &target_table_name,
    );
    let _ = state.connections.disconnect(&state.db, &info.connection_id);
    result
}

// ---------------------------------------------------------------------
// Credenciais seguras (chaveiro Linux Secret Service) — v0.8.0
// ---------------------------------------------------------------------

#[tauri::command]
fn credentials_set(state: State<'_, AppState>, key: String, secret: String) -> Result<(), String> {
    state.credentials.set(&key, &secret)
}

#[tauri::command]
fn credentials_get_exists(state: State<'_, AppState>, key: String) -> Result<bool, String> {
    state.credentials.exists(&key)
}

#[tauri::command]
fn credentials_delete(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state.credentials.delete(&key)
}

/// Usado exclusivamente pela migração da chave Gemini (`aiConfig.ts`):
/// lê o valor salvo no chaveiro para reconstituir a sessão de IA em
/// memória do frontend sem jamais persistir a chave fora do keyring.
#[tauri::command]
fn credentials_get(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    state.credentials.get(&key)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Migra dados do PrismBI (identifier antigo) para o VetroBI ANTES de
    // qualquer janela/webview ser criada — veja migrate_legacy.rs. Usa a
    // crate `dirs` diretamente (sem AppHandle) para rodar o mais cedo
    // possível em run(), sem depender da inicialização do Tauri.
    migrate_legacy::migrate_legacy_app_dirs(
        &[
            dirs::data_dir(),
            dirs::config_dir(),
            dirs::cache_dir(),
            dirs::data_local_dir(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>(),
    );

    let db = DuckDbStore::new_in_memory().expect("Falha ao inicializar DuckDB nativo");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db,
            dax_model: Mutex::new(DaxModel::new()),
            pending_update: Mutex::new(None),
            connections: ConnectionManager::new(),
            credentials: Box::new(NativeKeyringStore::new()),
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
            db_connect,
            db_list_schemas,
            db_list_tables,
            db_preview_table,
            db_query_preview,
            db_import_table,
            db_disconnect,
            db_refresh_import,
            credentials_set,
            credentials_get_exists,
            credentials_delete,
            credentials_get,
            etl::commands::etl_load_raw,
            etl::commands::etl_preview,
            etl::commands::etl_apply,
            etl::commands::etl_to_m,
            etl::commands::etl_from_m,
            etl::commands::etl_step_m,
        ])
        .run(tauri::generate_context!())
        .expect("Erro ao executar VetroBI");
}
