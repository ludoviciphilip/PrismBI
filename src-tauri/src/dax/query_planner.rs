use crate::dax::{build_filter_context, functions, DaxModel, DaxValue, FilterContext, NativeFilterInput, ValueKey};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Instant;

/// v0.10 Frente G: um nível de agrupamento (eixo de drill ou linha de matriz).
/// `date_part` ("year" | "quarter" | "month" | "day") agrupa uma coluna de
/// data pela parte (hierarquia automática de datas).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGroupLevel {
    pub field: String,
    #[serde(default)]
    pub date_part: Option<String>,
}

/// v0.10 Frente G: um passo do caminho de drill (valor escolhido num nível
/// acima do atual). Vira um `NativeFilterInput` concreto e entra no contexto
/// de filtro via `build_filter_context`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDrillFilter {
    pub field: String,
    #[serde(default)]
    pub date_part: Option<String>,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Frente G: quando não vazio, substitui `x_axis_field` como agrupamento
    /// do eixo (1 nível = drill; N níveis = "expandir todos os níveis", com
    /// as categorias sendo as tuplas que têm dados).
    #[serde(default)]
    pub category_levels: Vec<NativeGroupLevel>,
    /// Frente G: caminho de drill (filtros dos níveis acima do atual).
    #[serde(default)]
    pub drill_filters: Vec<NativeDrillFilter>,
    /// Frente G: linhas da matriz (N níveis). Com 2+ níveis a resposta traz
    /// `matrix` (árvore com subtotais avaliados pelo motor em cada nível).
    #[serde(default)]
    pub matrix_rows: Vec<NativeGroupLevel>,
    /// Linhas com profundidade < `matrix_expand_depth` vêm expandidas.
    #[serde(default)]
    pub matrix_expand_depth: usize,
    /// Caminhos (chaves de cada nível) expandidos individualmente (+).
    #[serde(default)]
    pub matrix_expanded: Vec<Vec<JsonValue>>,
    /// Caminhos recolhidos individualmente (−), vencem `matrix_expand_depth`.
    #[serde(default)]
    pub matrix_collapsed: Vec<Vec<JsonValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualSeries {
    pub name: String,
    pub data: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualQueryResult {
    pub categories: Vec<String>,
    pub series: Vec<NativeVisualSeries>,
    pub summary_value: Option<f64>,
    pub query_time_ms: f64,
    pub engine: String,
    /// Frente G: valor (chave) de cada nível para cada categoria, alinhado
    /// com `categories`. Só presente quando a requisição usa `category_levels`
    /// ou `matrix_rows`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category_keys: Vec<Vec<JsonValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<NativeMatrixResult>,
}

/// Frente G: uma linha da matriz hierárquica. `values` é a medida avaliada no
/// contexto de filtro deste nó (subtotal do nível, NUNCA a soma dos filhos).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMatrixNode {
    pub key: JsonValue,
    pub label: String,
    pub values: Vec<Option<f64>>,
    /// Com legenda (colunas): a medida no contexto da linha sem o filtro da coluna.
    pub row_total: Option<f64>,
    pub has_children: bool,
    pub expanded: bool,
    pub children: Vec<NativeMatrixNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMatrixResult {
    pub rows: Vec<NativeMatrixNode>,
    /// Total geral por coluna (série), avaliado no contexto base.
    pub column_totals: Vec<Option<f64>>,
    /// Com legenda: total geral da medida principal (canto inferior direito).
    pub grand_total: Option<f64>,
}

// ===========================================================================
// v0.11 Rodada 2, Frente E: native "table" visual query.
//
// Replaces the TS path (`src/engine/query/planner.ts` + `dataEngine.ts`,
// called from `VisualCard.tsx`) which aggregates `table.rows` — a preview
// capped at `PREVIEW_ROW_LIMIT` (5000) for DuckDB-backed tables, hence the
// "amostra de 5.000 linhas" warning. This path instead runs over EVERY row
// via `model.get_visible_row_indices` (which reads the full DuckDB-backed
// `Table`, not the capped preview), with the total row evaluated by the
// engine (measure in the *total* filter context, never a sum of the
// returned page), server-side sort, Top N, and offset/limit pagination so
// a 10M-row table never crosses the Tauri IPC boundary in one shot.
// ===========================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTableQueryRequest {
    pub table_id: String,
    /// Group-by columns ("resumir" in Power BI). Empty = raw/detail rows
    /// ("não resumir"): one row per underlying fact row.
    #[serde(default)]
    pub group_fields: Vec<String>,
    /// Detail columns shown as-is when `group_fields` is empty.
    #[serde(default)]
    pub raw_fields: Vec<String>,
    /// Measure (or column+aggregation) columns, evaluated in each row's context.
    #[serde(default)]
    pub value_fields: Vec<String>,
    pub aggregation: Option<String>,
    #[serde(default)]
    pub filters: Vec<NativeFilterInput>,
    /// Name of a column in `columns` (below) to sort the full result by,
    /// before Top N / pagination — mirrors the table's `sortByColumn`.
    pub sort_field: Option<String>,
    #[serde(default)]
    pub sort_order: String, // "asc" | "desc", default asc
    /// Keep only the first N rows after sorting (0 = no Top N).
    #[serde(default)]
    pub top_n: usize,
    /// Pagination window applied after sort + Top N.
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NativeTableRow {
    /// One JSON value per entry of `NativeTableQueryResult::columns`, in order.
    pub values: Vec<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTableQueryResult {
    pub columns: Vec<String>,
    /// The requested page (`offset..offset+limit`) of the sorted/Top-N'd result.
    pub rows: Vec<NativeTableRow>,
    /// Total number of rows AFTER grouping/Top N but BEFORE pagination, so the
    /// frontend can page/scroll without ever receiving the full set.
    pub total_row_count: usize,
    /// Value columns in the total row, each evaluated by the engine in the
    /// TOTAL filter context (NOT a client-side sum of the returned page).
    pub total_row: Option<NativeTableRow>,
    pub query_time_ms: f64,
    pub engine: String,
}

fn cmp_json_for_sort(a: &JsonValue, b: &JsonValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a
            .as_str()
            .unwrap_or_default()
            .cmp(b.as_str().unwrap_or_default()),
    }
}

/// Builds grouped table rows recursively, one `group_fields` level at a
/// time, so each level's distinct values are only those actually present
/// under the levels above it (never a full cross product).
fn build_grouped_table_rows(
    model: &DaxModel,
    req: &NativeTableQueryRequest,
    ctx: &FilterContext,
    levels: &[String],
    acc: &mut Vec<JsonValue>,
    out: &mut Vec<Vec<JsonValue>>,
) {
    if levels.is_empty() {
        let agg = req.aggregation.as_deref().unwrap_or("SUM");
        let mut row = acc.clone();
        for vf in &req.value_fields {
            row.push(
                eval_field_opt(model, ctx, &req.table_id, vf, agg)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
        }
        out.push(row);
        return;
    }
    let fake_req = NativeVisualQueryRequest {
        table_id: req.table_id.clone(),
        ..Default::default()
    };
    let (tbl_name, col_name, groups) = collect_groups(
        model,
        &fake_req,
        ctx,
        &NativeGroupLevel { field: levels[0].clone(), date_part: None },
    );
    for g in groups {
        if g.label.is_empty() {
            continue;
        }
        let mut ctx2 = ctx.clone();
        ctx2.set_column_filter(&tbl_name, &col_name, g.allowed.clone());
        acc.push(g.key.clone());
        build_grouped_table_rows(model, req, &ctx2, &levels[1..], acc, out);
        acc.pop();
    }
}

/// Executes a native "table" visual query over ALL rows of the model (no
/// 5.000-row preview cap). See module header above for the design.
pub fn execute_table_query(
    model: &DaxModel,
    req: &NativeTableQueryRequest,
) -> Result<NativeTableQueryResult, String> {
    let t0 = Instant::now();
    let base_ctx = build_filter_context(&req.filters, model);

    let (columns, mut all_rows): (Vec<String>, Vec<Vec<JsonValue>>) = if !req.group_fields.is_empty() {
        let mut cols = req.group_fields.clone();
        cols.extend(req.value_fields.iter().cloned());
        let mut out = Vec::new();
        build_grouped_table_rows(model, req, &base_ctx, &req.group_fields, &mut Vec::new(), &mut out);
        (cols, out)
    } else {
        // Raw/detail rows ("não resumir"): one row per visible fact row.
        let mut cols = req.raw_fields.clone();
        cols.extend(req.value_fields.iter().cloned());
        let tbl = model
            .get_table(&req.table_id)
            .ok_or_else(|| format!("tabela '{}' não encontrada", req.table_id))?;
        let raw_idx: Vec<Option<usize>> = req.raw_fields.iter().map(|f| tbl.get_col_idx(f)).collect();
        let mut out = Vec::new();
        for r_idx in model.get_visible_row_indices(&tbl.name, &base_ctx) {
            let mut row = Vec::with_capacity(cols.len());
            for cidx in &raw_idx {
                row.push(match cidx {
                    Some(c) => tbl.rows[r_idx][*c].to_json(),
                    None => JsonValue::Null,
                });
            }
            // Value fields in raw mode show the row's own column value when
            // it IS a plain column (no aggregation makes sense per-row);
            // measures fall back to the table-wide total context — a known
            // gap, see docs/V011.md Rodada 2 Frente E report.
            for vf in &req.value_fields {
                let clean = vf.trim().trim_start_matches('Σ').trim();
                if let Some(c) = tbl.get_col_idx(clean) {
                    row.push(tbl.rows[r_idx][c].to_json());
                } else {
                    row.push(
                        eval_field_opt(model, &base_ctx, &req.table_id, clean, req.aggregation.as_deref().unwrap_or("SUM"))
                            .map(JsonValue::from)
                            .unwrap_or(JsonValue::Null),
                    );
                }
            }
            out.push(row);
        }
        (cols, out)
    };

    if let Some(sort_field) = req.sort_field.as_deref() {
        if let Some(sort_idx) = columns.iter().position(|c| c == sort_field) {
            let desc = req.sort_order.eq_ignore_ascii_case("desc");
            all_rows.sort_by(|a, b| {
                let ord = cmp_json_for_sort(&a[sort_idx], &b[sort_idx]);
                if desc { ord.reverse() } else { ord }
            });
        }
    }

    if req.top_n > 0 && all_rows.len() > req.top_n {
        all_rows.truncate(req.top_n);
    }

    let total_row_count = all_rows.len();
    let offset = req.offset.min(all_rows.len());
    let end = req.limit.map(|l| (offset + l).min(all_rows.len())).unwrap_or(all_rows.len());
    let page: Vec<NativeTableRow> = all_rows[offset..end]
        .iter()
        .cloned()
        .map(|values| NativeTableRow { values })
        .collect();

    let agg = req.aggregation.as_deref().unwrap_or("SUM");
    let total_row = if !req.value_fields.is_empty() {
        let mut vals = vec![JsonValue::Null; req.group_fields.len().max(req.raw_fields.len())];
        for vf in &req.value_fields {
            vals.push(
                eval_field_opt(model, &base_ctx, &req.table_id, vf, agg)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
        }
        Some(NativeTableRow { values: vals })
    } else {
        None
    };

    Ok(NativeTableQueryResult {
        columns,
        rows: page,
        total_row_count,
        total_row,
        query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
        engine: "rust-native".to_string(),
    })
}

// ===========================================================================
// v0.11 Rodada 2, Frente E: native "scatter" visual query, with Power-BI-style
// deterministic sampling (default cap mirrors Power BI's ~3.500 points) so a
// detail field with millions of distinct values never ships every point
// over IPC. Sampling is a fixed stride over the sorted point list, so the
// same input always yields the same sampled subset (no RNG).
// ===========================================================================

pub const SCATTER_DEFAULT_MAX_POINTS: usize = 3500;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeScatterQueryRequest {
    pub table_id: String,
    /// Field with one point per distinct value (Power BI's "Detalhes").
    pub detail_field: String,
    pub x_field: String,
    pub y_field: String,
    pub size_field: Option<String>,
    pub legend_field: Option<String>,
    pub aggregation: Option<String>,
    #[serde(default)]
    pub filters: Vec<NativeFilterInput>,
    /// 0 = use `SCATTER_DEFAULT_MAX_POINTS`.
    #[serde(default)]
    pub max_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeScatterPoint {
    pub label: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub size: Option<f64>,
    pub legend: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeScatterQueryResult {
    pub points: Vec<NativeScatterPoint>,
    pub total_points: usize,
    pub sampled: bool,
    pub query_time_ms: f64,
    pub engine: String,
}

pub fn execute_scatter_query(
    model: &DaxModel,
    req: &NativeScatterQueryRequest,
) -> Result<NativeScatterQueryResult, String> {
    let t0 = Instant::now();
    let base_ctx = build_filter_context(&req.filters, model);
    let agg = req.aggregation.as_deref().unwrap_or("SUM");

    let fake_req = NativeVisualQueryRequest {
        table_id: req.table_id.clone(),
        ..Default::default()
    };
    let (tbl_name, col_name, groups) = collect_groups(
        model,
        &fake_req,
        &base_ctx,
        &NativeGroupLevel { field: req.detail_field.clone(), date_part: None },
    );

    let mut all_points = Vec::with_capacity(groups.len());
    for g in groups {
        if g.label.is_empty() {
            continue;
        }
        let mut ctx2 = base_ctx.clone();
        ctx2.set_column_filter(&tbl_name, &col_name, g.allowed.clone());
        let x = eval_field_opt(model, &ctx2, &req.table_id, &req.x_field, agg);
        let y = eval_field_opt(model, &ctx2, &req.table_id, &req.y_field, agg);
        let size = req
            .size_field
            .as_deref()
            .and_then(|sf| eval_field_opt(model, &ctx2, &req.table_id, sf, agg));
        let legend = req.legend_field.as_deref().and_then(|lf| {
            let (lt, lc) = resolve_table_and_col(model, &req.table_id, lf);
            let tbl = model.get_table(&lt)?;
            let cidx = tbl.get_col_idx(&lc)?;
            model
                .get_visible_row_indices(&tbl.name, &ctx2)
                .first()
                .map(|&i| tbl.rows[i][cidx].as_display_string())
        });
        all_points.push(NativeScatterPoint { label: g.label, x, y, size, legend });
    }

    let total_points = all_points.len();
    let max_points = if req.max_points == 0 { SCATTER_DEFAULT_MAX_POINTS } else { req.max_points };
    let sampled = total_points > max_points;
    let points = if sampled {
        // Deterministic fixed-stride sample: evenly spread across the
        // (already deterministically ordered) point list, always picking
        // index 0 and walking by a float step to land close to `max_points`
        // samples without any randomness.
        let step = total_points as f64 / max_points as f64;
        let mut out = Vec::with_capacity(max_points);
        let mut i = 0.0f64;
        let mut seen = std::collections::HashSet::new();
        while out.len() < max_points && (i as usize) < total_points {
            let idx = i as usize;
            if seen.insert(idx) {
                out.push(all_points[idx].clone());
            }
            i += step;
        }
        out
    } else {
        all_points
    };

    Ok(NativeScatterQueryResult {
        points,
        total_points,
        sampled,
        query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
        engine: "rust-native".to_string(),
    })
}

pub(crate) fn resolve_table_and_col(model: &DaxModel, default_table: &str, raw_field: &str) -> (String, String) {
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
    eval_field_opt(model, ctx, default_table, field_name, agg).unwrap_or(0.0)
}

/// Like `eval_field_value`, but `None` when the result is BLANK (a measure
/// returning BLANK, or an implicit aggregation over no non-blank cells), so
/// grouped queries can drop rows with no data, as Power BI does.
fn eval_field_opt(
    model: &DaxModel,
    ctx: &FilterContext,
    default_table: &str,
    field_name: &str,
    agg: &str,
) -> Option<f64> {
    let clean_name = field_name.trim().trim_start_matches('Σ').trim();
    if model.get_measure(clean_name).is_some() {
        return model
            .evaluate_dax(&format!("[{}]", clean_name), ctx)
            .ok()
            .filter(|v| !v.is_blank())
            .and_then(|v| v.as_f64());
    }

    let (tbl_name, col_name) = resolve_table_and_col(model, default_table, clean_name);
    let tbl = model.get_table(&tbl_name)?;
    let cidx = tbl.get_col_idx(&col_name)?;
    let visible = model.get_visible_row_indices(&tbl.name, ctx);

    let non_zero = |n: f64| if n == 0.0 { None } else { Some(n) };
    match agg.to_uppercase().as_str() {
        "COUNT" => non_zero(visible.iter().filter(|&&i| !tbl.rows[i][cidx].is_blank()).count() as f64),
        "DISTINCTCOUNT" => non_zero(
            visible
                .iter()
                .filter(|&&i| !tbl.rows[i][cidx].is_blank())
                .map(|&i| tbl.rows[i][cidx].canonical_key())
                .collect::<HashSet<_>>()
                .len() as f64,
        ),
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
                return None;
            }
            Some(match agg.to_uppercase().as_str() {
                "AVG" | "AVERAGE" => nums.iter().sum::<f64>() / nums.len() as f64,
                "MIN" => nums.iter().copied().fold(f64::INFINITY, f64::min),
                "MAX" => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                _ => nums.iter().sum(),
            })
        }
    }
}

pub fn execute_visual_query(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
) -> Result<NativeVisualQueryResult, String> {
    execute_visual_query_with_conn(model, req, None)
}

/// v0.11 Frente A: like `execute_visual_query`, but when `conn` is given and
/// `VETROBI_PUSHDOWN` isn't `"0"`, first tries to translate the request into
/// DuckDB SQL (see `dax::pushdown`). Any rejection (unsupported shape) or SQL
/// error falls back to this function's normal DAX-engine path unchanged —
/// pushdown never turns a working query into a hard error, only a slower one.
pub fn execute_visual_query_with_conn(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    conn: Option<&duckdb::Connection>,
) -> Result<NativeVisualQueryResult, String> {
    let t0 = Instant::now();
    let agg = req.aggregation.as_deref().unwrap_or("SUM");

    if let Some(conn) = conn {
        if crate::dax::pushdown::pushdown_enabled() {
            match crate::dax::pushdown::try_pushdown(model, conn, req, agg, t0) {
                Ok(result) => return Ok(result),
                Err(reason) => {
                    eprintln!(
                        "pushdown: fallback para o motor DAX (visual '{}'): {}",
                        req.visual_id, reason
                    );
                }
            }
        }
    }
    execute_visual_query_dax(model, req, t0)
}

fn execute_visual_query_dax(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    t0: Instant,
) -> Result<NativeVisualQueryResult, String> {
    let agg = req.aggregation.as_deref().unwrap_or("SUM");
    let base_ctx = build_query_context(model, req);

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
                data: vec![Some(summary)],
            }],
            summary_value: Some(summary),
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
            category_keys: Vec::new(),
            matrix: None,
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
                data: vals.into_iter().map(Some).collect(),
            }],
            summary_value: summary,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
            category_keys: Vec::new(),
            matrix: None,
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
        let slicer_ctx = build_filter_context(&remaining_filters, model);
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
        let ones = vec![Some(1.0); cat_vec.len()];
        return Ok(NativeVisualQueryResult {
            categories: cat_vec,
            series: vec![NativeVisualSeries {
                name: slicer_col,
                data: ones,
            }],
            summary_value: None,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
            category_keys: Vec::new(),
            matrix: None,
        });
    }

    // 4a. Frente G: matriz com N níveis (subtotal por nível avaliado pelo motor)
    if !req.matrix_rows.is_empty() {
        return Ok(execute_matrix_query(model, req, &base_ctx, agg, t0));
    }

    // 4b. Frente G: eixo por níveis (drill / expandir todos os níveis / hierarquia de data)
    if !req.category_levels.is_empty() {
        return Ok(execute_levels_query(model, req, &base_ctx, agg, t0));
    }

    // 4. Grouped visuals & matrix
    let raw_cat = req.x_axis_field.as_deref().unwrap_or("");
    let (cat_tbl_name, cat_col) = resolve_table_and_col(model, &req.table_id, raw_cat);
    let cat_entries = collect_sorted_entries(model, &cat_tbl_name, &cat_col, &base_ctx, true);

    if let Some(ref leg_raw) = req.legend_field.as_ref().filter(|s| !s.trim().is_empty()) {
        let (leg_tbl_name, leg_col) = resolve_table_and_col(model, &req.table_id, leg_raw);
        let leg_entries = collect_sorted_entries(model, &leg_tbl_name, &leg_col, &base_ctx, false);
        let primary_y = req.y_axis_fields.first().map(String::as_str).unwrap_or("");

        // Evaluate each (cat, leg) cell via eval_field_opt; keep only rows where at
        // least one cell is non-blank, and columns that have data on kept rows.
        let mut grid: Vec<Vec<Option<f64>>> = Vec::with_capacity(cat_entries.len());
        let mut kept_cats: Vec<String> = Vec::with_capacity(cat_entries.len());
        for (cat_label, cat_key) in &cat_entries {
            let mut row_vals = Vec::with_capacity(leg_entries.len());
            for (_, leg_key) in &leg_entries {
                let mut cell_ctx = base_ctx.clone();
                let mut s1 = HashSet::new();
                s1.insert(cat_key.clone());
                cell_ctx.set_column_filter(&cat_tbl_name, &cat_col, s1);
                let mut s2 = HashSet::new();
                s2.insert(leg_key.clone());
                cell_ctx.set_column_filter(&leg_tbl_name, &leg_col, s2);
                let val = if primary_y.is_empty() {
                    None
                } else {
                    eval_field_opt(model, &cell_ctx, &req.table_id, primary_y, agg)
                };
                row_vals.push(val);
            }
            if row_vals.iter().any(Option::is_some) {
                kept_cats.push(cat_label.clone());
                grid.push(row_vals);
            }
        }

        let mut out_series = Vec::with_capacity(leg_entries.len());
        for (l_idx, (leg_label, _)) in leg_entries.iter().enumerate() {
            let data: Vec<Option<f64>> = grid.iter().map(|r| r[l_idx]).collect();
            if data.iter().any(Option::is_some) {
                out_series.push(NativeVisualSeries {
                    name: leg_label.clone(),
                    data,
                });
            }
        }
        return Ok(NativeVisualQueryResult {
            categories: kept_cats,
            series: out_series,
            summary_value: None,
            query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
            engine: "rust-native".to_string(),
            category_keys: Vec::new(),
            matrix: None,
        });
    }

    // Standard multi-measure or single-measure grouped series
    let y_names: Vec<String> = req
        .y_axis_fields
        .iter()
        .map(|yf| yf.trim().trim_start_matches('Σ').trim().to_string())
        .collect();
    let mut kept_cats: Vec<String> = Vec::with_capacity(cat_entries.len());
    let mut grid: Vec<Vec<Option<f64>>> = Vec::with_capacity(cat_entries.len());
    for (cat_label, cat_key) in &cat_entries {
        let mut cctx = base_ctx.clone();
        let mut s = HashSet::new();
        s.insert(cat_key.clone());
        cctx.set_column_filter(&cat_tbl_name, &cat_col, s);
        let row_vals: Vec<Option<f64>> = y_names
            .iter()
            .map(|clean_y| eval_field_opt(model, &cctx, &req.table_id, clean_y, agg))
            .collect();
        if row_vals.iter().any(Option::is_some) {
            kept_cats.push(cat_label.clone());
            grid.push(row_vals);
        }
    }

    let mut out_series = Vec::with_capacity(y_names.len());
    for (y_idx, clean_y) in y_names.into_iter().enumerate() {
        out_series.push(NativeVisualSeries {
            name: clean_y,
            data: grid.iter().map(|r| r[y_idx]).collect(),
        });
    }

    Ok(NativeVisualQueryResult {
        categories: kept_cats,
        series: out_series,
        summary_value: None,
        query_time_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
        engine: "rust-native".to_string(),
        category_keys: Vec::new(),
        matrix: None,
    })
}

