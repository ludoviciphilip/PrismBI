//! Loads an `EtlSource` into `raw__<query>` exactly as Power Query sees it
//! before any step: Excel sheets and CSV files arrive as `Column1..N` with the
//! header row still in the data (that is what `promoteHeaders` is for).
//!
//! Every raw table gets a `__row_idx BIGINT` column (0-based source order)
//! that the SQL chain uses for stable ordering.

use super::model::{EtlSource, Scalar, SourceKind};
use super::sql::{
    describe, etl_type_name, flag_name, is_hidden, lit_str, quote_ident, raw_table_name, ColInfo, ROW_IDX,
};
use crate::engine::duckdb_store::format_days_since_epoch;
use crate::pbi::m_loader::resolve_source_path;
use calamine::{open_workbook_auto, Data, Reader};
use duckdb::{params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtlColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLoadResult {
    pub raw_table: String,
    pub columns: Vec<EtlColumn>,
    pub row_count: usize,
    /// Resolved file path (excel/csv/parquet/json), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

pub fn visible_columns(schema: &[ColInfo]) -> Vec<EtlColumn> {
    schema
        .iter()
        .filter(|c| !is_hidden(&c.name))
        .map(|c| EtlColumn {
            name: c.name.clone(),
            ty: etl_type_name(&c.ty).to_string(),
        })
        .collect()
}

fn resolve_path(raw: &str, pbip_root: Option<&Path>) -> Result<PathBuf, String> {
    let direct = PathBuf::from(raw);
    if direct.is_file() {
        return Ok(direct);
    }
    if let Some(root) = pbip_root {
        if let Some(p) = resolve_source_path(raw, root) {
            return Ok(p);
        }
    }
    Err(format!("Arquivo fonte '{}' não encontrado.", raw))
}

/// Canonical, culture-independent text of a typed number cell.
pub fn canonical_number(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

/// Excel serial → ISO date (`YYYY-MM-DD`) or datetime (`YYYY-MM-DD HH:MM:SS`).
pub fn excel_serial_to_iso(serial: f64) -> Option<String> {
    if !serial.is_finite() || serial < 0.0 {
        return None;
    }
    let mut days = serial.floor();
    let mut secs = ((serial - days) * 86400.0).round() as i64;
    if secs >= 86400 {
        days += 1.0;
        secs -= 86400;
    }
    // Excel's fictitious 1900-02-29: serials < 60 are one day off.
    let adj = if serial >= 60.0 { 25569.0 } else { 25568.0 };
    let date = format_days_since_epoch((days - adj) as i32);
    if secs == 0 {
        Some(date)
    } else {
        Some(format!("{} {:02}:{:02}:{:02}", date, secs / 3600, (secs / 60) % 60, secs % 60))
    }
}

enum Cell {
    Null,
    Text(String),
    Error,
}

fn excel_cell(c: &Data) -> Cell {
    match c {
        Data::Empty => Cell::Null,
        Data::String(s) => Cell::Text(s.clone()),
        Data::Int(i) => Cell::Text(i.to_string()),
        Data::Float(f) => Cell::Text(canonical_number(*f)),
        Data::Bool(b) => Cell::Text(if *b { "true".into() } else { "false".into() }),
        Data::DateTime(dt) => match excel_serial_to_iso(dt.as_f64()) {
            Some(s) => Cell::Text(s),
            None => Cell::Text(dt.to_string()),
        },
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.clone()),
        Data::Error(_) => Cell::Error,
    }
}

fn insert_rows(
    conn: &Connection,
    table: &str,
    ncols: usize,
    rows: impl Iterator<Item = Vec<duckdb::types::Value>>,
) -> Result<(), String> {
    let placeholders = vec!["?"; ncols].join(", ");
    let sql = format!("INSERT INTO {} VALUES ({})", quote_ident(table), placeholders);
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| format!("Erro ao iniciar transação: {}", e))?;
    let res: Result<(), String> = (|| {
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Erro ao preparar INSERT: {}", e))?;
        for row in rows {
            let params: Vec<&dyn ToSql> = row.iter().map(|v| v as &dyn ToSql).collect();
            stmt.execute(params_from_iter(params))
                .map_err(|e| format!("Erro ao inserir linha bruta: {}", e))?;
        }
        Ok(())
    })();
    match res {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|e| format!("Erro ao confirmar transação: {}", e)),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn load_excel(conn: &Connection, raw: &str, path: &Path, sheet: Option<&str>) -> Result<(), String> {
    let mut wb = open_workbook_auto(path).map_err(|e| format!("Falha ao abrir '{}': {}", path.display(), e))?;
    let names = wb.sheet_names();
    let chosen = match sheet.filter(|s| !s.trim().is_empty()) {
        Some(s) => names
            .iter()
            .find(|n| n.as_str() == s)
            .or_else(|| names.iter().find(|n| n.eq_ignore_ascii_case(s.trim())))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Aba '{}' não encontrada em '{}'. Abas: {}",
                    s,
                    path.display(),
                    names.join(", ")
                )
            })?,
        None => names.first().cloned().ok_or("Planilha Excel sem abas.")?,
    };
    let range = wb
        .worksheet_range(&chosen)
        .map_err(|e| format!("Falha ao ler aba '{}': {}", chosen, e))?;
    let width = range.width();
    let cells: Vec<Vec<Cell>> = range.rows().map(|r| r.iter().map(excel_cell).collect()).collect();
    let mut err_cols = vec![false; width];
    for r in &cells {
        for (i, c) in r.iter().enumerate() {
            if matches!(c, Cell::Error) {
                err_cols[i] = true;
            }
        }
    }
    let mut defs = vec![format!("{} BIGINT", quote_ident(ROW_IDX))];
    for i in 0..width {
        defs.push(format!("{} VARCHAR", quote_ident(&format!("Column{}", i + 1))));
    }
    for (i, has) in err_cols.iter().enumerate() {
        if *has {
            defs.push(format!("{} BOOLEAN", quote_ident(&flag_name(&format!("Column{}", i + 1)))));
        }
    }
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} ({})", quote_ident(raw), defs.join(", ")))
        .map_err(|e| format!("Erro ao criar '{}': {}", raw, e))?;
    let ncols = defs.len();
    let rows = cells.into_iter().enumerate().map(|(idx, r)| {
        use duckdb::types::Value;
        let mut vals = Vec::with_capacity(ncols);
        vals.push(Value::BigInt(idx as i64));
        let mut errs = Vec::new();
        for i in 0..width {
            let c = r.get(i).unwrap_or(&Cell::Null);
            vals.push(match c {
                Cell::Text(s) => Value::Text(s.clone()),
                _ => Value::Null,
            });
            if err_cols[i] {
                errs.push(Value::Boolean(matches!(c, Cell::Error)));
            }
        }
        vals.extend(errs);
        vals
    });
    insert_rows(conn, raw, ncols, rows)
}

