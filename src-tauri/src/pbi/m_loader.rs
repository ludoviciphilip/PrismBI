use crate::engine::duckdb_store::{
    calamine_cell_to_json, format_days_since_epoch, sanitize_identifier, DuckDbStore,
    ImportedTablePayload,
};
use crate::pbi::PbiTableMeta;
use calamine::{open_workbook_auto, Data, Reader};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTableRequest {
    pub table_name: String,
    pub source_type: String,
    pub file_path: Option<String>,
    pub sheet_name: Option<String>,
    pub m_script: Option<String>,
    pub pbip_root: Option<String>,
}

/// Materializes `.pbip` model tables into DuckDB by interpreting Power Query M and DAX `DATATABLE` partitions.
///
/// M partitions go through the v0.9 ETL engine first (`m_parse` → `m_from` →
/// SQL chain on DuckDB, see `crate::etl`). Only when that path fails (an
/// unsupported step/source, a missing file, …) the legacy text-search
/// interpreter below is used, and a warning says so.
pub fn materialize_pbip_tables(
    tables: &[PbiTableMeta],
    pbip_root: &Path,
    db: &DuckDbStore,
    warnings: &mut Vec<String>,
) -> Vec<ImportedTablePayload> {
    let mut loaded = Vec::new();
    let mut m_tables: Vec<(&PbiTableMeta, &str)> = Vec::new();

    for table in tables {
        if table.name.starts_with("DateTableTemplate_") || table.name.starts_with("LocalDateTable_")
        {
            continue;
        }
        let Some(part) = table.partitions.first() else {
            continue;
        };

        let expr = part.expression.trim();
        if expr.is_empty() {
            continue;
        }

        // 1. Calculated DATATABLE(...) partition (e.g. dEstrutura): DAX, not M.
        if part.source_type.eq_ignore_ascii_case("calculated") || expr.to_uppercase().contains("DATATABLE") {
            match parse_and_load_datatable(table, expr, db) {
                Ok(Some(payload)) => loaded.push(payload),
                Ok(None) => {}
                Err(e) => warnings.push(format!("Tabela calculada '{}': {}", table.name, e)),
            }
            continue;
        }
        m_tables.push((table, expr));
    }

    // Queries that merge/append/reference other queries need those model
    // tables first: process in dependency order (stable otherwise).
    for (table, expr) in order_by_etl_dependencies(&m_tables) {
        match materialize_via_etl(table, expr, pbip_root, db) {
            Ok(Some(payload)) => {
                loaded.push(payload);
                continue;
            }
            // Query without columns (e.g. the measures host table): nothing to load.
            Ok(None) => continue,
            Err(e) => warnings.push(format!(
                "ETL: '{}' carregada pelo interpretador M legado ({})",
                table.name, e
            )),
        }
        materialize_m_partition_legacy(table, expr, pbip_root, db, warnings, &mut loaded);
    }

    loaded
}

/// v0.9 path for one M partition. `Ok(None)`: the query yields no columns.
pub fn materialize_via_etl(
    table: &PbiTableMeta,
    expr: &str,
    pbip_root: &Path,
    db: &DuckDbStore,
) -> Result<Option<ImportedTablePayload>, String> {
    use crate::etl::model::{SourceKind, StepKind};
    let parsed = crate::etl::m_from::query_from_m(&table.name, expr)?;
    let q = parsed.query;
    if q.source.kind == SourceKind::OpaqueM {
        return Err("fonte M não suportada".into());
    }
    if let Some(s) = q.steps.iter().find(|s| matches!(s.kind, StepKind::OpaqueM { .. })) {
        return Err(format!("etapa '{}' não suportada", s.name));
    }
    let raw = crate::etl::commands::etl_load_raw_impl(db, &q.name, &q.source, Some(pbip_root))?;
    let payload = crate::etl::commands::etl_apply_inner(db, &q, &table.name)?;
    Ok(payload.map(|mut p| {
        if raw.source_path.is_some() {
            p.source_path = raw.source_path.clone();
        }
        p
    }))
}

