use calamine::{open_workbook_auto, Data, Reader};
use duckdb::types::ValueRef;
use duckdb::{params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub engine: String,
    pub version: String,
    pub table_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSchema {
    pub name: String,
    pub original_name: String,
    pub data_type: String,
    pub duckdb_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedTablePayload {
    pub table_name: String,
    pub original_name: String,
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Map<String, JsonValue>>,
    pub total_rows: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Map<String, JsonValue>>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSummary {
    pub table_name: String,
    pub row_count: usize,
    pub columns: Vec<ColumnSchema>,
}

const PREVIEW_ROW_LIMIT: usize = 5000;

pub struct DuckDbStore {
    conn: Mutex<Connection>,
}

impl DuckDbStore {
    pub fn new_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("Falha ao abrir DuckDB em memória: {}", e))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get_engine_info(&self) -> Result<EngineInfo, String> {
        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let version: String = conn
            .query_row("SELECT version()", [], |row| row.get(0))
            .unwrap_or_else(|_| "unknown".to_string());
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'main'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(EngineInfo {
            engine: "duckdb-native".to_string(),
            version,
            table_count: table_count.max(0) as usize,
        })
    }

    pub fn import_csv(
        &self,
        path: &Path,
        table_name: Option<&str>,
    ) -> Result<ImportedTablePayload, String> {
        if !path.exists() {
            return Err(format!("Arquivo não encontrado: {}", path.display()));
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("tabela_csv");
        let raw_name = table_name.unwrap_or(stem);
        let safe_name = sanitize_identifier(raw_name);
        let path_str = path.to_string_lossy().to_string();

        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let sql = format!(
            "CREATE OR REPLACE TABLE {} AS SELECT * FROM read_csv_auto(?, normalize_names=false)",
            quote_ident(&safe_name)
        );
        conn.execute(&sql, [&path_str])
            .map_err(|e| format!("Erro ao importar CSV '{}': {}", path.display(), e))?;

        build_table_payload(&conn, &safe_name, raw_name)
    }

    pub fn import_xlsx(&self, path: &Path) -> Result<Vec<ImportedTablePayload>, String> {
        if !path.exists() {
            return Err(format!("Arquivo não encontrado: {}", path.display()));
        }
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| format!("Erro ao abrir planilha Excel '{}': {}", path.display(), e))?;

        let sheet_names = workbook.sheet_names();
        if sheet_names.is_empty() {
            return Err("A planilha Excel não contém abas.".to_string());
        }

        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let mut imported = Vec::new();
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("excel");

        for sheet_name in sheet_names {
            let range = match workbook.worksheet_range(&sheet_name) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let mut rows_iter = range.rows();
            let Some(header_row) = rows_iter.next() else {
                continue;
            };

            let col_names = normalize_headers(header_row);
            if col_names.is_empty() {
                continue;
            }

            let data_rows: Vec<&[Data]> = rows_iter
                .filter(|r| r.iter().any(|c| !matches!(c, Data::Empty)))
                .collect();

            let col_types = infer_excel_column_types(&col_names, &data_rows);
            let raw_table_name = if sheet_name.eq_ignore_ascii_case("Sheet1")
                || sheet_name.eq_ignore_ascii_case("Planilha1")
            {
                file_stem.to_string()
            } else {
                sheet_name.clone()
            };
            let safe_table_name = sanitize_identifier(&raw_table_name);

            let col_defs: Vec<String> = col_names
                .iter()
                .zip(col_types.iter())
                .map(|(name, dtype)| format!("{} {}", quote_ident(name), dtype.duckdb_sql_type()))
                .collect();

            let create_sql = format!(
                "CREATE OR REPLACE TABLE {} ({})",
                quote_ident(&safe_table_name),
                col_defs.join(", ")
            );
            conn.execute(&create_sql, [])
                .map_err(|e| format!("Erro ao criar tabela '{}': {}", safe_table_name, e))?;

            if !data_rows.is_empty() {
                let placeholders: Vec<&str> = (0..col_names.len()).map(|_| "?").collect();
                let insert_sql = format!(
                    "INSERT INTO {} VALUES ({})",
                    quote_ident(&safe_table_name),
                    placeholders.join(", ")
                );

                conn.execute("BEGIN TRANSACTION", [])
                    .map_err(|e| format!("Erro ao iniciar transação DuckDB: {}", e))?;

                let insert_res: Result<(), String> = (|| {
                    let mut stmt = conn
                        .prepare(&insert_sql)
                        .map_err(|e| format!("Erro ao preparar INSERT: {}", e))?;

                    for row in &data_rows {
                        let mut values: Vec<duckdb::types::Value> = Vec::with_capacity(col_names.len());
                        for (idx, dtype) in col_types.iter().enumerate() {
                            let cell = row.get(idx).unwrap_or(&Data::Empty);
                            values.push(calamine_to_duckdb_value(cell, *dtype));
                        }
                        let params: Vec<&dyn ToSql> =
                            values.iter().map(|v| v as &dyn ToSql).collect();
                        stmt.execute(params_from_iter(params))
                            .map_err(|e| format!("Erro ao inserir linha em '{}': {}", safe_table_name, e))?;
                    }
                    Ok(())
                })();

                match insert_res {
                    Ok(()) => {
                        conn.execute("COMMIT", [])
                            .map_err(|e| format!("Erro ao confirmar transação DuckDB: {}", e))?;
                    }
                    Err(err) => {
                        let _ = conn.execute("ROLLBACK", []);
                        return Err(err);
                    }
                }
            }

            let payload = build_table_payload(&conn, &safe_table_name, &raw_table_name)?;
            imported.push(payload);
        }

        if imported.is_empty() {
            return Err("Nenhuma aba com cabeçalho válido foi encontrada na planilha.".to_string());
        }

        Ok(imported)
    }

    pub fn query_sql(&self, sql: &str) -> Result<SqlQueryResult, String> {
        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        execute_select_query(&conn, sql)
    }

    pub fn list_tables(&self) -> Result<Vec<TableSummary>, String> {
        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let mut stmt = conn
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_schema = 'main' ORDER BY table_name")
            .map_err(|e| format!("Erro ao listar tabelas: {}", e))?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("Erro ao ler tabelas: {}", e))?
            .filter_map(Result::ok)
            .collect();

        let mut summaries = Vec::with_capacity(table_names.len());
        for name in table_names {
            let columns = read_table_schema(&conn, &name)?;
            let count_sql = format!("SELECT COUNT(*) FROM {}", quote_ident(&name));
            let row_count: i64 = conn
                .query_row(&count_sql, [], |row| row.get(0))
                .unwrap_or(0);
            summaries.push(TableSummary {
                table_name: name,
                row_count: row_count.max(0) as usize,
                columns,
            });
        }
        Ok(summaries)
    }

    pub fn get_table_preview(&self, table_name: &str, limit: usize) -> Result<SqlQueryResult, String> {
        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let safe_limit = limit.clamp(1, PREVIEW_ROW_LIMIT);
        let sql = format!("SELECT * FROM {} LIMIT {}", quote_ident(table_name), safe_limit);
        execute_select_query(&conn, &sql)
    }

    /// Creates or replaces a DuckDB table from pre-parsed columns and JSON rows.
    pub fn insert_table_rows(
        &self,
        table_name: &str,
        columns: &[(String, String)],
        rows: &[Map<String, JsonValue>],
    ) -> Result<ImportedTablePayload, String> {
        let conn = self.conn.lock().map_err(|_| "Lock DuckDB corrompido".to_string())?;
        let safe_name = sanitize_identifier(table_name);
        let col_defs: Vec<String> = if columns.is_empty() {
            vec!["\"_id\" VARCHAR".to_string()]
        } else {
            columns
                .iter()
                .map(|(cname, bi_type)| {
                    let sql_t = match bi_type.as_str() {
                        "number" | "currency" | "percentage" => "DOUBLE",
                        "boolean" => "BOOLEAN",
                        _ => "VARCHAR",
                    };
                    format!("{} {}", quote_ident(cname), sql_t)
                })
                .collect()
        };

        let create_sql = format!(
            "CREATE OR REPLACE TABLE {} ({})",
            quote_ident(&safe_name),
            col_defs.join(", ")
        );
        conn.execute(&create_sql, [])
            .map_err(|e| format!("Erro ao criar tabela '{}': {}", safe_name, e))?;

        if !rows.is_empty() && !columns.is_empty() {
            let placeholders: Vec<&str> = (0..columns.len()).map(|_| "?").collect();
            let insert_sql = format!(
                "INSERT INTO {} VALUES ({})",
                quote_ident(&safe_name),
                placeholders.join(", ")
            );

            conn.execute("BEGIN TRANSACTION", [])
                .map_err(|e| format!("Erro ao iniciar transação: {}", e))?;

            let res: Result<(), String> = (|| {
                let mut stmt = conn
                    .prepare(&insert_sql)
                    .map_err(|e| format!("Erro ao preparar INSERT: {}", e))?;
                for row in rows {
                    let mut vals: Vec<duckdb::types::Value> = Vec::with_capacity(columns.len());
                    for (cname, bi_type) in columns {
                        let cell = row.get(cname).unwrap_or(&JsonValue::Null);
                        vals.push(json_to_duckdb_value(cell, bi_type));
                    }
                    let params: Vec<&dyn ToSql> = vals.iter().map(|v| v as &dyn ToSql).collect();
                    stmt.execute(params_from_iter(params))
                        .map_err(|e| format!("Erro ao inserir linha em '{}': {}", safe_name, e))?;
                }
                Ok(())
            })();

            match res {
                Ok(()) => {
                    conn.execute("COMMIT", [])
                        .map_err(|e| format!("Erro ao confirmar transação: {}", e))?;
                }
                Err(e) => {
                    let _ = conn.execute("ROLLBACK", []);
                    return Err(e);
                }
            }
        }

        build_table_payload(&conn, &safe_name, table_name)
    }
}

fn json_to_duckdb_value(val: &JsonValue, bi_type: &str) -> duckdb::types::Value {
    use duckdb::types::Value;
    match val {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(b) => match bi_type {
            "number" | "currency" | "percentage" => Value::Double(if *b { 1.0 } else { 0.0 }),
            "boolean" => Value::Boolean(*b),
            _ => Value::Text(b.to_string()),
        },
        JsonValue::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            match bi_type {
                "number" | "currency" | "percentage" => Value::Double(f),
                "boolean" => Value::Boolean(f != 0.0),
                _ => Value::Text(n.to_string()),
            }
        }
        JsonValue::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Value::Null;
            }
            match bi_type {
                "number" | "currency" | "percentage" => trimmed
                    .replace(',', ".")
                    .parse::<f64>()
                    .map(Value::Double)
                    .unwrap_or(Value::Null),
                "boolean" => Value::Boolean(matches!(
                    trimmed.to_lowercase().as_str(),
                    "true" | "sim" | "1" | "yes"
                )),
                _ => Value::Text(trimmed.to_string()),
            }
        }
        other => Value::Text(other.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredType {
    Number,
    Boolean,
    Date,
    Text,
}

impl InferredType {
    fn duckdb_sql_type(self) -> &'static str {
        match self {
            InferredType::Number => "DOUBLE",
            InferredType::Boolean => "BOOLEAN",
            InferredType::Date => "VARCHAR",
            InferredType::Text => "VARCHAR",
        }
    }
}

