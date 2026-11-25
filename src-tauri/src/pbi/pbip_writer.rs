use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportColumnPayload {
    pub name: String,
    pub data_type: String,
    pub format_string: Option<String>,
    pub sort_by_column: Option<String>,
    pub is_hidden: Option<bool>,
    pub is_calculated: Option<bool>,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportMeasurePayload {
    pub name: String,
    pub table_name: String,
    pub formula: String,
    pub format_string: Option<String>,
    pub display_folder: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFilterPayload {
    pub table: String,
    pub column: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTablePayload {
    pub name: String,
    pub columns: Vec<ExportColumnPayload>,
    pub is_hidden: Option<bool>,
    #[serde(default)]
    pub m_script: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRelationshipPayload {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub cross_filter_direction: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportVisualPayload {
    pub id: String,
    pub visual_type: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub table_id: String,
    pub x_axis_field: Option<String>,
    pub legend_field: Option<String>,
    pub y_axis_fields: Vec<String>,
    #[serde(default)]
    pub card_columns: Option<u32>,
    #[serde(default)]
    pub slicer_mode: Option<String>,
    #[serde(default)]
    pub text_content: Option<String>,
    #[serde(default)]
    pub action_type: Option<String>,
    #[serde(default)]
    pub action_target: Option<String>,
    #[serde(default)]
    pub show_title: Option<bool>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub filters: Vec<ExportFilterPayload>,
    /// PrismBI `dataConfig.sortField` (column or measure name, optionally "Tabela.Coluna").
    #[serde(default)]
    pub sort_field: Option<String>,
    /// PrismBI `dataConfig.sortOrder` ("asc" | "desc").
    #[serde(default)]
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPagePayload {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub visuals: Vec<ExportVisualPayload>,
    #[serde(default)]
    pub filters: Vec<ExportFilterPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPbipPayload {
    pub project_name: String,
    pub tables: Vec<ExportTablePayload>,
    pub relationships: Vec<ExportRelationshipPayload>,
    pub measures: Vec<ExportMeasurePayload>,
    pub pages: Vec<ExportPagePayload>,
    #[serde(default)]
    pub theme_name: Option<String>,
    #[serde(default)]
    pub theme_palette: Option<Vec<String>>,
}

pub fn save_pbip_project(target_dir: &Path, payload: &ExportPbipPayload) -> Result<String, String> {
    let safe_project = sanitize_fs_name(&payload.project_name);
    let root = target_dir;
    fs::create_dir_all(root).map_err(|e| format!("Falha ao criar pasta '{}': {}", root.display(), e))?;

    let report_folder_name = format!("{}.Report", safe_project);
    let model_folder_name = format!("{}.SemanticModel", safe_project);

    // 1. Write <Project>.pbip
    let pbip_path = root.join(format!("{}.pbip", safe_project));
    let pbip_json = json!({
        "$schema": "https://developer.microsoft.com/json-schemas/fabric/pbip/pbipProperties/1.0.0/schema.json",
        "version": "1.0",
        "artifacts": [
            { "report": { "path": report_folder_name } },
            { "semanticModel": { "path": model_folder_name } }
        ],
        "settings": { "enableAutoRecovery": true }
    });
    fs::write(&pbip_path, serde_json::to_string_pretty(&pbip_json).unwrap())
        .map_err(|e| format!("Erro ao gravar .pbip: {}", e))?;

    // 2. Write SemanticModel (TMDL)
    let sem_dir = root.join(&model_folder_name);
    let def_dir = sem_dir.join("definition");
    let tables_dir = def_dir.join("tables");
    fs::create_dir_all(&tables_dir).map_err(|e| format!("Erro ao criar pasta tables: {}", e))?;

    fs::write(
        sem_dir.join("definition.pbism"),
        serde_json::to_string_pretty(&json!({ "version": "4.0" })).unwrap(),
    )
    .map_err(|e| format!("Erro ao gravar definition.pbism: {}", e))?;

    fs::write(
        def_dir.join("model.tmdl"),
        format!("model Model\n\tculture: pt-BR\n\tdefaultPowerBIDataSourceVersion: powerBI_V3\n"),
    )
    .map_err(|e| format!("Erro ao gravar model.tmdl: {}", e))?;

    for tbl in &payload.tables {
        let mut tmdl = String::new();
        tmdl.push_str(&format!("table {}\n", quote_tmdl_ident(&tbl.name)));
        if tbl.is_hidden.unwrap_or(false) {
            tmdl.push_str("\tisHidden\n");
        }
        tmdl.push('\n');

        // Measures belonging to this table
        for m in payload
            .measures
            .iter()
            .filter(|m| m.table_name.eq_ignore_ascii_case(&tbl.name))
        {
            tmdl.push_str(&format!("\tmeasure {} =\n\t\t\t```\n", quote_tmdl_ident(&m.name)));
            for line in m.formula.lines() {
                tmdl.push_str(&format!("\t\t\t{}\n", line));
            }
            tmdl.push_str("\t\t\t```\n");
            if let Some(fs_str) = &m.format_string {
                tmdl.push_str(&format!("\t\tformatString: {}\n", fs_str));
            }
            if let Some(df) = &m.display_folder {
                tmdl.push_str(&format!("\t\tdisplayFolder: {}\n", df));
            }
            tmdl.push('\n');
        }

        // Columns
        for col in &tbl.columns {
            if col.is_calculated.unwrap_or(false) {
                if let Some(expr) = &col.expression {
                    tmdl.push_str(&format!("\tcolumn {} = {}\n", quote_tmdl_ident(&col.name), expr));
                } else {
                    tmdl.push_str(&format!("\tcolumn {}\n", quote_tmdl_ident(&col.name)));
                }
            } else {
                tmdl.push_str(&format!("\tcolumn {}\n", quote_tmdl_ident(&col.name)));
            }
            tmdl.push_str(&format!("\t\tdataType: {}\n", map_bi_type_to_tmdl(&col.data_type)));
            if let Some(fs_str) = &col.format_string {
                tmdl.push_str(&format!("\t\tformatString: {}\n", fs_str));
            }
            if let Some(sbc) = &col.sort_by_column {
                tmdl.push_str(&format!("\t\tsortByColumn: {}\n", quote_tmdl_ident(sbc)));
            }
            if col.is_hidden.unwrap_or(false) {
                tmdl.push_str("\t\tisHidden\n");
            }
            tmdl.push('\n');
        }

        if let Some(ref m_code) = tbl.m_script.as_deref().filter(|s| !s.trim().is_empty()) {
            tmdl.push_str(&format!("\tpartition {} = m\n\t\tmode: import\n\t\tsource =\n", quote_tmdl_ident(&tbl.name)));
            for line in m_code.lines() {
                tmdl.push_str(&format!("\t\t\t\t{}\n", line));
            }
            tmdl.push('\n');
        }

        let tfile = tables_dir.join(format!("{}.tmdl", sanitize_fs_name(&tbl.name)));
        fs::write(&tfile, tmdl).map_err(|e| format!("Erro ao gravar '{}': {}", tfile.display(), e))?;
    }

    // Relationships
    if !payload.relationships.is_empty() {
        let mut rel_tmdl = String::new();
        for (idx, rel) in payload.relationships.iter().enumerate() {
            rel_tmdl.push_str(&format!("relationship rel_{}\n", idx + 1));
            rel_tmdl.push_str(&format!(
                "\tfromColumn: {}.{}\n",
                quote_tmdl_ident(&rel.from_table),
                quote_tmdl_ident(&rel.from_column)
            ));
            rel_tmdl.push_str(&format!(
                "\ttoColumn: {}.{}\n",
                quote_tmdl_ident(&rel.to_table),
                quote_tmdl_ident(&rel.to_column)
            ));
            if rel.cross_filter_direction.eq_ignore_ascii_case("both") {
                rel_tmdl.push_str("\tcrossFilteringBehavior: bothDirections\n");
            }
            if !rel.is_active {
                rel_tmdl.push_str("\tisActive: false\n");
            }
            rel_tmdl.push('\n');
        }
        fs::write(def_dir.join("relationships.tmdl"), rel_tmdl)
            .map_err(|e| format!("Erro ao gravar relationships.tmdl: {}", e))?;
    }

    // 3. Write Report (PBIR)
    let rep_dir = root.join(&report_folder_name);
    let rep_def_dir = rep_dir.join("definition");
    let pages_dir = rep_def_dir.join("pages");
    fs::create_dir_all(&pages_dir).map_err(|e| format!("Erro ao criar pasta pages: {}", e))?;

    fs::write(
        rep_dir.join("definition.pbir"),
        serde_json::to_string_pretty(&json!({
            "version": "4.0",
            "datasetReference": { "byPath": { "path": format!("../{}", model_folder_name) } }
        }))
        .unwrap(),
    )
    .map_err(|e| format!("Erro ao gravar definition.pbir: {}", e))?;

    if let Some(ref palette) = payload.theme_palette.as_ref().filter(|p| !p.is_empty()) {
        let res_dir = rep_dir.join("StaticResources").join("RegisteredResources");
        fs::create_dir_all(&res_dir).map_err(|e| format!("Erro ao criar StaticResources: {}", e))?;
        let theme_json = json!({
            "name": payload.theme_name.clone().unwrap_or_else(|| "PrismTheme".to_string()),
            "dataColors": palette,
        });
        fs::write(
            res_dir.join("PrismTheme.json"),
            serde_json::to_string_pretty(&theme_json).unwrap(),
        )
        .map_err(|e| format!("Erro ao gravar PrismTheme.json: {}", e))?;

        let report_json = json!({
            "themeCollection": {
                "customTheme": {
                    "name": "PrismTheme.json",
                    "type": "RegisteredResources"
                }
            }
        });
        fs::write(
            rep_def_dir.join("report.json"),
            serde_json::to_string_pretty(&report_json).unwrap(),
        )
        .map_err(|e| format!("Erro ao gravar report.json: {}", e))?;
    }

    let page_ids: Vec<String> = payload
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let fallback = format!("page_{}", i + 1);
            sanitize_fs_name(if p.id.is_empty() { &fallback } else { &p.id })
        })
        .collect();

    fs::write(
        pages_dir.join("pages.json"),
        serde_json::to_string_pretty(&json!({
            "pageOrder": page_ids,
            "activePageName": page_ids.first().cloned().unwrap_or_default()
        }))
        .unwrap(),
    )
    .map_err(|e| format!("Erro ao gravar pages.json: {}", e))?;

    for (p_idx, page) in payload.pages.iter().enumerate() {
        let pid = &page_ids[p_idx];
        let pdir = pages_dir.join(pid);
        let vdir_root = pdir.join("visuals");
        fs::create_dir_all(&vdir_root).map_err(|e| format!("Erro ao criar visuals dir: {}", e))?;

        let mut page_json = serde_json::Map::new();
        page_json.insert("name".into(), json!(pid));
        page_json.insert("displayName".into(), json!(page.name));
        page_json.insert("width".into(), json!(page.width));
        page_json.insert("height".into(), json!(page.height));
        if !page.filters.is_empty() {
            page_json.insert("filterConfig".into(), build_pbir_filter_config(&page.filters));
        }

        fs::write(
            pdir.join("page.json"),
            serde_json::to_string_pretty(&serde_json::Value::Object(page_json)).unwrap(),
        )
        .map_err(|e| format!("Erro ao gravar page.json: {}", e))?;

        for (v_idx, vis) in page.visuals.iter().enumerate() {
            let fallback_vid = format!("vis_{}", v_idx + 1);
            let vid = sanitize_fs_name(if vis.id.is_empty() { &fallback_vid } else { &vis.id });
            let vdir = vdir_root.join(&vid);
            fs::create_dir_all(&vdir).map_err(|e| format!("Erro ao criar visual dir: {}", e))?;

            let pbi_vtype = if vis.visual_type == "slicer"
                && vis.slicer_mode.as_deref() == Some("tiles")
            {
                "advancedSlicerVisual"
            } else {
                map_prism_to_pbi_visual_type(&vis.visual_type)
            };
            let mut query_state = serde_json::Map::new();

            if let Some(cat) = &vis.x_axis_field {
                let qref = if cat.contains('.') {
                    cat.clone()
                } else {
                    format!("{}.{}", vis.table_id, cat)
                };
                let role = if vis.visual_type == "matrix" {
                    "Rows"
                } else if vis.visual_type == "slicer" {
                    "Values"
                } else {
                    "Category"
                };
                query_state.insert(role.to_string(), json!({ "projections": [{ "queryRef": qref }] }));
            }

            if let Some(leg) = &vis.legend_field {
                let qref = if leg.contains('.') {
                    leg.clone()
                } else {
                    format!("{}.{}", vis.table_id, leg)
                };
                let role = if vis.visual_type == "matrix" { "Columns" } else { "Series" };
                query_state.insert(role.to_string(), json!({ "projections": [{ "queryRef": qref }] }));
            }

            if !vis.y_axis_fields.is_empty() {
                let projs: Vec<_> = vis
                    .y_axis_fields
                    .iter()
                    .map(|f| {
                        let measure_tbl = payload
                            .measures
                            .iter()
                            .find(|m| m.name.eq_ignore_ascii_case(f))
                            .map(|m| m.table_name.as_str())
                            .unwrap_or(&vis.table_id);
                        json!({ "queryRef": format!("{}.{}", measure_tbl, f) })
                    })
                    .collect();
                let role = if vis.visual_type == "card" || vis.visual_type == "multi-card" {
                    "Data"
                } else if vis.visual_type == "matrix" || vis.visual_type == "table" {
                    "Values"
                } else {
                    "Y"
                };
                query_state.insert(role.to_string(), json!({ "projections": projs }));
            }

            let mut visual_inner = serde_json::Map::new();
            visual_inner.insert("visualType".into(), json!(pbi_vtype));
            if !query_state.is_empty() {
                let mut query_obj = serde_json::Map::new();
                query_obj.insert("queryState".into(), serde_json::Value::Object(query_state));
                if let Some(sort_def) = build_sort_definition(vis, &payload.measures) {
                    query_obj.insert("sortDefinition".into(), sort_def);
                }
                visual_inner.insert("query".into(), serde_json::Value::Object(query_obj));
            }

            let mut objects_map = serde_json::Map::new();
            if let Some(cc) = vis.card_columns.filter(|&c| c > 0) {
                objects_map.insert(
                    "layout".into(),
                    json!([{
                        "properties": {
                            "columnCount": { "expr": { "Literal": { "Value": format!("{}L", cc) } } }
                        }
                    }]),
                );
            }
            if vis.visual_type == "textbox" {
                let txt = vis.text_content.as_deref().unwrap_or(&vis.title);
                objects_map.insert(
                    "general".into(),
                    json!([{
                        "properties": {
                            "paragraphs": [{ "textRuns": [{ "value": txt }] }]
                        }
                    }]),
                );
            }
            if !objects_map.is_empty() {
                visual_inner.insert("objects".into(), serde_json::Value::Object(objects_map));
            }

            let mut container_objects = serde_json::Map::new();
            if !vis.title.trim().is_empty() {
                let escaped_title = vis.title.replace('\'', "''");
                container_objects.insert(
                    "title".into(),
                    json!([{
                        "properties": {
                            "show": { "expr": { "Literal": { "Value": if vis.show_title.unwrap_or(true) { "true" } else { "false" } } } },
                            "text": { "expr": { "Literal": { "Value": format!("'{}'", escaped_title) } } }
                        }
                    }]),
                );
            }
            if vis.visual_type == "action-button" {
                let act_lit = if vis.action_type.as_deref() == Some("page-navigation") {
                    "'PageNavigation'"
                } else {
                    "'Back'"
                };
                let mut link_props = serde_json::Map::new();
                link_props.insert("type".into(), json!({ "expr": { "Literal": { "Value": act_lit } } }));
                if let Some(ref target) = vis.action_target {
                    let esc_target = target.replace('\'', "''");
                    link_props.insert(
                        "navigationSection".into(),
                        json!({ "expr": { "Literal": { "Value": format!("'{}'", esc_target) } } }),
                    );
                }
                container_objects.insert(
                    "visualLink".into(),
                    json!([{ "properties": serde_json::Value::Object(link_props) }]),
                );
            }
            if !container_objects.is_empty() {
                visual_inner.insert("visualContainerObjects".into(), serde_json::Value::Object(container_objects));
            }

            let mut vjson_map = serde_json::Map::new();
            vjson_map.insert("name".into(), json!(vid));
            vjson_map.insert("position".into(), json!({ "x": vis.x, "y": vis.y, "width": vis.w, "height": vis.h }));
            vjson_map.insert("visual".into(), serde_json::Value::Object(visual_inner));
            if !vis.filters.is_empty() {
                vjson_map.insert("filterConfig".into(), build_pbir_filter_config(&vis.filters));
            }

            fs::write(
                vdir.join("visual.json"),
                serde_json::to_string_pretty(&serde_json::Value::Object(vjson_map)).unwrap(),
            )
            .map_err(|e| format!("Erro ao gravar visual.json: {}", e))?;
        }
    }

    Ok(pbip_path.to_string_lossy().to_string())
}

fn build_pbir_filter_config(filters: &[ExportFilterPayload]) -> serde_json::Value {
    let filter_items: Vec<serde_json::Value> = filters
        .iter()
        .map(|f| {
            let escaped_vals: Vec<serde_json::Value> = f
                .values
                .iter()
                .map(|v| {
                    let esc = v.replace('\'', "''");
                    json!([{ "Literal": { "Value": format!("'{}'", esc) } }])
                })
                .collect();
            json!({
                "field": {
                    "Column": {
                        "Expression": { "SourceRef": { "Entity": f.table } },
                        "Property": f.column
                    }
                },
                "type": "Categorical",
                "filter": {
                    "Where": [{
                        "Condition": {
                            "In": {
                                "Values": escaped_vals
                            }
                        }
                    }]
                }
            })
        })
        .collect();
    json!({ "filters": filter_items })
}

fn quote_tmdl_ident(name: &str) -> String {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_empty() {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn sanitize_fs_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let t = cleaned.trim();
    if t.is_empty() { "PrismProject".to_string() } else { t.to_string() }
}

fn map_bi_type_to_tmdl(dt: &str) -> &'static str {
    match dt.to_lowercase().as_str() {
        "number" => "double",
        "currency" => "decimal",
        "percentage" => "double",
        "boolean" => "boolean",
        "date" => "dateTime",
        _ => "string",
    }
}

/// Maps PrismBI `sortField`/`sortOrder` to PBIR `query.sortDefinition`
/// (`{ sort: [{ field: { Column|Measure: {...} }, direction }], isDefaultSort }`).
/// A slicer (or any visual) with only `sortOrder` sorts by its category field.
fn build_sort_definition(
    vis: &ExportVisualPayload,
    measures: &[ExportMeasurePayload],
) -> Option<serde_json::Value> {
    let order = vis.sort_order.as_deref();
    let field = vis
        .sort_field
        .as_deref()
        .filter(|f| !f.trim().is_empty())
        .or_else(|| order.and(vis.x_axis_field.as_deref()))?;
    let bare = field.trim_start_matches("Σ ").trim();
    let field_expr = if let Some(m) = measures.iter().find(|m| m.name.eq_ignore_ascii_case(bare)) {
        json!({ "Measure": { "Expression": { "SourceRef": { "Entity": m.table_name } }, "Property": m.name } })
    } else {
        let (entity, prop) = match bare.split_once('.') {
            Some((t, c)) if !t.is_empty() && !c.is_empty() => (t.to_string(), c.to_string()),
            _ => (vis.table_id.clone(), bare.to_string()),
        };
        json!({ "Column": { "Expression": { "SourceRef": { "Entity": entity } }, "Property": prop } })
    };
    let direction = if order == Some("desc") { "Descending" } else { "Ascending" };
    Some(json!({
        "sort": [{ "field": field_expr, "direction": direction }],
        "isDefaultSort": false
    }))
}

fn map_prism_to_pbi_visual_type(vt: &str) -> &'static str {
    match vt {
        "column" => "clusteredColumnChart",
        "stacked-column" => "stackedColumnChart",
        "bar" => "clusteredBarChart",
        "stacked-bar" => "stackedBarChart",
        "line" => "lineChart",
        "area" => "areaChart",
        "combo" => "lineClusteredColumnComboChart",
        "pie" => "pieChart",
        "donut" => "donutChart",
        "treemap" => "treemap",
        "waterfall" => "waterfallChart",
        "funnel" => "funnel",
        "scatter" => "scatterChart",
        "gauge" => "gauge",
        "card" | "multi-card" => "cardVisual",
        "kpi" => "kpi",
        "matrix" => "pivotTable",
        "table" => "tableEx",
        "slicer" => "slicer",
        "textbox" => "textbox",
        "action-button" => "actionButton",
        _ => "clusteredColumnChart",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbi::pbip::open_pbip_project;
    use tempfile::tempdir;

    #[test]
    fn test_pbip_round_trip_save_and_reopen() {
        let dir = tempdir().unwrap();
        let payload = ExportPbipPayload {
            project_name: "Projeto RoundTrip".into(),
            tables: vec![ExportTablePayload {
                name: "f_Vendas".into(),
                is_hidden: Some(false),
                m_script: Some("let\n    Fonte = Csv.Document(File.Contents(\"vendas.csv\"))\nin\n    Fonte".into()),
                columns: vec![
                    ExportColumnPayload {
                        name: "Categoria".into(),
                        data_type: "text".into(),
                        format_string: None,
                        sort_by_column: None,
                        is_hidden: None,
                        is_calculated: None,
                        expression: None,
                    },
                    ExportColumnPayload {
                        name: "Valor".into(),
                        data_type: "currency".into(),
                        format_string: Some("\"R$\" #,0.00".into()),
                        sort_by_column: None,
                        is_hidden: None,
                        is_calculated: None,
                        expression: None,
                    },
                ],
            }],
            relationships: vec![],
            measures: vec![ExportMeasurePayload {
                name: "Total Vendas".into(),
                table_name: "f_Vendas".into(),
                formula: "SUM(f_Vendas[Valor])".into(),
                format_string: Some("\"R$\" #,0.00".into()),
                display_folder: Some("KPIs".into()),
                description: None,
            }],
            pages: vec![ExportPagePayload {
                id: "pg_1".into(),
                name: "Resumo".into(),
                width: 1280.0,
                height: 720.0,
                filters: vec![ExportFilterPayload {
                    table: "f_Vendas".into(),
                    column: "Categoria".into(),
                    values: vec!["Eletronicos".into()],
                }],
                visuals: vec![
                    ExportVisualPayload {
                        id: "vis_1".into(),
                        visual_type: "matrix".into(),
                        title: "Matriz de Categorias".into(),
                        x: 10.0,
                        y: 20.0,
                        w: 500.0,
                        h: 300.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Categoria".into()),
                        legend_field: None,
                        y_axis_fields: vec!["Total Vendas".into()],
                        card_columns: None,
                        slicer_mode: None,
                        text_content: None,
                        action_type: None,
                        action_target: None,
                        show_title: Some(true),
                        background_color: None,
                        border_color: None,
                        filters: vec![],
                        sort_field: Some("Total Vendas".into()),
                        sort_order: Some("desc".into()),
                    },
                    ExportVisualPayload {
                        id: "vis_slicer".into(),
                        visual_type: "slicer".into(),
                        title: "Filtro Categoria".into(),
                        x: 520.0,
                        y: 20.0,
                        w: 300.0,
                        h: 80.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Categoria".into()),
                        legend_field: None,
                        y_axis_fields: vec![],
                        card_columns: None,
                        slicer_mode: Some("tiles".into()),
                        text_content: None,
                        action_type: None,
                        action_target: None,
                        show_title: Some(false),
                        background_color: None,
                        border_color: None,
                        filters: vec![],
                        sort_field: None,
                        sort_order: Some("desc".into()),
                    },
                ],
            }],
            theme_name: Some("Tema PrismBI".into()),
            theme_palette: Some(vec!["#f59e0b".into(), "#3b82f6".into()]),
        };

        let pbip_path = save_pbip_project(dir.path(), &payload).expect("save pbip");
        let reopened = open_pbip_project(Path::new(&pbip_path), None).expect("reopen pbip");

        assert_eq!(reopened.tables.len(), 1);
        assert_eq!(reopened.tables[0].measures.len(), 1);
        assert_eq!(reopened.tables[0].measures[0].name, "Total Vendas");
        assert_eq!(reopened.power_query_scripts.len(), 1);
        assert_eq!(reopened.pages.len(), 1);
        assert_eq!(reopened.pages[0].filters.len(), 1);
        assert_eq!(reopened.pages[0].filters[0].values, vec!["Eletronicos"]);
        assert_eq!(reopened.pages[0].visuals[0].visual_type, "matrix");
        assert_eq!(reopened.pages[0].visuals[0].title, "Matriz de Categorias");
        // Sort round-trips through PBIR query.sortDefinition.
        assert_eq!(reopened.pages[0].visuals[0].sort_field.as_deref(), Some("Total Vendas"));
        assert_eq!(reopened.pages[0].visuals[0].sort_order.as_deref(), Some("desc"));
        let slicer = reopened.pages[0].visuals.iter().find(|v| v.visual_type == "slicer").expect("slicer");
        assert_eq!(slicer.sort_field.as_deref(), Some("f_Vendas.Categoria"));
        assert_eq!(slicer.sort_order.as_deref(), Some("desc"));
        assert_eq!(reopened.pages[0].visuals[1].slicer_mode.as_deref(), Some("tiles"));
    }

    #[test]
    fn test_build_sort_definition_maps_measure_and_column() {
        let measures = vec![ExportMeasurePayload {
            name: "Total Vendas".into(),
            table_name: "Medidas".into(),
            formula: "1".into(),
            format_string: None,
            display_folder: None,
            description: None,
        }];
        let mut vis: ExportVisualPayload = serde_json::from_value(serde_json::json!({
            "id": "v", "visualType": "column", "title": "t", "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0,
            "tableId": "f_Vendas", "xAxisField": "Categoria", "legendField": null, "yAxisFields": ["Total Vendas"],
            "sortField": "Total Vendas", "sortOrder": "desc"
        }))
        .unwrap();
        let sd = build_sort_definition(&vis, &measures).unwrap();
        assert_eq!(sd["sort"][0]["direction"], "Descending");
        assert_eq!(sd["sort"][0]["field"]["Measure"]["Expression"]["SourceRef"]["Entity"], "Medidas");
        assert_eq!(sd["sort"][0]["field"]["Measure"]["Property"], "Total Vendas");
        assert_eq!(sd["isDefaultSort"], false);

        vis.sort_field = Some("d_Produto.Nome".into());
        vis.sort_order = Some("asc".into());
        let sd = build_sort_definition(&vis, &measures).unwrap();
        assert_eq!(sd["sort"][0]["direction"], "Ascending");
        assert_eq!(sd["sort"][0]["field"]["Column"]["Expression"]["SourceRef"]["Entity"], "d_Produto");
        assert_eq!(sd["sort"][0]["field"]["Column"]["Property"], "Nome");

        vis.sort_field = None;
        vis.sort_order = None;
        assert!(build_sort_definition(&vis, &measures).is_none());
    }
}