fn order_by_etl_dependencies<'a>(items: &[(&'a PbiTableMeta, &'a str)]) -> Vec<(&'a PbiTableMeta, &'a str)> {
    let names: HashSet<String> = items.iter().map(|(t, _)| t.name.clone()).collect();
    let deps: Vec<Vec<String>> = items
        .iter()
        .map(|(t, e)| {
            crate::etl::m_from::query_from_m(&t.name, e)
                .map(|r| {
                    r.query
                        .referenced_queries()
                        .into_iter()
                        .filter(|d| names.contains(d) && *d != t.name)
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    let mut done: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    let mut remaining: Vec<usize> = (0..items.len()).collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        remaining.retain(|&i| {
            if deps[i].iter().all(|d| done.contains(d)) {
                done.insert(items[i].0.name.clone());
                out.push(items[i]);
                false
            } else {
                true
            }
        });
        if remaining.len() == before {
            // cycle: keep original order for the rest
            out.extend(remaining.iter().map(|&i| items[i]));
            break;
        }
    }
    out
}

/// Legacy text-search interpreter for one M partition (fallback only).
pub(crate) fn materialize_m_partition_legacy(
    table: &PbiTableMeta,
    expr: &str,
    pbip_root: &Path,
    db: &DuckDbStore,
    warnings: &mut Vec<String>,
    loaded: &mut Vec<ImportedTablePayload>,
) {
    // 2. Empty Measures Host Table (#table(1, ...) + Table.RemoveColumns)
    if expr.contains("Table.RemoveColumns") && expr.contains("#table(") {
        return;
    }

    // 3. Calendar M partition (Query1(#date(Y, M, D), count, #duration(...)) or List.Dates)
    if (expr.contains("#date(") && expr.contains("#duration(")) || expr.contains("List.Dates") {
        match generate_and_load_calendar(table, expr, db) {
            Ok(payload) => loaded.push(payload),
            Err(e) => warnings.push(format!("Calendário '{}': {}", table.name, e)),
        }
        return;
    }

    // 4. Excel.Workbook(File.Contents("..."))
    if expr.contains("Excel.Workbook") && expr.contains("File.Contents") {
        match load_excel_partition(table, expr, pbip_root, db, None, None) {
            Ok(payload) => loaded.push(payload),
            Err(e) => warnings.push(format!("Excel '{}': {}", table.name, e)),
        }
        return;
    }

    // 5. Csv.Document(File.Contents("..."))
    if expr.contains("Csv.Document") && expr.contains("File.Contents") {
        if let Some(raw_path) = extract_quoted_arg_after(expr, "File.Contents(") {
            if let Some(resolved) = resolve_source_path(&raw_path, pbip_root) {
                match db.import_csv(&resolved, Some(&table.name)) {
                    Ok(mut payload) => {
                        payload.table_name = table.name.clone();
                        payload.source_path = Some(resolved.to_string_lossy().to_string());
                        loaded.push(payload);
                    }
                    Err(e) => warnings.push(format!("CSV '{}': {}", table.name, e)),
                }
            } else {
                warnings.push(format!(
                    "Arquivo CSV '{}' referenciado em '{}' não encontrado localmente.",
                    raw_path, table.name
                ));
            }
        }
    }
}

pub fn refresh_single_table(
    req: &RefreshTableRequest,
    db: &DuckDbStore,
) -> Result<ImportedTablePayload, String> {
    let pbip_root = req
        .pbip_root
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.exists());

    let resolved_path: PathBuf = if let Some(fp) = req.file_path.as_deref().filter(|s| !s.trim().is_empty()) {
        let direct = PathBuf::from(fp);
        if direct.is_file() {
            direct
        } else if let Some(root) = pbip_root.as_deref() {
            resolve_source_path(fp, root).ok_or_else(|| {
                format!(
                    "Arquivo fonte '{}' da tabela '{}' não foi encontrado.",
                    fp, req.table_name
                )
            })?
        } else {
            return Err(format!(
                "Arquivo fonte '{}' da tabela '{}' não foi encontrado.",
                fp, req.table_name
            ));
        }
    } else if let Some((m, root)) = req.m_script.as_deref().zip(pbip_root.as_deref()) {
        let raw = extract_quoted_arg_after(m, "File.Contents(")
            .ok_or_else(|| format!("Caminho fonte não definido para tabela '{}'.", req.table_name))?;
        resolve_source_path(&raw, root).ok_or_else(|| {
            format!(
                "Arquivo fonte '{}' referenciado em '{}' não foi encontrado.",
                raw, req.table_name
            )
        })?
    } else {
        return Err(format!(
            "Tabela '{}' não possui arquivo fonte vinculado.",
            req.table_name
        ));
    };

    let src_lower = req.source_type.trim().to_lowercase();
    let mut payload = match src_lower.as_str() {
        "xlsx" | "xls" => {
            if let Some(m) = req.m_script.as_deref().filter(|s| s.contains("Table.") || s.contains("Excel.Workbook")) {
                let dummy_meta = PbiTableMeta {
                    name: req.table_name.clone(),
                    columns: vec![],
                    measures: vec![],
                    partitions: vec![],
                    is_calculated: false,
                    is_hidden: false,
                };
                load_excel_partition(
                    &dummy_meta,
                    m,
                    pbip_root.as_deref().unwrap_or_else(|| resolved_path.parent().unwrap_or(Path::new("."))),
                    db,
                    req.sheet_name.as_deref(),
                    Some(&resolved_path),
                )?
            } else {
                db.import_xlsx_sheet(&resolved_path, req.sheet_name.as_deref(), Some(&req.table_name))?
            }
        }
        "csv" | "tsv" => db.import_csv(&resolved_path, Some(&req.table_name))?,
        "parquet" => db.import_parquet(&resolved_path, Some(&req.table_name))?,
        "json" | "ndjson" => db.import_json_file(&resolved_path, Some(&req.table_name))?,
        _ => return Err(format!("Tipo de fonte não suportado para atualização: {}", req.source_type)),
    };

    payload.table_name = req.table_name.clone();
    payload.source_path = Some(resolved_path.to_string_lossy().to_string());
    if let Some(sh) = req.sheet_name.as_deref().filter(|s| !s.trim().is_empty()) {
        if payload.sheet_name.is_none() {
            payload.sheet_name = Some(sh.to_string());
        }
    }
    Ok(payload)
}

fn load_excel_partition(
    table: &PbiTableMeta,
    expr: &str,
    pbip_root: &Path,
    db: &DuckDbStore,
    sheet_override: Option<&str>,
    resolved_override: Option<&Path>,
) -> Result<ImportedTablePayload, String> {
    let resolved = if let Some(p) = resolved_override {
        p.to_path_buf()
    } else {
        let raw_path = extract_quoted_arg_after(expr, "File.Contents(")
            .ok_or_else(|| "Caminho File.Contents não encontrado na partição M.".to_string())?;
        resolve_source_path(&raw_path, pbip_root).ok_or_else(|| {
            format!(
                "Arquivo Excel '{}' não encontrado em '{}'.",
                raw_path,
                pbip_root.display()
            )
        })?
    };

    let target_sheet = sheet_override
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| extract_excel_sheet_item(expr));
    let renames = extract_rename_pairs(expr);
    let removed_cols = extract_removed_columns(expr);
    let selected_cols = extract_selected_columns(expr);
    let row_filters = extract_row_filters(expr);

    let mut workbook = open_workbook_auto(&resolved)
        .map_err(|e| format!("Falha ao abrir '{}': {}", resolved.display(), e))?;
    let sheet_names = workbook.sheet_names();
    let chosen_sheet = target_sheet
        .and_then(|s| sheet_names.iter().find(|n| n.eq_ignore_ascii_case(&s)).cloned())
        .or_else(|| sheet_names.into_iter().next())
        .ok_or_else(|| "Planilha Excel sem abas.".to_string())?;

    let range = workbook
        .worksheet_range(&chosen_sheet)
        .map_err(|e| format!("Falha ao ler aba '{}': {}", chosen_sheet, e))?;

    let mut rows_iter = range.rows();
    let header_row = rows_iter
        .next()
        .ok_or_else(|| format!("Aba '{}' está vazia.", chosen_sheet))?;

    let mut headers: Vec<String> = header_row
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let raw = c.to_string().trim().to_string();
            let base = if raw.is_empty() {
                format!("Coluna_{}", i + 1)
            } else {
                raw
            };
            renames.get(&base).cloned().unwrap_or(base)
        })
        .collect();

    // Match TMDL columns order/types if available
    let tmdl_types: HashMap<String, String> = table
        .columns
        .iter()
        .map(|c| (c.name.to_lowercase(), c.data_type.clone()))
        .collect();

    // Ensure headers match unique names
    let mut seen = HashSet::new();
    for h in &mut headers {
        let orig = h.clone();
        let mut idx = 2;
        while seen.contains(h) {
            *h = format!("{}_{}", orig, idx);
            idx += 1;
        }
        seen.insert(h.clone());
    }

    let columns_schema: Vec<(String, String)> = headers
        .iter()
        .map(|h| {
            let dtype = tmdl_types
                .get(&h.to_lowercase())
                .cloned()
                .unwrap_or_else(|| "text".to_string());
            (h.clone(), dtype)
        })
        .collect();

    let mut json_rows = Vec::new();
    for row in rows_iter {
        if !row.iter().any(|c| !matches!(c, Data::Empty)) {
            continue;
        }
        let mut map = Map::with_capacity(headers.len());
        let mut any_non_null = false;
        for (idx, (col_name, bi_type)) in columns_schema.iter().enumerate() {
            let cell = row.get(idx).unwrap_or(&Data::Empty);
            let jv = calamine_cell_to_json(cell, bi_type);
            if !jv.is_null() {
                any_non_null = true;
            }
            map.insert(col_name.clone(), jv);
        }
        if any_non_null && passes_row_filters(&map, &row_filters) {
            json_rows.push(map);
        }
    }

    // Apply M Table.RemoveColumns / Table.SelectColumns when not overridden by explicit TMDL metadata
    let mut final_schema = columns_schema;
    if !removed_cols.is_empty() {
        let remove_lower: HashSet<String> = removed_cols.iter().map(|c| c.to_lowercase()).collect();
        final_schema.retain(|(name, _)| !remove_lower.contains(&name.to_lowercase()));
        for r in &mut json_rows {
            for col in &removed_cols {
                r.remove(col);
            }
        }
    }
    if !selected_cols.is_empty() && table.columns.is_empty() {
        let select_lower: HashSet<String> = selected_cols.iter().map(|c| c.to_lowercase()).collect();
        final_schema.retain(|(name, _)| select_lower.contains(&name.to_lowercase()));
        for r in &mut json_rows {
            r.retain(|k, _| select_lower.contains(&k.to_lowercase()));
        }
    }

    // Add any missing TMDL columns (e.g. optional columns not present in Excel) as null
    for col in &table.columns {
        if !final_schema
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(&col.name))
        {
            final_schema.push((col.name.clone(), col.data_type.clone()));
            for r in &mut json_rows {
                r.insert(col.name.clone(), JsonValue::Null);
            }
        }
    }

    let mut payload = db.insert_table_rows(&table.name, &final_schema, &json_rows)?;
    payload.table_name = table.name.clone();
    payload.original_name = table.name.clone();
    payload.source_path = Some(resolved.to_string_lossy().to_string());
    payload.sheet_name = Some(chosen_sheet);
    Ok(payload)
}

