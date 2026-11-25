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
                if let Some(pw) = password {
                    if !pw.is_empty() {
                        pairs.push(("password", pw.clone()));
                    }
                }
                if let Some(mode) = ssl_mode {
                    if !mode.is_empty() {
                        pairs.push(("sslmode", mode.clone()));
                    }
                }
                let conn_str = build_keyvalue_conn_string(&pairs);
                Ok(Some(format!(
                    "ATTACH {} AS {} (TYPE POSTGRES, READ_ONLY)",
                    quote_sql_literal(&conn_str),
                    quoted_alias
                )))
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
                let mut pairs: Vec<(&str, String)> = vec![
                    ("host", host.clone()),
                    ("port", port.to_string()),
                    ("database", database.clone()),
                    ("user", user.clone()),
                ];
                if let Some(pw) = password {
                    if !pw.is_empty() {
                        pairs.push(("password", pw.clone()));
                    }
                }
                let conn_str = build_keyvalue_conn_string(&pairs);
                Ok(Some(format!(
                    "ATTACH {} AS {} (TYPE MYSQL, READ_ONLY)",
                    quote_sql_literal(&conn_str),
                    quoted_alias
                )))
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
        assert!(sql.starts_with("ATTACH '"));
        assert!(sql.contains("host=db.internal"));
        assert!(sql.contains("port=5432"));
        assert!(sql.contains("dbname=sales"));
        assert!(sql.contains("user=reader"));
        assert!(sql.contains("password=s3cret"));
        assert!(sql.contains("sslmode=require"));
        assert!(sql.ends_with("AS \"pg_1\" (TYPE POSTGRES, READ_ONLY)"));
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
        // password must be single-quoted (libpq style) with internal quote/
        // backslash escaped, then the whole ATTACH literal re-escapes single
        // quotes again (standard SQL '' doubling) since it is itself a SQL
        // string literal.
        assert!(sql.contains("password=''p@ss w\\''rd\\\\x'''"));
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
        assert!(sql.contains("host=mysql.internal"));
        assert!(sql.contains("database=shop"));
        assert!(sql.contains("password=hunter2"));
        assert!(sql.ends_with("AS \"my_1\" (TYPE MYSQL, READ_ONLY)"));
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
}
