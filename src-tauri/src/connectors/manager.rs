//! Gerenciador de conexões ativas (ATTACH) sobre a conexão DuckDB
//! compartilhada. Cada `connect()` gera um `connection_id` opaco (UUID)
//! mapeado para um alias de catálogo interno; nomes de schema/tabela
//! usados nas consultas são sempre validados contra o catálogo real do
//! DuckDB (`information_schema`) antes de compor SQL, e identificadores
//! são sempre citados com `quote_ident`.

use std::collections::HashMap;
use std::sync::Mutex;

use duckdb::Connection;
use serde::{Deserialize, Serialize};

use crate::engine::duckdb_store::{execute_select_query, quote_ident, sanitize_identifier};
use crate::engine::{DuckDbStore, SqlQueryResult};

use super::config::{ConnectorConfig, ConnectorKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbConnectionInfo {
    pub connection_id: String,
    pub kind: ConnectorKind,
    /// Configuração sem senha, segura para devolver ao frontend e salvar
    /// como `TableSourceConfig.dbConfig`.
    pub config: ConnectorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbSchemaInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTableInfo {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Map<String, serde_json::Value>>,
    pub row_count: usize,
}

impl From<SqlQueryResult> for PreviewResult {
    fn from(r: SqlQueryResult) -> Self {
        Self {
            columns: r.columns,
            rows: r.rows,
            row_count: r.row_count,
        }
    }
}

struct ActiveConnection {
    alias: String,
    kind: ConnectorKind,
    #[allow(dead_code)] // exposed for future "reconnect to show saved config" UI
    redacted_config: ConnectorConfig,
    /// Parquet "virtual" connections têm `alias == connection_id` mas não
    /// fazem ATTACH; a config completa (com URL) é reaplicada nas queries.
    full_config_for_parquet: Option<ConnectorConfig>,
}

#[derive(Default)]
pub struct ConnectionManager {
    connections: Mutex<HashMap<String, ActiveConnection>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub fn connect(
        &self,
        db: &DuckDbStore,
        config: ConnectorConfig,
    ) -> Result<DbConnectionInfo, String> {
        let connection_id = uuid::Uuid::new_v4().to_string();
        let alias = sanitize_identifier(&format!("conn_{}", connection_id.replace('-', "")));

        let conn_guard = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;

        if let Some(ext) = config.required_extension() {
            install_and_load_extension(&conn_guard, ext)?;
        }

        let is_parquet = matches!(config, ConnectorConfig::ParquetGlob { .. });
        if let Some(attach_sql) = config.build_attach_sql(&alias)? {
            conn_guard
                .execute(&attach_sql, [])
                .map_err(|e| format!("Falha ao conectar: {}", e))?;
        }
        drop(conn_guard);

        let redacted = config.redacted();
        let kind = config.kind();
        let mut map = self
            .connections
            .lock()
            .map_err(|_| "Lock de conexões corrompido".to_string())?;
        map.insert(
            connection_id.clone(),
            ActiveConnection {
                alias: alias.clone(),
                kind,
                redacted_config: redacted.clone(),
                full_config_for_parquet: if is_parquet { Some(config) } else { None },
            },
        );

        Ok(DbConnectionInfo {
            connection_id,
            kind,
            config: redacted,
        })
    }

    fn get_alias(&self, connection_id: &str) -> Result<(String, ConnectorKind), String> {
        let map = self
            .connections
            .lock()
            .map_err(|_| "Lock de conexões corrompido".to_string())?;
        let entry = map
            .get(connection_id)
            .ok_or_else(|| format!("Conexão '{}' não encontrada ou já desconectada", connection_id))?;
        Ok((entry.alias.clone(), entry.kind))
    }

    fn get_parquet_config(&self, connection_id: &str) -> Result<ConnectorConfig, String> {
        let map = self
            .connections
            .lock()
            .map_err(|_| "Lock de conexões corrompido".to_string())?;
        let entry = map
            .get(connection_id)
            .ok_or_else(|| format!("Conexão '{}' não encontrada", connection_id))?;
        entry
            .full_config_for_parquet
            .clone()
            .ok_or_else(|| "Conexão não é do tipo Parquet".to_string())
    }

    pub fn list_schemas(
        &self,
        db: &DuckDbStore,
        connection_id: &str,
    ) -> Result<Vec<DbSchemaInfo>, String> {
        let (alias, kind) = self.get_alias(connection_id)?;
        if kind == ConnectorKind::ParquetGlob {
            return Ok(vec![]);
        }
        let conn = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT schema_name FROM information_schema.schemata \
                 WHERE catalog_name = ? ORDER BY schema_name",
            )
            .map_err(|e| format!("Erro ao listar schemas: {}", e))?;
        let names: Vec<String> = stmt
            .query_map([&alias], |row| row.get(0))
            .map_err(|e| format!("Erro ao ler schemas: {}", e))?
            .filter_map(Result::ok)
            .collect();
        Ok(names.into_iter().map(|name| DbSchemaInfo { name }).collect())
    }

    pub fn list_tables(
        &self,
        db: &DuckDbStore,
        connection_id: &str,
        schema: Option<&str>,
    ) -> Result<Vec<DbTableInfo>, String> {
        let (alias, kind) = self.get_alias(connection_id)?;
        if kind == ConnectorKind::ParquetGlob {
            return Ok(vec![]);
        }
        let conn = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let sql = "SELECT table_schema, table_name FROM information_schema.tables \
                    WHERE table_catalog = ? AND (?2 IS NULL OR table_schema = ?2) \
                    ORDER BY table_schema, table_name";
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("Erro ao listar tabelas: {}", e))?;
        let rows: Vec<(String, String)> = stmt
            .query_map(duckdb::params![&alias, schema], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| format!("Erro ao ler tabelas: {}", e))?
            .filter_map(Result::ok)
            .collect();
        Ok(rows
            .into_iter()
            .map(|(schema, name)| DbTableInfo { schema, name })
            .collect())
    }

    /// Pré-visualiza uma tabela do catálogo anexado. `schema` e `table`
    /// são validados contra `list_tables` antes de montar o SQL, evitando
    /// que valores arbitrários entrem na consulta sem checagem — mesmo
    /// citados, só prosseguimos se o par (schema, table) existir de fato.
    pub fn preview_table(
        &self,
        db: &DuckDbStore,
        connection_id: &str,
        schema: &str,
        table: &str,
        limit: usize,
    ) -> Result<PreviewResult, String> {
        let (alias, kind) = self.get_alias(connection_id)?;
        if kind == ConnectorKind::ParquetGlob {
            return Err("preview_table não se aplica a conexões Parquet; use query_preview".to_string());
        }
        let known_tables = self.list_tables(db, connection_id, Some(schema))?;
        if !known_tables.iter().any(|t| t.schema == schema && t.name == table) {
            return Err(format!(
                "Tabela '{}.{}' não encontrada no catálogo da conexão",
                schema, table
            ));
        }
        let sql = format!(
            "SELECT * FROM {}.{}.{} LIMIT {}",
            quote_ident(&alias),
            quote_ident(schema),
            quote_ident(table),
            limit
        );
        let conn = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        execute_select_query(&conn, &sql).map(Into::into)
    }

    /// Executa SQL arbitrário fornecido deliberadamente pelo usuário no
    /// editor SQL (não dados externos não confiáveis), limitado a leitura
    /// porque as conexões externas são sempre `ATTACH ... READ_ONLY`.
    pub fn query_preview(
        &self,
        db: &DuckDbStore,
        connection_id: &str,
        sql: &str,
        limit: usize,
    ) -> Result<PreviewResult, String> {
        let (_, kind) = self.get_alias(connection_id)?;
        let conn = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let wrapped = if kind == ConnectorKind::ParquetGlob {
            let cfg = self.get_parquet_config(connection_id)?;
            cfg.build_parquet_select(Some(limit))?
        } else {
            format!("SELECT * FROM ({}) AS _prismbi_preview LIMIT {}", sql, limit)
        };
        execute_select_query(&conn, &wrapped).map(Into::into)
    }

    /// Importa uma tabela ou consulta SQL do catálogo anexado para uma
    /// tabela nativa do modelo (`CREATE OR REPLACE TABLE ... AS SELECT`),
    /// no mesmo padrão dos importadores de CSV/XLSX existentes.
    pub fn import_table(
        &self,
        db: &DuckDbStore,
        connection_id: &str,
        source: ImportSource,
        target_table_name: &str,
    ) -> Result<crate::engine::ImportedTablePayload, String> {
        let (alias, kind) = self.get_alias(connection_id)?;
        let safe_name = sanitize_identifier(target_table_name);

        let select_sql = match (&source, kind) {
            (ImportSource::Table { schema, table }, ConnectorKind::ParquetGlob) => {
                let _ = (schema, table);
                return Err("Conexões Parquet não possuem tabelas; use Sql".to_string());
            }
            (ImportSource::Table { schema, table }, _) => {
                let known = self.list_tables(db, connection_id, Some(schema))?;
                if !known.iter().any(|t| &t.schema == schema && &t.name == table) {
                    return Err(format!("Tabela '{}.{}' não encontrada", schema, table));
                }
                format!(
                    "SELECT * FROM {}.{}.{}",
                    quote_ident(&alias),
                    quote_ident(schema),
                    quote_ident(table)
                )
            }
            (ImportSource::Sql(user_sql), ConnectorKind::ParquetGlob) => {
                let _ = user_sql;
                let cfg = self.get_parquet_config(connection_id)?;
                cfg.build_parquet_select(None)?
            }
            (ImportSource::Sql(user_sql), _) => user_sql.clone(),
        };

        let conn = db
            .connection()
            .lock()
            .map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let create_sql = format!(
            "CREATE OR REPLACE TABLE {} AS {}",
            quote_ident(&safe_name),
            select_sql
        );
        conn.execute(&create_sql, [])
            .map_err(|e| format!("Erro ao importar dados: {}", e))?;

        crate::engine::duckdb_store::build_table_payload_pub(&conn, &safe_name, target_table_name)
    }

    pub fn disconnect(&self, db: &DuckDbStore, connection_id: &str) -> Result<(), String> {
        let (alias, kind) = self.get_alias(connection_id)?;
        if kind != ConnectorKind::ParquetGlob {
            let conn = db
                .connection()
                .lock()
                .map_err(|_| "Lock DuckDB corrompido".to_string())?;
            let _ = conn.execute(&format!("DETACH {}", quote_ident(&alias)), []);
        }
        let mut map = self
            .connections
            .lock()
            .map_err(|_| "Lock de conexões corrompido".to_string())?;
        map.remove(connection_id);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ImportSource {
    Table { schema: String, table: String },
    Sql(String),
}

fn install_and_load_extension(conn: &Connection, extension: &str) -> Result<(), String> {
    // INSTALL faz download de extensions.duckdb.org na primeira vez e é
    // idempotente; requer rede. LOAD é necessário mesmo se já instalada.
    conn.execute(&format!("INSTALL {}", extension), [])
        .map_err(|e| {
            format!(
                "Falha ao instalar extensão DuckDB '{}' (requer acesso à rede \
                 extensions.duckdb.org na primeira execução): {}",
                extension, e
            )
        })?;
    conn.execute(&format!("LOAD {}", extension), [])
        .map_err(|e| format!("Falha ao carregar extensão DuckDB '{}': {}", extension, e))?;
    Ok(())
}
