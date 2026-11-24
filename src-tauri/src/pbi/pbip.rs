use crate::engine::DuckDbStore;
use crate::pbi::{
    m_loader, map_pbi_visual_type, parse_legacy_report_json, parse_tmsl_model_json, split_query_ref,
    tmdl, PbiFilterMeta, PbiPageMeta, PbiPowerQueryScript, PbiProjectSummary, PbiVisualMeta,
};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn open_pbip_project(input_path: &Path, db: Option<&DuckDbStore>) -> Result<PbiProjectSummary, String> {
    let pbip_file = resolve_pbip_file(input_path)?;
    let root_dir = pbip_file
        .parent()
        .ok_or_else(|| "Diretório pai do .pbip inválido".to_string())?;
    let project_name = pbip_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Projeto Power BI")
        .to_string();

    let pbip_raw = fs::read_to_string(&pbip_file)
        .map_err(|e| format!("Erro ao ler '{}': {}", pbip_file.display(), e))?;
    let pbip_json: JsonValue = serde_json::from_str(&pbip_raw).unwrap_or(JsonValue::Null);

    let (report_dir, semantic_dir) = discover_pbip_folders(root_dir, &project_name, &pbip_json);

    let mut tables = Vec::new();
    let mut relationships = Vec::new();
    let mut pages = Vec::new();
    let mut power_query_scripts = Vec::new();
    let mut warnings = Vec::new();

    if let Some(sem_dir) = &semantic_dir {
        let def_dir = sem_dir.join("definition");
        let tables_dir = def_dir.join("tables");
        if tables_dir.is_dir() {
            for entry in WalkDir::new(&tables_dir).max_depth(2).into_iter().flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("tmdl") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        match tmdl::parse_table_tmdl(&content) {
                            Ok(tbl) => {
                                for p in &tbl.partitions {
                                    if p.source_type.eq_ignore_ascii_case("m")
                                        && !p.expression.is_empty()
                                    {
                                        power_query_scripts.push(PbiPowerQueryScript {
                                            name: tbl.name.clone(),
                                            script: p.expression.clone(),
                                        });
                                    }
                                }
                                if !tbl.name.starts_with("DateTableTemplate_")
                                    && !tbl.name.starts_with("LocalDateTable_")
                                {
                                    tables.push(tbl);
                                }
                            }
                            Err(err) => warnings.push(format!(
                                "TMDL '{}': {}",
                                entry.path().file_name().and_then(|s| s.to_str()).unwrap_or(""),
                                err
                            )),
                        }
                    }
                }
            }
            let rels_file = def_dir.join("relationships.tmdl");
            if rels_file.is_file() {
                if let Ok(rel_content) = fs::read_to_string(&rels_file) {
                    relationships = tmdl::parse_relationships_tmdl(&rel_content)
                        .into_iter()
                        .filter(|r| {
                            !r.from_table.starts_with("LocalDateTable_")
                                && !r.to_table.starts_with("LocalDateTable_")
                                && !r.from_table.starts_with("DateTableTemplate_")
                                && !r.to_table.starts_with("DateTableTemplate_")
                        })
                        .collect();
                }
            }
        } else {
            let model_bim = sem_dir.join("model.bim");
            if model_bim.is_file() {
                let bim_raw = fs::read_to_string(&model_bim)
                    .map_err(|e| format!("Erro ao ler model.bim: {}", e))?;
                let (t, r) = parse_tmsl_model_json(&bim_raw)?;
                tables = t;
                relationships = r;
            } else {
                warnings.push("Pasta SemanticModel encontrada sem definition/tables/*.tmdl ou model.bim.".to_string());
            }
        }
    } else {
        warnings.push("Pasta .SemanticModel não encontrada ao lado do arquivo .pbip.".to_string());
    }

    if let Some(rep_dir) = &report_dir {
        let pbir_pages_dir = rep_dir.join("definition").join("pages");
        if pbir_pages_dir.is_dir() {
            pages = parse_pbir_pages(&pbir_pages_dir);
        } else {
            let legacy_report = rep_dir.join("report.json");
            if legacy_report.is_file() {
                let raw = fs::read_to_string(&legacy_report)
                    .map_err(|e| format!("Erro ao ler report.json: {}", e))?;
                pages = parse_legacy_report_json(&raw)?;
            } else {
                warnings.push("Pasta .Report encontrada sem definition/pages ou report.json.".to_string());
            }
        }
    } else {
        warnings.push("Pasta .Report não encontrada ao lado do arquivo .pbip.".to_string());
    }

    let loaded_tables = if let Some(store) = db {
        m_loader::materialize_pbip_tables(&tables, root_dir, store, &mut warnings)
    } else {
        Vec::new()
    };

    Ok(PbiProjectSummary {
        source_type: "pbip".to_string(),
        project_name,
        root_path: root_dir.to_string_lossy().to_string(),
        tables,
        relationships,
        pages,
        power_query_scripts,
        loaded_tables,
        warnings,
    })
}

