use crate::dax::{build_filter_context, DaxModel, FilterContext, NativeFilterInput};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualQueryRequest {
    pub visual_id: String,
    pub visual_type: String,
    pub table_id: String,
    pub x_axis_field: Option<String>,
    pub legend_field: Option<String>,
    pub y_axis_fields: Vec<String>,
    pub aggregation: Option<String>,
    #[serde(default)]
    pub filters: Vec<NativeFilterInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualSeries {
    pub name: String,
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualQueryResult {
    pub categories: Vec<String>,
    pub series: Vec<NativeVisualSeries>,
    pub summary_value: Option<f64>,
    pub query_time_ms: f64,
    pub engine: String,
}

fn resolve_table_and_col(model: &DaxModel, default_table: &str, raw_field: &str) -> (String, String) {
    let clean = raw_field.trim();
    if let Some((t, c)) = clean.split_once('.') {
        if model.get_table(t).is_some() {
            return (t.trim().to_string(), c.trim().to_string());
        }
    }
    if let Some(tbl) = model.get_table(default_table) {
        if tbl.get_col_idx(clean).is_some() {
            return (tbl.name.clone(), clean.to_string());
        }
    }
    for tbl in model.tables.values() {
        if tbl.get_col_idx(clean).is_some() {
            return (tbl.name.clone(), clean.to_string());
        }
    }
    (default_table.to_string(), clean.to_string())
}

fn eval_field_value(
    model: &DaxModel,
    ctx: &FilterContext,
    default_table: &str,
    field_name: &str,
    agg: &str,
) -> f64 {
    let clean_name = field_name.trim().trim_start_matches('Σ').trim();
    if model.get_measure(clean_name).is_some() {
        return model
            .evaluate_dax(&format!("[{}]", clean_name), ctx)
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
    }

    let (tbl_name, col_name) = resolve_table_and_col(model, default_table, clean_name);
    let Some(tbl) = model.get_table(&tbl_name) else {
        return 0.0;
    };
    let Some(cidx) = tbl.get_col_idx(&col_name) else {
        return 0.0;
    };
    let visible = model.get_visible_row_indices(&tbl.name, ctx);

    match agg.to_uppercase().as_str() {
        "COUNT" => visible.iter().filter(|&&i| !tbl.rows[i][cidx].is_blank()).count() as f64,
        "DISTINCTCOUNT" => visible
            .iter()
            .filter(|&&i| !tbl.rows[i][cidx].is_blank())
            .map(|&i| tbl.rows[i][cidx].canonical_key())
            .collect::<HashSet<_>>()
            .len() as f64,
        _ => {
            let nums: Vec<f64> = visible
                .iter()
                .filter_map(|&i| {
                    let v = &tbl.rows[i][cidx];
                    if v.is_blank() {
                        None
                    } else {
                        v.as_f64()
                    }
                })
                .collect();
            if nums.is_empty() {
                return 0.0;
            }
            match agg.to_uppercase().as_str() {
                "AVG" | "AVERAGE" => nums.iter().sum::<f64>() / nums.len() as f64,
                "MIN" => nums.iter().copied().fold(f64::INFINITY, f64::min),
                "MAX" => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                _ => nums.iter().sum(),
            }
        }
    }
}

pub fn execute_visual_query(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
) -> Result<NativeVisualQueryResult, String> {
    let t0 = Instant::now();
    let agg = req.aggregation.as_deref().unwrap_or("SUM");
    let base_ctx = build_filter_context(&req.filters);

    // 1. Card, KPI, or Gauge
    if matches!(req.visual_type.as_str(), "card" | "kpi" | "gauge") {
        let y_field = req.y_axis_fields.first().map(String::as_str).unwrap_or("");
        let summary = if y_field.is_empty() {
            0.0
        } else {
            eval_field_value(model, &base_ctx, &req.table_id, y_field, agg)
        };
        return Ok(NativeVisualQueryResult {
            categories: vec![y_field.to_string()],
            series: vec![NativeVisualSeries {
                name: y_field.to_string(),
                data: vec![summary],
            }],
            summary_value: Some(summary),
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
        });
    }

    // 2. Multi-card or measure-only visuals (no x-axis category)
    if req.visual_type == "multi-card"
        || (req.x_axis_field.is_none()
            && !req.y_axis_fields.is_empty()
            && !matches!(req.visual_type.as_str(), "slicer" | "table" | "action-button" | "textbox"))
    {
        let mut categories = Vec::with_capacity(req.y_axis_fields.len());
        let mut vals = Vec::with_capacity(req.y_axis_fields.len());
        for yf in &req.y_axis_fields {
            let clean = yf.trim().trim_start_matches('Σ').trim();
            categories.push(clean.to_string());
            vals.push(eval_field_value(model, &base_ctx, &req.table_id, clean, agg));
        }
        let summary = vals.first().copied();
        return Ok(NativeVisualQueryResult {
            categories,
            series: vec![NativeVisualSeries {
                name: "Valores".to_string(),
                data: vals,
            }],
            summary_value: summary,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
        });
    }

    // 3. Slicer (exclude slicer's own column filter so all options stay visible)
    if req.visual_type == "slicer" {
        let raw_col = req.x_axis_field.as_deref().unwrap_or("");
        let (slicer_tbl_name, slicer_col) = resolve_table_and_col(model, &req.table_id, raw_col);
        let remaining_filters: Vec<NativeFilterInput> = req
            .filters
            .iter()
            .filter(|f| {
                !(f.table_id.eq_ignore_ascii_case(&slicer_tbl_name)
                    && f.column.eq_ignore_ascii_case(&slicer_col))
            })
            .cloned()
            .collect();
        let slicer_ctx = build_filter_context(&remaining_filters);
        let mut cats = BTreeSet::new();
        if let Some(tbl) = model.get_table(&slicer_tbl_name) {
            if let Some(cidx) = tbl.get_col_idx(&slicer_col) {
                for r_idx in model.get_visible_row_indices(&tbl.name, &slicer_ctx) {
                    let s = tbl.rows[r_idx][cidx].as_display_string();
                    if !s.is_empty() {
                        cats.insert(s);
                    }
                }
            }
        }
        let cat_vec: Vec<String> = cats.into_iter().collect();
        let ones = vec![1.0; cat_vec.len()];
        return Ok(NativeVisualQueryResult {
            categories: cat_vec,
            series: vec![NativeVisualSeries {
                name: slicer_col,
                data: ones,
            }],
            summary_value: None,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
        });
    }

    // 4. Grouped visuals & matrix
    let raw_cat = req.x_axis_field.as_deref().unwrap_or("");
    let (cat_tbl_name, cat_col) = resolve_table_and_col(model, &req.table_id, raw_cat);
    let mut cat_entries: BTreeMap<String, String> = BTreeMap::new(); // display -> canonical_key
    if let Some(tbl) = model.get_table(&cat_tbl_name) {
        if let Some(cidx) = tbl.get_col_idx(&cat_col) {
            for r_idx in model.get_visible_row_indices(&tbl.name, &base_ctx) {
                let v = &tbl.rows[r_idx][cidx];
                let disp = v.as_display_string();
                let key = v.canonical_key();
                let label = if disp.is_empty() { "Sem dados".to_string() } else { disp };
                cat_entries.entry(label).or_insert(key);
            }
        }
    }

    let categories: Vec<String> = cat_entries.keys().cloned().collect();
    if let Some(ref leg_raw) = req.legend_field.as_ref().filter(|s| !s.trim().is_empty()) {
        let (leg_tbl_name, leg_col) = resolve_table_and_col(model, &req.table_id, leg_raw);
        let mut leg_entries: BTreeMap<String, String> = BTreeMap::new();
        if let Some(tbl) = model.get_table(&leg_tbl_name) {
            if let Some(cidx) = tbl.get_col_idx(&leg_col) {
                for r_idx in model.get_visible_row_indices(&tbl.name, &base_ctx) {
                    let v = &tbl.rows[r_idx][cidx];
                    let disp = v.as_display_string();
                    if !disp.is_empty() {
                        leg_entries.entry(disp).or_insert_with(|| v.canonical_key());
                    }
                }
            }
        }
        let primary_y = req.y_axis_fields.first().map(String::as_str).unwrap_or("");
        let mut out_series = Vec::with_capacity(leg_entries.len());
        for (leg_label, leg_key) in &leg_entries {
            let mut data = Vec::with_capacity(categories.len());
            for cat_label in &categories {
                let mut cell_ctx = base_ctx.clone();
                if let Some(cat_key) = cat_entries.get(cat_label) {
                    let mut s1 = HashSet::new();
                    s1.insert(cat_key.clone());
                    cell_ctx.set_column_filter(&cat_tbl_name, &cat_col, s1);
                }
                let mut s2 = HashSet::new();
                s2.insert(leg_key.clone());
                cell_ctx.set_column_filter(&leg_tbl_name, &leg_col, s2);
                let val = if primary_y.is_empty() {
                    0.0
                } else {
                    eval_field_value(model, &cell_ctx, &req.table_id, primary_y, agg)
                };
                data.push(val);
            }
            out_series.push(NativeVisualSeries {
                name: leg_label.clone(),
                data,
            });
        }
        return Ok(NativeVisualQueryResult {
            categories,
            series: out_series,
            summary_value: None,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
        });
    }

    // Standard multi-measure or single-measure grouped series
    let mut cell_contexts: HashMap<String, FilterContext> = HashMap::with_capacity(categories.len());
    for (cat_label, cat_key) in &cat_entries {
        let mut cctx = base_ctx.clone();
        let mut s = HashSet::new();
        s.insert(cat_key.clone());
        cctx.set_column_filter(&cat_tbl_name, &cat_col, s);
        cell_contexts.insert(cat_label.clone(), cctx);
    }

    let mut out_series = Vec::with_capacity(req.y_axis_fields.len());
    for yf in &req.y_axis_fields {
        let clean_y = yf.trim().trim_start_matches('Σ').trim();
        let mut data = Vec::with_capacity(categories.len());
        for cat_label in &categories {
            let cctx = &cell_contexts[cat_label];
            data.push(eval_field_value(model, cctx, &req.table_id, clean_y, agg));
        }
        out_series.push(NativeVisualSeries {
            name: clean_y.to_string(),
            data,
        });
    }

    Ok(NativeVisualQueryResult {
        categories,
        series: out_series,
        summary_value: None,
        query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
        engine: "rust-native".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dax::{DaxMeasureInput, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};

    fn sample_model() -> DaxModel {
        let mut model = DaxModel::new();
        let make_row = |cat: &str, val: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Categoria".into(), serde_json::Value::String(cat.into()));
            m.insert("Valor".into(), serde_json::json!(val));
            m
        };
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput {
                name: "fVendas".into(),
                columns: vec!["Categoria".into(), "Valor".into()],
                rows: vec![
                    make_row("A", 100.0),
                    make_row("A", 50.0),
                    make_row("B", 200.0),
                ],
            }],
            relationships: Vec::<DaxRelationshipInput>::new(),
            measures: vec![
                DaxMeasureInput {
                    name: "Receita".into(),
                    table_name: "fVendas".into(),
                    formula: "SUM(fVendas[Valor])".into(),
                },
                DaxMeasureInput {
                    name: "Qtd".into(),
                    table_name: "fVendas".into(),
                    formula: "COUNTROWS(fVendas)".into(),
                },
            ],
        });
        model
    }

    #[test]
    fn test_native_query_planner_card_multicard_slicer_and_grouped() {
        let model = sample_model();

        // Card
        let card_res = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "c1".into(),
                visual_type: "card".into(),
                table_id: "fVendas".into(),
                x_axis_field: None,
                legend_field: None,
                y_axis_fields: vec!["Receita".into()],
                aggregation: Some("NONE".into()),
                filters: vec![],
            },
        )
        .unwrap();
        assert_eq!(card_res.summary_value, Some(350.0));
        assert_eq!(card_res.engine, "rust-native");

        // Multi-card
        let mc_res = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "mc1".into(),
                visual_type: "multi-card".into(),
                table_id: "fVendas".into(),
                x_axis_field: None,
                legend_field: None,
                y_axis_fields: vec!["Receita".into(), "Qtd".into()],
                aggregation: Some("NONE".into()),
                filters: vec![],
            },
        )
        .unwrap();
        assert_eq!(mc_res.categories, vec!["Receita", "Qtd"]);
        assert_eq!(mc_res.series[0].data, vec![350.0, 3.0]);

        // Slicer keeps all options even when filtered on its own column
        let sl_res = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "sl1".into(),
                visual_type: "slicer".into(),
                table_id: "fVendas".into(),
                x_axis_field: Some("Categoria".into()),
                legend_field: None,
                y_axis_fields: vec![],
                aggregation: None,
                filters: vec![NativeFilterInput {
                    table_id: "fVendas".into(),
                    column: "Categoria".into(),
                    values: vec![serde_json::json!("A")],
                }],
            },
        )
        .unwrap();
        assert_eq!(sl_res.categories, vec!["A", "B"]);

        // Grouped column / matrix
        let col_res = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "g1".into(),
                visual_type: "column".into(),
                table_id: "fVendas".into(),
                x_axis_field: Some("Categoria".into()),
                legend_field: None,
                y_axis_fields: vec!["Receita".into()],
                aggregation: Some("NONE".into()),
                filters: vec![],
            },
        )
        .unwrap();
        assert_eq!(col_res.categories, vec!["A", "B"]);
        assert_eq!(col_res.series[0].data, vec![150.0, 200.0]);
    }
}