fn generate_and_load_calendar(
    table: &PbiTableMeta,
    expr: &str,
    db: &DuckDbStore,
) -> Result<ImportedTablePayload, String> {
    let (start_year, start_month, start_day, count) = parse_calendar_params(expr)
        .unwrap_or((2026, 1, 1, 365));

    let start_epoch_days = civil_to_days(start_year, start_month, start_day);

    let columns: Vec<(String, String)> = if table.columns.is_empty() {
        vec![
            ("Data".into(), "date".into()),
            ("Ano".into(), "number".into()),
            ("Mês".into(), "number".into()),
            ("Nome do Mês".into(), "text".into()),
            ("Dia".into(), "number".into()),
            ("Nome do Dia".into(), "text".into()),
            ("Semana do Mês".into(), "number".into()),
            ("Semestre".into(), "text".into()),
            ("Mês/Ano".into(), "text".into()),
        ]
    } else {
        table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.data_type.clone()))
            .collect()
    };

    const MONTHS_PT: [&str; 12] = [
        "janeiro", "fevereiro", "março", "abril", "maio", "junho",
        "julho", "agosto", "setembro", "outubro", "novembro", "dezembro",
    ];
    const MONTHS_SHORT_PT: [&str; 12] = [
        "Jan", "Fev", "Mar", "Abr", "Mai", "Jun",
        "Jul", "Ago", "Set", "Out", "Nov", "Dez",
    ];
    const DAYS_PT: [&str; 7] = [
        "quinta-feira", "sexta-feira", "sábado", "domingo", "segunda-feira", "terça-feira", "quarta-feira",
    ];

    let mut rows = Vec::with_capacity(count as usize);
    for offset in 0..count {
        let days = start_epoch_days + offset;
        let iso = format_days_since_epoch(days);
        let (y, m, d) = days_to_civil(days);
        let m_idx = (m as usize).saturating_sub(1).min(11);
        let dow_idx = ((days.rem_euclid(7)) as usize).min(6);
        let week_of_month = ((d - 1) / 7) + 1;
        let semestre = if m <= 6 { "S1" } else { "S2" };
        let mes_ano = format!("{}/{}", MONTHS_SHORT_PT[m_idx], y);

        let mut map = Map::new();
        for (col_name, _) in &columns {
            let lower = col_name.to_lowercase();
            let val = if lower == "data" || lower == "date" {
                JsonValue::String(iso.clone())
            } else if lower == "ano" || lower == "year" {
                JsonValue::from(y)
            } else if lower == "mês" || lower == "mes" || lower == "month" {
                JsonValue::from(m)
            } else if lower == "nome do mês" || lower == "month name" {
                JsonValue::String(MONTHS_PT[m_idx].to_string())
            } else if lower == "dia" || lower == "day" {
                JsonValue::from(d)
            } else if lower == "nome do dia" || lower == "day name" {
                JsonValue::String(DAYS_PT[dow_idx].to_string())
            } else if lower == "semana do mês" {
                JsonValue::from(week_of_month)
            } else if lower == "semestre" {
                JsonValue::String(semestre.to_string())
            } else if lower == "mês/ano" || lower == "mes/ano" {
                JsonValue::String(mes_ano.clone())
            } else {
                JsonValue::Null
            };
            map.insert(col_name.clone(), val);
        }
        rows.push(map);
    }

    let mut payload = db.insert_table_rows(&table.name, &columns, &rows)?;
    payload.table_name = table.name.clone();
    payload.original_name = table.name.clone();
    Ok(payload)
}

