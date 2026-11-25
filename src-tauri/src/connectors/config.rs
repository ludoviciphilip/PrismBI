//! Configuração de conectores e geração segura de SQL (`ATTACH` / strings
//! de conexão). Nenhuma entrada do usuário é interpolada sem escaping:
//! identificadores passam por `quote_ident` (aspas duplas, `"` -> `""`) e
//! valores de string por `quote_sql_literal` (aspas simples, `'` -> `''`).

use crate::engine::duckdb_store::quote_ident;
use serde::{Deserialize, Serialize};

/// Configuração completa de um conector, incluindo a senha (quando houver).
/// A senha NUNCA deve ser serializada de volta ao frontend nem persistida
/// em `.insight`/`.pbip`/localStorage — ela só circula em memória do
/// backend Rust (vinda do keyring ou de um formulário de conexão) durante
/// a chamada de `ATTACH`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectorConfig {
    Sqlite {
        path: String,
    },
    DuckdbFile {
        path: String,
    },
    Postgres {
        host: String,
        port: u16,
        database: String,
        user: String,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        schema: Option<String>,
        #[serde(default)]
        ssl_mode: Option<String>,
    },
    Mysql {
        host: String,
        port: u16,
        database: String,
        user: String,
        #[serde(default)]
        password: Option<String>,
    },
    /// Parquet particionado (glob local), HTTP(S) ou S3, lido via
    /// `read_parquet` com `hive_partitioning` opcional (sem ATTACH).
    ParquetGlob {
        url: String,
        #[serde(default)]
        hive_partitioning: bool,
    },
}

/// Identificador textual estável do tipo de conector (usado em
/// `TableSourceConfig.connectorType` no frontend e nos testes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    Sqlite,
    DuckdbFile,
    Postgres,
    Mysql,
    ParquetGlob,
}

impl ConnectorConfig {
    pub fn kind(&self) -> ConnectorKind {
        match self {
            ConnectorConfig::Sqlite { .. } => ConnectorKind::Sqlite,
            ConnectorConfig::DuckdbFile { .. } => ConnectorKind::DuckdbFile,
            ConnectorConfig::Postgres { .. } => ConnectorKind::Postgres,
            ConnectorConfig::Mysql { .. } => ConnectorKind::Mysql,
            ConnectorConfig::ParquetGlob { .. } => ConnectorKind::ParquetGlob,
        }
    }