fn normalize_headers(header_row: &[Data]) -> Vec<String> {
    let mut used = HashSet::new();
    let mut out = Vec::with_capacity(header_row.len());
    for (idx, cell) in header_row.iter().enumerate() {
        let raw = match cell {
            Data::Empty => format!("Coluna_{}", idx + 1),
            other => {
                let s = other.to_string().trim().to_string();
                if s.is_empty() {
                    format!("Coluna_{}", idx + 1)
                } else {
                    s
                }
            }
        };
        let mut candidate = raw.clone();
        let mut suffix = 2;
        while used.contains(&candidate) {
            candidate = format!("{}_{}", raw, suffix);
            suffix += 1;
        }
        used.insert(candidate.clone());
        out.push(candidate);
    }
    out
}

fn infer_excel_column_types(col_names: &[String], rows: &[&[Data]]) -> Vec<InferredType> {
    let sample_len = rows.len().min(200);
    col_names
        .iter()
        .enumerate()
        .map(|(col_idx, col_name)| {
            let mut non_empty = 0usize;
            let mut numeric = 0usize;
            let mut boolean = 0usize;
            let mut date = 0usize;

            for row in rows.iter().take(sample_len) {
                let cell = row.get(col_idx).unwrap_or(&Data::Empty);
                match cell {
                    Data::Empty => {}
                    Data::Int(_) | Data::Float(_) => {
                        non_empty += 1;
                        numeric += 1;
                    }
                    Data::Bool(_) => {
                        non_empty += 1;
                        boolean += 1;
                    }
                    Data::DateTime(_) | Data::DateTimeIso(_) | Data::DurationIso(_) => {
                        non_empty += 1;
                        date += 1;
                    }
                    Data::String(s) => {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            non_empty += 1;
                            if trimmed.parse::<f64>().is_ok() {
                                numeric += 1;
                            } else if matches!(
                                trimmed.to_lowercase().as_str(),
                                "true" | "false" | "sim" | "não" | "nao"
                            ) {
                                boolean += 1;
                            }
                        }
                    }
                    Data::Error(_) => {}
                }
            }

            if non_empty == 0 {
                return InferredType::Text;
            }
            let lower_name = col_name.to_lowercase();
            if date * 10 >= non_empty * 6
                || ((lower_name.contains("data") || lower_name.contains("date")) && date > 0)
            {
                return InferredType::Date;
            }
            if boolean * 10 >= non_empty * 8 {
                return InferredType::Boolean;
            }
            if numeric * 10 >= non_empty * 7 {
                return InferredType::Number;
            }
            InferredType::Text
        })
        .collect()
}

