//! Conectores nativos de banco de dados via DuckDB (ATTACH) para o
//! roadmap v0.8.0: PostgreSQL, SQLite, MySQL/MariaDB, arquivos `.duckdb`
//! e Parquet particionado / HTTP(S) / S3, sem depender de gateway externo.
//!
//! Este módulo é propositalmente separado de `engine::duckdb_store` para
//! manter o arquivo principal do motor enxuto (ver ROADMAP.md "PRACTICES").

pub mod config;
pub mod keyring_store;
pub mod manager;

pub use config::{ConnectorConfig, ConnectorKind};
pub use keyring_store::{CredentialStore, InMemoryCredentialStore, LinuxKeyringStore};
pub use manager::{
    ConnectionManager, DbConnectionInfo, DbSchemaInfo, DbTableInfo, PreviewResult,
};
