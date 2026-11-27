//! v0.11 Rodada 2, Frente I: N níveis nas COLUNAS da matriz hierárquica.
//!
//! Paridade com a matriz do Power BI Desktop: a árvore de colunas é construída
//! UMA VEZ a partir do contexto base (igual à legenda de hoje, que já é um caso
//! particular de 1 nível), e cada nó — folha ou subtotal recolhido — carrega o
//! "caminho de filtros" (`ColumnLeafPath`) para chegar até ele. Para cada linha
//! (em qualquer profundidade, incluindo subtotais de linha), os valores visíveis
//! são a medida avaliada no contexto da linha INTERSECTADO com esse caminho —
//! nunca a soma dos filhos, dos dois lados (linha e coluna). A interseção
//! subtotal de linha × subtotal de coluna sai de brinde: é só mais um par
//! (contexto de linha, caminho de coluna) avaliado da mesma forma.
use super::query_planner::{
    collect_groups, eval_field_opt, NativeGroupLevel, NativeMatrixNode, NativeVisualQueryRequest, NativeVisualQueryResult,
};
use crate::dax::{DaxModel, FilterContext, ValueKey};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

/// Um nó da árvore de colunas (cabeçalho). `value` é o subtotal do nível (ou o
/// valor da coluna-folha), avaliado no contexto BASE (sem filtro de linha) —
/// o "Total geral" dessa coluna/subtotal, que vira a linha de totais da matriz.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMatrixColumnNode {
    pub key: JsonValue,
    pub label: String,
    pub has_children: bool,
    pub expanded: bool,
    pub value: Option<f64>,
    pub children: Vec<NativeMatrixColumnNode>,
}

/// Um passo de filtro (tabela, coluna, valores permitidos) para alcançar uma
/// coluna VISÍVEL (folha de verdade, ou um nível intermediário recolhido, que
/// nesse caso age como folha). A lista inteira intersectada no contexto de
/// qualquer linha dá o valor daquela célula.
#[derive(Clone)]
struct ColumnLeafPath {
    steps: Vec<(String, String, HashSet<ValueKey>)>,
}

fn is_column_expanded(req: &NativeVisualQueryRequest, path: &[JsonValue]) -> bool {
    if req.matrix_column_collapsed.iter().any(|p| p.as_slice() == path) {
        return false;
    }
    if req.matrix_column_expanded.iter().any(|p| p.as_slice() == path) {
        return true;
    }
    path.len().saturating_sub(1) < req.matrix_column_expand_depth
}

/// Constrói a árvore de colunas a partir do contexto base, achatando (DFS) as
/// colunas VISÍVEIS em `leaves_out` — na mesma ordem em que `value` aparece na
/// árvore, para ficar alinhado com `rows[].values` depois.
#[allow(clippy::too_many_arguments)]
fn build_tree(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    base_ctx: &FilterContext,
    agg: &str,
    primary_field: &str,
    depth: usize,
    path: &mut Vec<JsonValue>,
    steps: &mut Vec<(String, String, HashSet<ValueKey>)>,
    leaves_out: &mut Vec<ColumnLeafPath>,
) -> Vec<NativeMatrixColumnNode> {
    let levels = &req.matrix_columns;
    let (tbl, col, groups) = collect_groups(model, req, base_ctx, &levels[depth]);
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        path.push(g.key.clone());
        steps.push((tbl.clone(), col.clone(), g.allowed.clone()));
        let mut vctx = base_ctx.clone();
        for (t, c, allowed) in steps.iter() {
            vctx.set_column_filter(t, c, allowed.clone());
        }
        let value = if primary_field.is_empty() {
            None
        } else {
            eval_field_opt(model, &vctx, &req.table_id, primary_field, agg)
        };
        let has_children = depth + 1 < levels.len();
        let expanded = has_children && is_column_expanded(req, path);
        let children = if expanded {
            build_tree(model, req, &vctx, agg, primary_field, depth + 1, path, steps, leaves_out)
        } else {
            leaves_out.push(ColumnLeafPath { steps: steps.clone() });
            Vec::new()
        };
        steps.pop();
        path.pop();
        out.push(NativeMatrixColumnNode {
            key: g.key,
            label: g.label,
            has_children,
            expanded,
            value,
            children,
        });
    }
    out
}

