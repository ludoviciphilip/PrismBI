//! v0.11 Rodada 2 (bugfix): contract test for the "Snippets DAX" button.
//! Every `DAX_SNIPPETS` entry (`src/engine/shared/intellisense.ts`) is
//! resolved on the frontend (`src/engine/shared/snippetResolver.ts`)
//! against the CURRENT model's real tables/columns/measures into a full
//! `DEFINE MEASURE ... EVALUATE SUMMARIZECOLUMNS(...)` query (TS-side
//! coverage: `src/components/daxquery/__tests__/daxQuerySnippetParity.test.ts`).
//! This is the Rust-side half of that same contract: the exact same
//! resolved query shapes, run through the REAL engine
//! (`dax_query_execute`'s `execute_query`), against a model with the same
//! data as the TS fixture, so values can be cross-checked by hand.
use super::qexec::{execute_query, DaxQueryOptions};
use crate::dax::{DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use chrono::{Duration, NaiveDate};
use serde_json::json;

/// One row per day from `start` to `end` (inclusive), `(Data, Ano)` -- same
/// shape `generateCalendarRows` builds in the TS fixture. Exact-date
/// relationship matching needs every fact row's date to exist here.
fn daily_calendar_rows(start: &str, end: &str) -> Vec<serde_json::Map<String, serde_json::Value>> {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
    let mut rows = Vec::new();
    let mut d = start;
    while d <= end {
        rows.push(json!({"Data": d.format("%Y-%m-%d").to_string(), "Ano": d.format("%Y").to_string().parse::<i64>().unwrap()}).as_object().unwrap().clone());
        d += Duration::days(1);
    }
    rows
}

/// f_Vendas(Data, Produto, Valor), d_Calendario(Data, Ano), d_Produtos(Produto),
/// measure Receita = SUM(f_Vendas[Valor]) -- same shape and data as the TS
/// fixture in `daxQuerySnippetParity.test.ts`.
fn model() -> DaxModel {
    let mut m = DaxModel::new();
    let f_vendas = DaxTableInput {
        name: "f_Vendas".into(),
        columns: vec!["Data".into(), "Produto".into(), "Valor".into()],
        rows: vec![
            json!({"Data": "2023-01-10", "Produto": "A", "Valor": 100}).as_object().unwrap().clone(),
            json!({"Data": "2023-06-01", "Produto": "B", "Valor": 50}).as_object().unwrap().clone(),
            json!({"Data": "2024-01-10", "Produto": "A", "Valor": 200}).as_object().unwrap().clone(),
            json!({"Data": "2024-02-01", "Produto": "B", "Valor": 20}).as_object().unwrap().clone(),
            json!({"Data": "2024-03-01", "Produto": "C", "Valor": 5}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("date".into()), Some("text".into()), Some("number".into())],
        ..Default::default()
    };
    let d_calendario = DaxTableInput {
        name: "d_Calendario".into(),
        columns: vec!["Data".into(), "Ano".into()],
        rows: daily_calendar_rows("2023-01-01", "2024-12-31"),
        column_types: vec![Some("date".into()), Some("number".into())],
        ..Default::default()
    };
    let d_produtos = DaxTableInput {
        name: "d_Produtos".into(),
        columns: vec!["Produto".into()],
        rows: vec![
            json!({"Produto": "A"}).as_object().unwrap().clone(),
            json!({"Produto": "B"}).as_object().unwrap().clone(),
            json!({"Produto": "C"}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("text".into())],
        ..Default::default()
    };
    let rel_data = DaxRelationshipInput {
        from_table: "f_Vendas".into(),
        from_column: "Data".into(),
        to_table: "d_Calendario".into(),
        to_column: "Data".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let rel_produto = DaxRelationshipInput {
        from_table: "f_Vendas".into(),
        from_column: "Produto".into(),
        to_table: "d_Produtos".into(),
        to_column: "Produto".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    m.sync_from_payload(DaxModelSyncPayload {
        tables: vec![f_vendas, d_calendario, d_produtos],
        relationships: vec![rel_data, rel_produto],
        measures: vec![DaxMeasureInput { name: "Receita".into(), table_name: "f_Vendas".into(), formula: "SUM(f_Vendas[Valor])".into(), format_string: None }],
    });
    m
}

fn run(query: &str) -> super::qexec::DaxQueryExecuteResult {
    execute_query(&model(), query, &DaxQueryOptions::default())
}

/// `snippet_yoy` resolved against `model()`.
#[test]
fn snippet_yoy_resolved_query_runs_and_matches_hand_derived_value() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Crescimento YoY %] =\n    VAR ValorAtual = [Receita]\n    VAR AnoAnterior = CALCULATE([Receita], SAMEPERIODLASTYEAR('d_Calendario'[Data]))\n    RETURN DIVIDE(ValorAtual - AnoAnterior, AnoAnterior, 0)\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Calendario'[Ano],\n    \"Crescimento YoY %\", [Crescimento YoY %]\n  )\nORDER BY\n  'd_Calendario'[Ano] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let by_year: std::collections::HashMap<String, String> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    // 2023 Receita = 100 + 50 = 150; 2024 Receita = 200 + 20 + 5 = 225.
    // YoY 2024 = (225 - 150) / 150 = 0.5.
    assert_eq!(by_year.get("2024").map(String::as_str), Some("0.5"), "rows: {:?}", by_year);
}

/// `snippet_ytd` resolved against `model()`.
#[test]
fn snippet_ytd_resolved_query_runs_and_matches_hand_derived_value() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Acumulado no Ano (YTD)] =\n    TOTALYTD([Receita], 'd_Calendario'[Data])\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Calendario'[Ano],\n    \"Acumulado no Ano (YTD)\", [Acumulado no Ano (YTD)]\n  )\nORDER BY\n  'd_Calendario'[Ano] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let by_year: std::collections::HashMap<String, String> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(by_year.get("2023").map(String::as_str), Some("150"));
    assert_eq!(by_year.get("2024").map(String::as_str), Some("225"));
}

/// `snippet_mom` resolved against `model()`: just needs to run without error
/// (DATEADD month arithmetic isn't hand-derived here, parity test covers it).
#[test]
fn snippet_mom_resolved_query_runs_without_error() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Crescimento MoM %] =\n    VAR MesAtual = [Receita]\n    VAR MesAnterior = CALCULATE([Receita], DATEADD('d_Calendario'[Data], -1, MONTH))\n    RETURN DIVIDE(MesAtual - MesAnterior, MesAnterior, 0)\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Calendario'[Ano],\n    \"Crescimento MoM %\", [Crescimento MoM %]\n  )\nORDER BY\n  'd_Calendario'[Ano] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    assert!(!res.result_sets[0].rows.is_empty());
}