    /// Nome da extensão DuckDB necessária para este conector (`None` quando
    /// nenhuma extensão externa é necessária, ex.: DuckDB nativo).
    pub fn required_extension(&self) -> Option<&'static str> {
        match self {
            ConnectorConfig::Sqlite { .. } => Some("sqlite"),
            ConnectorConfig::DuckdbFile { .. } => None,
            ConnectorConfig::Postgres { .. } => Some("postgres"),
            ConnectorConfig::Mysql { .. } => Some("mysql"),
            ConnectorConfig::ParquetGlob { .. } => Some("httpfs"),
        }
    }

    /// Retorna uma cópia sem a senha, segura para devolver ao frontend ou
    /// persistir como `TableSourceConfig` (nunca contém credenciais).
    pub fn redacted(&self) -> ConnectorConfig {
        match self.clone() {
            ConnectorConfig::Postgres {
                host,
                port,
                database,
                user,
                schema,
                ssl_mode,
                ..
            } => ConnectorConfig::Postgres {
                host,
                port,
                database,
                user,
                password: None,
                schema,
                ssl_mode,
            },
            ConnectorConfig::Mysql {
                host,
                port,
                database,
                user,
                ..
            } => ConnectorConfig::Mysql {
                host,
                port,
                database,
                user,
                password: None,
            },
            other => other,
        }
    }

    /// Extrai a senha em texto puro deste config (se houver), para ser
    /// persistida no keyring pelo backend logo após um `db_connect` bem
    /// sucedido. Nunca deve ser reenviada ao frontend.
    pub fn extract_password(&self) -> Option<String> {
        match self {
            ConnectorConfig::Postgres { password, .. } => password.clone(),
            ConnectorConfig::Mysql { password, .. } => password.clone(),
            _ => None,
        }
    }

    /// Retorna uma cópia com a senha injetada, usada ao reconectar durante
    /// "Atualizar Dados" com a credencial recuperada do keyring (o
    /// frontend nunca vê nem guarda essa senha).
    pub fn with_password(&self, password: Option<String>) -> ConnectorConfig {
        match self.clone() {
            ConnectorConfig::Postgres {
                host,
                port,
                database,
                user,
                schema,
                ssl_mode,
                ..
            } => ConnectorConfig::Postgres {
                host,
                port,
                database,
                user,
                password,
                schema,
                ssl_mode,
            },
            ConnectorConfig::Mysql {
                host,
                port,
                database,
                user,
                ..
            } => ConnectorConfig::Mysql {
                host,
                port,
                database,
                user,
                password,
            },
            other => other,
        }
    }

    /// Monta a instrução SQL `ATTACH ... (TYPE ..., READ_ONLY)` para este
    /// conector, usando `alias` (já sanitizado/gerado internamente, nunca
    /// vindo diretamente do usuário) como nome do catálogo anexado.
    ///
    /// Para `ParquetGlob` não há ATTACH: retorna `None` (o chamador deve
    /// usar `read_parquet` diretamente — ver `build_parquet_select`).
    pub fn build_attach_sql(&self, alias: &str) -> Result<Option<String>, String> {
        let quoted_alias = quote_ident(alias);
        match self {
            ConnectorConfig::Sqlite { path } => Ok(Some(format!(
                "ATTACH {} AS {} (TYPE SQLITE, READ_ONLY)",
                quote_sql_literal(path),
                quoted_alias
            ))),
            ConnectorConfig::DuckdbFile { path } => Ok(Some(format!(
                "ATTACH {} AS {} (READ_ONLY)",
                quote_sql_literal(path),
                quoted_alias
            ))),
            ConnectorConfig::Postgres {
                host,
                port,
                database,
                user,
                password,
                schema: _,
                ssl_mode,
            } => {
                if host.trim().is_empty() || database.trim().is_empty() || user.trim().is_empty()
                {
                    return Err("Host, banco de dados e usuário são obrigatórios para PostgreSQL".to_string());
                }
                let mut pairs: Vec<(&str, String)> = vec![
                    ("host", host.clone()),
                    ("port", port.to_string()),
                    ("dbname", database.clone()),
                    ("user", user.clone()),
                ];
                if let Some(mode) = ssl_mode {
                    if !mode.is_empty() {
                        pairs.push(("sslmode", mode.clone()));
                    }
                }
                Ok(Some(attach_with_secret("POSTGRES", alias, &pairs, password.as_deref())))
            }
            ConnectorConfig::Mysql {
                host,
                port,
                database,
                user,
                password,
            } => {
                if host.trim().is_empty() || database.trim().is_empty() || user.trim().is_empty()
                {
                    return Err("Host, banco de dados e usuário são obrigatórios para MySQL".to_string());
                }
                let pairs: Vec<(&str, String)> = vec![
                    ("host", host.clone()),
                    ("port", port.to_string()),
                    ("database", database.clone()),
                    ("user", user.clone()),
                ];
                Ok(Some(attach_with_secret("MYSQL", alias, &pairs, password.as_deref())))
            }
            ConnectorConfig::ParquetGlob { .. } => Ok(None),
        }
    }

    /// Para `ParquetGlob`: monta `SELECT * FROM read_parquet(...)`. Suporta
    /// URLs locais (glob), `http(s)://` e `s3://` (requer extensão httpfs,
    /// já declarada em `required_extension`).
    pub fn build_parquet_select(&self, limit: Option<usize>) -> Result<String, String> {
        match self {
            ConnectorConfig::ParquetGlob { url, hive_partitioning } => {
                if url.trim().is_empty() {
                    return Err("URL/caminho do Parquet não pode ser vazio".to_string());
                }
                let mut sql = format!(
                    "SELECT * FROM read_parquet({}",
                    quote_sql_literal(url)
                );
                if *hive_partitioning {
                    sql.push_str(", hive_partitioning=true");
                }
                sql.push(')');
                if let Some(n) = limit {
                    sql.push_str(&format!(" LIMIT {}", n));
                }
                Ok(sql)
            }
            _ => Err("build_parquet_select só se aplica a ConnectorConfig::ParquetGlob".to_string()),
        }
    }
}

/// Nome do SECRET temporário associado a um alias de conexão.
pub fn secret_name_for(alias: &str) -> String {
    format!("{}_secret", alias)
}