/// `CREATE OR REPLACE TABLE raw AS SELECT rowid AS __row_idx, <cols> FROM tmp`.
fn copy_with_row_idx(conn: &Connection, raw: &str, select_from_tmp: &str, rename: Option<&[ColInfo]>) -> Result<(), String> {
    let tmp = "__etl_tmp_load";
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", quote_ident(tmp)))
        .map_err(|e| e.to_string())?;
    conn.execute_batch(&format!("CREATE TEMP TABLE {} AS {}", quote_ident(tmp), select_from_tmp))
        .map_err(|e| format!("Erro ao ler a fonte: {}", e))?;
    let schema = describe(conn, &format!("SELECT * FROM {}", quote_ident(tmp)))?;
    let cols: Vec<String> = match rename {
        // CSV: column0.. → Column1.., all text; empty fields are "" like Csv.Document.
        Some(_) => schema
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "COALESCE(CAST({} AS VARCHAR), '') AS {}",
                    quote_ident(&c.name),
                    quote_ident(&format!("Column{}", i + 1))
                )
            })
            .collect(),
        None => schema
            .iter()
            .map(|c| {
                if is_hidden(&c.name) {
                    format!("{} AS {}", quote_ident(&c.name), quote_ident(&format!("{}_src", c.name)))
                } else {
                    quote_ident(&c.name)
                }
            })
            .collect(),
    };
    let list = if cols.is_empty() { String::new() } else { format!(", {}", cols.join(", ")) };
    let res = conn.execute_batch(&format!(
        "CREATE OR REPLACE TABLE {} AS SELECT CAST(rowid AS BIGINT) AS {}{} FROM {} ORDER BY rowid",
        quote_ident(raw),
        quote_ident(ROW_IDX),
        list,
        quote_ident(tmp)
    ));
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", quote_ident(tmp)));
    res.map_err(|e| format!("Erro ao criar '{}': {}", raw, e))
}