/// `snippet_pareto` resolved against `model()`.
#[test]
fn snippet_pareto_resolved_query_runs_and_matches_hand_derived_value() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Acumulado % (Pareto)] =\n    VAR TotalGeral = CALCULATE([Receita], ALL('d_Produtos'))\n    VAR ValorItem = [Receita]\n    VAR Acumulado =\n      CALCULATE(\n        [Receita],\n        FILTER(ALL('d_Produtos'), [Receita] >= ValorItem)\n      )\n    RETURN DIVIDE(Acumulado, TotalGeral, 0)\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Produtos'[Produto],\n    \"Acumulado % (Pareto)\", [Acumulado % (Pareto)]\n  )\nORDER BY\n  'd_Produtos'[Produto] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let by_produto: std::collections::HashMap<String, String> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    // Produto A = 300 (the biggest), total = 300 + 70 + 5 = 375 -> A's own
    // accumulated bucket (items >= A's value) is just A itself: 300 / 375.
    assert_eq!(by_produto.get("A").map(String::as_str), Some("0.8"), "rows: {:?}", by_produto);
}

/// `snippet_topn_others` resolved against `model()`: just needs to run
/// without error (RANKX/TOPN's exact split isn't hand-derived here).
#[test]
fn snippet_topn_others_resolved_query_runs_without_error() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Top 5 ou Outros] =\n    VAR Ranking = RANKX(ALL('d_Produtos'[Produto]), [Receita], , DESC)\n    RETURN\n      IF(\n        Ranking <= 5,\n        [Receita],\n        CALCULATE([Receita], ALL('d_Produtos')) -\n          CALCULATE([Receita], TOPN(5, ALL('d_Produtos'[Produto]), [Receita], DESC))\n      )\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Produtos'[Produto],\n    \"Top 5 ou Outros\", [Top 5 ou Outros]\n  )\nORDER BY\n  'd_Produtos'[Produto] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    assert!(!res.result_sets[0].rows.is_empty());
}

/// `snippet_query_template` resolved against `model()`.
#[test]
fn snippet_query_template_resolved_query_runs_and_matches_hand_derived_value() {
    let query = "DEFINE\n  MEASURE 'f_Vendas'[Resultado da Consulta] =\n    [Receita] * 1\n\nEVALUATE\n  SUMMARIZECOLUMNS(\n    'd_Calendario'[Ano],\n    \"Resultado da Consulta\", [Resultado da Consulta]\n  )\nORDER BY\n  'd_Calendario'[Ano] ASC";
    let res = run(query);
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let by_year: std::collections::HashMap<String, String> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(by_year.get("2023").map(String::as_str), Some("150"));
    assert_eq!(by_year.get("2024").map(String::as_str), Some("225"));
}
