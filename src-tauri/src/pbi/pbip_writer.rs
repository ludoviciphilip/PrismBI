use crate::pbi::pbip::operator_to_comparison_kind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// VetroBI `dataConfig.sortField` (column or measure name, optionally "Tabela.Coluna").
    #[serde(default)]
    pub sort_field: Option<String>,
    /// VetroBI `dataConfig.sortOrder` ("asc" | "desc").
    #[serde(default)]
    pub sort_order: Option<String>,
    /// v0.10 Frente H: formatação condicional (gradiente/regras/barras/ícones),
    /// gravada em `objects.values` do visual.json (ver `build_pbir_cond_format_objects`).
    #[serde(default)]
    pub cond_formats: Vec<ExportCondFormatPayload>,
    /// v0.10 Frente H (round-trip do segmentador, Bloco C da Frente de Segmentadores):
    /// modo 'between', gravado no self-filter (`objects.general.properties.filter`).
    #[serde(default)]
    pub slicer_between_start: Option<String>,
    #[serde(default)]
    pub slicer_between_end: Option<String>,
    /// Modo 'relative-date', gravado no mesmo self-filter como `DateSpan`.
    #[serde(default)]
    pub slicer_relative_anchor: Option<String>,
    #[serde(default)]
    pub slicer_relative_count: Option<f64>,
    #[serde(default)]
    pub slicer_relative_unit: Option<String>,
    #[serde(default)]
    pub slicer_relative_include_today: Option<bool>,
    /// Caixa de busca do segmentador (`objects.searchBox[0].properties.show`).
    #[serde(default)]
    pub slicer_show_search: Option<bool>,
    /// v0.10 Rodada 2 (seams H↔G): hierarquia de drill de um gráfico (níveis
    /// abaixo de `x_axis_field`), exportada como projeções extras em Category.
    #[serde(default)]
    pub drill_fields: Vec<String>,
    /// v0.10 Rodada 2 (seams H↔G): campos das linhas de uma matriz hierárquica
    /// (N níveis). O 1º espelha `x_axis_field` (não duplicado na exportação).
    #[serde(default)]
    pub matrix_row_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportCondFormatStopPayload {
    pub kind: String,
    #[serde(default)]
    pub value: Option<f64>,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportCondFormatRulePayload {
    pub operator: String,
    pub value: f64,
    #[serde(default)]
    pub value2: Option<f64>,
    #[serde(default)]
    pub color: String,
}

/// Espelha `ConditionalFormat` de `src/types/bi.ts`, achatado para o que o
/// PBIR consegue representar: `icons: { set, rules }` entra como
/// `iconSet` + `rules`, `dataBar: { positiveColor, negativeColor }` entra
/// como `dataBarPositive`/`dataBarNegative`. `basedOn` (cor calculada a
/// partir de outro campo) e regras com operador `"between"` não têm
/// representação direta em `Conditional.Cases` do PBIR (um único
/// `ComparisonKind`) — são ignorados na exportação com um aviso no
/// relatório da Frente H, não travam o resto da gravação.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCondFormatPayload {
    pub id: String,
    pub target: String,
    pub apply_to: String,
    pub mode: String,
    #[serde(default)]
    pub apply_to_totals: Option<bool>,
    #[serde(default)]
    pub rules: Vec<ExportCondFormatRulePayload>,
    #[serde(default)]
    pub gradient_min: Option<ExportCondFormatStopPayload>,
    #[serde(default)]
    pub gradient_mid: Option<ExportCondFormatStopPayload>,
    #[serde(default)]
    pub gradient_max: Option<ExportCondFormatStopPayload>,
    #[serde(default)]
    pub data_bar_positive: Option<String>,
    #[serde(default)]
    pub data_bar_negative: Option<String>,
    #[serde(default)]
    pub icon_set: Option<String>,
}