// ---------------------------------------------------------------------------
// v0.10 Frente G: drill-down (caminho de drill, hierarquia de datas, expandir
// todos os níveis) e matriz com N níveis. Toda medida é avaliada pelo motor no
// contexto de filtro do nó/categoria; nada é somado a partir dos filhos.
// ---------------------------------------------------------------------------

/// Valor impossível usado quando o caminho de drill não casa com nada (um
/// filtro com conjunto vazio seria ignorado por `build_filter_context`).
const NO_MATCH_SENTINEL: &str = "\u{0}__vetrobi_drill_no_match__";
const MAX_MATRIX_NODES: usize = 50_000;
const MAX_LEVEL_CATEGORIES: usize = 10_000;

/// Parte de uma data (via `functions::serial_to_civil`, o mesmo cálculo de
/// YEAR/MONTH/DAY do motor): ano, trimestre 1-4, mês 1-12 ou dia 1-31.
fn date_part_of(v: &DaxValue, part: &str) -> Option<i64> {
    let serial = v.as_date_serial()?;
    if !serial.is_finite() {
        return None;
    }
    let (y, m, d) = functions::serial_to_civil(serial);
    Some(match part.to_ascii_lowercase().as_str() {
        "year" => y as i64,
        "quarter" => (m as i64 - 1) / 3 + 1,
        "month" => m as i64,
        "day" => d as i64,
        _ => return None,
    })
}

