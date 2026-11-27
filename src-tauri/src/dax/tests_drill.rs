//! v0.10 Frente G: drill-down e matriz com N níveis no planner nativo.
//!
//! Todos os valores esperados foram derivados à mão a partir das linhas de
//! cada fixture (comentários ao lado), nunca copiados da saída do motor.

use super::*;
use crate::dax::{DaxMeasureInput, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use serde_json::json;

fn row(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    m
}

fn lvl(field: &str) -> NativeGroupLevel {
    NativeGroupLevel { field: field.into(), date_part: None }
}

fn date_lvl(field: &str, part: &str) -> NativeGroupLevel {
    NativeGroupLevel { field: field.into(), date_part: Some(part.into()) }
}

/// Regiao → Cidade → Loja, com clientes que se repetem entre lojas/cidades.
///
/// | Regiao | Cidade   | Loja | Cliente | Valor |
/// |--------|----------|------|---------|-------|
/// | Sul    | POA      | L1   | c1      | 10    |
/// | Sul    | POA      | L1   | c2      | 20    |
/// | Sul    | POA      | L2   | c1      | 30    |
/// | Sul    | Curitiba | L3   | c1      | 40    |
/// | Sul    | Curitiba | L3   | c3      | 50    |
/// | Norte  | Manaus   | L4   | c2      | 60    |
/// | Norte  | Manaus   | L4   | c4      | 70    |
fn stores_model() -> DaxModel {
    let data = [
        ("Sul", "POA", "L1", "c1", 10),
        ("Sul", "POA", "L1", "c2", 20),
        ("Sul", "POA", "L2", "c1", 30),
        ("Sul", "Curitiba", "L3", "c1", 40),
        ("Sul", "Curitiba", "L3", "c3", 50),
        ("Norte", "Manaus", "L4", "c2", 60),
        ("Norte", "Manaus", "L4", "c4", 70),
    ];
    let rows = data
        .iter()
        .map(|(r, c, l, cli, v)| {
            row(&[
                ("Regiao", json!(r)),
                ("Cidade", json!(c)),
                ("Loja", json!(l)),
                ("Cliente", json!(cli)),
                ("Valor", json!(v)),
            ])
        })
        .collect();
    let mut model = DaxModel::new();
    model.sync_from_payload(DaxModelSyncPayload {
        tables: vec![DaxTableInput {
            duckdb_table: None,
            name: "fVendas".into(),
            columns: vec!["Regiao".into(), "Cidade".into(), "Loja".into(), "Cliente".into(), "Valor".into()],
            rows,
            column_types: vec![],
            sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
        }],
        relationships: Vec::<DaxRelationshipInput>::new(),
        measures: vec![
            DaxMeasureInput {
                name: "Clientes".into(),
                table_name: "fVendas".into(),
                formula: "DISTINCTCOUNT(fVendas[Cliente])".into(),
            },
            DaxMeasureInput {
                name: "Media".into(),
                table_name: "fVendas".into(),
                formula: "AVERAGE(fVendas[Valor])".into(),
            },
        ],
    });
    model
}

fn matrix_req(y: &[&str], depth: usize) -> NativeVisualQueryRequest {
    NativeVisualQueryRequest {
        visual_id: "m1".into(),
        visual_type: "matrix".into(),
        table_id: "fVendas".into(),
        x_axis_field: Some("Regiao".into()),
        y_axis_fields: y.iter().map(|s| s.to_string()).collect(),
        aggregation: Some("NONE".into()),
        matrix_rows: vec![lvl("Regiao"), lvl("Cidade"), lvl("Loja")],
        matrix_expand_depth: depth,
        ..Default::default()
    }
}

fn find<'a>(nodes: &'a [NativeMatrixNode], label: &str) -> &'a NativeMatrixNode {
    nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("nó {label} não encontrado em {:?}", nodes.iter().map(|n| &n.label).collect::<Vec<_>>()))
}