fn calamine_to_duckdb_value(cell: &Data, dtype: InferredType) -> duckdb::types::Value {
    use duckdb::types::Value;
    match cell {
        Data::Empty | Data::Error(_) => Value::Null,
        Data::Int(i) => match dtype {
            InferredType::Number => Value::Double(*i as f64),
            InferredType::Boolean => Value::Boolean(*i != 0),
            _ => Value::Text(i.to_string()),
        },
        Data::Float(f) => match dtype {
            InferredType::Number => Value::Double(*f),
            InferredType::Boolean => Value::Boolean(*f != 0.0),
            _ => Value::Text(f.to_string()),
        },
        Data::Bool(b) => match dtype {
            InferredType::Boolean => Value::Boolean(*b),
            InferredType::Number => Value::Double(if *b { 1.0 } else { 0.0 }),
            _ => Value::Text(b.to_string()),
        },
        Data::DateTime(dt) => {
            let serial = dt.as_f64();
            if serial.is_finite() && serial >= 1.0 {
                let adj = if serial >= 60.0 { 25569 } else { 25568 };
                let days = serial.floor() as i32 - adj;
                Value::Text(format_days_since_epoch(days))
            } else {
                Value::Text(dt.to_string())
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => Value::Text(s.clone()),
        Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Value::Null;
            }
            match dtype {
                InferredType::Number => trimmed
                    .replace(',', ".")
                    .parse::<f64>()
                    .map(Value::Double)
                    .unwrap_or(Value::Null),
                InferredType::Boolean => {
                    let l = trimmed.to_lowercase();
                    Value::Boolean(matches!(l.as_str(), "true" | "sim" | "1" | "yes"))
                }
                _ => Value::Text(trimmed.to_string()),
            }
        }
    }
}

