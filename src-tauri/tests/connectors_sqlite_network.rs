//! Teste de integração do conector SQLite real via DuckDB `ATTACH ...
//! (TYPE SQLITE)`. Requer rede na primeira execução (`INSTALL sqlite`
//! baixa de extensions.duckdb.org); roda normalmente em ambientes com
//! acesso à internet. Mantido em arquivo separado para que CI sem rede
//! possa excluí-lo facilmente (`cargo test --workspace --exclude-tests
//! connectors_sqlite_network`, ou simplesmente ignorar falhas deste alvo).

use prismbi_lib::connectors::manager::ImportSource;
use prismbi_lib::connectors::{ConnectionManager, ConnectorConfig};
use prismbi_lib::engine::DuckDbStore;

#[test]
fn sqlite_connector_full_roundtrip_via_duckdb_attach() {
    // Cria o arquivo .sqlite de origem usando o próprio DuckDB (ATTACH
    // TYPE SQLITE em modo leitura/escrita), sem depender de rusqlite.
    let dir = tempfile::tempdir().expect("tempdir");
    let sqlite_path = dir.path().join("legacy.sqlite");

    {
        let setup = duckdb::Connection::open_in_memory().expect("open setup conn");
        setup
            .execute("INSTALL sqlite", [])
            .expect("install sqlite extension (requires network)");
        setup.execute("LOAD sqlite", []).expect("load sqlite extension");
        setup
            .execute_batch(&format!(
                "ATTACH '{}' AS src (TYPE SQLITE); \
                 CREATE TABLE src.produtos (id INTEGER, nome TEXT, preco REAL); \
                 INSERT INTO src.produtos VALUES (1, 'Caneta', 2.5), (2, 'Caderno', 12.9), (3, 'Borracha', 1.2);",
                sqlite_path.to_string_lossy()
            ))
            .expect("seed sqlite file via duckdb attach");
    }

    let store = DuckDbStore::new_in_memory().expect("init target duckdb");
    let manager = ConnectionManager::new();

    let info = manager
        .connect(
            &store,
            ConnectorConfig::Sqlite {
                path: sqlite_path.to_string_lossy().to_string(),
            },
        )
        .expect("db_connect sqlite");

    let tables = manager
        .list_tables(&store, &info.connection_id, None)
        .expect("db_list_tables");
    assert!(tables.iter().any(|t| t.name == "produtos"));

    let schema = tables
        .iter()
        .find(|t| t.name == "produtos")
        .map(|t| t.schema.clone())
        .expect("schema of produtos");

    let preview = manager
        .preview_table(&store, &info.connection_id, &schema, "produtos", 10)
        .expect("db_preview_table");
    assert_eq!(preview.row_count, 3);

    let imported = manager
        .import_table(
            &store,
            &info.connection_id,
            ImportSource::Table {
                schema,
                table: "produtos".to_string(),
            },
            "produtos_sqlite",
        )
        .expect("db_import_table");
    assert_eq!(imported.total_rows, 3);
    assert_eq!(imported.table_name, "produtos_sqlite");

    manager
        .disconnect(&store, &info.connection_id)
        .expect("db_disconnect");
}
