use crate::engine::DuckDbStore;
use crate::pbi::{
    m_loader, map_pbi_visual_type, parse_legacy_report_json, parse_tmsl_model_json, split_query_ref,
    tmdl, PbiPageMeta, PbiPowerQueryScript, PbiProjectSummary, PbiVisualMeta,
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
        "actionbutton" | "textbox" | "shape" | "image" | "basicshape"
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

    let id = vj
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}_v{}", page_id, v_idx + 1));
    let title = value_fields
        .first()
        .cloned()
        .or_else(|| category_field.clone())
        .unwrap_or_else(|| format!("Visual {}", v_idx + 1));

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
    })
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
}
