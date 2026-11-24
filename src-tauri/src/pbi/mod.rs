pub mod m_loader;
pub mod pbip;
pub mod pbip_writer;
pub mod pbix;
pub mod tmdl;

use crate::engine::ImportedTablePayload;
use encoding_rs::{UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiColumnMeta {
    pub name: String,
    pub data_type: String,
    pub format_string: Option<String>,
    pub sort_by_column: Option<String>,
    pub is_hidden: bool,
    pub is_calculated: bool,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiMeasureMeta {
    pub name: String,
    pub table_name: String,
    pub expression: String,
    pub format_string: Option<String>,
    pub display_folder: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiPartitionMeta {
    pub name: String,
    pub mode: String,
    pub source_type: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiTableMeta {
    pub name: String,
    pub columns: Vec<PbiColumnMeta>,
    pub measures: Vec<PbiMeasureMeta>,
    pub partitions: Vec<PbiPartitionMeta>,
    pub is_calculated: bool,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiRelationshipMeta {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub cardinality: String,
    pub cross_filter_direction: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiVisualMeta {
    pub id: String,
    pub visual_type: String,
    pub original_visual_type: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub table_hint: Option<String>,
    pub category_field: Option<String>,
    pub legend_field: Option<String>,
    pub value_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiPageMeta {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub visuals: Vec<PbiVisualMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiPowerQueryScript {
    pub name: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbiProjectSummary {
    pub source_type: String,
    pub project_name: String,
    pub root_path: String,
    pub tables: Vec<PbiTableMeta>,
    pub relationships: Vec<PbiRelationshipMeta>,
    pub pages: Vec<PbiPageMeta>,
    pub power_query_scripts: Vec<PbiPowerQueryScript>,
    #[serde(default)]
    pub loaded_tables: Vec<ImportedTablePayload>,
    pub warnings: Vec<String>,
}

/// Decodes UTF-8 or UTF-16LE/BE (with or without BOM, common in .pbix Report/Layout).
pub fn decode_utf8_or_utf16(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).to_string();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, _) = UTF_16LE.decode(&bytes[2..]);
        return cow.into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, _) = UTF_16BE.decode(&bytes[2..]);
        return cow.into_owned();
    }

    // Heuristic for BOM-less UTF-16LE (standard in Power BI Report/Layout and DataModelSchema)
    if bytes.len() >= 4 && bytes.len() % 2 == 0 {
        let sample_pairs = (bytes.len() / 2).min(64);
        let zero_odd = (0..sample_pairs).filter(|&i| bytes[i * 2 + 1] == 0).count();
        let nonzero_even = (0..sample_pairs).filter(|&i| bytes[i * 2] != 0).count();
        if zero_odd * 10 >= sample_pairs * 4 && nonzero_even * 10 >= sample_pairs * 6 {
            let (cow, _, _) = UTF_16LE.decode(bytes);
            return cow.into_owned();
        }
    }

    String::from_utf8_lossy(bytes).to_string()
}

pub fn map_pbi_visual_type(raw: &str) -> &'static str {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "clusteredcolumnchart" | "columnchart" => "column",
        "stackedcolumnchart" | "hundredpercentstackedcolumnchart" => "stacked-column",
        "clusteredbarChart" | "clustered-barchart" | "clusteredbarchart" | "barchart" => "bar",
        "stackedbarchart" | "hundredpercentstackedbarchart" => "stacked-bar",
        "linechart" => "line",
        "areachart" | "stackedareachart" => "area",
        "lineclusteredcolumncombochart" | "linestackedcolumncombochart" => "combo",
        "piechart" => "pie",
        "donutchart" => "donut",
        "treemap" => "treemap",
        "waterfallchart" => "waterfall",
        "funnel" => "funnel",
        "scatterchart" => "scatter",
        "gauge" => "gauge",
        "card" | "cardvisual" => "card",
        "multirowcard" => "multi-card",
        "kpi" => "kpi",
        "pivottable" | "matrix" => "matrix",
        "tableex" | "table" => "table",
        "slicer" | "advancedslicervisual" => "slicer",
        _ => "column",
    }
}

pub fn map_tmsl_datatype(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "int64" | "double" | "decimal" | "currency" => "number",
        "boolean" => "boolean",
        "datetime" | "date" => "date",
        _ => "text",
    }
}

/// Parses TMSL JSON (`model.bim` or `DataModelSchema`).
pub fn parse_tmsl_model_json(
    json_str: &str,
) -> Result<(Vec<PbiTableMeta>, Vec<PbiRelationshipMeta>), String> {
    let root: JsonValue =
        serde_json::from_str(json_str).map_err(|e| format!("JSON TMSL inválido: {}", e))?;
    let model = root.get("model").unwrap_or(&root);

    let mut tables = Vec::new();
    if let Some(tbl_arr) = model.get("tables").and_then(|v| v.as_array()) {
        for tbl in tbl_arr {
            let name = tbl
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Tabela")
                .to_string();
            // Skip internal Power BI auto-date tables unless useful
            if name.starts_with("DateTableTemplate_") || name.starts_with("LocalDateTable_") {
                continue;
            }
            let is_hidden = tbl.get("isHidden").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut columns = Vec::new();
            if let Some(cols) = tbl.get("columns").and_then(|v| v.as_array()) {
                for c in cols {
                    let col_name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if col_name.is_empty() || col_name.starts_with("RowNumber-") {
                        continue;
                    }
                    let raw_dt = c.get("dataType").and_then(|v| v.as_str()).unwrap_or("string");
                    let expr = extract_json_string_or_lines(c.get("expression"));
                    let col_type = c.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    columns.push(PbiColumnMeta {
                        name: col_name,
                        data_type: map_tmsl_datatype(raw_dt).to_string(),
                        format_string: c
                            .get("formatString")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        sort_by_column: c
                            .get("sortByColumn")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        is_hidden: c.get("isHidden").and_then(|v| v.as_bool()).unwrap_or(false),
                        is_calculated: col_type.eq_ignore_ascii_case("calculated") || expr.is_some(),
                        expression: expr,
                    });
                }
            }

            let mut measures = Vec::new();
            if let Some(ms) = tbl.get("measures").and_then(|v| v.as_array()) {
                for m in ms {
                    let m_name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if m_name.is_empty() {
                        continue;
                    }
                    let expr = extract_json_string_or_lines(m.get("expression")).unwrap_or_default();
                    measures.push(PbiMeasureMeta {
                        name: m_name,
                        table_name: name.clone(),
                        expression: expr,
                        format_string: m
                            .get("formatString")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        display_folder: m
                            .get("displayFolder")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        description: m
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                    });
                }
            }

            let mut partitions = Vec::new();
            let mut is_calculated_table = false;
            if let Some(parts) = tbl.get("partitions").and_then(|v| v.as_array()) {
                for p in parts {
                    let p_name = p.get("name").and_then(|v| v.as_str()).unwrap_or("Partition").to_string();
                    let mode = p.get("mode").and_then(|v| v.as_str()).unwrap_or("import").to_string();
                    let src = p.get("source");
                    let src_type = src
                        .and_then(|s| s.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("m")
                        .to_string();
                    if src_type.eq_ignore_ascii_case("calculated") {
                        is_calculated_table = true;
                    }
                    let expr = src
                        .and_then(|s| extract_json_string_or_lines(s.get("expression")))
                        .unwrap_or_default();
                    partitions.push(PbiPartitionMeta {
                        name: p_name,
                        mode,
                        source_type: src_type,
                        expression: expr,
                    });
                }
            }

            tables.push(PbiTableMeta {
                name,
                columns,
                measures,
                partitions,
                is_calculated: is_calculated_table,
                is_hidden,
            });
        }
    }

    let mut relationships = Vec::new();
    if let Some(rels) = model.get("relationships").and_then(|v| v.as_array()) {
        for r in rels {
            let from_table = r.get("fromTable").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let from_column = r.get("fromColumn").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let to_table = r.get("toTable").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let to_column = r.get("toColumn").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if from_table.is_empty() || to_table.is_empty() {
                continue;
            }
            if from_table.starts_with("LocalDateTable_") || to_table.starts_with("LocalDateTable_") {
                continue;
            }
            let cross = r
                .get("crossFilteringBehavior")
                .and_then(|v| v.as_str())
                .map(|s| {
                    if s.eq_ignore_ascii_case("bothDirections") {
                        "both"
                    } else {
                        "single"
                    }
                })
                .unwrap_or("single")
                .to_string();
            let is_active = r.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true);
            relationships.push(PbiRelationshipMeta {
                from_table,
                from_column,
                to_table,
                to_column,
                cardinality: "N:1".to_string(),
                cross_filter_direction: cross,
                is_active,
            });
        }
    }

    Ok((tables, relationships))
}

fn extract_json_string_or_lines(val: Option<&JsonValue>) -> Option<String> {
    let v = val?;
    if let Some(s) = v.as_str() {
        Some(s.to_string())
    } else if let Some(arr) = v.as_array() {
        let lines: Vec<&str> = arr.iter().filter_map(|x| x.as_str()).collect();
        Some(lines.join("\n"))
    } else {
        None
    }
}

/// Parses Power BI Legacy `Report/Layout` or PBIR-Legacy `report.json` sections.
pub fn parse_legacy_report_json(json_str: &str) -> Result<Vec<PbiPageMeta>, String> {
    let root: JsonValue =
        serde_json::from_str(json_str).map_err(|e| format!("JSON de relatório inválido: {}", e))?;
    let mut pages = Vec::new();

    let Some(sections) = root.get("sections").and_then(|v| v.as_array()) else {
        return Ok(pages);
    };

    for (idx, sec) in sections.iter().enumerate() {
        let page_id = sec
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("page_{}", idx + 1));
        let page_name = sec
            .get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or(&page_id)
            .to_string();
        let width = sec.get("width").and_then(|v| v.as_f64()).unwrap_or(1280.0);
        let height = sec.get("height").and_then(|v| v.as_f64()).unwrap_or(720.0);

        let mut visuals = Vec::new();
        if let Some(containers) = sec.get("visualContainers").and_then(|v| v.as_array()) {
            for (v_idx, vc) in containers.iter().enumerate() {
                let x = vc.get("x").and_then(|v| v.as_f64()).unwrap_or(20.0);
                let y = vc.get("y").and_then(|v| v.as_f64()).unwrap_or(20.0);
                let w = vc.get("width").and_then(|v| v.as_f64()).unwrap_or(320.0);
                let h = vc.get("height").and_then(|v| v.as_f64()).unwrap_or(240.0);

                let config_val: Option<JsonValue> = vc.get("config").and_then(|c| {
                    if let Some(s) = c.as_str() {
                        serde_json::from_str(s).ok()
                    } else if c.is_object() {
                        Some(c.clone())
                    } else {
                        None
                    }
                });

                let single_visual = config_val.as_ref().and_then(|c| c.get("singleVisual"));
                let Some(sv) = single_visual else {
                    // Skip textboxes/shapes without singleVisual or map basic ones? Let's skip shapes/actionButtons cleanly.
                    continue;
                };

                let orig_type = sv
                    .get("visualType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("clusteredColumnChart")
                    .to_string();
                let mapped_type = map_pbi_visual_type(&orig_type).to_string();

                let mut table_hint: Option<String> = None;
                let mut category_field: Option<String> = None;
                let mut legend_field: Option<String> = None;
                let mut value_fields = Vec::new();

                if let Some(projs) = sv.get("projections").and_then(|p| p.as_object()) {
                    for (role, items) in projs {
                        let role_l = role.to_lowercase();
                        let Some(arr) = items.as_array() else { continue };
                        for item in arr {
                            let Some(qref) = item.get("queryRef").and_then(|v| v.as_str()) else {
                                continue;
                            };
                            let (tbl, fld) = split_query_ref(qref);
                            if table_hint.is_none() {
                                table_hint = tbl;
                            }
                            if role_l == "category"
                                || role_l == "rows"
                                || role_l == "values" && orig_type.eq_ignore_ascii_case("slicer")
                                || role_l == "x"
                            {
                                if category_field.is_none() {
                                    category_field = Some(fld);
                                }
                            } else if role_l == "series" || role_l == "columns" || role_l == "legend" {
                                if legend_field.is_none() {
                                    legend_field = Some(fld);
                                }
                            } else if role_l == "y" || role_l == "values" || role_l == "size" {
                                value_fields.push(fld);
                            }
                        }
                    }
                }

                let title = extract_legacy_visual_title(sv)
                    .or_else(|| value_fields.first().cloned())
                    .or_else(|| category_field.clone())
                    .unwrap_or_else(|| format!("Visual {}", v_idx + 1));

                let vid = config_val
                    .as_ref()
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}_v{}", page_id, v_idx + 1));

                visuals.push(PbiVisualMeta {
                    id: vid,
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
                });
            }
        }

        pages.push(PbiPageMeta {
            id: page_id,
            name: page_name,
            width,
            height,
            visuals,
        });
    }

    Ok(pages)
}

fn extract_legacy_visual_title(sv: &JsonValue) -> Option<String> {
    let title_arr = sv
        .get("vcObjects")?
        .get("title")?
        .as_array()?;
    let props = title_arr.first()?.get("properties")?;
    let literal = props
        .get("text")?
        .get("expr")?
        .get("Literal")?
        .get("Value")?
        .as_str()?;
    Some(literal.trim_matches('\'').to_string())
}

pub fn split_query_ref(qref: &str) -> (Option<String>, String) {
    // Handles "Sum(Sales.Amount)" or "Sales.Category"
    let inner = if let Some(start) = qref.find('(') {
        if qref.ends_with(')') {
            &qref[start + 1..qref.len() - 1]
        } else {
            qref
        }
    } else {
        qref
    };

    if let Some((t, c)) = inner.split_once('.') {
        (Some(t.trim().to_string()), c.trim().to_string())
    } else {
        (None, inner.trim().to_string())
    }
}