fn parse_and_load_datatable(
    table: &PbiTableMeta,
    expr: &str,
    db: &DuckDbStore,
) -> Result<Option<ImportedTablePayload>, String> {
    let upper = expr.to_uppercase();
    let Some(dt_pos) = upper.find("DATATABLE") else {
        return Ok(None);
    };
    let rest = &expr[dt_pos + "DATATABLE".len()..];
    let Some(open_paren) = rest.find('(') else {
        return Ok(None);
    };
    let body = &rest[open_paren + 1..];

    // Find the opening '{' of rows block
    let Some(rows_brace) = body.find('{') else {
        return Ok(None);
    };
    let header_part = &body[..rows_brace];
    let rows_part = &body[rows_brace + 1..];

    // Parse column pairs: "ColName", TYPE, ...
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut header_tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in header_part.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            cur.push(ch);
        } else if ch == ',' && !in_quotes {
            let t = cur.trim().to_string();
            if !t.is_empty() {
                header_tokens.push(t);
            }
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        header_tokens.push(cur.trim().to_string());
    }

    let mut idx = 0;
    while idx + 1 < header_tokens.len() {
        let cname = header_tokens[idx].trim_matches('"').to_string();
        let ctype_raw = header_tokens[idx + 1].to_uppercase();
        let bi_type = if ctype_raw.contains("INT") || ctype_raw.contains("DOUBLE") || ctype_raw.contains("CURRENCY") {
            "number"
        } else if ctype_raw.contains("BOOL") {
            "boolean"
        } else {
            "text"
        }
        .to_string();
        columns.push((cname, bi_type));
        idx += 2;
    }

    if columns.is_empty() {
        return Ok(None);
    }

    // Parse inner tuples { "val", 1 } inside rows_part
    let mut rows = Vec::new();
    let mut depth = 1i32;
    let mut tuple_buf = String::new();
    let mut in_tuple = false;
    in_quotes = false;

    for ch in rows_part.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            if in_tuple {
                tuple_buf.push(ch);
            }
            continue;
        }
        if !in_quotes {
            if ch == '{' {
                depth += 1;
                if depth == 2 {
                    in_tuple = true;
                    tuple_buf.clear();
                    continue;
                }
            } else if ch == '}' {
                if depth == 2 && in_tuple {
                    in_tuple = false;
                    let vals = split_csv_respecting_quotes(&tuple_buf);
                    let mut map = Map::new();
                    for (c_idx, (cname, bi_type)) in columns.iter().enumerate() {
                        let raw_val = vals.get(c_idx).map(|s| s.trim()).unwrap_or("");
                        let jv = if raw_val.starts_with('"') && raw_val.ends_with('"') && raw_val.len() >= 2 {
                            JsonValue::String(raw_val[1..raw_val.len() - 1].to_string())
                        } else if bi_type == "number" {
                            raw_val
                                .parse::<f64>()
                                .ok()
                                .and_then(serde_json::Number::from_f64)
                                .map(JsonValue::Number)
                                .unwrap_or(JsonValue::Null)
                        } else {
                            JsonValue::String(raw_val.to_string())
                        };
                        map.insert(cname.clone(), jv);
                    }
                    rows.push(map);
                }
                depth -= 1;
                if depth <= 0 {
                    break;
                }
                continue;
            }
        }
        if in_tuple {
            tuple_buf.push(ch);
        }
    }

    let mut payload = db.insert_table_rows(&table.name, &columns, &rows)?;
    payload.table_name = table.name.clone();
    payload.original_name = table.name.clone();
    Ok(Some(payload))
}