#[test]
fn matrix_three_levels_distinctcount_subtotals_are_evaluated_not_summed() {
    let model = stores_model();
    let res = execute_visual_query(&model, &matrix_req(&["Clientes"], 2)).unwrap();
    let m = res.matrix.expect("resposta de matriz");

    // Nível 3 (folhas): L1 {c1,c2}=2, L2 {c1}=1, L3 {c1,c3}=2, L4 {c2,c4}=2.
    let sul = find(&m.rows, "Sul");
    let poa = find(&sul.children, "POA");
    let l1 = find(&poa.children, "L1");
    let l2 = find(&poa.children, "L2");
    assert_eq!(l1.values, vec![Some(2.0)]);
    assert_eq!(l2.values, vec![Some(1.0)]);
    assert!(!l1.has_children);

    // Subtotal nível 2: POA = clientes distintos de L1 ∪ L2 = {c1,c2} = 2,
    // e NÃO a soma das lojas (2 + 1 = 3).
    assert_eq!(poa.values, vec![Some(2.0)]);
    let children_sum: f64 = poa.children.iter().filter_map(|n| n.values[0]).sum();
    assert_eq!(children_sum, 3.0);
    assert_ne!(poa.values[0], Some(children_sum));

    // Curitiba = {c1,c3} = 2.
    let cwb = find(&sul.children, "Curitiba");
    assert_eq!(cwb.values, vec![Some(2.0)]);

    // Subtotal nível 1: Sul = {c1,c2,c3} = 3, não POA + Curitiba = 4.
    assert_eq!(sul.values, vec![Some(3.0)]);
    assert_ne!(sul.values[0], Some(poa.values[0].unwrap() + cwb.values[0].unwrap()));

    // Norte = {c2,c4} = 2; Total geral = {c1,c2,c3,c4} = 4, não 3 + 2 = 5.
    let norte = find(&m.rows, "Norte");
    assert_eq!(norte.values, vec![Some(2.0)]);
    assert_eq!(m.column_totals, vec![Some(4.0)]);
    assert_ne!(m.column_totals[0], Some(sul.values[0].unwrap() + norte.values[0].unwrap()));
}

#[test]
fn matrix_three_levels_implicit_distinctcount_and_average_subtotals() {
    let model = stores_model();

    // Agregação implícita DISTINCTCOUNT de uma coluna (sem medida).
    let mut req = matrix_req(&["Cliente"], 2);
    req.aggregation = Some("DISTINCTCOUNT".into());
    let m = execute_visual_query(&model, &req).unwrap().matrix.unwrap();
    let sul = find(&m.rows, "Sul");
    assert_eq!(sul.values, vec![Some(3.0)]);
    assert_eq!(find(&sul.children, "POA").values, vec![Some(2.0)]);
    assert_eq!(m.column_totals, vec![Some(4.0)]);

    // AVERAGE: POA = (10+20+30)/3 = 20, não média das médias das lojas
    // ((15 + 30) / 2 = 22.5) nem a soma (45). Sul = 150/5 = 30.
    // Total = 280/7 = 40.
    let m = execute_visual_query(&model, &matrix_req(&["Media"], 2)).unwrap().matrix.unwrap();
    let sul = find(&m.rows, "Sul");
    let poa = find(&sul.children, "POA");
    assert_eq!(find(&poa.children, "L1").values, vec![Some(15.0)]);
    assert_eq!(find(&poa.children, "L2").values, vec![Some(30.0)]);
    assert_eq!(poa.values, vec![Some(20.0)]);
    assert_eq!(sul.values, vec![Some(30.0)]);
    assert_eq!(m.column_totals, vec![Some(40.0)]);
}