fn build_table_payload(
    conn: &Connection,
    safe_name: &str,
    original_name: &str,
) -> Result<ImportedTablePayload, String> {
    let columns = read_table_schema(conn, safe_name)?;
    let count_sql = format!("SELECT COUNT(*) FROM {}", quote_ident(safe_name));
    let total_rows: i64 = conn
        .query_row(&count_sql, [], |row| row.get(0))
        .map_err(|e| format!("Erro ao contar linhas de '{}': {}", safe_name, e))?;

    let preview_sql = format!(
        "SELECT * FROM {} LIMIT {}",
        quote_ident(safe_name),
        PREVIEW_ROW_LIMIT
    );
    let preview = execute_select_query(conn, &preview_sql)?;
    let total = total_rows.max(0) as usize;

    Ok(ImportedTablePayload {
        table_name: safe_name.to_string(),
        original_name: original_name.to_string(),
        columns,
        rows: preview.rows,
        total_rows: total,
        truncated: total > PREVIEW_ROW_LIMIT,
    })
}

fn read_table_schema(conn: &Connection, table_name: &str) -> Result<Vec<ColumnSchema>, String> {
    let sql = format!("DESCRIBE {}", quote_ident(table_name));
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Erro ao inspecionar schema de '{}': {}", table_name, e))?;

    let cols = stmt
        .query_map([], |row| {
            let col_name: String = row.get(0)?;
            let duckdb_type: String = row.get(1)?;
            Ok((col_name, duckdb_type))
        })
        .map_err(|e| format!("Erro ao ler colunas de '{}': {}", table_name, e))?
        .filter_map(Result::ok)
        .map(|(name, duckdb_type)| ColumnSchema {
            original_name: name.clone(),
            name,
            data_type: map_duckdb_type_to_bi(&duckdb_type).to_string(),
            duckdb_type,
        })
        .collect();

    Ok(cols)
}