fn canonical_table(model: &DaxModel, t: &str) -> String {
    model
        .get_table(t)
        .map(|x| x.name.to_lowercase())
        .unwrap_or_else(|| t.to_lowercase())
}

/// Valores concretos da coluna que um passo do caminho de drill seleciona.
fn drill_filter_values(model: &DaxModel, tbl_name: &str, col: &str, df: &NativeDrillFilter) -> Vec<JsonValue> {
    let Some(part) = df.date_part.as_deref() else {
        return vec![df.value.clone()];
    };
    let Some(want) = df.value.as_f64().map(|f| f.round() as i64) else {
        return Vec::new();
    };
    let Some(tbl) = model.get_table(tbl_name) else {
        return Vec::new();
    };
    let Some(cidx) = tbl.get_col_idx(col) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in &tbl.rows {
        let v = &r[cidx];
        if date_part_of(v, part) == Some(want) && seen.insert(v.value_key()) {
            out.push(v.to_json());
        }
    }
    out
}

/// Acrescenta um filtro do caminho de drill. Se já existe filtro na mesma
/// coluna (segmentador, filtro de página...), os dois se intersectam em vez
/// de um sobrescrever o outro dentro de `build_filter_context`.
fn merge_drill_filter(model: &DaxModel, filters: &mut Vec<NativeFilterInput>, new: NativeFilterInput) {
    let col_type = model
        .get_table(&new.table_id)
        .and_then(|t| t.get_col_idx(&new.column).and_then(|i| t.col_types.get(i).copied()));
    let key = |v: &JsonValue| DaxValue::from_json_as(v, col_type).value_key();
    let target_tbl = canonical_table(model, &new.table_id);
    let mut values = new.values;
    let existing = filters
        .iter()
        .rposition(|f| canonical_table(model, &f.table_id) == target_tbl && f.column.eq_ignore_ascii_case(&new.column));
    if let Some(idx) = existing.filter(|&i| !filters[i].values.is_empty()) {
        let allowed: HashSet<ValueKey> = filters[idx].values.iter().map(key).collect();
        values.retain(|v| allowed.contains(&key(v)));
    }
    if values.is_empty() {
        values.push(JsonValue::String(NO_MATCH_SENTINEL.to_string()));
    }
    match existing {
        Some(idx) => filters[idx].values = values,
        None => filters.push(NativeFilterInput {
            table_id: new.table_id,
            column: new.column,
            values,
        }),
    }
}