#[test]
fn matrix_expansion_depth_and_explicit_paths() {
    let model = stores_model();

    // depth 0 = tudo recolhido: só o nível 1, com has_children e sem filhos calculados.
    let m = execute_visual_query(&model, &matrix_req(&["Clientes"], 0)).unwrap().matrix.unwrap();
    assert_eq!(m.rows.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(), vec!["Norte", "Sul"]);
    assert!(m.rows.iter().all(|n| n.has_children && !n.expanded && n.children.is_empty()));

    // Expandir só Sul (+) e, dentro dele, só POA.
    let mut req = matrix_req(&["Clientes"], 0);
    req.matrix_expanded = vec![vec![json!("Sul")], vec![json!("Sul"), json!("POA")]];
    let m = execute_visual_query(&model, &req).unwrap().matrix.unwrap();
    let sul = find(&m.rows, "Sul");
    assert!(sul.expanded);
    assert!(!find(&m.rows, "Norte").expanded);
    assert!(find(&sul.children, "POA").expanded);
    assert!(!find(&sul.children, "Curitiba").expanded);
    assert!(find(&sul.children, "Curitiba").children.is_empty());

    // Expandir tudo (depth 2) e recolher Sul (−).
    let mut req = matrix_req(&["Clientes"], 2);
    req.matrix_collapsed = vec![vec![json!("Sul")]];
    let m = execute_visual_query(&model, &req).unwrap().matrix.unwrap();
    assert!(!find(&m.rows, "Sul").expanded);
    assert!(find(&m.rows, "Norte").expanded);
    assert!(find(&find(&m.rows, "Norte").children, "Manaus").expanded);
}

#[test]
fn matrix_with_legend_columns_has_engine_row_totals() {
    let model = stores_model();
    // Linhas Regiao → Cidade, colunas (legenda) = Loja, valor = Clientes.
    let mut req = matrix_req(&["Clientes"], 1);
    req.matrix_rows = vec![lvl("Regiao"), lvl("Cidade")];
    req.legend_field = Some("Loja".into());
    let res = execute_visual_query(&model, &req).unwrap();
    assert_eq!(res.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["L1", "L2", "L3", "L4"]);
    let m = res.matrix.unwrap();
    let sul = find(&m.rows, "Sul");
    // Sul: L1 {c1,c2}=2, L2 {c1}=1, L3 {c1,c3}=2, L4 nada → BLANK (v0.11
    // Frente H item d/fix(dax): DISTINCTCOUNT sobre um conjunto vazio de
    // linhas é BLANK no Power BI, não 0 — dax.guide/DISTINCTCOUNT: "When the
    // function finds no rows to count, it returns a BLANK".
    assert_eq!(sul.values, vec![Some(2.0), Some(1.0), Some(2.0), None]);
    // Total da linha Sul = {c1,c2,c3} = 3 (não 2+1+2 = 5).
    assert_eq!(sul.row_total, Some(3.0));
    // Totais das colunas: L1=2, L2=1, L3=2, L4=2; total geral = 4.
    assert_eq!(m.column_totals, vec![Some(2.0), Some(1.0), Some(2.0), Some(2.0)]);
    assert_eq!(m.grand_total, Some(4.0));
}

/// | Data       | Valor |
/// |------------|-------|
/// | 2023-11-05 | 10    |
/// | 2024-01-10 | 5     |
/// | 2024-02-20 | 7     |
/// | 2024-04-01 | 3     |
/// | 2024-12-31 | 1     |
fn dates_model() -> DaxModel {
    let data = [("2023-11-05", 10), ("2024-01-10", 5), ("2024-02-20", 7), ("2024-04-01", 3), ("2024-12-31", 1)];
    let mut model = DaxModel::new();
    model.sync_from_payload(DaxModelSyncPayload {
        tables: vec![DaxTableInput {
            duckdb_table: None,
            name: "fPedidos".into(),
            columns: vec!["Data".into(), "Valor".into()],
            rows: data.iter().map(|(d, v)| row(&[("Data", json!(d)), ("Valor", json!(v))])).collect(),
            column_types: vec![Some("date".into()), Some("number".into())],
            sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
        }],
        relationships: Vec::<DaxRelationshipInput>::new(),
        measures: vec![DaxMeasureInput {
            name: "Total".into(),
            table_name: "fPedidos".into(),
            formula: "SUM(fPedidos[Valor])".into(),
        }],
    });
    model
}