fn map_duckdb_type_to_bi(duckdb_type: &str) -> &'static str {
    let upper = duckdb_type.to_uppercase();
    if upper.contains("INT")
        || upper.contains("DOUBLE")
        || upper.contains("FLOAT")
        || upper.contains("DECIMAL")
        || upper.contains("NUMERIC")
        || upper.contains("REAL")
        || upper.contains("HUGEINT")
    {
        "number"
    } else if upper.contains("BOOL") {
        "boolean"
    } else if upper.contains("DATE") || upper.contains("TIME") {
        "date"
    } else {
        "text"
    }
}

fn execute_select_query(conn: &Connection, sql: &str) -> Result<SqlQueryResult, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Erro DuckDB ao preparar SQL: {}", e))?;

    let mut rows_cursor = stmt
        .query([])
        .map_err(|e| format!("Erro DuckDB ao executar SQL: {}", e))?;

    let col_count = rows_cursor.as_ref().map(|s| s.column_count()).unwrap_or(0);
    let mut columns = Vec::with_capacity(col_count);
    if let Some(s) = rows_cursor.as_ref() {
        for idx in 0..col_count {
            let name = s
                .column_name(idx)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| format!("col_{}", idx));
            columns.push(name);
        }
    }

    let mut rows = Vec::new();
    while let Some(row) = rows_cursor
        .next()
        .map_err(|e| format!("Erro ao iterar resultado DuckDB: {}", e))?
    {
        let mut map = Map::with_capacity(col_count);
        for (idx, col_name) in columns.iter().enumerate() {
            let val = match row.get_ref(idx) {
                Ok(vref) => value_ref_to_json(vref),
                Err(_) => JsonValue::Null,
            };
            map.insert(col_name.clone(), val);
        }
        rows.push(map);
    }

    let row_count = rows.len();
    Ok(SqlQueryResult {
        columns,
        rows,
        row_count,
    })
}

fn value_ref_to_json(vref: ValueRef<'_>) -> JsonValue {
    match vref {
        ValueRef::Null => JsonValue::Null,
        ValueRef::Boolean(b) => JsonValue::Bool(b),
        ValueRef::TinyInt(i) => JsonValue::from(i),
        ValueRef::SmallInt(i) => JsonValue::from(i),
        ValueRef::Int(i) => JsonValue::from(i),
        ValueRef::BigInt(i) => JsonValue::from(i),
        ValueRef::HugeInt(i) => {
            if let Ok(n) = i64::try_from(i) {
                JsonValue::from(n)
            } else {
                JsonValue::String(i.to_string())
            }
        }
        ValueRef::UTinyInt(u) => JsonValue::from(u),
        ValueRef::USmallInt(u) => JsonValue::from(u),
        ValueRef::UInt(u) => JsonValue::from(u),
        ValueRef::UBigInt(u) => JsonValue::from(u),
        ValueRef::Float(f) => serde_json::Number::from_f64(f as f64)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        ValueRef::Double(d) => serde_json::Number::from_f64(d)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        ValueRef::Decimal(d) => {
            let s = d.to_string();
            s.parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::String(s))
        }
        ValueRef::Text(bytes) => JsonValue::String(String::from_utf8_lossy(bytes).to_string()),
        ValueRef::Blob(bytes) => JsonValue::String(format!("<blob {} bytes>", bytes.len())),
        ValueRef::Date32(days) => {
            // Days since 1970-01-01
            JsonValue::String(format_days_since_epoch(days))
        }
        ValueRef::Timestamp(_, micros) => JsonValue::from(micros),
        other => JsonValue::String(format!("{:?}", other)),
    }
}