fn resolve_pbip_file(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("pbip") {
                    return Ok(p);
                }
            }
        }
    }
    Err(format!("Nenhum arquivo .pbip encontrado em '{}'", path.display()))
}

fn discover_pbip_folders(
    root_dir: &Path,
    project_name: &str,
    pbip_json: &JsonValue,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut report_dir = None;
    let mut semantic_dir = None;

    if let Some(artifacts) = pbip_json.get("artifacts").and_then(|v| v.as_array()) {
        for art in artifacts {
            if let Some(rp) = art.get("report").and_then(|r| r.get("path")).and_then(|v| v.as_str()) {
                let cand = root_dir.join(rp);
                if cand.is_dir() {
                    report_dir = Some(cand);
                }
            }
            if let Some(sp) = art
                .get("semanticModel")
                .and_then(|r| r.get("path"))
                .and_then(|v| v.as_str())
            {
                let cand = root_dir.join(sp);
                if cand.is_dir() {
                    semantic_dir = Some(cand);
                }
            }
        }
    }

    if report_dir.is_none() {
        let default_rep = root_dir.join(format!("{}.Report", project_name));
        if default_rep.is_dir() {
            report_dir = Some(default_rep);
        }
    }
    if semantic_dir.is_none() {
        let default_sem = root_dir.join(format!("{}.SemanticModel", project_name));
        if default_sem.is_dir() {
            semantic_dir = Some(default_sem);
        } else {
            let legacy_dataset = root_dir.join(format!("{}.Dataset", project_name));
            if legacy_dataset.is_dir() {
                semantic_dir = Some(legacy_dataset);
            }
        }
    }

    // Fallback: scan directory for *.Report / *.SemanticModel / *.Dataset
    if report_dir.is_none() || semantic_dir.is_none() {
        if let Ok(entries) = fs::read_dir(root_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if report_dir.is_none() && name.ends_with(".Report") {
                    report_dir = Some(p);
                } else if semantic_dir.is_none()
                    && (name.ends_with(".SemanticModel") || name.ends_with(".Dataset"))
                {
                    semantic_dir = Some(p);
                }
            }
        }
    }

    (report_dir, semantic_dir)
}

fn parse_pbir_pages(pages_dir: &Path) -> Vec<PbiPageMeta> {
    let mut pages = Vec::new();
    let Ok(entries) = fs::read_dir(pages_dir) else {
        return pages;
    };

    let mut page_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    page_dirs.sort();

    for pdir in page_dirs {
        let page_json_path = pdir.join("page.json");
        if !page_json_path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&page_json_path) else {
            continue;
        };
        let Ok(pj) = serde_json::from_str::<JsonValue>(&raw) else {
            continue;
        };

        let id = pj
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                pdir.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("page")
                    .to_string()
            });
        let name = pj
            .get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let width = pj.get("width").and_then(|v| v.as_f64()).unwrap_or(1280.0);
        let height = pj.get("height").and_then(|v| v.as_f64()).unwrap_or(720.0);
        let page_filters = extract_pbir_filters(&pj);

        let mut visuals = Vec::new();
        let visuals_dir = pdir.join("visuals");
        if visuals_dir.is_dir() {
            if let Ok(v_entries) = fs::read_dir(&visuals_dir) {
                let mut v_dirs: Vec<PathBuf> = v_entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                v_dirs.sort();

                for (v_idx, vdir) in v_dirs.into_iter().enumerate() {
                    let vj_path = vdir.join("visual.json");
                    if let Ok(vraw) = fs::read_to_string(&vj_path) {
                        if let Ok(vj) = serde_json::from_str::<JsonValue>(&vraw) {
                            if let Some(vmeta) = parse_pbir_visual_json(&vj, &id, v_idx) {
                                visuals.push(vmeta);
                            }
                        }
                    }
                }
            }
        }

        pages.push(PbiPageMeta {
            id,
            name,
            width,
            height,
            visuals,
            filters: page_filters,
        });
    }

    pages
}

