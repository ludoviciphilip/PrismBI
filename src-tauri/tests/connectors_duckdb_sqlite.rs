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

use vetrobi_lib::connectors::manager::ImportSource;
use vetrobi_lib::connectors::{ConnectionManager, ConnectorConfig};
use vetrobi_lib::engine::DuckDbStore;

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

    // O SQL do usuário roda com o banco conectado como catálogo padrão:
    // `clientes` sem prefixo resolve na fonte, sem precisar conhecer o alias
    // interno (conn_<uuid>), que muda a cada conexão.
    let preview = manager
        .query_preview(
            &store,
            &info.connection_id,
            "SELECT uf, COUNT(*) AS qtd FROM clientes GROUP BY uf ORDER BY uf",
            100,
        )
        .expect("SQL sem prefixo deve resolver no banco conectado");
    assert_eq!(preview.row_count, 2);

    // O catálogo padrão do app é restaurado depois da consulta.
    let current: String = store
        .connection()
        .lock()
        .unwrap()
        .query_row("SELECT current_database()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(current, "memory");

    manager.disconnect(&store, &info.connection_id).unwrap();
}

/// Regressão: tabela importada por SQL precisa continuar atualizável depois
/// de reconectar (o alias interno muda; o SQL salvo não pode depender dele).
#[test]
fn duckdb_file_sql_import_survives_reconnect_like_refresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("source3.duckdb");
    {
        let setup = duckdb::Connection::open(&db_path).expect("create source duckdb");
        setup
            .execute_batch(
                "CREATE TABLE vendas (id INTEGER, valor DOUBLE); \
                 INSERT INTO vendas VALUES (1, 10.0), (2, 32.5);",
            )
            .expect("seed");
    }
    let store = DuckDbStore::new_in_memory().expect("init target duckdb");
    let manager = ConnectionManager::new();
    let cfg = || ConnectorConfig::DuckdbFile {
        path: db_path.to_string_lossy().to_string(),
    };
    let sql = "SELECT id, valor * 2 AS dobro FROM vendas".to_string();

    for round in 0..2 {
        let info = manager.connect(&store, cfg()).expect("connect");
        let payload = manager
            .import_table(&store, &info.connection_id, ImportSource::Sql(sql.clone()), "f_Dobro")
            .unwrap_or_else(|e| panic!("rodada {}: {}", round, e));
        assert_eq!(payload.rows.len(), 2, "rodada {}", round);
        manager.disconnect(&store, &info.connection_id).unwrap();
    }

    // A tabela importada vive no catálogo principal do app.
    let n: i64 = store
        .connection()
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM memory.main.\"f_Dobro\"", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 2);
}

#[test]
#[ignore = "requer servidor PostgreSQL real acessível pela rede; cobertura de geração de SQL em connectors::config::tests"]
fn postgres_live_connection_smoke_test() {
    live_server_roundtrip("VETROBI_TEST_PG", |host, port, user, password, database| {
        ConnectorConfig::Postgres {
            host,
            port,
            database,
            user,
            password: Some(password),
            schema: None,
            ssl_mode: Some("disable".into()),
        }
    }, "vendas");
}

#[test]
#[ignore = "requer servidor MySQL/MariaDB real acessível pela rede; cobertura de geração de SQL em connectors::config::tests"]
fn mysql_live_connection_smoke_test() {
    live_server_roundtrip("VETROBI_TEST_MYSQL", |host, port, user, password, database| {
        ConnectorConfig::Mysql { host, port, database: database.clone(), user, password: Some(password) }
    }, "loja");
}

/// Teste de ponta a ponta contra um servidor real. Variáveis:
/// `<PREFIX>_HOST`, `_PORT`, `_USER`, `_PASSWORD`, `_DB`. A tabela
/// `<schema>.pedidos(id, cliente, valor)` com 3 linhas (Ana 10.50, Bruno 20,
/// Ana 5.25) precisa existir. O CI sobe os serviços (ver ci.yml); localmente:
/// `podman run -e POSTGRES_PASSWORD=... -p 55432:5432 postgres:16-alpine`.
fn live_server_roundtrip(
    prefix: &str,
    make: impl Fn(String, u16, String, String, String) -> ConnectorConfig,
    schema: &str,
) {
    let var = |k: &str| std::env::var(format!("{}_{}", prefix, k)).ok();
    let (Some(host), Some(port), Some(user), Some(password), Some(db)) =
        (var("HOST"), var("PORT"), var("USER"), var("PASSWORD"), var("DB"))
    else {
        panic!("defina {}_HOST/_PORT/_USER/_PASSWORD/_DB para rodar este teste", prefix);
    };
    let store = DuckDbStore::new_in_memory().expect("duckdb");
    let manager = ConnectionManager::new();
    let info = manager
        .connect(&store, make(host, port.parse().unwrap(), user, password, db))
        .expect("connect");

    let tables = manager.list_tables(&store, &info.connection_id, Some(schema)).expect("list_tables");
    assert!(tables.iter().any(|t| t.name == "pedidos"), "pedidos não listada: {:?}", tables.iter().map(|t| &t.name).collect::<Vec<_>>());

    let preview = manager
        .preview_table(&store, &info.connection_id, schema, "pedidos", 10)
        .expect("preview_table");
    assert_eq!(preview.row_count, 3);

    // SQL com schema explícito, rodando no banco conectado.
    let sql = format!("SELECT cliente, SUM(valor) AS total FROM {}.pedidos GROUP BY cliente ORDER BY cliente", schema);
    let q = manager.query_preview(&store, &info.connection_id, &sql, 10).expect("query_preview");
    assert_eq!(q.row_count, 2);

    let imported = manager
        .import_table(&store, &info.connection_id, ImportSource::Sql(sql), "f_TotalCliente")
        .expect("import sql");
    assert_eq!(imported.rows.len(), 2);
    manager.disconnect(&store, &info.connection_id).unwrap();
}