fn split_csv_respecting_quotes(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    for ch in s.chars() {
        if ch == '"' {
            in_q = !in_q;
            cur.push(ch);
        } else if ch == ',' && !in_q {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

pub fn resolve_source_path(raw_path: &str, pbip_root: &Path) -> Option<PathBuf> {
    let direct = PathBuf::from(raw_path);
    if direct.is_file() {
        return Some(direct);
    }

    let normalized = raw_path.replace('\\', "/");
    let norm_buf = PathBuf::from(&normalized);
    if norm_buf.is_file() {
        return Some(norm_buf);
    }

    // Handle Windows / RDP \\tsclient\Linux\... paths -> $HOME/...
    let lower_norm = normalized.to_lowercase();
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        if let Some(idx) = lower_norm.find("/linux/") {
            let tail = &normalized[idx + "/linux/".len()..];
            let cand = home_path.join(tail);
            if cand.is_file() {
                return Some(cand);
            }
        }
        // Handle Windows C:\Users\<user>\<tail> -> $HOME/<tail> (including Documents <-> Documentos)
        if let Some(users_idx) = lower_norm.find(":/users/") {
            let after_users = &normalized[users_idx + ":/users/".len()..];
            if let Some(slash_pos) = after_users.find('/') {
                let tail = &after_users[slash_pos + 1..];
                let mut tail_candidates = vec![tail.to_string()];
                if tail.starts_with("Documents/") {
                    tail_candidates.push(tail.replacen("Documents/", "Documentos/", 1));
                } else if tail.starts_with("Documentos/") {
                    tail_candidates.push(tail.replacen("Documentos/", "Documents/", 1));
                }
                for cand_tail in tail_candidates {
                    let cand = home_path.join(cand_tail);
                    if cand.is_file() {
                        return Some(cand);
                    }
                }
            }
        }
    }

    // Try basename inside pbip_root, parent, and common data subdirs
    if let Some(fname) = normalized.rsplit('/').next().filter(|s| !s.is_empty()) {
        let mut dirs_to_check = vec![pbip_root.to_path_buf()];
        if let Some(parent) = pbip_root.parent() {
            dirs_to_check.push(parent.to_path_buf());
        }
        let subdirs = ["data", "dados", "excel", "bases", "arquivos", "sources", "Data", "Dados"];
        let base_dirs = dirs_to_check.clone();
        for base in &base_dirs {
            for sub in &subdirs {
                dirs_to_check.push(base.join(sub));
            }
        }

        for dir in &dirs_to_check {
            let exact = dir.join(fname);
            if exact.is_file() {
                return Some(exact);
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file()
                        && p.file_name()
                            .and_then(|s| s.to_str())
                            .is_some_and(|n| n.eq_ignore_ascii_case(fname))
                    {
                        return Some(p);
                    }
                }
            }
        }

        // Shallow recursive scan (depth <= 2) under pbip_root and parent
        for base in &base_dirs {
            for entry in WalkDir::new(base).max_depth(2).into_iter().flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.eq_ignore_ascii_case(fname))
                {
                    return Some(p.to_path_buf());
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RowFilterOp {
    NotNull,
    NotEmpty,
    Equals(String),
    NotEquals(String),
}

#[derive(Debug, Clone)]
struct RowFilterRule {
    column: String,
    op: RowFilterOp,
}

fn extract_column_list_after(expr: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(pos) = expr.find(marker) else {
        return out;
    };
    let rest = &expr[pos..];
    let Some(open_brace) = rest.find('{') else {
        return out;
    };
    let mut depth = 0i32;
    let mut in_q = false;
    let mut buf = String::new();
    for ch in rest[open_brace..].chars() {
        if ch == '"' {
            in_q = !in_q;
            buf.push(ch);
            continue;
        }
        if !in_q {
            if ch == '{' {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
        buf.push(ch);
    }
    for part in split_csv_respecting_quotes(&buf) {
        let cleaned = part.trim().trim_matches('"').to_string();
        if !cleaned.is_empty() {
            out.push(cleaned);
        }
    }
    out
}

fn extract_removed_columns(expr: &str) -> Vec<String> {
    extract_column_list_after(expr, "Table.RemoveColumns")
}

fn extract_selected_columns(expr: &str) -> Vec<String> {
    extract_column_list_after(expr, "Table.SelectColumns")
}

fn extract_row_filters(expr: &str) -> Vec<RowFilterRule> {
    let mut rules = Vec::new();
    let mut search_start = 0usize;
    while let Some(rel_pos) = expr[search_start..].find("Table.SelectRows") {
        let pos = search_start + rel_pos;
        let rest = &expr[pos..];
        let Some(end_paren) = rest.find(')') else {
            break;
        };
        let clause = &rest[..end_paren];
        // Parse `each [Col] <> null`, `each [Col] <> ""`, `each [Col] = "X"`, `each [Col] <> "X"`
        if let Some(b_start) = clause.find('[') {
            if let Some(b_rel_end) = clause[b_start + 1..].find(']') {
                let col = clause[b_start + 1..b_start + 1 + b_rel_end].trim().to_string();
                let rhs = clause[b_start + 1 + b_rel_end + 1..].trim();
                if let Some(after_ne) = rhs.strip_prefix("<>").map(str::trim) {
                    if after_ne.eq_ignore_ascii_case("null") {
                        rules.push(RowFilterRule {
                            column: col,
                            op: RowFilterOp::NotNull,
                        });
                    } else if after_ne == "\"\"" {
                        rules.push(RowFilterRule {
                            column: col,
                            op: RowFilterOp::NotEmpty,
                        });
                    } else if let Some(val) = after_ne.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                        rules.push(RowFilterRule {
                            column: col,
                            op: RowFilterOp::NotEquals(val.to_string()),
                        });
                    }
                } else if let Some(after_eq) = rhs.strip_prefix('=').map(str::trim) {
                    if let Some(val) = after_eq.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                        rules.push(RowFilterRule {
                            column: col,
                            op: RowFilterOp::Equals(val.to_string()),
                        });
                    }
                }
            }
        }
        search_start = pos + end_paren + 1;
    }
    rules
}

fn passes_row_filters(row: &Map<String, JsonValue>, rules: &[RowFilterRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    for rule in rules {
        let val = row
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&rule.column))
            .map(|(_, v)| v);
        match &rule.op {
            RowFilterOp::NotNull => {
                if val.is_none_or(|v| v.is_null()) {
                    return false;
                }
            }
            RowFilterOp::NotEmpty => {
                let is_empty = match val {
                    None | Some(JsonValue::Null) => true,
                    Some(JsonValue::String(s)) => s.trim().is_empty(),
                    _ => false,
                };
                if is_empty {
                    return false;
                }
            }
            RowFilterOp::Equals(expected) => {
                let actual = match val {
                    Some(JsonValue::String(s)) => s.clone(),
                    Some(JsonValue::Number(n)) => n.to_string(),
                    Some(JsonValue::Bool(b)) => b.to_string(),
                    _ => String::new(),
                };
                if !actual.eq_ignore_ascii_case(expected) {
                    return false;
                }
            }
            RowFilterOp::NotEquals(forbidden) => {
                let actual = match val {
                    Some(JsonValue::String(s)) => s.clone(),
                    Some(JsonValue::Number(n)) => n.to_string(),
                    Some(JsonValue::Bool(b)) => b.to_string(),
                    _ => String::new(),
                };
                if actual.eq_ignore_ascii_case(forbidden) {
                    return false;
                }
            }
        }
    }
    true
}

fn extract_quoted_arg_after(haystack: &str, marker: &str) -> Option<String> {
    let pos = haystack.find(marker)?;
    let rest = &haystack[pos + marker.len()..];
    let q1 = rest.find('"')?;
    let after_q1 = &rest[q1 + 1..];
    let q2 = after_q1.find('"')?;
    Some(after_q1[..q2].to_string())
}

fn extract_excel_sheet_item(expr: &str) -> Option<String> {
    let pos = expr.find("Item=")?;
    let rest = &expr[pos + "Item=".len()..];
    let q1 = rest.find('"')?;
    let after = &rest[q1 + 1..];
    let q2 = after.find('"')?;
    Some(after[..q2].to_string())
}

fn extract_rename_pairs(expr: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(pos) = expr.find("Table.RenameColumns") else {
        return map;
    };
    let rest = &expr[pos..];
    let Some(list_start) = rest.find("{{") else {
        return map;
    };
    let Some(list_end) = rest[list_start..].find("}}") else {
        return map;
    };
    let block = &rest[list_start + 1..list_start + list_end + 1];

    let mut depth = 0i32;
    let mut buf = String::new();
    let mut in_q = false;
    for ch in block.chars() {
        if ch == '"' {
            in_q = !in_q;
            buf.push(ch);
            continue;
        }
        if !in_q {
            if ch == '{' {
                depth += 1;
                if depth == 1 {
                    buf.clear();
                    continue;
                }
            } else if ch == '}' {
                if depth == 1 {
                    let parts = split_csv_respecting_quotes(&buf);
                    if parts.len() == 2 {
                        let old = parts[0].trim().trim_matches('"').to_string();
                        let new = parts[1].trim().trim_matches('"').to_string();
                        map.insert(old, new);
                    }
                }
                depth -= 1;
                continue;
            }
        }
        if depth >= 1 {
            buf.push(ch);
        }
    }
    map
}

fn parse_calendar_params(expr: &str) -> Option<(i32, u32, u32, i32)> {
    let pos = expr.find("#date(")?;
    let rest = &expr[pos + "#date(".len()..];
    let end = rest.find(')')?;
    let date_args: Vec<i32> = rest[..end]
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();
    if date_args.len() != 3 {
        return None;
    }
    let after_date = &rest[end + 1..];
    let count = after_date
        .split(',')
        .nth(1)
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(365)
        .clamp(1, 20000);

    Some((date_args[0], date_args[1] as u32, date_args[2] as u32, count))
}

fn civil_to_days(y: i32, m: u32, d: u32) -> i32 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let d = d as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe as i64 - 719468) as i32
}

fn days_to_civil(days: i32) -> (i32, u32, u32) {
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

#[allow(dead_code)]
fn _keep_sanitize_import(s: &str) -> String {
    sanitize_identifier(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m_parsers_remove_select_and_filter() {
        let m = r#"
let
    Source = Excel.Workbook(File.Contents("C:\Users\Teste\Documents\Rhino\Base.xlsx")),
    Fato = Source{[Item="Fato",Kind="Sheet"]}[Data],
    Removed = Table.RemoveColumns(Fato, {"Obs", "Temp"}),
    Filtered = Table.SelectRows(Removed, each [Status] <> "Cancelado"),
    Selected = Table.SelectColumns(Filtered, {"Data", "Valor", "Status"})
in
    Selected
"#;
        assert_eq!(extract_removed_columns(m), vec!["Obs", "Temp"]);
        assert_eq!(extract_selected_columns(m), vec!["Data", "Valor", "Status"]);
        let filters = extract_row_filters(m);
        assert_eq!(filters.len(), 1);

        let mut row_ok = Map::new();
        row_ok.insert("Status".into(), JsonValue::String("Aprovado".into()));
        assert!(passes_row_filters(&row_ok, &filters));

        let mut row_bad = Map::new();
        row_bad.insert("Status".into(), JsonValue::String("Cancelado".into()));
        assert!(!passes_row_filters(&row_bad, &filters));
    }

    #[test]
    fn test_resolve_source_path_subdirs_and_windows_mapping() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let data_dir = tmp.path().join("dados");
        std::fs::create_dir_all(&data_dir).unwrap();
        let file = data_dir.join("base_fatos.xlsx");
        std::fs::write(&file, b"stub").unwrap();

        let resolved = resolve_source_path(r"D:\Projetos\Empresa\base_fatos.xlsx", tmp.path())
            .expect("should find in dados subdir");
        assert_eq!(resolved, file);
    }

    #[test]
    fn test_refresh_single_table_csv_preserves_target_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let csv_path = tmp.path().join("export_erp.csv");
        std::fs::write(&csv_path, "Mes,Total\nJan,100\nFev,250\n").unwrap();

        let db = DuckDbStore::new_in_memory().expect("init duckdb");
        let req = RefreshTableRequest {
            table_name: "fLancamentos".into(),
            source_type: "csv".into(),
            file_path: Some(csv_path.to_string_lossy().to_string()),
            sheet_name: None,
            m_script: None,
            pbip_root: None,
        };
        let payload = refresh_single_table(&req, &db).expect("refresh csv");
        assert_eq!(payload.table_name, "fLancamentos");
        assert_eq!(payload.total_rows, 2);
        assert_eq!(
            payload.source_path.as_deref(),
            Some(csv_path.to_string_lossy().as_ref())
        );
    }
}