/// Reconstrói o contexto de filtro de um caminho de LINHA (chaves de cada nível
/// de `matrix_rows` até `path.len()`), andando pelos mesmos `collect_groups`
/// usados para construir a árvore de linhas — então reproduz exatamente os
/// mesmos conjuntos `allowed` por chave.
fn row_context_for_path(model: &DaxModel, req: &NativeVisualQueryRequest, base_ctx: &FilterContext, path: &[JsonValue]) -> FilterContext {
    let mut ctx = base_ctx.clone();
    for (depth, key) in path.iter().enumerate() {
        let Some(level) = req.matrix_rows.get(depth) else { break };
        let (tbl, col, groups) = collect_groups(model, req, &ctx, level);
        if let Some(g) = groups.into_iter().find(|g| &g.key == key) {
            ctx.set_column_filter(&tbl, &col, g.allowed);
        }
    }
    ctx
}

fn row_values_for_leaves(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    row_ctx: &FilterContext,
    leaves: &[ColumnLeafPath],
    agg: &str,
    primary_field: &str,
) -> Vec<Option<f64>> {
    leaves
        .iter()
        .map(|lp| {
            if primary_field.is_empty() {
                return None;
            }
            let mut c = row_ctx.clone();
            for (t, col, allowed) in &lp.steps {
                c.set_column_filter(t, col, allowed.clone());
            }
            eval_field_opt(model, &c, &req.table_id, primary_field, agg)
        })
        .collect()
}

fn walk_rows(
    model: &DaxModel,
    req: &NativeVisualQueryRequest,
    base_ctx: &FilterContext,
    agg: &str,
    primary_field: &str,
    leaves: &[ColumnLeafPath],
    node: &mut NativeMatrixNode,
    path: &mut Vec<JsonValue>,
) {
    path.push(node.key.clone());
    let row_ctx = row_context_for_path(model, req, base_ctx, path);
    node.values = row_values_for_leaves(model, req, &row_ctx, leaves, agg, primary_field);
    for child in node.children.iter_mut() {
        walk_rows(model, req, base_ctx, agg, primary_field, leaves, child, path);
    }
    path.pop();
}

fn flatten_values(nodes: &[NativeMatrixColumnNode], out: &mut Vec<Option<f64>>) {
    for n in nodes {
        if n.expanded {
            flatten_values(&n.children, out);
        } else {
            out.push(n.value);
        }
    }
}