/// Contexto de filtro da consulta: filtros do relatório/página/visual mais o
/// caminho de drill, tudo pelo `build_filter_context` (inclusive a sombra do
/// ALLSELECTED, que assim também respeita o drill, como no Power BI).
fn build_query_context(model: &DaxModel, req: &NativeVisualQueryRequest) -> FilterContext {
    if req.drill_filters.is_empty() {
        return build_filter_context(&req.filters, model);
    }
    let mut filters = req.filters.clone();
    for df in &req.drill_filters {
        let (tbl_name, col) = resolve_table_and_col(model, &req.table_id, &df.field);
        let values = drill_filter_values(model, &tbl_name, &col, df);
        merge_drill_filter(
            model,
            &mut filters,
            NativeFilterInput {
                table_id: tbl_name,
                column: col,
                values,
            },
        );
    }
    build_filter_context(&filters, model)
}

fn row_sort_key_opt(tbl: &crate::dax::DaxTableData, col_idx: usize, r_idx: usize) -> Option<ValueKey> {
    let sb_idx = tbl.sort_by_col_idx(col_idx)?;
    let sv = &tbl.rows[r_idx][sb_idx];
    if sv.is_blank() {
        None
    } else {
        Some(sv.value_key())
    }
}

fn collect_sorted_entries(
    model: &DaxModel,
    tbl_name: &str,
    col_name: &str,
    ctx: &FilterContext,
    keep_blank_as_sem_dados: bool,
) -> Vec<(String, ValueKey)> {
    let Some(tbl) = model.get_table(tbl_name) else {
        return Vec::new();
    };
    let Some(cidx) = tbl.get_col_idx(col_name) else {
        return Vec::new();
    };
    let mut map: HashMap<String, (Option<ValueKey>, ValueKey)> = HashMap::new();
    for r_idx in model.get_visible_row_indices(&tbl.name, ctx) {
        let v = &tbl.rows[r_idx][cidx];
        let disp = v.as_display_string();
        if disp.is_empty() && !keep_blank_as_sem_dados {
            continue;
        }
        let label = if disp.is_empty() { "Sem dados".to_string() } else { disp };
        let key = v.value_key();
        let sb_key = row_sort_key_opt(tbl, cidx, r_idx);
        let entry = map.entry(label).or_insert((None, key));
        if entry.0.is_none() && sb_key.is_some() {
            entry.0 = sb_key;
        }
    }
    let mut vec: Vec<(ValueKey, String, ValueKey)> = map
        .into_iter()
        .map(|(label, (sb_opt, vkey))| {
            let order_key = sb_opt.unwrap_or_else(|| vkey.clone());
            (order_key, label, vkey)
        })
        .collect();
    vec.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    vec.into_iter().map(|(_, label, vkey)| (label, vkey)).collect()
}