fn chart_req(levels: Vec<NativeGroupLevel>, drill: Vec<NativeDrillFilter>) -> NativeVisualQueryRequest {
    NativeVisualQueryRequest {
        visual_id: "c1".into(),
        visual_type: "column".into(),
        table_id: "fPedidos".into(),
        x_axis_field: Some("Data".into()),
        y_axis_fields: vec!["Total".into()],
        aggregation: Some("NONE".into()),
        category_levels: levels,
        drill_filters: drill,
        ..Default::default()
    }
}

#[test]
fn chart_date_hierarchy_year_level() {
    let model = dates_model();
    let res = execute_visual_query(&model, &chart_req(vec![date_lvl("Data", "year")], vec![])).unwrap();
    // 2023: 10; 2024: 5+7+3+1 = 16.
    assert_eq!(res.category_keys, vec![vec![json!(2023)], vec![json!(2024)]]);
    assert_eq!(res.series[0].data, vec![Some(10.0), Some(16.0)]);
}

#[test]
fn chart_drill_into_year_shows_quarters_filtered_by_drill_path() {
    let model = dates_model();
    let drill = vec![NativeDrillFilter { field: "Data".into(), date_part: Some("year".into()), value: json!(2024) }];
    let res = execute_visual_query(&model, &chart_req(vec![date_lvl("Data", "quarter")], drill)).unwrap();
    // 2024 → T1: 5+7 = 12; T2: 3; T4: 1 (T3 sem dados some, como no Power BI).
    assert_eq!(res.category_keys, vec![vec![json!(1)], vec![json!(2)], vec![json!(4)]]);
    assert_eq!(res.series[0].data, vec![Some(12.0), Some(3.0), Some(1.0)]);
}

#[test]
fn chart_next_level_without_drill_filter_aggregates_all_years() {
    let model = dates_model();
    let res = execute_visual_query(&model, &chart_req(vec![date_lvl("Data", "quarter")], vec![])).unwrap();
    // T1: 5+7 = 12; T2: 3; T4: 10 (2023-11) + 1 (2024-12) = 11.
    assert_eq!(res.category_keys, vec![vec![json!(1)], vec![json!(2)], vec![json!(4)]]);
    assert_eq!(res.series[0].data, vec![Some(12.0), Some(3.0), Some(11.0)]);
}

#[test]
fn chart_expand_all_levels_groups_by_year_and_quarter_tuples() {
    let model = dates_model();
    let res = execute_visual_query(
        &model,
        &chart_req(vec![date_lvl("Data", "year"), date_lvl("Data", "quarter")], vec![]),
    )
    .unwrap();
    // (2023,T4)=10, (2024,T1)=12, (2024,T2)=3, (2024,T4)=1.
    assert_eq!(
        res.category_keys,
        vec![vec![json!(2023), json!(4)], vec![json!(2024), json!(1)], vec![json!(2024), json!(2)], vec![json!(2024), json!(4)]]
    );
    assert_eq!(res.series[0].data, vec![Some(10.0), Some(12.0), Some(3.0), Some(1.0)]);
}

#[test]
fn drill_filter_intersects_with_existing_filter_on_same_column() {
    let model = dates_model();
    let drill = vec![NativeDrillFilter { field: "Data".into(), date_part: Some("year".into()), value: json!(2024) }];
    let mut req = chart_req(vec![date_lvl("Data", "quarter")], drill);
    // Segmentador: só 2023-11-05 e 2024-01-10. Com o drill em 2024 sobra 2024-01-10 (T1 = 5).
    req.filters = vec![NativeFilterInput {
        table_id: "fPedidos".into(),
        column: "Data".into(),
        values: vec![json!("2023-11-05"), json!("2024-01-10")],
     ..Default::default() }];
    let res = execute_visual_query(&model, &req).unwrap();
    assert_eq!(res.category_keys, vec![vec![json!(1)]]);
    assert_eq!(res.series[0].data, vec![Some(5.0)]);

    // Drill num ano que o segmentador exclui = nada (não "sem filtro").
    let drill = vec![NativeDrillFilter { field: "Data".into(), date_part: Some("year".into()), value: json!(2022) }];
    let mut req = chart_req(vec![date_lvl("Data", "quarter")], drill);
    req.filters = vec![NativeFilterInput { table_id: "fPedidos".into(), column: "Data".into(), values: vec![json!("2024-01-10")], ..Default::default() }];
    let res = execute_visual_query(&model, &req).unwrap();
    assert!(res.categories.is_empty());
}

