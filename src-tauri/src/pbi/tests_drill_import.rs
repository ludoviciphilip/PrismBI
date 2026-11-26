//! v0.10 Frente G: várias projeções em Category (gráficos) / Rows (matriz)
//! viram a hierarquia de drill (`drill_fields`) na importação PBIR.

use super::*;
use serde_json::json;

fn col_proj(entity: &str, prop: &str) -> JsonValue {
    json!({
        "field": { "Column": { "Expression": { "SourceRef": { "Entity": entity } }, "Property": prop } },
        "queryRef": format!("{entity}.{prop}")
    })
}

fn measure_proj(prop: &str) -> JsonValue {
    json!({
        "field": { "Measure": { "Expression": { "SourceRef": { "Entity": "Medidas" } }, "Property": prop } },
        "queryRef": format!("Medidas.{prop}")
    })
}

#[test]
fn pbir_chart_with_several_category_projections_maps_to_drill_fields() {
    let vj = json!({
        "name": "col1",
        "position": { "x": 0, "y": 0, "width": 400, "height": 300 },
        "visual": {
            "visualType": "clusteredColumnChart",
            "query": { "queryState": {
                "Category": { "projections": [col_proj("dProduto", "Categoria"), col_proj("dProduto", "Subcategoria"), col_proj("dProduto", "Produto")] },
                "Y": { "projections": [measure_proj("Receita")] }
            } }
        }
    });
    let meta = parse_pbir_visual_json(&vj, "p1", 0).expect("visual");
    assert_eq!(meta.category_field.as_deref(), Some("dProduto.Categoria"));
    assert_eq!(meta.drill_fields, vec!["dProduto.Subcategoria", "dProduto.Produto"]);
    assert_eq!(meta.value_fields, vec!["Receita"]);
}

#[test]
fn pbir_matrix_rows_hierarchy_maps_to_drill_fields_and_single_projection_stays_flat() {
    let vj = json!({
        "name": "mx1",
        "position": { "x": 0, "y": 0, "width": 400, "height": 300 },
        "visual": {
            "visualType": "pivotTable",
            "query": { "queryState": {
                "Rows": { "projections": [col_proj("dLoja", "Regiao"), col_proj("dLoja", "Cidade"), col_proj("dLoja", "Loja")] },
                "Values": { "projections": [measure_proj("Clientes")] }
            } }
        }
    });
    let meta = parse_pbir_visual_json(&vj, "p1", 0).expect("visual");
    assert_eq!(meta.visual_type, "matrix");
    assert_eq!(meta.category_field.as_deref(), Some("dLoja.Regiao"));
    assert_eq!(meta.drill_fields, vec!["dLoja.Cidade", "dLoja.Loja"]);

    let flat = json!({
        "name": "mx2",
        "visual": {
            "visualType": "pivotTable",
            "query": { "queryState": {
                "Rows": { "projections": [col_proj("dLoja", "Regiao")] },
                "Values": { "projections": [measure_proj("Clientes")] }
            } }
        }
    });
    let meta = parse_pbir_visual_json(&flat, "p1", 0).expect("visual");
    assert!(meta.drill_fields.is_empty());
    // serializa como `drillFields` (camelCase) para o TS
    let js = serde_json::to_value(&meta).unwrap();
    assert_eq!(js["drillFields"], json!([]));
}

#[test]
fn pbir_slicer_values_do_not_become_drill_fields() {
    let vj = json!({
        "name": "sl1",
        "visual": {
            "visualType": "slicer",
            "query": { "queryState": {
                "Values": { "projections": [col_proj("dLoja", "Regiao"), col_proj("dLoja", "Cidade")] }
            } }
        }
    });
    let meta = parse_pbir_visual_json(&vj, "p1", 0).expect("visual");
    assert!(meta.drill_fields.is_empty());
}