pub fn calamine_cell_to_json(cell: &Data, bi_type: &str) -> JsonValue {
    match cell {
        Data::Empty | Data::Error(_) => JsonValue::Null,
        Data::Int(i) => match bi_type {
            "number" | "currency" | "percentage" => JsonValue::from(*i as f64),
            "boolean" => JsonValue::Bool(*i != 0),
            _ => JsonValue::String(i.to_string()),
        },
        Data::Float(f) => match bi_type {
            "number" | "currency" | "percentage" => serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            "boolean" => JsonValue::Bool(*f != 0.0),
            _ => JsonValue::String(f.to_string()),
        },
        Data::Bool(b) => match bi_type {
            "boolean" => JsonValue::Bool(*b),
            "number" | "currency" | "percentage" => JsonValue::from(if *b { 1.0 } else { 0.0 }),
            _ => JsonValue::String(b.to_string()),
        },
        Data::DateTime(dt) => {
            let serial = dt.as_f64();
            if serial.is_finite() && serial >= 1.0 {
                let adj = if serial >= 60.0 { 25569 } else { 25568 };
                let days = serial.floor() as i32 - adj;
                JsonValue::String(format_days_since_epoch(days))
            } else {
                JsonValue::String(dt.to_string())
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => JsonValue::String(s.clone()),
        Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return JsonValue::Null;
            }
            match bi_type {
                "number" | "currency" | "percentage" => trimmed
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null),
                "boolean" => JsonValue::Bool(matches!(
                    trimmed.to_lowercase().as_str(),
                    "true" | "sim" | "1" | "yes"
                )),
                _ => JsonValue::String(trimmed.to_string()),
            }
        }
    }
}

pub fn format_days_since_epoch(days: i32) -> String {
    // Civil calendar conversion from Howard Hinnant's algorithm
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

pub fn sanitize_identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.trim().chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '-' || ch == '.' {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "tabela".to_string()
    } else if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("t_{}", trimmed)
    } else {
        trimmed
    }
}

pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_duckdb_import_csv_and_query() {
        let store = DuckDbStore::new_in_memory().expect("init duckdb");
        let mut tmp = NamedTempFile::new().expect("temp csv");
        writeln!(tmp, "Categoria,Valor,Qtd").unwrap();
        writeln!(tmp, "Eletronicos,1500.50,3").unwrap();
        writeln!(tmp, "Moveis,800.00,2").unwrap();
        writeln!(tmp, "Eletronicos,500.00,1").unwrap();

        let imported = store
            .import_csv(tmp.path(), Some("f_Vendas"))
            .expect("import csv");

        assert_eq!(imported.table_name, "f_Vendas");
        assert_eq!(imported.total_rows, 3);
        assert_eq!(imported.columns.len(), 3);

        let res = store
            .query_sql("SELECT Categoria, SUM(Valor) AS Total FROM \"f_Vendas\" GROUP BY Categoria ORDER BY Total DESC")
            .expect("query sql");
        assert_eq!(res.row_count, 2);
        assert_eq!(res.rows[0].get("Categoria").and_then(|v| v.as_str()), Some("Eletronicos"));
        let total = res.rows[0].get("Total").and_then(|v| v.as_f64()).unwrap();
        assert!((total - 2000.5).abs() < 0.001);

        let info = store.get_engine_info().expect("engine info");
        assert_eq!(info.engine, "duckdb-native");
        assert!(info.table_count >= 1);
    }
}