/// `ATTACH` de Postgres/MySQL. A senha vai num `TEMPORARY SECRET` (vive só na
/// memória do processo) em vez da connection string: o driver MySQL não
/// entende o escaping libpq, e a connection string aparece em
/// `duckdb_databases()` e em mensagens de erro (que só mascaravam parte dela).
fn attach_with_secret(
    db_type: &str,
    alias: &str,
    pairs: &[(&str, String)],
    password: Option<&str>,
) -> String {
    let conn_str = build_keyvalue_conn_string(pairs);
    let quoted_alias = quote_ident(alias);
    match password.filter(|p| !p.is_empty()) {
        Some(pw) => {
            let secret = quote_ident(&secret_name_for(alias));
            format!(
                "CREATE OR REPLACE TEMPORARY SECRET {secret} (TYPE {db_type}, PASSWORD {pw}); \
                 ATTACH {conn} AS {quoted_alias} (TYPE {db_type}, SECRET {secret}, READ_ONLY)",
                pw = quote_sql_literal(pw),
                conn = quote_sql_literal(&conn_str),
            )
        }
        None => format!(
            "ATTACH {} AS {} (TYPE {}, READ_ONLY)",
            quote_sql_literal(&conn_str),
            quoted_alias,
            db_type
        ),
    }
}

