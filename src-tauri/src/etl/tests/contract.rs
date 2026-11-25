//! TS ⇄ Rust contract: `tests/etl-contract/all_steps.json` is generated from
//! the typed fixture in `src/services/etl/__tests__/etlContract.test.ts`
//! (the shape the frontend actually sends). Every step must deserialize,
//! survive the M round-trip and, for `allSteps`, execute. Expected values
//! were derived by hand from Power Query semantics.

use super::*;
use crate::etl::commands::{etl_from_m_impl, etl_to_m_impl};
use serde_json::json;

fn contract() -> JsonValue {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/etl-contract/all_steps.json");
    serde_json::from_str(&std::fs::read_to_string(p).expect("fixture do contrato")).expect("json")
}

fn q(key: &str) -> EtlQuery {
    serde_json::from_value(contract()[key].clone()).unwrap_or_else(|e| panic!("{key}: JSON do frontend rejeitado: {e}"))
}

fn cell<'a>(r: &'a Run, row: usize, col: &str) -> &'a JsonValue {
    let i = r.cols.iter().position(|c| c == col).unwrap_or_else(|| panic!("coluna {col} ausente em {:?}", r.cols));
    &r.rows[row][i]
}

#[test]
fn every_frontend_step_deserializes_and_reserializes_identically() {
    let c = contract();
    for key in ["allSteps", "shapeSteps"] {
        let parsed = q(key);
        let back = serde_json::to_value(&parsed).unwrap();
        assert_eq!(back, c[key], "{key}: serde não é simétrico com o formato do frontend");
    }
}

#[test]
fn every_frontend_step_round_trips_through_m() {
    for key in ["allSteps", "shapeSteps"] {
        let original = q(key);
        let m = etl_to_m_impl(&original).unwrap();
        let back = etl_from_m_impl(&original.name, &m).unwrap();
        assert_eq!(back.query.without_ids(), original.without_ids(), "{key}: round-trip M diferente\nM:\n{m}");
        assert!(
            !back.query.steps.iter().any(|s| matches!(s.kind, StepKind::OpaqueM { .. })),
            "{key}: etapa virou opaqueM"
        );
    }
}

#[test]
fn frontend_pipeline_executes_with_hand_derived_results() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let query = q("allSteps");

    // Até "Linhas Classificadas" (índice 14): duplicata removida, lápis herda
    // Curitiba (fillDown), "azul" -> "preta", cidades em maiúsculas, Cod
    // dividido e mesclado com "/", Total = Valor * Qtd, ordenado por Total desc.
    let mid = preview(&db, &query, Some(14));
    assert!(mid.res.failed_step.is_none(), "{:?}", mid.res.failed_step);
    assert_eq!(mid.rows.len(), 3);
    let expect = [
        ("caneta preta", "CURITIBA", 21.0, "Alta", "A/1", 10.5, 1),
        ("borracha", "LONDRINA", 5.0, "Baixa", "C/3", 1.25, 3),
        ("lápis", "CURITIBA", 3.0, "Baixa", "B/2", 3.0, 2),
    ];
    for (i, (prod, cid, total, faixa, cod, preco, idx)) in expect.iter().enumerate() {
        assert_eq!(cell(&mid, i, "Produto"), &json!(prod), "linha {i}");
        assert_eq!(cell(&mid, i, "Cidade"), &json!(cid), "linha {i}");
        assert!((cell(&mid, i, "Total").as_f64().unwrap() - total).abs() < 1e-9, "linha {i}");
        assert_eq!(cell(&mid, i, "Faixa"), &json!(faixa), "linha {i}");
        assert_eq!(cell(&mid, i, "Código"), &json!(cod), "linha {i}");
        assert!((cell(&mid, i, "Preço").as_f64().unwrap() - preco).abs() < 1e-9, "linha {i}");
        assert_eq!(cell(&mid, i, "Índice").as_f64().unwrap() as i64, *idx as i64, "linha {i}");
    }
    assert!(!mid.cols.iter().any(|c| c == "Cod" || c == "Cod.1" || c == "Cod.2"), "{:?}", mid.cols);

    // Final: agrupado por Cidade (ordem de primeira ocorrência).
    let fin = run_in(&db, &query);
    assert_cols(&fin, &["Cidade", "Soma", "Linhas"]);
    assert_rows(&fin, &[json!(["CURITIBA", 24.0, 2]), json!(["LONDRINA", 5.0, 1])]);
}

#[test]
fn apply_refuses_a_query_that_reads_its_own_target_table() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let mut query = q("allSteps");
    query.steps.clear();
    etl_load_raw_impl(&db, &query.name, &query.source, None).unwrap();
    etl_apply_impl(&db, &query, "Base").unwrap();
    let self_ref = EtlQuery {
        name: "Base".into(),
        source: EtlSource::table_ref("Base"),
        steps: vec![],
    };
    let err = etl_apply_impl(&db, &self_ref, "Base").unwrap_err();
    assert!(err.contains("própria tabela"), "{err}");
}
