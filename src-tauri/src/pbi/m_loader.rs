use crate::engine::duckdb_store::{
    calamine_cell_to_json, format_days_since_epoch, sanitize_identifier, DuckDbStore,
    ImportedTablePayload,
};
use crate::pbi::PbiTableMeta;
use calamine::{open_workbook_auto, Data, Reader};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Materializes `.pbip` model tables into DuckDB by interpreting Power Query M and DAX `DATATABLE` partitions.
pub fn materialize_pbip_tables(
    tables: &[PbiTableMeta],
    pbip_root: &Path,
    db: &DuckDbStore,
    warnings: &mut Vec<String>,
) -> Vec<ImportedTablePayload> {
    let mut loaded = Vec::new();

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

        // 1. Calculated DATATABLE(...) partition (e.g. dEstrutura)
        if part.source_type.eq_ignore_ascii_case("calculated") || expr.to_uppercase().contains("DATATABLE") {
            match parse_and_load_datatable(table, expr, db) {
                Ok(Some(payload)) => loaded.push(payload),
                Ok(None) => {}
                Err(e) => warnings.push(format!("Tabela calculada '{}': {}", table.name, e)),
            }
            continue;
        }

        // 2. Empty Measures Host Table (#table(1, ...) + Table.RemoveColumns)
        if expr.contains("Table.RemoveColumns") && expr.contains("#table(") {
            continue;
        }

        // 3. Calendar M partition (Query1(#date(Y, M, D), count, #duration(...)) or List.Dates)
        if (expr.contains("#date(") && expr.contains("#duration(")) || expr.contains("List.Dates") {
            match generate_and_load_calendar(table, expr, db) {
                Ok(payload) => loaded.push(payload),
                Err(e) => warnings.push(format!("Calendário '{}': {}", table.name, e)),
            }
            continue;
        }

        // 4. Excel.Workbook(File.Contents("..."))
        if expr.contains("Excel.Workbook") && expr.contains("File.Contents") {
            match load_excel_partition(table, expr, pbip_root, db) {
                Ok(payload) => loaded.push(payload),
                Err(e) => warnings.push(format!("Excel '{}': {}", table.name, e)),
            }
            continue;
        }

        // 5. Csv.Document(File.Contents("..."))
        if expr.contains("Csv.Document") && expr.contains("File.Contents") {
            if let Some(raw_path) = extract_quoted_arg_after(expr, "File.Contents(") {
                if let Some(resolved) = resolve_source_path(&raw_path, pbip_root) {
                    match db.import_csv(&resolved, Some(&table.name)) {
                        Ok(mut payload) => {
                            payload.table_name = table.name.clone();
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

    loaded
}

fn load_excel_partition(
    table: &PbiTableMeta,
    expr: &str,
    pbip_root: &Path,
    db: &DuckDbStore,
) -> Result<ImportedTablePayload, String> {
    let raw_path = extract_quoted_arg_after(expr, "File.Contents(")
        .ok_or_else(|| "Caminho File.Contents não encontrado na partição M.".to_string())?;
    let resolved = resolve_source_path(&raw_path, pbip_root).ok_or_else(|| {
        format!(
            "Arquivo Excel '{}' não encontrado em '{}'.",
            raw_path,
            pbip_root.display()
        )
    })?;

    let target_sheet = extract_excel_sheet_item(expr);
    let renames = extract_rename_pairs(expr);

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
    let mut seen = std::collections::HashSet::new();
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
        if any_non_null {
            json_rows.push(map);
        }
    }

    // Add any missing TMDL columns (e.g. optional columns not present in Excel) as null
    let mut final_schema = columns_schema;
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

    // Handle Windows / RDP \\tsclient\Linux\... paths -> $HOME/...
    let normalized = raw_path.replace('\\', "/");
    if let Some(idx) = normalized.to_lowercase().find("/linux/") {
        let tail = &normalized[idx + "/linux/".len()..];
        if let Ok(home) = std::env::var("HOME") {
            let cand = PathBuf::from(home).join(tail);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }

    // Try basename inside pbip_root or parent
    if let Some(fname) = normalized.rsplit('/').next().filter(|s| !s.is_empty()) {
        let in_root = pbip_root.join(fname);
        if in_root.is_file() {
            return Some(in_root);
        }
        if let Some(parent) = pbip_root.parent() {
            let in_parent = parent.join(fname);
            if in_parent.is_file() {
                return Some(in_parent);
            }
        }
        // Case-insensitive match inside pbip_root
        if let Ok(entries) = std::fs::read_dir(pbip_root) {
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

    None
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