fn parse_pbir_visual_json(vj: &JsonValue, page_id: &str, v_idx: usize) -> Option<PbiVisualMeta> {
    let visual_obj = vj.get("visual")?;
    let orig_type = visual_obj
        .get("visualType")
        .and_then(|v| v.as_str())
        .unwrap_or("clusteredColumnChart")
        .to_string();

    let orig_lower = orig_type.to_lowercase();
    if matches!(
        orig_lower.as_str(),
        "shape" | "image" | "basicshape"
    ) {
        return None;
    }

    let mut mapped_type = map_pbi_visual_type(&orig_type).to_string();

    let pos = vj.get("position");
    let x = pos.and_then(|p| p.get("x")).and_then(|v| v.as_f64()).unwrap_or(20.0);
    let y = pos.and_then(|p| p.get("y")).and_then(|v| v.as_f64()).unwrap_or(20.0);
    let w = pos.and_then(|p| p.get("width")).and_then(|v| v.as_f64()).unwrap_or(320.0);
    let h = pos.and_then(|p| p.get("height")).and_then(|v| v.as_f64()).unwrap_or(240.0);

    let mut table_hint: Option<String> = None;
    let mut category_field: Option<String> = None;
    let mut legend_field: Option<String> = None;
    let mut value_fields = Vec::new();

    if let Some(qstate) = visual_obj
        .get("query")
        .and_then(|q| q.get("queryState"))
        .and_then(|s| s.as_object())
    {
        for (role, role_obj) in qstate {
            let role_l = role.to_lowercase();
            let Some(projs) = role_obj.get("projections").and_then(|p| p.as_array()) else {
                continue;
            };
            for proj in projs {
                let qref = proj.get("queryRef").and_then(|v| v.as_str());
                let extracted = if qref.is_some_and(|q| q.contains(".Variation.Date Hierarchy.")) {
                    qref.map(normalize_pbir_query_ref)
                } else if let Some(field) = proj.get("field") {
                    extract_pbir_field_entity_prop(field)
                        .or_else(|| qref.map(normalize_pbir_query_ref))
                } else {
                    qref.map(normalize_pbir_query_ref)
                };

                let Some((tbl, fld)) = extracted else {
                    continue;
                };

                if let Some(ref t) = tbl {
                    let is_measure_host = t.eq_ignore_ascii_case("medidas") || t.eq_ignore_ascii_case("measures");
                    if table_hint.is_none() || (!is_measure_host && table_hint.as_deref().is_some_and(|cur| cur.eq_ignore_ascii_case("medidas"))) {
                        table_hint = Some(t.clone());
                    }
                }

                let is_slicer = mapped_type == "slicer";
                if role_l == "category"
                    || role_l == "rows"
                    || role_l == "x"
                    || (is_slicer && role_l == "values")
                {
                    if category_field.is_none() {
                        let qualified = match &tbl {
                            Some(t) if !t.eq_ignore_ascii_case("medidas") => format!("{}.{}", t, fld),
                            _ => fld,
                        };
                        category_field = Some(qualified);
                    }
                } else if role_l == "series" || role_l == "columns" || role_l == "legend" {
                    if legend_field.is_none() {
                        let qualified = match &tbl {
                            Some(t) if !t.eq_ignore_ascii_case("medidas") => format!("{}.{}", t, fld),
                            _ => fld,
                        };
                        legend_field = Some(qualified);
                    }
                } else if role_l == "y"
                    || role_l == "y2"
                    || role_l == "values"
                    || role_l == "size"
                    || role_l == "data"
                {
                    value_fields.push(fld);
                }
            }
        }
    }

    if mapped_type == "card" && value_fields.len() > 1 {
        mapped_type = "multi-card".to_string();
    }

    let column_count = extract_pbir_column_count(visual_obj);
    let slicer_mode = match orig_lower.as_str() {
        "advancedslicervisual" => Some("tiles".to_string()),
        "slicer" => Some("dropdown".to_string()),
        _ => None,
    };
    let (action_type, action_target) = if mapped_type == "action-button" {
        extract_pbir_action_link(visual_obj)
    } else {
        (None, None)
    };
    let text_content = if mapped_type == "textbox" {
        extract_pbir_textbox_text(visual_obj)
    } else {
        None
    };
    let filters = extract_pbir_filters(vj);

    let id = vj
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}_v{}", page_id, v_idx + 1));
    let mut title = extract_pbir_visual_title(visual_obj)
        .or_else(|| value_fields.first().cloned())
        .or_else(|| category_field.clone())
        .unwrap_or_else(|| format!("Visual {}", v_idx + 1));

    if mapped_type == "action-button" {
        title = match action_type.as_deref() {
            Some("back") => "Voltar".to_string(),
            Some("page-navigation") => "Navegar".to_string(),
            _ => "Botão".to_string(),
        };
    } else if mapped_type == "textbox" {
        if let Some(ref txt) = text_content {
            let first_line = txt.lines().next().unwrap_or("Texto").trim();
            if !first_line.is_empty() {
                title = first_line.chars().take(48).collect();
            }
        }
    }

    Some(PbiVisualMeta {
        id,
        visual_type: mapped_type,
        original_visual_type: orig_type,
        title,
        x,
        y,
        w,
        h,
        table_hint,
        category_field,
        legend_field,
        value_fields,
        column_count,
        slicer_mode,
        action_type,
        action_target,
        text_content,
        filters,
    })
}