struct LevelGroup {
    key: JsonValue,
    label: String,
    allowed: HashSet<ValueKey>,
}

/// Valores distintos de um nível visíveis em `ctx`, em ordem (chave tipada ou
/// `sortByColumn` configurada; partes de data em ordem numérica). Cada grupo
/// carrega o conjunto de valores brutos da coluna que ele representa.
fn collect_groups(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    ctx: &FilterContext,
    level: &NativeGroupLevel,
) -> (String, String, Vec<LevelGroup>) {
    let (tbl_name, col) = resolve_table_and_col(model, &req.table_id, &level.field);
    let mut map: HashMap<ValueKey, (Option<ValueKey>, LevelGroup)> = HashMap::new();
    let Some(tbl) = model.get_table(&tbl_name) else {
        return (tbl_name, col, Vec::new());
    };
    let Some(cidx) = tbl.get_col_idx(&col) else {
        return (tbl.name.clone(), col, Vec::new());
    };
    for r_idx in model.get_visible_row_indices(&tbl.name, ctx) {
        let v = &tbl.rows[r_idx][cidx];
        let (group_key, sort_opt, key, label) = match level.date_part.as_deref() {
            Some(part) => match date_part_of(v, part) {
                Some(n) => {
                    let nk = ValueKey::Num(n.saturating_mul(1_000_000));
                    (nk.clone(), Some(nk), JsonValue::from(n), n.to_string())
                }
                None => (ValueKey::Blank, Some(ValueKey::Blank), JsonValue::Null, String::new()),
            },
            None => (
                v.value_key(),
                row_sort_key_opt(tbl, cidx, r_idx),
                v.to_json(),
                v.as_display_string(),
            ),
        };
        let entry = map.entry(group_key).or_insert_with(|| {
            (
                None,
                LevelGroup {
                    key,
                    label,
                    allowed: HashSet::new(),
                },
            )
        });
        if entry.0.is_none() && sort_opt.is_some() {
            entry.0 = sort_opt;
        }
        entry.1.allowed.insert(v.value_key());
    }
    let mut vec: Vec<(ValueKey, ValueKey, LevelGroup)> = map
        .into_iter()
        .map(|(gkey, (sb_opt, group))| {
            let order_key = sb_opt.unwrap_or_else(|| gkey.clone());
            (order_key, gkey, group)
        })
        .collect();
    vec.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    (tbl.name.clone(), col, vec.into_iter().map(|(_, _, g)| g).collect())
}

/// Séries do visual: uma por campo de valor, ou uma por valor da legenda
/// (colunas da matriz) com a medida principal.
enum SeriesSpec {
    Fields(Vec<String>),
    Legend {
        tbl: String,
        col: String,
        entries: Vec<(String, HashSet<ValueKey>)>,
        primary: String,
    },
}

impl SeriesSpec {
    fn new(model: &DaxModel, req: &NativeVisualQueryRequest, base_ctx: &FilterContext) -> Self {
        let fields: Vec<String> = req
            .y_axis_fields
            .iter()
            .map(|y| y.trim().trim_start_matches('Σ').trim().to_string())
            .collect();
        match req.legend_field.as_ref().filter(|s| !s.trim().is_empty()) {
            Some(leg) => {
                let (tbl, col, groups) = collect_groups(
                    model,
                    req,
                    base_ctx,
                    &NativeGroupLevel {
                        field: leg.clone(),
                        date_part: None,
                    },
                );
                SeriesSpec::Legend {
                    tbl,
                    col,
                    entries: groups
                        .into_iter()
                        .filter(|g| !g.label.is_empty())
                        .map(|g| (g.label, g.allowed))
                        .collect(),
                    primary: fields.first().cloned().unwrap_or_default(),
                }
            }
            None => SeriesSpec::Fields(fields),
        }
    }