fn infer_inline_type(rows: &[Vec<Scalar>], col: usize) -> &'static str {
    let vals: Vec<&Scalar> = rows.iter().filter_map(|r| r.get(col)).filter(|v| !v.is_null()).collect();
    if !vals.is_empty() && vals.iter().all(|v| matches!(v, Scalar::Number(_))) {
        "DOUBLE"
    } else if !vals.is_empty() && vals.iter().all(|v| matches!(v, Scalar::Bool(_))) {
        "BOOLEAN"
    } else {
        "VARCHAR"
    }
}

fn load_inline(conn: &Connection, raw: &str, columns: &[String], rows: &[Vec<Scalar>]) -> Result<(), String> {
    let types: Vec<&str> = (0..columns.len()).map(|i| infer_inline_type(rows, i)).collect();
    let mut defs = vec![format!("{} BIGINT", quote_ident(ROW_IDX))];
    for (c, t) in columns.iter().zip(&types) {
        super::sql::check_user_name(c)?;
        defs.push(format!("{} {}", quote_ident(c), t));
    }
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} ({})", quote_ident(raw), defs.join(", ")))
        .map_err(|e| format!("Erro ao criar '{}': {}", raw, e))?;
    let ncols = defs.len();
    let iter = rows.iter().enumerate().map(|(idx, r)| {
        use duckdb::types::Value;
        let mut vals = vec![Value::BigInt(idx as i64)];
        for (i, t) in types.iter().enumerate() {
            vals.push(match (r.get(i).unwrap_or(&Scalar::Null), *t) {
                (Scalar::Null, _) => Value::Null,
                (Scalar::Number(n), "DOUBLE") => Value::Double(*n),
                (Scalar::Bool(b), "BOOLEAN") => Value::Boolean(*b),
                (Scalar::Number(n), _) => Value::Text(canonical_number(*n)),
                (Scalar::Bool(b), _) => Value::Text(b.to_string()),
                (Scalar::Text(s), _) => Value::Text(s.clone()),
            });
        }
        vals
    });
    insert_rows(conn, raw, ncols, iter)
}

fn parse_iso_date(s: &str) -> Result<(i64, i64, i64), String> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    let bad = || format!("data inicial inválida '{}' (esperado AAAA-MM-DD)", s);
    if parts.len() != 3 {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: i64 = parts[1].parse().map_err(|_| bad())?;
    let d: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    Ok((y, m, d))
}