#[test]
fn plain_field_drill_filter_and_legacy_requests_unchanged() {
    let model = stores_model();
    // Drill em Regiao = Sul, nível Cidade, medida Clientes: POA {c1,c2}=2, Curitiba {c1,c3}=2.
    let req = NativeVisualQueryRequest {
        visual_id: "c2".into(),
        visual_type: "bar".into(),
        table_id: "fVendas".into(),
        x_axis_field: Some("Regiao".into()),
        y_axis_fields: vec!["Clientes".into()],
        aggregation: Some("NONE".into()),
        category_levels: vec![lvl("Cidade")],
        drill_filters: vec![NativeDrillFilter { field: "Regiao".into(), date_part: None, value: json!("Sul") }],
        ..Default::default()
    };
    let res = execute_visual_query(&model, &req).unwrap();
    assert_eq!(res.category_keys, vec![vec![json!("Curitiba")], vec![json!("POA")]]);
    assert_eq!(res.series[0].data, vec![Some(2.0), Some(2.0)]);

    // Requisição antiga (sem os campos novos) continua igual e sem chaves/matriz.
    let legacy: NativeVisualQueryRequest = serde_json::from_value(json!({
        "visualId": "x", "visualType": "column", "tableId": "fVendas",
        "xAxisField": "Regiao", "legendField": null, "yAxisFields": ["Valor"], "aggregation": "SUM", "filters": []
    }))
    .unwrap();
    let res = execute_visual_query(&model, &legacy).unwrap();
    assert_eq!(res.categories, vec!["Norte", "Sul"]);
    assert_eq!(res.series[0].data, vec![Some(130.0), Some(150.0)]);
    assert!(res.category_keys.is_empty());
    assert!(res.matrix.is_none());
    let js = serde_json::to_value(&res).unwrap();
    assert!(js.get("matrix").is_none());
    assert!(js.get("categoryKeys").is_none());
}

#[test]
fn request_json_shape_sent_by_the_frontend_deserializes() {
    // Forma exata montada por src/components/report/drill/engineQuery.ts
    // (toNativeLevels / toNativeDrillFilters / matrixExpanded como arrays de chaves).
    let model = stores_model();
    let req: NativeVisualQueryRequest = serde_json::from_value(json!({
        "visualId": "m1", "visualType": "matrix", "tableId": "fVendas",
        "xAxisField": "Regiao", "yAxisFields": ["Clientes"], "aggregation": "NONE", "filters": [],
        "matrixRows": [{ "field": "Regiao", "datePart": null }, { "field": "Cidade", "datePart": null }, { "field": "Loja", "datePart": null }],
        "matrixExpandDepth": 0,
        "matrixExpanded": [["Sul"]],
        "matrixCollapsed": [],
        "drillFilters": [{ "field": "Regiao", "datePart": null, "value": "Sul" }]
    }))
    .unwrap();
    let res = execute_visual_query(&model, &req).unwrap();
    let js = serde_json::to_value(&res).unwrap();
    // Drill em Sul: só Sul; expandido (POA/Curitiba), subtotal 3, total geral = 3 (contexto com o drill).
    assert_eq!(js["matrix"]["rows"][0]["label"], json!("Sul"));
    assert_eq!(js["matrix"]["rows"][0]["values"], json!([3.0]));
    assert_eq!(js["matrix"]["rows"][0]["hasChildren"], json!(true));
    assert_eq!(js["matrix"]["rows"][0]["expanded"], json!(true));
    assert_eq!(js["matrix"]["rows"][0]["children"].as_array().unwrap().len(), 2);
    assert_eq!(js["matrix"]["columnTotals"], json!([3.0]));
    assert!(js["matrix"]["rows"][0].get("rowTotal").is_some());
    assert_eq!(js["categoryKeys"], json!([["Sul"]]));
}