/// Escapa uma string para uso como literal SQL (`'...'`), dobrando aspas
/// simples internas. Usado para caminhos de arquivo e connection strings
/// que vão dentro de `ATTACH '...'`.
pub fn quote_sql_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Monta uma connection string no formato `{key}={value}` (estilo libpq,
/// usado tanto pelo `postgres_scanner` quanto pelo `mysql_scanner` do
/// DuckDB). Valores contendo espaço, aspas simples ou barra invertida são
/// envolvidos em aspas simples com escaping `\'`/`\\`, seguindo a convenção
/// libpq — nunca concatenados cruamente.
pub fn build_keyvalue_conn_string(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, libpq_quote_value(v)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn libpq_quote_value(v: &str) -> String {
    let needs_quoting = v.is_empty()
        || v.chars()
            .any(|c| c.is_whitespace() || c == '\'' || c == '\\');
    if !needs_quoting {
        return v.to_string();
    }
    let mut out = String::with_capacity(v.len() + 2);
    out.push('\'');
    for ch in v.chars() {
        if ch == '\'' || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_attach_sql_quotes_path() {
        let cfg = ConnectorConfig::Sqlite {
            path: "/tmp/my db's file.sqlite".to_string(),
        };
        let sql = cfg.build_attach_sql("conn_1").unwrap().unwrap();
        assert_eq!(
            sql,
            "ATTACH '/tmp/my db''s file.sqlite' AS \"conn_1\" (TYPE SQLITE, READ_ONLY)"
        );
    }

    #[test]
    fn duckdb_file_attach_sql() {
        let cfg = ConnectorConfig::DuckdbFile {
            path: "/data/warehouse.duckdb".to_string(),
        };
        let sql = cfg.build_attach_sql("wh").unwrap().unwrap();
        assert_eq!(sql, "ATTACH '/data/warehouse.duckdb' AS \"wh\" (READ_ONLY)");
    }

    #[test]
    fn postgres_attach_sql_builds_keyvalue_conn_string() {
        let cfg = ConnectorConfig::Postgres {
            host: "db.internal".to_string(),
            port: 5432,
            database: "sales".to_string(),
            user: "reader".to_string(),
            password: Some("s3cret".to_string()),
            schema: Some("public".to_string()),
            ssl_mode: Some("require".to_string()),
        };
        let sql = cfg.build_attach_sql("pg_1").unwrap().unwrap();
        // Senha vai num SECRET temporário (só em memória), nunca na
        // connection string — que aparece em duckdb_databases() e em
        // mensagens de erro do driver.
        assert!(sql.starts_with(
            "CREATE OR REPLACE TEMPORARY SECRET \"pg_1_secret\" (TYPE POSTGRES, PASSWORD 's3cret');"
        ));
        let attach = sql.split_once(';').unwrap().1.trim();
        assert!(attach.starts_with("ATTACH '"));
        assert!(attach.contains("host=db.internal"));
        assert!(attach.contains("port=5432"));
        assert!(attach.contains("dbname=sales"));
        assert!(attach.contains("user=reader"));
        assert!(attach.contains("sslmode=require"));
        assert!(!attach.contains("s3cret"));
        assert!(attach.ends_with("AS \"pg_1\" (TYPE POSTGRES, SECRET \"pg_1_secret\", READ_ONLY)"));
    }

    #[test]
    fn postgres_attach_sql_quotes_password_with_special_chars() {
        let cfg = ConnectorConfig::Postgres {
            host: "localhost".to_string(),
            port: 5432,
            database: "db".to_string(),
            user: "u".to_string(),
            password: Some("p@ss w'rd\\x".to_string()),
            schema: None,
            ssl_mode: None,
        };
        let sql = cfg.build_attach_sql("pg_2").unwrap().unwrap();
        // No SECRET a senha é um literal SQL comum: só '' dobrado.
        assert!(sql.contains("PASSWORD 'p@ss w''rd\\x'"));
        assert!(!sql.split_once(';').unwrap().1.contains("p@ss"));
    }

    #[test]
    fn postgres_rejects_missing_required_fields() {
        let cfg = ConnectorConfig::Postgres {
            host: "".to_string(),
            port: 5432,
            database: "db".to_string(),
            user: "u".to_string(),
            password: None,
            schema: None,
            ssl_mode: None,
        };
        assert!(cfg.build_attach_sql("x").is_err());
    }

    #[test]
    fn mysql_attach_sql_builds_keyvalue_conn_string() {
        let cfg = ConnectorConfig::Mysql {
            host: "mysql.internal".to_string(),
            port: 3306,
            database: "shop".to_string(),
            user: "admin".to_string(),
            password: Some("hunter2".to_string()),
        };
        let sql = cfg.build_attach_sql("my_1").unwrap().unwrap();
        assert!(sql.starts_with(
            "CREATE OR REPLACE TEMPORARY SECRET \"my_1_secret\" (TYPE MYSQL, PASSWORD 'hunter2');"
        ));
        let attach = sql.split_once(';').unwrap().1.trim();
        assert!(attach.contains("host=mysql.internal"));
        assert!(attach.contains("database=shop"));
        assert!(!attach.contains("hunter2"));
        assert!(attach.ends_with("AS \"my_1\" (TYPE MYSQL, SECRET \"my_1_secret\", READ_ONLY)"));
    }

    #[test]
    fn attach_without_password_has_no_secret() {
        let cfg = ConnectorConfig::Mysql {
            host: "h".into(),
            port: 3306,
            database: "d".into(),
            user: "u".into(),
            password: None,
        };
        let sql = cfg.build_attach_sql("my_2").unwrap().unwrap();
        assert!(sql.starts_with("ATTACH '"));
        assert!(!sql.contains("SECRET"));
    }

    #[test]
    fn redacted_strips_password_but_keeps_other_fields() {
        let cfg = ConnectorConfig::Postgres {
            host: "h".to_string(),
            port: 1,
            database: "d".to_string(),
            user: "u".to_string(),
            password: Some("secret".to_string()),
            schema: Some("public".to_string()),
            ssl_mode: None,
        };
        let redacted = cfg.redacted();
        match &redacted {
            ConnectorConfig::Postgres { password, host, .. } => {
                assert!(password.is_none());
                assert_eq!(host, "h");
            }
            _ => panic!("unexpected variant"),
        }
        // Serializing the redacted config must never contain the password.
        let json = serde_json::to_string(&redacted).unwrap();
        assert!(!json.contains("secret"));
    }

    #[test]
    fn parquet_glob_select_with_hive_partitioning_and_limit() {
        let cfg = ConnectorConfig::ParquetGlob {
            url: "s3://bucket/data/*/*.parquet".to_string(),
            hive_partitioning: true,
        };
        let sql = cfg.build_parquet_select(Some(100)).unwrap();
        assert_eq!(
            sql,
            "SELECT * FROM read_parquet('s3://bucket/data/*/*.parquet', hive_partitioning=true) LIMIT 100"
        );
    }

    #[test]
    fn parquet_glob_rejects_empty_url() {
        let cfg = ConnectorConfig::ParquetGlob {
            url: "".to_string(),
            hive_partitioning: false,
        };
        assert!(cfg.build_parquet_select(None).is_err());
    }

    #[test]
    fn required_extension_mapping() {
        assert_eq!(
            ConnectorConfig::Sqlite { path: "x".into() }.required_extension(),
            Some("sqlite")
        );
        assert_eq!(
            ConnectorConfig::DuckdbFile { path: "x".into() }.required_extension(),
            None
        );
        assert_eq!(
            ConnectorConfig::ParquetGlob {
                url: "x".into(),
                hive_partitioning: false
            }
            .required_extension(),
            Some("httpfs")
        );
    }

    #[test]
    fn extract_and_with_password_roundtrip() {
        let cfg = ConnectorConfig::Postgres {
            host: "h".to_string(),
            port: 5432,
            database: "d".to_string(),
            user: "u".to_string(),
            password: Some("real-secret".to_string()),
            schema: None,
            ssl_mode: None,
        };
        assert_eq!(cfg.extract_password().as_deref(), Some("real-secret"));
        let redacted = cfg.redacted();
        assert_eq!(redacted.extract_password(), None);
        let restored = redacted.with_password(Some("from-keyring".to_string()));
        assert_eq!(restored.extract_password().as_deref(), Some("from-keyring"));
    }

    #[test]
    fn sqlite_has_no_extractable_password() {
        let cfg = ConnectorConfig::Sqlite { path: "x".into() };
        assert_eq!(cfg.extract_password(), None);
    }
}