    fn names(&self) -> Vec<String> {
        match self {
            SeriesSpec::Fields(f) => f.clone(),
            SeriesSpec::Legend { entries, .. } => entries.iter().map(|(n, _)| n.clone()).collect(),
        }
    }

    fn eval_values(&self, model: &DaxModel, req: &NativeVisualQueryRequest, ctx: &FilterContext, agg: &str) -> Vec<Option<f64>> {
        match self {
            SeriesSpec::Fields(fields) => fields
                .iter()
                .map(|f| eval_field_opt(model, ctx, &req.table_id, f, agg))
                .collect(),
            SeriesSpec::Legend { tbl, col, entries, primary } => entries
                .iter()
                .map(|(_, allowed)| {
                    if primary.is_empty() {
                        return None;
                    }
                    let mut cctx = ctx.clone();
                    cctx.set_column_filter(tbl, col, allowed.clone());
                    eval_field_opt(model, &cctx, &req.table_id, primary, agg)
                })
                .collect(),
        }
    }

    fn eval_row_total(&self, model: &DaxModel, req: &NativeVisualQueryRequest, ctx: &FilterContext, agg: &str) -> Option<f64> {
        match self {
            SeriesSpec::Fields(_) => None,
            SeriesSpec::Legend { primary, .. } if !primary.is_empty() => eval_field_opt(model, ctx, &req.table_id, primary, agg),
            SeriesSpec::Legend { .. } => None,
        }
    }
}

type Leaf = (Vec<JsonValue>, Vec<String>, FilterContext);

#[allow(clippy::too_many_arguments)]
fn enumerate_leaves(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    ctx: &FilterContext,
    levels: &[NativeGroupLevel],
    depth: usize,
    keys: &mut Vec<JsonValue>,
    labels: &mut Vec<String>,
    out: &mut Vec<Leaf>,
) {
    let (tbl, col, groups) = collect_groups(model, req, ctx, &levels[depth]);
    for g in groups {
        if out.len() >= MAX_LEVEL_CATEGORIES {
            return;
        }
        let mut cctx = ctx.clone();
        cctx.set_column_filter(&tbl, &col, g.allowed);
        keys.push(g.key);
        labels.push(g.label);
        if depth + 1 == levels.len() {
            out.push((keys.clone(), labels.clone(), cctx));
        } else {
            enumerate_leaves(model, req, &cctx, levels, depth + 1, keys, labels, out);
        }
        keys.pop();
        labels.pop();
    }
}

fn elapsed_ms(t0: Instant) -> f64 {
    (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0
}

/// Eixo agrupado por `category_levels` (1 nível = drill; N = tuplas com dados,
/// rótulos concatenados). Categorias sem nenhum valor (tudo BLANK) somem.
fn execute_levels_query(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    base_ctx: &FilterContext,
    agg: &str,
    t0: Instant,
) -> NativeVisualQueryResult {
    let spec = SeriesSpec::new(model, req, base_ctx);
    let names = spec.names();
    let mut leaves = Vec::new();
    enumerate_leaves(model, req, base_ctx, &req.category_levels, 0, &mut Vec::new(), &mut Vec::new(), &mut leaves);
    let mut categories = Vec::with_capacity(leaves.len());
    let mut category_keys = Vec::with_capacity(leaves.len());
    let mut data: Vec<Vec<Option<f64>>> = vec![Vec::with_capacity(leaves.len()); names.len()];
    for (keys, labels, ctx) in leaves {
        let vals = spec.eval_values(model, req, &ctx, agg);
        if vals.iter().all(Option::is_none) {
            continue;
        }
        categories.push(labels.join(" "));
        category_keys.push(keys);
        for (i, v) in vals.into_iter().enumerate() {
            data[i].push(v);
        }
    }
    NativeVisualQueryResult {
        categories,
        series: names
            .into_iter()
            .zip(data)
            .map(|(name, data)| NativeVisualSeries { name, data })
            .collect(),
        summary_value: None,
        query_time_ms: elapsed_ms(t0),
        engine: "rust-native".to_string(),
        category_keys,
        matrix: None,
    }
}

fn is_matrix_path_expanded(req: &NativeVisualQueryRequest, path: &[JsonValue]) -> bool {
    if req.matrix_collapsed.iter().any(|p| p.as_slice() == path) {
        return false;
    }
    if req.matrix_expanded.iter().any(|p| p.as_slice() == path) {
        return true;
    }
    path.len().saturating_sub(1) < req.matrix_expand_depth
}

#[allow(clippy::too_many_arguments)]
fn build_matrix_nodes(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    ctx: &FilterContext,
    agg: &str,
    spec: &SeriesSpec,
    depth: usize,
    path: &mut Vec<JsonValue>,
    budget: &mut usize,
) -> Vec<NativeMatrixNode> {
    let levels = &req.matrix_rows;
    let (tbl, col, groups) = collect_groups(model, req, ctx, &levels[depth]);
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        if *budget == 0 {
            break;
        }
        let mut nctx = ctx.clone();
        nctx.set_column_filter(&tbl, &col, g.allowed);
        let vals = spec.eval_values(model, req, &nctx, agg);
        let row_total = spec.eval_row_total(model, req, &nctx, agg);
        if vals.iter().all(Option::is_none) && row_total.is_none() {
            continue;
        }
        *budget -= 1;
        path.push(g.key.clone());
        let has_children = depth + 1 < levels.len();
        let expanded = has_children && is_matrix_path_expanded(req, path);
        let children = if expanded {
            build_matrix_nodes(model, req, &nctx, agg, spec, depth + 1, path, budget)
        } else {
            Vec::new()
        };
        path.pop();
        out.push(NativeMatrixNode {
            key: g.key,
            label: g.label,
            values: vals,
            row_total,
            has_children,
            expanded,
            children,
        });
    }
    out
}

/// Matriz com N níveis nas linhas. Cada nó (e cada total) é a medida
/// avaliada no próprio contexto de filtro, então medidas não aditivas
/// (DISTINCTCOUNT, AVERAGE...) têm subtotais corretos.
fn execute_matrix_query(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    base_ctx: &FilterContext,
    agg: &str,
    t0: Instant,
) -> NativeVisualQueryResult {
    let spec = SeriesSpec::new(model, req, base_ctx);
    let names = spec.names();
    let mut budget = MAX_MATRIX_NODES;
    let rows = build_matrix_nodes(model, req, base_ctx, agg, &spec, 0, &mut Vec::new(), &mut budget);
    let column_totals = spec.eval_values(model, req, base_ctx, agg);
    let grand_total = spec.eval_row_total(model, req, base_ctx, agg);
    let series = names
        .into_iter()
        .enumerate()
        .map(|(i, name)| NativeVisualSeries {
            name,
            data: rows.iter().map(|r| r.values.get(i).copied().flatten()).collect(),
        })
        .collect();
    NativeVisualQueryResult {
        categories: rows.iter().map(|r| r.label.clone()).collect(),
        series,
        summary_value: None,
        query_time_ms: elapsed_ms(t0),
        engine: "rust-native".to_string(),
        category_keys: rows.iter().map(|r| vec![r.key.clone()]).collect(),
        matrix: Some(NativeMatrixResult {
            rows,
            column_totals,
            grand_total,
        }),
    }
}