/// (Re)creates `raw__<query>` from `source`. `conn` must not be locked by the caller twice.
pub fn load_raw(conn: &Connection, query: &str, source: &EtlSource, pbip_root: Option<&Path>) -> Result<RawLoadResult, String> {
    let raw = raw_table_name(query);
    let mut source_path = None;
    let need_path = || -> Result<PathBuf, String> {
        let p = source
            .path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| "fonte sem 'path'".to_string())?;
        resolve_path(p, pbip_root)
    };
    match source.kind {
        SourceKind::Excel => {
            let p = need_path()?;
            load_excel(conn, &raw, &p, source.sheet.as_deref())?;
            source_path = Some(p.to_string_lossy().to_string());
        }
        SourceKind::Csv => {
            let p = need_path()?;
            let ps = lit_str(&p.to_string_lossy());
            copy_with_row_idx(
                conn,
                &raw,
                &format!(
                    "SELECT * FROM read_csv({}, header = false, all_varchar = true, null_padding = true)",
                    ps
                ),
                Some(&[]),
            )?;
            source_path = Some(p.to_string_lossy().to_string());
        }
        SourceKind::Parquet => {
            let p = need_path()?;
            copy_with_row_idx(conn, &raw, &format!("SELECT * FROM read_parquet({})", lit_str(&p.to_string_lossy())), None)?;
            source_path = Some(p.to_string_lossy().to_string());
        }
        SourceKind::Json => {
            let p = need_path()?;
            copy_with_row_idx(conn, &raw, &format!("SELECT * FROM read_json_auto({})", lit_str(&p.to_string_lossy())), None)?;
            source_path = Some(p.to_string_lossy().to_string());
        }
        SourceKind::Table => {
            let t = source.table.as_deref().ok_or("fonte 'table' sem 'table'")?;
            copy_with_row_idx(conn, &raw, &format!("SELECT * FROM {}", quote_ident(t)), None)?;
        }
        SourceKind::Db => {
            // `table` = DuckDB relation path, e.g. an ATTACHed `catalog.schema.table`.
            let t = source.table.as_deref().ok_or("fonte 'db' sem 'table'")?;
            let rel = t.split('.').map(quote_ident).collect::<Vec<_>>().join(".");
            copy_with_row_idx(conn, &raw, &format!("SELECT * FROM {}", rel), None)?;
        }
        SourceKind::Inline => {
            let cols = source.columns.clone().unwrap_or_default();
            let rows = source.rows.clone().unwrap_or_default();
            if rows.iter().any(|r| r.len() != cols.len()) {
                return Err("fonte 'inline': linhas com número de células diferente das colunas".into());
            }
            load_inline(conn, &raw, &cols, &rows)?;
        }
        SourceKind::ListDates => {
            let (y, m, d) = parse_iso_date(source.start.as_deref().ok_or("fonte 'listDates' sem 'start'")?)?;
            let count = source.count.ok_or("fonte 'listDates' sem 'count'")?;
            if count > 1_000_000 {
                return Err("fonte 'listDates': quantidade de datas muito grande".into());
            }
            let step = source.step_days.unwrap_or(1);
            conn.execute_batch(&format!(
                "CREATE OR REPLACE TABLE {} AS SELECT CAST(i AS BIGINT) AS {}, CAST(make_date({}, {}, {}) + CAST(i * {} AS INTEGER) AS DATE) AS \"Column1\" FROM range({}) AS t(i) ORDER BY i",
                quote_ident(&raw),
                quote_ident(ROW_IDX),
                y,
                m,
                d,
                step,
                count
            ))
            .map_err(|e| format!("Erro ao gerar datas: {}", e))?;
        }
        SourceKind::OpaqueM => {
            return Err("Fonte M não suportada (opaqueM): não é possível carregar os dados brutos.".into());
        }
    }
    let schema = describe(conn, &format!("SELECT * FROM {}", quote_ident(&raw)))?;
    let row_count: i64 = conn
        .query_row(&format!("SELECT count(*) FROM {}", quote_ident(&raw)), [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(RawLoadResult {
        raw_table: raw,
        columns: visible_columns(&schema),
        row_count: row_count.max(0) as usize,
        source_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_and_number_encoding() {
        assert_eq!(excel_serial_to_iso(46113.0).as_deref(), Some("2026-04-01"));
        assert_eq!(excel_serial_to_iso(46113.5).as_deref(), Some("2026-04-01 12:00:00"));
        assert_eq!(canonical_number(2026.0), "2026");
        assert_eq!(canonical_number(1234.5), "1234.5");
        assert_eq!(canonical_number(-0.25), "-0.25");
    }
}