fn strip_pbir_literal(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
        return trimmed[1..trimmed.len() - 1].replace("''", "'");
    }
    if let Some(stripped) = trimmed.strip_suffix('L').or_else(|| trimmed.strip_suffix('D')) {
        if stripped.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.') {
            return stripped.to_string();
        }
    }
    trimmed.to_string()
}

fn extract_pbir_visual_title(visual_obj: &JsonValue) -> Option<String> {
    let titles = visual_obj
        .get("visualContainerObjects")
        .and_then(|o| o.get("title"))
        .and_then(|v| v.as_array())?;
    for item in titles {
        if let Some(raw) = item
            .get("properties")
            .and_then(|p| p.get("text"))
            .and_then(|t| t.get("expr"))
            .and_then(|e| e.get("Literal"))
            .and_then(|l| l.get("Value"))
            .and_then(|v| v.as_str())
        {
            let clean = strip_pbir_literal(raw);
            if !clean.is_empty() {
                return Some(clean);
            }
        }
    }
    None
}

fn extract_pbir_column_count(visual_obj: &JsonValue) -> Option<u32> {
    let layouts = visual_obj
        .get("objects")
        .and_then(|o| o.get("layout"))
        .and_then(|v| v.as_array())?;
    for lay in layouts {
        if let Some(raw) = lay
            .get("properties")
            .and_then(|p| p.get("columnCount"))
            .and_then(|c| c.get("expr"))
            .and_then(|e| e.get("Literal"))
            .and_then(|l| l.get("Value"))
            .and_then(|v| v.as_str())
        {
            if let Ok(parsed) = strip_pbir_literal(raw).parse::<u32>() {
                if parsed > 0 {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

fn extract_pbir_action_link(visual_obj: &JsonValue) -> (Option<String>, Option<String>) {
    let Some(links) = visual_obj
        .get("visualContainerObjects")
        .and_then(|o| o.get("visualLink"))
        .and_then(|v| v.as_array())
    else {
        return (None, None);
    };

    let mut action_type = None;
    let mut action_target = None;

    for link in links {
        let Some(props) = link.get("properties") else {
            continue;
        };
        if let Some(raw_type) = props
            .get("type")
            .and_then(|t| t.get("expr"))
            .and_then(|e| e.get("Literal"))
            .and_then(|l| l.get("Value"))
            .and_then(|v| v.as_str())
        {
            let clean = strip_pbir_literal(raw_type).to_lowercase();
            action_type = Some(match clean.as_str() {
                "back" => "back".to_string(),
                "pagenavigation" | "page-navigation" => "page-navigation".to_string(),
                other => other.to_string(),
            });
        }
        if let Some(raw_target) = props
            .get("navigationSection")
            .and_then(|t| t.get("expr"))
            .and_then(|e| e.get("Literal"))
            .and_then(|l| l.get("Value"))
            .and_then(|v| v.as_str())
        {
            let clean = strip_pbir_literal(raw_target);
            if !clean.is_empty() {
                action_target = Some(clean);
            }
        }
    }

    (action_type, action_target)
}

fn extract_pbir_textbox_text(visual_obj: &JsonValue) -> Option<String> {
    let general = visual_obj
        .get("objects")
        .and_then(|o| o.get("general"))
        .and_then(|v| v.as_array())?;
    let mut lines = Vec::new();
    for item in general {
        let Some(paragraphs) = item
            .get("properties")
            .and_then(|p| p.get("paragraphs"))
            .and_then(|p| p.as_array())
        else {
            continue;
        };
        for para in paragraphs {
            let mut para_text = String::new();
            if let Some(runs) = para.get("textRuns").and_then(|r| r.as_array()) {
                for run in runs {
                    if let Some(val) = run.get("value").and_then(|v| v.as_str()) {
                        para_text.push_str(val);
                    }
                }
            }
            if !para_text.is_empty() {
                lines.push(para_text);
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn extract_pbir_filters(container: &JsonValue) -> Vec<PbiFilterMeta> {
    let mut out = Vec::new();
    let Some(filters) = container
        .get("filterConfig")
        .and_then(|fc| fc.get("filters"))
        .and_then(|f| f.as_array())
    else {
        return out;
    };

    for f in filters {
        let Some((Some(table), column)) = f.get("field").and_then(extract_pbir_field_entity_prop)
        else {
            continue;
        };
        let Some(where_arr) = f
            .get("filter")
            .and_then(|flt| flt.get("Where"))
            .and_then(|w| w.as_array())
        else {
            continue;
        };
        let mut values = Vec::new();
        for clause in where_arr {
            let Some(cond) = clause.get("Condition") else {
                continue;
            };
            if let Some(in_vals) = cond
                .get("In")
                .and_then(|i| i.get("Values"))
                .and_then(|v| v.as_array())
            {
                for row in in_vals {
                    let Some(tuple) = row.as_array() else {
                        continue;
                    };
                    if let Some(lit) = tuple
                        .first()
                        .and_then(|cell| cell.get("Literal"))
                        .and_then(|l| l.get("Value"))
                        .and_then(|v| v.as_str())
                    {
                        values.push(strip_pbir_literal(lit));
                    }
                }
            } else if let Some(cmp) = cond.get("Comparison") {
                if cmp.get("ComparisonKind").and_then(|k| k.as_i64()) == Some(0) {
                    if let Some(lit) = cmp
                        .get("Right")
                        .and_then(|r| r.get("Literal"))
                        .and_then(|l| l.get("Value"))
                        .and_then(|v| v.as_str())
                    {
                        values.push(strip_pbir_literal(lit));
                    }
                }
            }
        }
        if !values.is_empty() {
            out.push(PbiFilterMeta {
                table,
                column,
                values,
            });
        }
    }

    out
}

fn normalize_pbir_query_ref(qref: &str) -> (Option<String>, String) {
    // Handle Date Hierarchy refs like "dCalendario.Data.Variation.Date Hierarchy.Year"
    if qref.contains(".Variation.Date Hierarchy.") {
        let mut parts = qref.split('.');
        let tbl = parts.next().map(|s| s.trim().to_string());
        let level = qref.rsplit('.').next().unwrap_or("Data").trim();
        let mapped_col = match level.to_lowercase().as_str() {
            "year" => "Ano",
            "month" => "Mês",
            "day" => "Dia",
            "quarter" => "Semestre",
            _ => "Data",
        };
        return (tbl, mapped_col.to_string());
    }
    split_query_ref(qref)
}

fn extract_pbir_field_entity_prop(field: &JsonValue) -> Option<(Option<String>, String)> {
    let node = field
        .get("Column")
        .or_else(|| field.get("Measure"))?;
    let entity = node
        .get("Expression")
        .and_then(|e| e.get("SourceRef"))
        .and_then(|s| s.get("Entity"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let prop = node.get("Property").and_then(|v| v.as_str())?.to_string();
    Some((entity, prop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_pbip_project_tmdl_and_pbir() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let pbip_path = root.join("Vendas.pbip");
        fs::write(
            &pbip_path,
            r#"{"version":"1.0","artifacts":[{"report":{"path":"Vendas.Report"}},{"semanticModel":{"path":"Vendas.SemanticModel"}}]}"#,
        )
        .unwrap();

        let tables_dir = root.join("Vendas.SemanticModel/definition/tables");
        fs::create_dir_all(&tables_dir).unwrap();
        fs::write(
            tables_dir.join("f_Vendas.tmdl"),
            "table f_Vendas\n\tmeasure Receita = SUM(f_Vendas[Valor])\n\tcolumn Categoria\n\t\tdataType: string\n\tcolumn Valor\n\t\tdataType: double\n",
        )
        .unwrap();

        let visuals_dir = root.join("Vendas.Report/definition/pages/page1/visuals/vis1");
        fs::create_dir_all(&visuals_dir).unwrap();
        fs::write(
            root.join("Vendas.Report/definition/pages/page1/page.json"),
            r#"{"name":"page1","displayName":"Visão Geral","width":1280,"height":720}"#,
        )
        .unwrap();
        fs::write(
            visuals_dir.join("visual.json"),
            r#"{"name":"vis1","position":{"x":40,"y":50,"width":500,"height":300},"visual":{"visualType":"pivotTable","query":{"queryState":{"Rows":{"projections":[{"queryRef":"f_Vendas.Categoria"}]},"Values":{"projections":[{"queryRef":"f_Vendas.Receita"}]}}}}}"#,
        )
        .unwrap();

        let summary = open_pbip_project(&pbip_path, None).expect("open pbip");
        assert_eq!(summary.source_type, "pbip");
        assert_eq!(summary.project_name, "Vendas");
        assert_eq!(summary.tables.len(), 1);
        assert_eq!(summary.tables[0].measures.len(), 1);
        assert_eq!(summary.pages.len(), 1);
        assert_eq!(summary.pages[0].name, "Visão Geral");
        assert_eq!(summary.pages[0].visuals.len(), 1);
        assert_eq!(summary.pages[0].visuals[0].visual_type, "matrix");
        assert_eq!(summary.pages[0].visuals[0].category_field.as_deref(), Some("f_Vendas.Categoria"));
        assert_eq!(summary.pages[0].visuals[0].value_fields, vec!["Receita"]);
    }

    #[test]
    fn test_pbir_prefers_field_property_over_stale_query_ref_and_keeps_date_hierarchy() {
        let vj: JsonValue = serde_json::json!({
            "name": "vis_renamed",
            "position": { "x": 10, "y": 20, "width": 400, "height": 200 },
            "visual": {
                "visualType": "cardVisual",
                "query": {
                    "queryState": {
                        "Category": {
                            "projections": [
                                {
                                    "field": {
                                        "Column": {
                                            "Expression": { "SourceRef": { "Entity": "dCalendario" } },
                                            "Property": "Data"
                                        }
                                    },
                                    "queryRef": "dCalendario.Data.Variation.Date Hierarchy.Year",
                                    "nativeQueryRef": "Data Year"
                                }
                            ]
                        },
                        "Data": {
                            "projections": [
                                {
                                    "field": {
                                        "Measure": {
                                            "Expression": { "SourceRef": { "Entity": "Medidas" } },
                                            "Property": "Salários"
                                        }
                                    },
                                    "queryRef": "Medidas.Despesas com Pessoal",
                                    "nativeQueryRef": "Salários"
                                }
                            ]
                        }
                    }
                }
            }
        });

        let meta = parse_pbir_visual_json(&vj, "page1", 0).expect("parsed visual");
        assert_eq!(meta.category_field.as_deref(), Some("dCalendario.Ano"));
        assert_eq!(meta.value_fields, vec!["Salários"]);
    }

    #[test]
    fn test_pbir_v03_column_count_tiles_slicer_action_button_and_filters() {
        let card_vj: JsonValue = serde_json::json!({
            "name": "vis_card3",
            "position": { "x": 0, "y": 0, "width": 600, "height": 180 },
            "visual": {
                "visualType": "cardVisual",
                "query": {
                    "queryState": {
                        "Data": {
                            "projections": [
                                { "field": { "Measure": { "Expression": { "SourceRef": { "Entity": "Medidas" } }, "Property": "A" } } },
                                { "field": { "Measure": { "Expression": { "SourceRef": { "Entity": "Medidas" } }, "Property": "B" } } },
                                { "field": { "Measure": { "Expression": { "SourceRef": { "Entity": "Medidas" } }, "Property": "C" } } }
                            ]
                        }
                    }
                },
                "objects": {
                    "layout": [
                        { "properties": { "columnCount": { "expr": { "Literal": { "Value": "3L" } } } } }
                    ]
                }
            },
            "filterConfig": {
                "filters": [
                    {
                        "field": { "Column": { "Expression": { "SourceRef": { "Entity": "dCalendario" } }, "Property": "Ano" } },
                        "filter": {
                            "Where": [
                                { "Condition": { "In": { "Values": [[{ "Literal": { "Value": "2026L" } }]] } } }
                            ]
                        }
                    }
                ]
            }
        });
        let card_meta = parse_pbir_visual_json(&card_vj, "p1", 0).expect("cardVisual");
        assert_eq!(card_meta.visual_type, "multi-card");
        assert_eq!(card_meta.column_count, Some(3));
        assert_eq!(card_meta.filters.len(), 1);
        assert_eq!(card_meta.filters[0].table, "dCalendario");
        assert_eq!(card_meta.filters[0].column, "Ano");
        assert_eq!(card_meta.filters[0].values, vec!["2026"]);

        let slicer_vj: JsonValue = serde_json::json!({
            "name": "vis_tiles",
            "position": { "x": 0, "y": 0, "width": 300, "height": 80 },
            "visual": {
                "visualType": "advancedSlicerVisual",
                "query": {
                    "queryState": {
                        "Values": {
                            "projections": [
                                { "field": { "Column": { "Expression": { "SourceRef": { "Entity": "dCalendario" } }, "Property": "Mês" } } }
                            ]
                        }
                    }
                }
            }
        });
        let slicer_meta = parse_pbir_visual_json(&slicer_vj, "p1", 1).expect("advancedSlicer");
        assert_eq!(slicer_meta.visual_type, "slicer");
        assert_eq!(slicer_meta.slicer_mode.as_deref(), Some("tiles"));

        let btn_vj: JsonValue = serde_json::json!({
            "name": "vis_btn",
            "position": { "x": 0, "y": 0, "width": 100, "height": 40 },
            "visual": {
                "visualType": "actionButton",
                "visualContainerObjects": {
                    "visualLink": [
                        {
                            "properties": {
                                "type": { "expr": { "Literal": { "Value": "'Back'" } } }
                            }
                        }
                    ]
                }
            }
        });
        let btn_meta = parse_pbir_visual_json(&btn_vj, "p1", 2).expect("actionButton");
        assert_eq!(btn_meta.visual_type, "action-button");
        assert_eq!(btn_meta.action_type.as_deref(), Some("back"));
    }
}
