//! Testes de integração dos conectores nativos (v0.8.0): exercitam o
//! fluxo completo `db_connect -> db_list_schemas -> db_list_tables ->
//! db_preview_table -> db_import_table -> db_disconnect` contra bancos
//! reais criados em arquivos temporários (SQLite via extensão `sqlite` do
//! DuckDB, e `.duckdb` nativo), sem depender de nenhum servidor externo.
//!
//! Os testes de SQLite requerem rede na primeira execução (`INSTALL
//! sqlite` baixa de extensions.duckdb.org); se a extensão já está em
//! cache local ou o ambiente tem rede, rodam normalmente. Em ambientes
//! sem rede e sem cache, eles falham e devem ser ignorados manualmente
//! (`cargo test -- --skip sqlite`) — não há como testar `postgres`/`mysql`
//! sem um servidor real, por isso cobrimos apenas geração/escaping de SQL
//! (ver `connectors::config::tests`) e os marcamos `#[ignore]` abaixo.

use prismbi_lib::connectors::manager::ImportSource;
use prismbi_lib::connectors::{ConnectionManager, ConnectorConfig};
use prismbi_lib::engine::DuckDbStore;

#[test]
fn duckdb_file_connector_full_roundtrip_offline() {
    // 100% offline: ATTACH de um arquivo .duckdb nativo não requer extensão.
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("source.duckdb");

    // Cria o banco .duckdb de origem com dados de teste.
    {
        let setup = duckdb::Connection::open(&db_path).expect("create source duckdb");
        setup
            .execute_batch(
                "CREATE SCHEMA analytics; \
                 CREATE TABLE analytics.vendas (id INTEGER, produto VARCHAR, valor DOUBLE); \
                 INSERT INTO analytics.vendas VALUES (1, 'Teclado', 150.0), (2, 'Mouse', 80.0);",
            )
            .expect("seed source duckdb");
    }

    let store = DuckDbStore::new_in_memory().expect("init target duckdb");
    let manager = ConnectionManager::new();

    let config = ConnectorConfig::DuckdbFile {
        path: db_path.to_string_lossy().to_string(),
    };
    let info = manager.connect(&store, config).expect("db_connect");

    let schemas = manager
        .list_schemas(&store, &info.connection_id)
        .expect("list_schemas");
    assert!(schemas.iter().any(|s| s.name == "analytics"));

    let tables = manager
        .list_tables(&store, &info.connection_id, Some("analytics"))
        .expect("list_tables");
    assert!(tables.iter().any(|t| t.name == "vendas" && t.schema == "analytics"));

    let preview = manager
        .preview_table(&store, &info.connection_id, "analytics", "vendas", 10)
        .expect("preview_table");
    assert_eq!(preview.row_count, 2);
    assert!(preview.columns.contains(&"produto".to_string()));

    let imported = manager
        .import_table(
            &store,
            &info.connection_id,
            ImportSource::Table {
                schema: "analytics".to_string(),
                table: "vendas".to_string(),
            },
            "vendas_importadas",
        )
        .expect("db_import_table");
    assert_eq!(imported.table_name, "vendas_importadas");
    assert_eq!(imported.total_rows, 2);

    // A tabela importada deve existir de forma independente no catálogo principal.
    let verify = store
        .query_sql("SELECT SUM(valor) AS total FROM vendas_importadas")
        .expect("query imported table");
    let total = verify.rows[0].get("total").and_then(|v| v.as_f64()).unwrap();
    assert!((total - 230.0).abs() < 0.001);

    manager
        .disconnect(&store, &info.connection_id)
        .expect("db_disconnect");
    assert!(manager
        .list_schemas(&store, &info.connection_id)
        .is_err());
}

#[test]
fn duckdb_file_connector_query_preview_isolated_from_main_catalog() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("source2.duckdb");
    {
        let setup = duckdb::Connection::open(&db_path).expect("create source duckdb");
        setup
            .execute_batch(
                "CREATE TABLE clientes (id INTEGER, nome VARCHAR, uf VARCHAR); \
                 INSERT INTO clientes VALUES (1, 'Ana', 'SP'), (2, 'Bruno', 'RJ'), (3, 'Carla', 'SP');",
            )
            .expect("seed");
    }

    let store = DuckDbStore::new_in_memory().expect("init target duckdb");
    let manager = ConnectionManager::new();
    let info = manager
        .connect(
            &store,
            ConnectorConfig::DuckdbFile {
                path: db_path.to_string_lossy().to_string(),
            },
        )
        .expect("connect");

    // Sem qualificar com o catálogo anexado, a consulta roda contra o
    // catálogo principal (vazio) — confirma que o ATTACH não "vaza" dados
    // da fonte externa para o namespace padrão sem qualificação explícita.
    let err = manager
        .query_preview(
            &store,
            &info.connection_id,
            "SELECT uf, COUNT(*) AS qtd FROM clientes GROUP BY uf ORDER BY uf",
            100,
        )
        .expect_err("clientes não deveria existir no catálogo principal");
    assert!(err.to_lowercase().contains("clientes") || err.to_lowercase().contains("not found") || err.to_lowercase().contains("catalog"));

    // Importar a tabela torna-a disponível no catálogo principal, e então
    // a mesma consulta (sem prefixo) passa a funcionar.
    manager
        .import_table(
            &store,
            &info.connection_id,
            ImportSource::Table {
                schema: "main".to_string(),
                table: "clientes".to_string(),
            },
            "clientes",
        )
        .expect("import_table");

    let preview = manager
        .query_preview(
            &store,
            &info.connection_id,
            "SELECT uf, COUNT(*) AS qtd FROM clientes GROUP BY uf ORDER BY uf",
            100,
        )
        .expect("query_preview after import");
    assert_eq!(preview.row_count, 2);

    manager.disconnect(&store, &info.connection_id).unwrap();
}

#[test]
#[ignore = "requer servidor PostgreSQL real acessível pela rede; cobertura de geração de SQL em connectors::config::tests"]
fn postgres_live_connection_smoke_test() {
    unreachable!("executar manualmente com um servidor Postgres de teste configurado");
}

#[test]
#[ignore = "requer servidor MySQL/MariaDB real acessível pela rede; cobertura de geração de SQL em connectors::config::tests"]
fn mysql_live_connection_smoke_test() {
    unreachable!("executar manualmente com um servidor MySQL de teste configurado");
}