/// Indicador (bookmark) no formato esperado pelo PBIR
/// (`definition/bookmarks/<name>.bookmark.json` + `bookmarks.json`).
/// Espelha só a interface `Bookmark` do contrato (V010.md, bloco E) — não
/// depende do código da Frente E (`feat/v0.10-bookmarks`), que está em
/// andamento em paralelo.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportBookmarkPayload {
    pub id: String,
    pub name: String,
    pub active_page_id: String,
    #[serde(default)]
    pub filters: Vec<ExportFilterPayload>,
    #[serde(default)]
    pub order: Option<i64>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub capture_data: Option<bool>,
    #[serde(default)]
    pub capture_display: Option<bool>,
    #[serde(default)]
    pub capture_current_page: Option<bool>,
    /// Visuais afetados; vazio/ausente = `'all'` (todos).
    #[serde(default)]
    pub scope: Vec<String>,
    /// `visualState`/`slicerSelections` do contrato: passados como JSON cru
    /// para dentro de `explorationState.objects.merge.vetroBiState` (não é
    /// um objeto nativo do PBIR — ver `write_bookmarks`), porque mapear cada
    /// um para o `objects`/`filters` real de cada visual exigiria conhecer o
    /// tipo de cada visual-alvo; a VetroBI relê essa extensão (e `prismBiState`
    /// legado) no próprio round-trip, mas o Power BI Desktop ignoraria esse bloco.
    #[serde(default)]
    pub visual_state: Option<serde_json::Value>,
    #[serde(default)]
    pub slicer_selections: Option<serde_json::Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// v0.10 Frente H: indicadores (Bookmarks), gravados em
    /// `definition/bookmarks/*.bookmark.json` + `bookmarks.json`.
    #[serde(default)]
    pub bookmarks: Vec<ExportBookmarkPayload>,
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
            "name": payload.theme_name.clone().unwrap_or_else(|| "VetroTheme".to_string()),
            "dataColors": palette,
        });
        fs::write(
            res_dir.join("VetroTheme.json"),
            serde_json::to_string_pretty(&theme_json).unwrap(),
        )
        .map_err(|e| format!("Erro ao gravar VetroTheme.json: {}", e))?;

        let report_json = json!({
            "themeCollection": {
                "customTheme": {
                    "name": "VetroTheme.json",
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
                map_vetro_to_pbi_visual_type(&vis.visual_type)
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
                // v0.10 Rodada 2 (seams H↔G): a hierarquia completa (drillFields do
                // gráfico, ou matrixRowFields[1..] da matriz) vira projeções extras na
                // MESMA role — exatamente o formato que a importação da G lê de volta
                // (`drill_fields`/`matrixRowFields`, pbip.rs linhas ~372-380).
                let extra_fields: &[String] = if vis.visual_type == "matrix" {
                    if vis.matrix_row_fields.len() > 1 {
                        &vis.matrix_row_fields[1..]
                    } else {
                        &[]
                    }
                } else if role == "Category" {
                    &vis.drill_fields
                } else {
                    &[]
                };
                let mut projections = vec![json!({ "queryRef": qref })];
                for f in extra_fields {
                    let fqref = if f.contains('.') {
                        f.clone()
                    } else {
                        format!("{}.{}", vis.table_id, f)
                    };
                    projections.push(json!({ "queryRef": fqref }));
                }
                query_state.insert(role.to_string(), json!({ "projections": projections }));
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
            if vis.visual_type == "slicer" {
                if let Some(general) = build_pbir_slicer_self_filter(vis, &payload.tables) {
                    objects_map.insert("general".into(), general);
                }
                if let Some(show_search) = vis.slicer_show_search {
                    objects_map.insert(
                        "searchBox".into(),
                        json!([{
                            "properties": {
                                "show": { "expr": { "Literal": { "Value": if show_search { "true" } else { "false" } } } }
                            }
                        }]),
                    );
                }
            }
            if !vis.cond_formats.is_empty() {
                if let Some(values_card) = build_pbir_cond_format_objects(&vis.cond_formats) {
                    objects_map.insert("values".into(), values_card);
                }
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
            if let Some(ref bg) = vis.background_color {
                container_objects.insert(
                    "background".into(),
                    json!([{
                        "properties": {
                            "show": { "expr": { "Literal": { "Value": "true" } } },
                            "color": { "solid": { "color": { "expr": { "Literal": { "Value": format!("'{}'", bg) } } } } }
                        }
                    }]),
                );
            }
            if let Some(ref border) = vis.border_color {
                container_objects.insert(
                    "border".into(),
                    json!([{
                        "properties": {
                            "show": { "expr": { "Literal": { "Value": "true" } } },
                            "color": { "solid": { "color": { "expr": { "Literal": { "Value": format!("'{}'", border) } } } } }
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

    write_bookmarks(&rep_def_dir, &payload.bookmarks)?;

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
    if t.is_empty() { "VetroProject".to_string() } else { t.to_string() }
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

/// Maps VetroBI `sortField`/`sortOrder` to PBIR `query.sortDefinition`
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

fn map_vetro_to_pbi_visual_type(vt: &str) -> &'static str {
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

/// Nome literal de ícone PBIR (`Value.Literal.Value` de cada `Case`) para
/// uma regra de ícone, de forma que `icon_set_from_literal` (em `pbip.rs`)
/// consiga detectar o `set` de volta na importação. Não são nomes reais do
/// Power BI Desktop (ex.: `'SymbolMedium'`) — são só substrings
/// identificáveis (`Arrows`/`Traffic`/`Flags`), suficiente para o
/// round-trip da VetroBI; documentado como aproximação no relatório.
fn icon_literal_for_rule(set: &str, idx: usize) -> String {
    let label = match set {
        "traffic" => "Traffic",
        "flags" => "Flags",
        _ => "Arrows",
    };
    format!("'{}Icon{}'", label, idx)
}

/// Gravação de um único `ConditionalFormat` (achatado em `ExportCondFormatPayload`)
/// como uma entrada do card `objects.values` do PBIR, espelhando o formato
/// lido por `extract_pbir_conditional_formats`/`parse_pbir_*` em `pbip.rs`.
/// Formatos não representáveis (regras com operador `"between"`, cor
/// calculada via `basedOn`) são ignorados aqui — ver o aviso na doc de
/// `ExportCondFormatPayload`.
fn build_pbir_cond_format_entry(cf: &ExportCondFormatPayload) -> Option<serde_json::Value> {
    let prop_name = match cf.target.as_str() {
        "background" => "backColor",
        "fontColor" => "fontColor",
        "dataBar" => "dataBars",
        "icon" => "icon",
        _ => return None,
    };

    let prop_value = match cf.target.as_str() {
        "dataBar" => {
            let mut bars = serde_json::Map::new();
            if let Some(ref pos) = cf.data_bar_positive {
                bars.insert(
                    "positiveColor".into(),
                    json!({ "solid": { "color": { "expr": { "Literal": { "Value": format!("'{}'", pos) } } } } }),
                );
            }
            if let Some(ref neg) = cf.data_bar_negative {
                bars.insert(
                    "negativeColor".into(),
                    json!({ "solid": { "color": { "expr": { "Literal": { "Value": format!("'{}'", neg) } } } } }),
                );
            }
            if bars.is_empty() {
                return None;
            }
            serde_json::Value::Object(bars)
        }
        "icon" => {
            let set = cf.icon_set.as_deref().unwrap_or("arrows");
            let cases: Vec<serde_json::Value> = cf
                .rules
                .iter()
                .enumerate()
                .filter_map(|(idx, rule)| {
                    let kind = operator_to_comparison_kind(&rule.operator)?;
                    Some(json!({
                        "Condition": {
                            "Comparison": {
                                "ComparisonKind": kind,
                                "Left": { "Measure": { "Property": cf.apply_to } },
                                "Right": { "Literal": { "Value": format!("{}D", rule.value) } }
                            }
                        },
                        "Value": { "Literal": { "Value": icon_literal_for_rule(set, idx) } }
                    }))
                })
                .collect();
            if cases.is_empty() {
                return None;
            }
            json!({
                "kind": "Icon",
                "layout": { "expr": { "Literal": { "Value": "'IconOnly'" } } },
                "value": { "expr": { "Conditional": { "Cases": cases } } }
            })
        }
        _ if cf.mode == "gradient" => {
            let min = cf.gradient_min.as_ref()?;
            let max = cf.gradient_max.as_ref()?;
            let stop_json = |s: &ExportCondFormatStopPayload| {
                let mut node = json!({ "color": { "expr": { "Literal": { "Value": format!("'{}'", s.color) } } } });
                if let Some(v) = s.value {
                    node["value"] = json!({ "Literal": { "Value": format!("{}D", v) } });
                }
                node
            };
            let fill_rule = if let Some(mid) = &cf.gradient_mid {
                json!({ "linearGradient3": { "min": stop_json(min), "mid": stop_json(mid), "max": stop_json(max) } })
            } else {
                json!({ "linearGradient2": { "min": stop_json(min), "max": stop_json(max) } })
            };
            json!({ "solid": { "color": { "expr": { "FillRule": fill_rule } } } })
        }
        _ => {
            // mode == "rules": Conditional.Cases por ComparisonKind. Regras
            // com operador "between" não têm ComparisonKind único — são
            // puladas (ver doc de ExportCondFormatPayload).
            let cases: Vec<serde_json::Value> = cf
                .rules
                .iter()
                .filter_map(|rule| {
                    let kind = operator_to_comparison_kind(&rule.operator)?;
                    Some(json!({
                        "Condition": {
                            "Comparison": {
                                "ComparisonKind": kind,
                                "Left": { "Measure": { "Property": cf.apply_to } },
                                "Right": { "Literal": { "Value": format!("{}D", rule.value) } }
                            }
                        },
                        "Value": { "Literal": { "Value": format!("'{}'", rule.color) } }
                    }))
                })
                .collect();
            if cases.is_empty() {
                return None;
            }
            json!({ "solid": { "color": { "expr": { "Conditional": { "Cases": cases } } } } })
        }
    };

    Some(json!({
        "properties": { prop_name: prop_value },
        "selector": { "metadata": cf.apply_to }
    }))
}

/// Gravação de todos os `ConditionalFormat` do visual no card `objects.values`.
fn build_pbir_cond_format_objects(cond_formats: &[ExportCondFormatPayload]) -> Option<serde_json::Value> {
    let entries: Vec<serde_json::Value> = cond_formats.iter().filter_map(build_pbir_cond_format_entry).collect();
    if entries.is_empty() {
        None
    } else {
        Some(serde_json::Value::Array(entries))
    }
}

/// Self-filter do segmentador (`objects.general[0].properties.filter.filter`),
/// para os modos `'between'` e `'relative-date'` (ver
/// `extract_pbir_slicer_self_filter` em `pbip.rs`, que lê de volta).
fn build_pbir_slicer_self_filter(vis: &ExportVisualPayload, tables: &[ExportTablePayload]) -> Option<serde_json::Value> {
    let field = vis.x_axis_field.as_deref()?;
    let (entity, prop) = match field.split_once('.') {
        Some((t, c)) if tables.iter().any(|t2| t2.name.eq_ignore_ascii_case(t)) => (t.to_string(), c.to_string()),
        _ => (vis.table_id.clone(), field.to_string()),
    };
    let column_expr = json!({ "Column": { "Expression": { "SourceRef": { "Entity": entity } }, "Property": prop } });

    let condition = if let (Some(start), Some(end)) = (&vis.slicer_between_start, &vis.slicer_between_end) {
        json!({
            "Between": {
                "Expression": column_expr,
                "LowerBound": { "Literal": { "Value": start } },
                "UpperBound": { "Literal": { "Value": end } }
            }
        })
    } else if let (Some(anchor), Some(count), Some(unit)) =
        (&vis.slicer_relative_anchor, vis.slicer_relative_count, &vis.slicer_relative_unit)
    {
        json!({
            "DateSpan": {
                "Expression": column_expr,
                "anchor": anchor,
                "count": count,
                "unit": unit,
                "includeToday": vis.slicer_relative_include_today.unwrap_or(false)
            }
        })
    } else {
        return None;
    };

    Some(json!([{
        "properties": {
            "filter": {
                "filter": {
                    "Version": 2,
                    "From": [{ "Name": "s", "Entity": entity, "Type": 0 }],
                    "Where": [{ "Condition": condition }]
                }
            }
        }
    }]))
}

/// Grava `definition/bookmarks/<id>.bookmark.json` (schema `bookmark/1.4.0`)
/// e `bookmarks.json` (schema `bookmarksMetadata/1.0.0`). `explorationState`
/// cobre `activeSection` e os filtros de dados (`filters.byExpr`); o estado
/// por visual (`visualState`/`slicerSelections` do contrato) é passado como
/// extensão em `objects.merge.vetroBiState` — não é lido pelo Power BI
/// Desktop, só pela própria VetroBI no round-trip (e `read_bookmarks` também
/// aceita a chave legada `prismBiState`; ver doc de `ExportBookmarkPayload`).
fn write_bookmarks(rep_def_dir: &Path, bookmarks: &[ExportBookmarkPayload]) -> Result<(), String> {
    if bookmarks.is_empty() {
        return Ok(());
    }
    let bm_dir = rep_def_dir.join("bookmarks");
    fs::create_dir_all(&bm_dir).map_err(|e| format!("Erro ao criar pasta bookmarks: {}", e))?;

    for bm in bookmarks {
        let safe_name = sanitize_fs_name(&bm.id);
        let mut exploration_state = serde_json::Map::new();
        exploration_state.insert("version".into(), json!("1.3"));
        exploration_state.insert("activeSection".into(), json!(bm.active_page_id));
        if !bm.filters.is_empty() {
            exploration_state.insert(
                "filters".into(),
                json!({ "byExpr": build_pbir_filter_config(&bm.filters)["filters"].clone() }),
            );
        }
        if bm.visual_state.is_some() || bm.slicer_selections.is_some() {
            exploration_state.insert(
                "objects".into(),
                json!({
                    "merge": {
                        "vetroBiState": {
                            "visualState": bm.visual_state.clone().unwrap_or(serde_json::Value::Null),
                            "slicerSelections": bm.slicer_selections.clone().unwrap_or(serde_json::Value::Null)
                        }
                    }
                }),
            );
        }

        let bookmark_json = json!({
            "$schema": "https://developer.microsoft.com/json-schemas/fabric/item/report/definition/bookmark/1.4.0/schema.json",
            "name": bm.id,
            "displayName": bm.name,
            "options": {
                "targetVisualNames": if bm.scope.is_empty() { json!([]) } else { json!(bm.scope) },
                "suppressData": !bm.capture_data.unwrap_or(true),
                "suppressDisplay": !bm.capture_display.unwrap_or(true),
                "suppressActiveSection": !bm.capture_current_page.unwrap_or(true),
                "applyOnlyToTargetVisuals": !bm.scope.is_empty()
            },
            "explorationState": exploration_state
        });
        fs::write(
            bm_dir.join(format!("{}.bookmark.json", safe_name)),
            serde_json::to_string_pretty(&bookmark_json).unwrap(),
        )
        .map_err(|e| format!("Erro ao gravar bookmark '{}': {}", bm.id, e))?;
    }

    // Agrupa por groupId preservando a ordem (`order`, quando presente).
    let mut ordered: Vec<&ExportBookmarkPayload> = bookmarks.iter().collect();
    ordered.sort_by_key(|b| b.order.unwrap_or(i64::MAX));
    let mut groups: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    let mut top_items: Vec<serde_json::Value> = Vec::new();
    for bm in &ordered {
        let item = json!({ "name": bm.id });
        match &bm.group_id {
            Some(g) => groups.entry(g.clone()).or_default().push(item),
            None => top_items.push(item),
        }
    }
    for (group_id, children) in groups {
        top_items.push(json!({ "name": group_id, "displayName": group_id, "children": children }));
    }

    let metadata_json = json!({
        "$schema": "https://developer.microsoft.com/json-schemas/fabric/item/report/definition/bookmarksMetadata/1.0.0/schema.json",
        "items": top_items
    });
    fs::write(
        rep_def_dir.join("bookmarks.json"),
        serde_json::to_string_pretty(&metadata_json).unwrap(),
    )
    .map_err(|e| format!("Erro ao gravar bookmarks.json: {}", e))?;
    Ok(())
}

/// Lê de volta `definition/bookmarks/*.bookmark.json` + `bookmarks.json`
/// (usado pelo teste de round-trip; não faz parte de `open_pbip_project`
/// porque a Frente E ainda não decidiu como os bookmarks entram em
/// `PbiProjectSummary`).
pub fn read_bookmarks(rep_def_dir: &Path) -> Vec<ExportBookmarkPayload> {
    let bm_dir = rep_def_dir.join("bookmarks");
    let Ok(entries) = fs::read_dir(&bm_dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else { continue };
        let Ok(v): Result<serde_json::Value, _> = serde_json::from_str(&raw) else { continue };
        let id = v.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let name = v.get("displayName").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let active_page_id = v
            .get("explorationState")
            .and_then(|e| e.get("activeSection"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let options = v.get("options");
        let capture_data = options.and_then(|o| o.get("suppressData")).and_then(|b| b.as_bool()).map(|b| !b);
        let capture_display = options.and_then(|o| o.get("suppressDisplay")).and_then(|b| b.as_bool()).map(|b| !b);
        let capture_current_page =
            options.and_then(|o| o.get("suppressActiveSection")).and_then(|b| b.as_bool()).map(|b| !b);
        let scope: Vec<String> = options
            .and_then(|o| o.get("targetVisualNames"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let vetro_state = v
            .get("explorationState")
            .and_then(|e| e.get("objects"))
            .and_then(|o| o.get("merge"))
            .and_then(|m| m.get("vetroBiState").or_else(|| m.get("prismBiState")));
        let visual_state = vetro_state.and_then(|p| p.get("visualState")).filter(|v| !v.is_null()).cloned();
        let slicer_selections = vetro_state.and_then(|p| p.get("slicerSelections")).filter(|v| !v.is_null()).cloned();

        out.push(ExportBookmarkPayload {
            id,
            name,
            active_page_id,
            filters: Vec::new(),
            order: None,
            group_id: None,
            capture_data,
            capture_display,
            capture_current_page,
            scope,
            visual_state,
            slicer_selections,
        });
    }
    out
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
                        ..Default::default()
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
                        ..Default::default()
                    },
                ],
            }],
            theme_name: Some("Tema VetroBI".into()),
            theme_palette: Some(vec!["#f59e0b".into(), "#3b82f6".into()]),
            ..Default::default()
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

    /// Frente H, item 3: round-trip completo — formatação condicional
    /// (gradiente, regras, barras, ícones), cor de fundo/borda, segmentador
    /// 'between'/'relative-date'/showSearch e indicadores (bookmarks).
    /// Importa a fixture, exporta, importa de novo e compara.
    #[test]
    fn test_pbip_round_trip_condfmt_slicer_and_bookmarks() {
        let dir = tempdir().unwrap();
        let payload = ExportPbipPayload {
            project_name: "Projeto CondFmt2".into(),
            tables: vec![ExportTablePayload {
                name: "f_Vendas".into(),
                is_hidden: Some(false),
                m_script: None,
                columns: vec![
                    ExportColumnPayload { name: "Categoria".into(), data_type: "text".into(), ..Default::default() },
                    ExportColumnPayload { name: "Receita".into(), data_type: "currency".into(), ..Default::default() },
                ],
            }],
            relationships: vec![],
            measures: vec![],
            pages: vec![ExportPagePayload {
                id: "pg_1".into(),
                name: "Resumo".into(),
                width: 1280.0,
                height: 720.0,
                filters: vec![],
                visuals: vec![
                    ExportVisualPayload {
                        id: "vis_table".into(),
                        visual_type: "table".into(),
                        title: "Tabela".into(),
                        w: 400.0,
                        h: 240.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Categoria".into()),
                        y_axis_fields: vec!["Receita".into()],
                        show_title: Some(true),
                        background_color: Some("#112233".into()),
                        border_color: Some("#445566".into()),
                        cond_formats: vec![
                            ExportCondFormatPayload {
                                id: "cf_grad".into(),
                                target: "background".into(),
                                apply_to: "Receita".into(),
                                mode: "gradient".into(),
                                apply_to_totals: Some(true),
                                rules: vec![],
                                gradient_min: Some(ExportCondFormatStopPayload { kind: "lowest".into(), value: None, color: "#FF0000".into() }),
                                gradient_mid: None,
                                gradient_max: Some(ExportCondFormatStopPayload { kind: "highest".into(), value: None, color: "#00FF00".into() }),
                                data_bar_positive: None,
                                data_bar_negative: None,
                                icon_set: None,
                            },
                            ExportCondFormatPayload {
                                id: "cf_icon".into(),
                                target: "icon".into(),
                                apply_to: "Receita".into(),
                                mode: "rules".into(),
                                apply_to_totals: None,
                                rules: vec![ExportCondFormatRulePayload { operator: ">".into(), value: 0.0, value2: None, color: String::new() }],
                                gradient_min: None,
                                gradient_mid: None,
                                gradient_max: None,
                                data_bar_positive: None,
                                data_bar_negative: None,
                                icon_set: Some("traffic".into()),
                            },
                        ],
                        ..Default::default()
                    },
                    ExportVisualPayload {
                        id: "vis_slicer".into(),
                        visual_type: "slicer".into(),
                        title: "Filtro".into(),
                        w: 240.0,
                        h: 80.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Receita".into()),
                        slicer_between_start: Some("100D".into()),
                        slicer_between_end: Some("500D".into()),
                        slicer_show_search: Some(true),
                        ..Default::default()
                    },
                ],
            }],
            bookmarks: vec![ExportBookmarkPayload {
                id: "bm1".into(),
                name: "Visão inicial".into(),
                active_page_id: "pg_1".into(),
                filters: vec![ExportFilterPayload {
                    table: "f_Vendas".into(),
                    column: "Categoria".into(),
                    values: vec!["Eletronicos".into()],
                }],
                order: Some(0),
                group_id: None,
                capture_data: Some(true),
                capture_display: Some(true),
                capture_current_page: Some(true),
                scope: vec![],
                visual_state: Some(serde_json::json!({ "vis_table": { "hidden": false } })),
                slicer_selections: None,
            }],
            ..Default::default()
        };

        let pbip_path = save_pbip_project(dir.path(), &payload).expect("save pbip");
        let reopened = open_pbip_project(Path::new(&pbip_path), None).expect("reopen pbip");
        let table_vis = reopened.pages[0].visuals.iter().find(|v| v.id == "vis_table").expect("vis_table");

        assert_eq!(table_vis.background_color.as_deref(), Some("#112233"));
        assert_eq!(table_vis.border_color.as_deref(), Some("#445566"));
        assert_eq!(table_vis.conditional_formats.len(), 2);
        let grad = table_vis.conditional_formats.iter().find(|c| c.target == "background").expect("gradiente");
        assert_eq!(grad.mode, "gradient");
        assert_eq!(grad.gradient_min.as_ref().unwrap().color, "#FF0000");
        assert_eq!(grad.gradient_max.as_ref().unwrap().color, "#00FF00");
        let icon = table_vis.conditional_formats.iter().find(|c| c.target == "icon").expect("icone");
        assert_eq!(icon.icon_set.as_deref(), Some("traffic"));
        assert_eq!(icon.rules.len(), 1);
        assert_eq!(icon.rules[0].operator, ">");

        let slicer_vis = reopened.pages[0].visuals.iter().find(|v| v.id == "vis_slicer").expect("vis_slicer");
        assert_eq!(slicer_vis.slicer_between_start.as_deref(), Some("100"));
        assert_eq!(slicer_vis.slicer_between_end.as_deref(), Some("500"));
        assert_eq!(slicer_vis.slicer_show_search, Some(true));

        // Bookmarks: lidos de volta via read_bookmarks (fora de open_pbip_project).
        let rep_def_dir = Path::new(&pbip_path)
            .parent()
            .unwrap()
            .join("Projeto CondFmt2.Report")
            .join("definition");
        let bookmarks = read_bookmarks(&rep_def_dir);
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].id, "bm1");
        assert_eq!(bookmarks[0].name, "Visão inicial");
        assert_eq!(bookmarks[0].active_page_id, "pg_1");
        assert_eq!(bookmarks[0].capture_data, Some(true));
        assert_eq!(
            bookmarks[0].visual_state,
            Some(serde_json::json!({ "vis_table": { "hidden": false } }))
        );

        // Segundo ciclo export -> import, para confirmar estabilidade do round-trip:
        // reconverte o que foi importado de volta em payload de exportação.
        let reexport_cond_formats: Vec<ExportCondFormatPayload> = table_vis
            .conditional_formats
            .iter()
            .enumerate()
            .map(|(idx, cf)| ExportCondFormatPayload {
                id: format!("re_{}", idx),
                target: cf.target.clone(),
                apply_to: cf.apply_to.clone(),
                mode: cf.mode.clone(),
                apply_to_totals: None,
                rules: cf
                    .rules
                    .iter()
                    .map(|r| ExportCondFormatRulePayload { operator: r.operator.clone(), value: r.value, value2: r.value2, color: r.color.clone() })
                    .collect(),
                gradient_min: cf.gradient_min.as_ref().map(|s| ExportCondFormatStopPayload { kind: s.kind.clone(), value: s.value, color: s.color.clone() }),
                gradient_mid: cf.gradient_mid.as_ref().map(|s| ExportCondFormatStopPayload { kind: s.kind.clone(), value: s.value, color: s.color.clone() }),
                gradient_max: cf.gradient_max.as_ref().map(|s| ExportCondFormatStopPayload { kind: s.kind.clone(), value: s.value, color: s.color.clone() }),
                data_bar_positive: cf.data_bar_positive.clone(),
                data_bar_negative: cf.data_bar_negative.clone(),
                icon_set: cf.icon_set.clone(),
            })
            .collect();
        let dir2 = tempdir().unwrap();
        let mut payload2 = payload.clone();
        payload2.pages[0].visuals[0].cond_formats = reexport_cond_formats;
        let pbip_path2 = save_pbip_project(dir2.path(), &payload2).expect("save pbip 2");
        let reopened2 = open_pbip_project(Path::new(&pbip_path2), None).expect("reopen pbip 2");
        let table_vis2 = reopened2.pages[0].visuals.iter().find(|v| v.id == "vis_table").expect("vis_table 2");
        assert_eq!(table_vis2.conditional_formats.len(), 2);
        let icon2 = table_vis2.conditional_formats.iter().find(|c| c.target == "icon").expect("icone 2");
        assert_eq!(icon2.icon_set.as_deref(), Some("traffic"));
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

    /// v0.10 Rodada 2 (seams H↔G): a hierarquia completa (além do 1º campo) não
    /// pode se perder no round-trip .pbip — drillFields de um gráfico e
    /// matrixRowFields de uma matriz hierárquica viram várias projeções em
    /// Category/Rows, exatamente o formato que a importação da G lê de volta.
    #[test]
    fn test_pbip_round_trip_preserves_drill_hierarchy_beyond_first_field() {
        let dir = tempdir().unwrap();
        let payload = ExportPbipPayload {
            project_name: "Projeto Hierarquia".into(),
            tables: vec![ExportTablePayload {
                name: "f_Vendas".into(),
                is_hidden: Some(false),
                m_script: None,
                columns: vec![
                    ExportColumnPayload {
                        name: "Regiao".into(),
                        data_type: "text".into(),
                        format_string: None,
                        sort_by_column: None,
                        is_hidden: None,
                        is_calculated: None,
                        expression: None,
                    },
                    ExportColumnPayload {
                        name: "Valor".into(),
                        data_type: "number".into(),
                        format_string: None,
                        sort_by_column: None,
                        is_hidden: None,
                        is_calculated: None,
                        expression: None,
                    },
                ],
            }],
            relationships: vec![],
            measures: vec![ExportMeasurePayload {
                name: "Total".into(),
                table_name: "f_Vendas".into(),
                formula: "SUM(f_Vendas[Valor])".into(),
                format_string: None,
                display_folder: None,
                description: None,
            }],
            pages: vec![ExportPagePayload {
                id: "pg_1".into(),
                name: "Resumo".into(),
                width: 1280.0,
                height: 720.0,
                filters: vec![],
                visuals: vec![
                    // Gráfico drillável: Categoria -> Produto -> SubProduto (3 níveis).
                    ExportVisualPayload {
                        id: "vis_chart".into(),
                        visual_type: "column".into(),
                        title: "Vendas".into(),
                        x: 0.0,
                        y: 0.0,
                        w: 400.0,
                        h: 300.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Regiao".into()),
                        legend_field: None,
                        y_axis_fields: vec!["Total".into()],
                        drill_fields: vec!["Produto".into(), "SubProduto".into()],
                        ..Default::default()
                    },
                    // Matriz hierárquica de 3 níveis (matrixRowFields[0] espelha xAxisField).
                    ExportVisualPayload {
                        id: "vis_matrix".into(),
                        visual_type: "matrix".into(),
                        title: "Matriz".into(),
                        x: 420.0,
                        y: 0.0,
                        w: 400.0,
                        h: 300.0,
                        table_id: "f_Vendas".into(),
                        x_axis_field: Some("Regiao".into()),
                        legend_field: None,
                        y_axis_fields: vec!["Total".into()],
                        matrix_row_fields: vec!["Regiao".into(), "Cidade".into(), "Loja".into()],
                        ..Default::default()
                    },
                ],
            }],
            ..Default::default()
        };

        let pbip_path = save_pbip_project(dir.path(), &payload).expect("save pbip");
        let reopened = open_pbip_project(Path::new(&pbip_path), None).expect("reopen pbip");

        let chart = reopened.pages[0].visuals.iter().find(|v| v.id == "vis_chart").expect("chart");
        assert_eq!(chart.category_field.as_deref(), Some("f_Vendas.Regiao"));
        assert_eq!(chart.drill_fields, vec!["f_Vendas.Produto".to_string(), "f_Vendas.SubProduto".to_string()]);

        let matrix = reopened.pages[0].visuals.iter().find(|v| v.id == "vis_matrix").expect("matrix");
        assert_eq!(matrix.category_field.as_deref(), Some("f_Vendas.Regiao"));
        // Os níveis ALÉM do primeiro (Cidade, Loja) não podem se perder.
        assert_eq!(matrix.drill_fields, vec!["f_Vendas.Cidade".to_string(), "f_Vendas.Loja".to_string()]);
    }

    #[test]
    fn read_bookmarks_accepts_legacy_prismbi_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rep_def_dir = dir.path().join("definition");
        let bm_dir = rep_def_dir.join("bookmarks");
        fs::create_dir_all(&bm_dir).unwrap();
        let bm_json = json!({
            "name": "bm_legacy",
            "displayName": "Legado",
            "explorationState": {
                "activeSection": "pg_1",
                "objects": {
                    "merge": {
                        "prismBiState": {
                            "visualState": { "v1": { "visible": false } },
                            "slicerSelections": { "s1": ["Sul"] }
                        }
                    }
                }
            }
        });
        fs::write(
            bm_dir.join("bm_legacy.bookmark.json"),
            serde_json::to_string_pretty(&bm_json).unwrap(),
        )
        .unwrap();
        let list = read_bookmarks(&rep_def_dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].visual_state, Some(json!({ "v1": { "visible": false } })));
        assert_eq!(list[0].slicer_selections, Some(json!({ "s1": ["Sul"] })));
    }
}