#[cfg(test)]
#[path = "tests_drill.rs"]
mod drill_tests;


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
            tables: vec![DaxTableInput { duckdb_table: None,
                name: "fVendas".into(),
                columns: vec!["Categoria".into(), "Valor".into()],
                rows: vec![
                    make_row("A", 100.0),
                    make_row("A", 50.0),
                    make_row("B", 200.0),
                ],
                column_types: vec![],
                sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
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
                ..Default::default()
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
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(mc_res.categories, vec!["Receita", "Qtd"]);
        assert_eq!(mc_res.series[0].data, vec![Some(350.0), Some(3.0)]);

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
                ..Default::default()
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
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(col_res.categories, vec!["A", "B"]);
        assert_eq!(col_res.series[0].data, vec![Some(150.0), Some(200.0)]);
    }

    #[test]
    fn test_matrix_legend_sort_by_column_and_sumx_blank_omits_empty_rows() {
        use serde_json::json;
        let row_map = |pairs: &[(&str, serde_json::Value)]| -> serde_json::Map<String, serde_json::Value> {
            pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
        };
        let mut model = DaxModel::new();
        let mut sort_by = HashMap::new();
        sort_by.insert("MesNome".to_string(), "MesNum".to_string());
        let mut cat_sort_by = HashMap::new();
        cat_sort_by.insert("Linha".to_string(), "Ordem".to_string());

        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "dCal".into(),
                    columns: vec!["Data".into(), "MesNome".into(), "MesNum".into()],
                    rows: vec![
                        row_map(&[("Data", json!("2026-01-10")), ("MesNome", json!("jan")), ("MesNum", json!(1))]),
                        row_map(&[("Data", json!("2026-04-10")), ("MesNome", json!("abr")), ("MesNum", json!(4))]),
                        row_map(&[("Data", json!("2026-11-10")), ("MesNome", json!("nov")), ("MesNum", json!(11))]),
                    ],
                    column_types: vec![Some("date".into()), Some("text".into()), Some("number".into())],
                    sort_by_columns: sort_by,
                    ..Default::default() 
                },
                DaxTableInput {
                    name: "dEstrutura".into(),
                    columns: vec!["Linha".into(), "Ordem".into()],
                    rows: vec![
                        row_map(&[("Linha", json!("Receita")), ("Ordem", json!(1))]),
                        row_map(&[("Linha", json!("ZeroReal")), ("Ordem", json!(2))]),
                        row_map(&[("Linha", json!("—")), ("Ordem", json!(3))]),
                    ],
                    column_types: vec![Some("text".into()), Some("number".into())],
                    sort_by_columns: cat_sort_by,
                    ..Default::default() 
                },
                DaxTableInput {
                    name: "fLanc".into(),
                    columns: vec!["Data".into(), "Linha".into(), "Valor".into()],
                    rows: vec![
                        // Only Jan & Nov have data for Receita (Apr has none -> None in grid, NOT 0)
                        row_map(&[("Data", json!("2026-01-10")), ("Linha", json!("Receita")), ("Valor", json!(1000))]),
                        row_map(&[("Data", json!("2026-11-10")), ("Linha", json!("Receita")), ("Valor", json!(2500))]),
                        // ZeroReal has an actual matching row of 0 in Apr -> preserved as Some(0.0)
                        row_map(&[("Data", json!("2026-04-10")), ("Linha", json!("ZeroReal")), ("Valor", json!(0))]),
                        // "—" has NO rows in fLanc -> SUMX returns BLANK on all months -> row dropped
                    ],
                    column_types: vec![Some("date".into()), Some("text".into()), Some("number".into())],
                    ..Default::default() 
                },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "fLanc".into(),
                    from_column: "Data".into(),
                    to_table: "dCal".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "fLanc".into(),
                    from_column: "Linha".into(),
                    to_table: "dEstrutura".into(),
                    to_column: "Linha".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
            ],
            measures: vec![DaxMeasureInput {
                name: "DRE Valor".into(),
                table_name: "fLanc".into(),
                formula: "-SUMX(FILTER(fLanc, fLanc[Valor] >= 0), fLanc[Valor])".into(),
            }],
        });

        // Empty filter SUMX & unary negation must return Blank
        let mut ctx_dash = FilterContext::default();
        let mut s_dash = HashSet::new();
        s_dash.insert(ValueKey::Text("—".into()));
        ctx_dash.set_column_filter("dEstrutura", "Linha", s_dash);
        assert_eq!(model.evaluate_dax("[DRE Valor]", &ctx_dash).unwrap(), DaxValue::Blank);

        // 1-level matrix: columns ordered by MesNum (jan, apr, nov) NOT alphabetical (abr, jan, nov)
        // Row "—" dropped cleanly; empty cells stay None, genuine 0 stays Some(0.0)
        let res = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "m1".into(),
                visual_type: "matrix".into(),
                table_id: "fLanc".into(),
                x_axis_field: Some("dEstrutura.Linha".into()),
                legend_field: Some("dCal.MesNome".into()),
                y_axis_fields: vec!["DRE Valor".into()],
                aggregation: Some("NONE".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(res.categories, vec!["Receita", "ZeroReal"]);
        assert_eq!(
            res.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["jan", "abr", "nov"]
        );
        assert_eq!(res.series[0].data, vec![Some(-1000.0), None]); // jan
        assert_eq!(res.series[1].data, vec![None, Some(0.0)]);    // abr (ZeroReal = 0)
        assert_eq!(res.series[2].data, vec![Some(-2500.0), None]); // nov

        // Multi-level hierarchical matrix also orders columns chronologically and preserves None
        let res_hier = execute_visual_query(
            &model,
            &NativeVisualQueryRequest {
                visual_id: "m2".into(),
                visual_type: "matrix".into(),
                table_id: "fLanc".into(),
                x_axis_field: Some("dEstrutura.Linha".into()),
                legend_field: Some("dCal.MesNome".into()),
                y_axis_fields: vec!["DRE Valor".into()],
                aggregation: Some("NONE".into()),
                matrix_rows: vec![NativeGroupLevel { field: "dEstrutura.Linha".into(), date_part: None }],
                matrix_expand_depth: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            res_hier.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["jan", "abr", "nov"]
        );
        let mx = res_hier.matrix.unwrap();
        assert_eq!(mx.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["Receita", "ZeroReal"]);
        assert_eq!(mx.rows[0].values, vec![Some(-1000.0), None, Some(-2500.0)]);
        assert_eq!(mx.rows[1].values, vec![None, Some(0.0), None]);
    }

    // ===== v0.11 Rodada 2, Frente E: native table / scatter =====

    #[test]
    fn test_native_table_query_grouped_sort_total_over_all_rows() {
        let model = sample_model();
        let res = execute_table_query(
            &model,
            &NativeTableQueryRequest {
                table_id: "fVendas".into(),
                group_fields: vec!["Categoria".into()],
                value_fields: vec!["Receita".into()],
                aggregation: Some("SUM".into()),
                sort_field: Some("Receita".into()),
                sort_order: "desc".into(),
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.columns, vec!["Categoria", "Receita"]);
        // B=200 > A=150(100+50) -> sorted desc by Receita
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[0].values, vec![JsonValue::from("B"), JsonValue::from(200.0)]);
        assert_eq!(res.rows[1].values, vec![JsonValue::from("A"), JsonValue::from(150.0)]);
        assert_eq!(res.total_row_count, 2);
        // Total row: engine measure in TOTAL context (350), not a sum of the page's two rows re-added
        assert_eq!(res.total_row.unwrap().values.last().unwrap(), &JsonValue::from(350.0));
        assert_eq!(res.engine, "rust-native");
    }

    #[test]
    fn test_native_table_query_raw_rows_topn_and_pagination() {
        let model = sample_model();
        // Raw ("não resumir"): one row per fact row, sorted by raw Valor asc, Top 2.
        let res = execute_table_query(
            &model,
            &NativeTableQueryRequest {
                table_id: "fVendas".into(),
                raw_fields: vec!["Categoria".into()],
                value_fields: vec!["Valor".into()],
                sort_field: Some("Valor".into()),
                sort_order: "asc".into(),
                top_n: 2,
                offset: 0,
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.columns, vec!["Categoria", "Valor"]);
        // After Top N=2 the universe is [50 (A), 100 (A)]; with limit=1 only the first page row comes back.
        assert_eq!(res.total_row_count, 2);
        assert_eq!(res.rows.len(), 1);
        assert_eq!(res.rows[0].values[0], JsonValue::from("A"));
        assert_eq!(res.rows[0].values[1].as_f64(), Some(50.0)); // raw column renders as an integer JSON number (50, not 50.0)
        // Total row sums the RAW column over the full (unpaginated, pre-TopN) table via the engine: 100+50+200=350,
        // never the client-visible page's 50.
        assert_eq!(res.total_row.unwrap().values.last().unwrap(), &JsonValue::from(350.0));
    }

    #[test]
    fn test_native_table_query_closed_form_total_over_20000_rows() {
        // SUM(1..20000) = 20000*20001/2 = 200010000 (closed form), with every
        // row read through `get_visible_row_indices` — never a 5.000-row preview.
        let mut model = DaxModel::new();
        let mut rows = Vec::with_capacity(20_000);
        for i in 1..=20_000i64 {
            let mut m = serde_json::Map::new();
            m.insert("Categoria".into(), serde_json::Value::String("All".into()));
            m.insert("Valor".into(), serde_json::json!(i));
            rows.push(m);
        }
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput {
                duckdb_table: None,
                name: "fBig".into(),
                columns: vec!["Categoria".into(), "Valor".into()],
                rows,
                column_types: vec![],
                sort_by_columns: Default::default(),
                is_calculated: false,
                expression: None,
                calculated_columns: ::std::collections::HashMap::new(),
            }],
            relationships: Vec::<DaxRelationshipInput>::new(),
            measures: vec![DaxMeasureInput {
                name: "Total".into(),
                table_name: "fBig".into(),
                formula: "SUM(fBig[Valor])".into(),
            }],
        });

        let res = execute_table_query(
            &model,
            &NativeTableQueryRequest {
                table_id: "fBig".into(),
                group_fields: vec!["Categoria".into()],
                value_fields: vec!["Total".into()],
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.rows.len(), 1);
        assert_eq!(res.rows[0].values, vec![JsonValue::from("All"), JsonValue::from(200_010_000.0)]);
        assert_eq!(res.total_row.unwrap().values.last().unwrap(), &JsonValue::from(200_010_000.0));
    }

    #[test]
    fn test_native_scatter_query_points_per_detail_value() {
        let model = sample_model();
        let res = execute_scatter_query(
            &model,
            &NativeScatterQueryRequest {
                table_id: "fVendas".into(),
                detail_field: "Categoria".into(),
                x_field: "Qtd".into(),
                y_field: "Receita".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.total_points, 2);
        assert!(!res.sampled);
        let mut by_label: HashMap<String, &NativeScatterPoint> =
            res.points.iter().map(|p| (p.label.clone(), p)).collect();
        let a = by_label.remove("A").unwrap();
        assert_eq!(a.x, Some(2.0)); // COUNTROWS over A's 2 rows
        assert_eq!(a.y, Some(150.0)); // SUM(Valor) over A's 2 rows (100+50)
        let b = by_label.remove("B").unwrap();
        assert_eq!(b.x, Some(1.0));
        assert_eq!(b.y, Some(200.0));
    }

    #[test]
    fn test_native_scatter_query_deterministic_sampling_caps_points() {
        let mut model = DaxModel::new();
        let mut rows = Vec::with_capacity(10);
        for i in 0..10 {
            let mut m = serde_json::Map::new();
            m.insert("Categoria".into(), serde_json::Value::String(format!("C{i}")));
            m.insert("Valor".into(), serde_json::json!(i as f64));
            rows.push(m);
        }
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput {
                duckdb_table: None,
                name: "fPts".into(),
                columns: vec!["Categoria".into(), "Valor".into()],
                rows,
                column_types: vec![],
                sort_by_columns: Default::default(),
                is_calculated: false,
                expression: None,
                calculated_columns: ::std::collections::HashMap::new(),
            }],
            relationships: Vec::<DaxRelationshipInput>::new(),
            measures: vec![DaxMeasureInput {
                name: "V".into(),
                table_name: "fPts".into(),
                formula: "SUM(fPts[Valor])".into(),
            }],
        });

        let req = NativeScatterQueryRequest {
            table_id: "fPts".into(),
            detail_field: "Categoria".into(),
            x_field: "V".into(),
            y_field: "V".into(),
            max_points: 3,
            ..Default::default()
        };
        let res1 = execute_scatter_query(&model, &req).unwrap();
        let res2 = execute_scatter_query(&model, &req).unwrap();
        assert_eq!(res1.total_points, 10);
        assert!(res1.sampled);
        assert!(res1.points.len() <= 3);
        // Deterministic: same input -> same sampled subset every time (no RNG).
        assert_eq!(
            res1.points.iter().map(|p| p.label.clone()).collect::<Vec<_>>(),
            res2.points.iter().map(|p| p.label.clone()).collect::<Vec<_>>()
        );
    }
}