/// Gancho chamado por `query_planner::execute_visual_query_dax` quando
/// `req.matrix_columns` não é vazio. Troca `result.matrix.rows[*].values`
/// (que vinham por série/legenda, 1 nível) pelos valores por coluna VISÍVEL
/// da árvore de N níveis, e troca `column_totals` pelos subtotais de coluna
/// no contexto base (o "Total geral" de cada coluna/subtotal).
pub fn pivot_matrix_columns(model: &DaxModel, req: &NativeVisualQueryRequest, base_ctx: &FilterContext, agg: &str, result: &mut NativeVisualQueryResult) {
    let Some(matrix) = result.matrix.as_mut() else { return };
    let primary = req.y_axis_fields.first().cloned().unwrap_or_default();
    let mut leaves = Vec::new();
    let header = build_tree(model, req, base_ctx, agg, &primary, 0, &mut Vec::new(), &mut Vec::new(), &mut leaves);

    let mut column_totals = Vec::with_capacity(leaves.len());
    flatten_values(&header, &mut column_totals);
    matrix.column_totals = column_totals;

    for row in matrix.rows.iter_mut() {
        walk_rows(model, req, base_ctx, agg, &primary, &leaves, row, &mut Vec::new());
    }
    matrix.column_header = Some(header);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dax::query_planner::execute_visual_query;
    use crate::dax::{DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};

    /// Dados pensados para provar que o subtotal de coluna NUNCA é a soma dos
    /// filhos: `Cliente` C1 aparece em A×Norte E em B×Norte, então
    /// DISTINCTCOUNT(Cliente) na coluna Norte (3) é MENOR que a soma ingênua
    /// dos subtotais de linha (A×Norte=2 + B×Norte=2 = 4). O mesmo vale para
    /// AVERAGE(Valor) na coluna Sul: contagens desiguais por linha fazem a
    /// média ponderada (41,25) diferir da média simples dos subtotais
    /// ((5 + 53,3333…) / 2 = 29,1666…).
    fn sample_model() -> DaxModel {
        let row = |cat: &str, reg: &str, cli: &str, val: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Categoria".into(), serde_json::json!(cat));
            m.insert("Regiao".into(), serde_json::json!(reg));
            m.insert("Cliente".into(), serde_json::json!(cli));
            m.insert("Valor".into(), serde_json::json!(val));
            m
        };
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput {
                name: "fVendas".into(),
                columns: vec!["Categoria".into(), "Regiao".into(), "Cliente".into(), "Valor".into()],
                rows: vec![
                    row("A", "Norte", "C1", 10.0),
                    row("A", "Norte", "C2", 20.0),
                    row("A", "Sul", "C1", 5.0),
                    row("B", "Norte", "C3", 100.0),
                    row("B", "Norte", "C1", 15.0),
                    row("B", "Sul", "C3", 50.0),
                    row("B", "Sul", "C4", 70.0),
                    row("B", "Sul", "C2", 40.0),
                ],
                ..Default::default()
            }],
            relationships: Vec::<DaxRelationshipInput>::new(),
            measures: Vec::new(),
        });
        model
    }

    fn req(y_field: &str, agg: &str) -> NativeVisualQueryRequest {
        NativeVisualQueryRequest {
            visual_id: "mx1".into(),
            visual_type: "matrix".into(),
            table_id: "fVendas".into(),
            x_axis_field: Some("Categoria".into()),
            y_axis_fields: vec![y_field.into()],
            aggregation: Some(agg.into()),
            matrix_rows: vec![NativeGroupLevel { field: "Categoria".into(), date_part: None }],
            matrix_columns: vec![NativeGroupLevel { field: "Regiao".into(), date_part: None }],
            ..Default::default()
        }
    }

    #[test]
    fn distinctcount_column_subtotal_is_not_sum_of_children() {
        let model = sample_model();
        let res = execute_visual_query(&model, &req("Cliente", "DISTINCTCOUNT")).unwrap();
        let mx = res.matrix.unwrap();
        assert_eq!(mx.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
        // A×Norte={C1,C2}=2, A×Sul={C1}=1, B×Norte={C3,C1}=2, B×Sul={C3,C4,C2}=3
        assert_eq!(mx.rows[0].values, vec![Some(2.0), Some(1.0)]); // A: Norte, Sul
        assert_eq!(mx.rows[1].values, vec![Some(2.0), Some(3.0)]); // B: Norte, Sul
        // Coluna Norte (subtotal, avaliado pelo motor): {C1,C2,C3} = 3, NÃO 2+2=4.
        // Coluna Sul: {C1,C3,C4,C2} = 4.
        assert_eq!(mx.column_totals, vec![Some(3.0), Some(4.0)]);
        let header = mx.column_header.unwrap();
        assert_eq!(header.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), vec!["Norte", "Sul"]);
        assert_eq!(header[0].value, Some(3.0));
        assert_eq!(header[1].value, Some(4.0));
        // Total geral (grand_total do motor, não deste módulo): {C1,C2,C3,C4}=4.
        assert_eq!(mx.grand_total.unwrap_or_default().round() as i64, 0); // sem legenda: rowTotal não aplicável aqui
    }

    #[test]
    fn average_column_subtotal_is_weighted_not_mean_of_children() {
        let model = sample_model();
        let res = execute_visual_query(&model, &req("Valor", "AVERAGE")).unwrap();
        let mx = res.matrix.unwrap();
        // A×Norte=(10+20)/2=15, A×Sul=5, B×Norte=(100+15)/2=57.5, B×Sul=(50+70+40)/3=53.3333...
        assert_eq!(mx.rows[0].values, vec![Some(15.0), Some(5.0)]);
        assert_eq!(mx.rows[1].values[0], Some(57.5));
        assert!((mx.rows[1].values[1].unwrap() - 160.0 / 3.0).abs() < 1e-9);
        // Coluna Sul (subtotal ponderado pelo motor): valores {5,50,70,40} -> 165/4 = 41.25,
        // bem diferente da média simples dos subtotais de linha (5 + 53.3333)/2 = 29.1666...
        let sul = mx.column_totals[1].unwrap();
        assert!((sul - 41.25).abs() < 1e-9);
        assert!((sul - 29.1666).abs() > 1.0);
        // Coluna Norte: {10,20,100,15} -> 145/4 = 36.25
        assert!((mx.column_totals[0].unwrap() - 36.25).abs() < 1e-9);
    }

    #[test]
    fn collapsed_column_group_acts_as_a_single_leaf() {
        let model = sample_model();
        let mut r = req("Cliente", "DISTINCTCOUNT");
        // Com um único nível de coluna "Regiao", recolher não muda nada aqui
        // (não há sub-nível); o teste cobre o caminho de `is_column_expanded`
        // quando o próprio nível de topo está fora de `matrix_column_expanded`.
        r.matrix_column_expand_depth = 0;
        let res = execute_visual_query(&model, &r).unwrap();
        let mx = res.matrix.unwrap();
        assert_eq!(mx.column_header.unwrap().len(), 2);
        assert_eq!(mx.column_totals.len(), 2);
    }

    /// Regressão: ao expandir um nível de coluna, o nível FILHO deve enumerar
    /// só os valores que co-ocorrem com o pai (ex.: cidades de "Norte"), nunca
    /// todas as cidades da tabela inteira — senão colunas de outro grupo
    /// "vazam" para dentro do grupo expandido.
    #[test]
    fn expanding_a_column_group_only_enumerates_its_own_children() {
        let row = |cat: &str, reg: &str, cid: &str, cli: &str, val: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Categoria".into(), serde_json::json!(cat));
            m.insert("Regiao".into(), serde_json::json!(reg));
            m.insert("Cidade".into(), serde_json::json!(cid));
            m.insert("Cliente".into(), serde_json::json!(cli));
            m.insert("Valor".into(), serde_json::json!(val));
            m
        };
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput {
                name: "fVendas".into(),
                columns: vec!["Categoria".into(), "Regiao".into(), "Cidade".into(), "Cliente".into(), "Valor".into()],
                rows: vec![
                    row("A", "Norte", "Belem", "c1", 1.0),
                    row("A", "Norte", "Manaus", "c2", 2.0),
                    row("A", "Sul", "POA", "c3", 3.0),
                    row("A", "Sul", "Curitiba", "c4", 4.0),
                ],
                ..Default::default()
            }],
            relationships: Vec::new(),
            measures: Vec::new(),
        });
        let mut r = req("Cliente", "DISTINCTCOUNT");
        r.matrix_columns = vec![
            NativeGroupLevel { field: "Regiao".into(), date_part: None },
            NativeGroupLevel { field: "Cidade".into(), date_part: None },
        ];
        r.matrix_column_expanded = vec![vec![serde_json::json!("Norte")]];
        let res = execute_visual_query(&model, &r).unwrap();
        let header = res.matrix.unwrap().column_header.unwrap();
        let norte = header.iter().find(|n| n.label == "Norte").unwrap();
        assert!(norte.expanded);
        // Só Belem e Manaus (cidades de Norte) — nunca POA/Curitiba (de Sul).
        assert_eq!(norte.children.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), vec!["Belem", "Manaus"]);
        let sul = header.iter().find(|n| n.label == "Sul").unwrap();
        assert!(!sul.expanded);
        assert!(sul.children.is_empty());
    }
}
