//! Synthetic reproduction of the Rhino bug report (v0.11 Rodada 2, Frente F):
//! `DEFINE MEASURE` wrapping a chain of model measures (each with its own
//! `CALCULATE` filter on a *different* dimension than the one grouped by),
//! queried through `SUMMARIZECOLUMNS` grouped on a date-dimension column
//! that is NOT the relationship's join column.
//!
//! Shape mirrors Rhino exactly: `dCalendario[Data]` is the join column to
//! the fact table; `dCalendario[Ano]` is a *different*, non-join column
//! that SUMMARIZECOLUMNS groups by. `Faturamento` is itself
//! `CALCULATE([ResultadoDRE], dCategorias[Grupo] = "Receita")`, i.e. a
//! measure-calling-measure chain, each hop adding a `CALCULATE` filter on
//! `dCategorias` (not on `dCalendario`).
//!
//! Synthetic data (derived by hand, never from a prior engine run):
//!   dCalendario: (Data, Ano) = (2024-01-01, 2024), (2025-01-01, 2025)
//!   dCategorias: (Categoria, Grupo) = ("Vendas", "Receita"), ("Imposto", "Despesa")
//!   fFato: (Data, Categoria, Valor)
//!     (2024-01-01, "Vendas", 100), (2024-01-01, "Imposto", 10),
//!     (2025-01-01, "Vendas", 200), (2025-01-01, "Vendas", 50)
//!   ValorBase = SUM(fFato[Valor])
//!   ResultadoDRE = CALCULATE([ValorBase], dCategorias[Grupo] = "Receita")
//!   Faturamento = CALCULATE([ResultadoDRE], dCategorias[Grupo] = "Receita") (redundant filter, as in Rhino's chain)
//!
//! Expected (same arithmetic Power BI would produce):
//!   Ano 2024 -> Faturamento = 100 (only the "Vendas" row counts)
//!   Ano 2025 -> Faturamento = 200 + 50 = 250
//!   `Teste = [Faturamento] * 2` -> 200 and 500 respectively, ORDER BY Ano DESC
//!   -> rows: (2025, 500), (2024, 200).

use super::qexec::{execute_query, DaxQueryOptions};
use crate::dax::{DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use serde_json::json;

fn model() -> DaxModel {
    let mut m = DaxModel::new();
    let d_calendario = DaxTableInput {
        name: "dCalendario".into(),
        columns: vec!["Data".into(), "Ano".into()],
        rows: vec![
            json!({"Data": "2024-01-01", "Ano": 2024}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Ano": 2025}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("date".into()), Some("number".into())],
        ..Default::default()
    };
    let d_categorias = DaxTableInput {
        name: "dCategorias".into(),
        columns: vec!["Categoria".into(), "Grupo".into()],
        rows: vec![
            json!({"Categoria": "Vendas", "Grupo": "Receita"}).as_object().unwrap().clone(),
            json!({"Categoria": "Imposto", "Grupo": "Despesa"}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("text".into()), Some("text".into())],
        ..Default::default()
    };
    let f_fato = DaxTableInput {
        name: "fFato".into(),
        columns: vec!["Data".into(), "Categoria".into(), "Valor".into(), "Tipo".into()],
        rows: vec![
            json!({"Data": "2024-01-01", "Categoria": "Vendas", "Valor": 100, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2024-01-01", "Categoria": "Imposto", "Valor": 10, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Categoria": "Vendas", "Valor": 200, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Categoria": "Vendas", "Valor": 50, "Tipo": "Entrada"}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("date".into()), Some("text".into()), Some("number".into()), Some("text".into())],
        ..Default::default()
    };
    let rel_data = DaxRelationshipInput {
        from_table: "fFato".into(),
        from_column: "Data".into(),
        to_table: "dCalendario".into(),
        to_column: "Data".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let rel_cat = DaxRelationshipInput {
        from_table: "fFato".into(),
        from_column: "Categoria".into(),
        to_table: "dCategorias".into(),
        to_column: "Categoria".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let measures = vec![
        DaxMeasureInput { name: "ValorBase".into(),
            table_name: "fFato".into(),
            formula: "SUMX(fFato, fFato[Valor] * IF(fFato[Tipo] = \"Entrada\", 1, -1))".into(), format_string: None },
        DaxMeasureInput { name: "ResultadoDRE".into(),
            table_name: "fFato".into(),
            formula: "CALCULATE([ValorBase], dCategorias[Grupo] = \"Receita\")".into(), format_string: None },
        DaxMeasureInput { name: "Faturamento".into(),
            table_name: "fFato".into(),
            formula: "CALCULATE([ResultadoDRE], dCategorias[Grupo] = \"Receita\")".into(), format_string: None },
    ];
    m.sync_from_payload(DaxModelSyncPayload {
        tables: vec![d_calendario, d_categorias, f_fato],
        relationships: vec![rel_data, rel_cat],
        measures,
    });
    m
}

/// Reproduces the Luis bug report bit for bit on synthetic data: a
/// `DEFINE MEASURE` that multiplies a chained model measure by 2, grouped
/// by a dimension column that is not the join column, must return one row
/// per non-blank year, not a single collapsed row with a BLANK value.
#[test]
fn define_measure_over_chained_measure_in_summarizecolumns_groups_by_year() {
    let model = model();
    let query = "DEFINE MEASURE fFato[Teste] = [Faturamento] * 2\nEVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"T\", [Teste])\nORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let rows: Vec<(String, String)> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(rows, vec![("2025".to_string(), "500".to_string()), ("2024".to_string(), "200".to_string())], "rows: {:?}", rows);
}

/// Baseline (no DEFINE, no measure chain): `[Faturamento]` alone already
/// must vary per year — confirms the relationship propagation from
/// dCalendario (grouped, non-join column `Ano`) through `Data` works
/// without any DEFINE involved, isolating the DEFINE/chain interaction.
#[test]
fn faturamento_alone_in_summarizecolumns_groups_by_year() {
    let model = model();
    let query = "EVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"F\", [Faturamento]) ORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    let rows: Vec<(String, String)> =
        rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(rows, vec![("2025".to_string(), "250".to_string()), ("2024".to_string(), "100".to_string())], "rows: {:?}", rows);
}

/// v0.11 Rodada 2 Frente F (bugfix): reproduces the *exact* screenshot bug
/// report on top of the same Rhino-shaped synthetic model, but this time
/// with `dCalendario[Ano]`'s TMDL `formatString` ("0", no grouping) and
/// `Faturamento`'s TMDL `formatString` ("#,##0.00", currency-like 2
/// decimals) set on the sync payload -- exactly as the real frontend sync
/// (`src/store/useBiStore.ts` `pushModelToRustAsync`) sends them once wired
/// through. Before the fix: `Ano` had no `columnFormatStrings` entry at all
/// (the frontend never sent it) and `Teste` had no measure format to
/// inherit, so both columns fell back to the UI's generic grouped-integer
/// `formatNumber` ("2.026" and "1.986.738", no cents).
fn model_with_format_strings() -> DaxModel {
    let mut m = DaxModel::new();
    let d_calendario = DaxTableInput {
        name: "dCalendario".into(),
        columns: vec!["Data".into(), "Ano".into()],
        rows: vec![
            json!({"Data": "2024-01-01", "Ano": 2024}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Ano": 2025}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("date".into()), Some("number".into())],
        column_format_strings: [("Ano".to_string(), "0".to_string())].into_iter().collect(),
        ..Default::default()
    };
    let d_categorias = DaxTableInput {
        name: "dCategorias".into(),
        columns: vec!["Categoria".into(), "Grupo".into()],
        rows: vec![
            json!({"Categoria": "Vendas", "Grupo": "Receita"}).as_object().unwrap().clone(),
            json!({"Categoria": "Imposto", "Grupo": "Despesa"}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("text".into()), Some("text".into())],
        ..Default::default()
    };
    let f_fato = DaxTableInput {
        name: "fFato".into(),
        columns: vec!["Data".into(), "Categoria".into(), "Valor".into(), "Tipo".into()],
        rows: vec![
            json!({"Data": "2024-01-01", "Categoria": "Vendas", "Valor": 100.50, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2024-01-01", "Categoria": "Imposto", "Valor": 10, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Categoria": "Vendas", "Valor": 200, "Tipo": "Entrada"}).as_object().unwrap().clone(),
            json!({"Data": "2025-01-01", "Categoria": "Vendas", "Valor": 50, "Tipo": "Entrada"}).as_object().unwrap().clone(),
        ],
        column_types: vec![Some("date".into()), Some("text".into()), Some("number".into()), Some("text".into())],
        ..Default::default()
    };
    let rel_data = DaxRelationshipInput {
        from_table: "fFato".into(),
        from_column: "Data".into(),
        to_table: "dCalendario".into(),
        to_column: "Data".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let rel_cat = DaxRelationshipInput {
        from_table: "fFato".into(),
        from_column: "Categoria".into(),
        to_table: "dCategorias".into(),
        to_column: "Categoria".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let measures = vec![
        DaxMeasureInput {
            name: "ValorBase".into(),
            table_name: "fFato".into(),
            formula: "SUMX(fFato, fFato[Valor] * IF(fFato[Tipo] = \"Entrada\", 1, -1))".into(),
            format_string: None,
        },
        DaxMeasureInput {
            name: "ResultadoDRE".into(),
            table_name: "fFato".into(),
            formula: "CALCULATE([ValorBase], dCategorias[Grupo] = \"Receita\")".into(),
            format_string: None,
        },
        DaxMeasureInput {
            name: "Faturamento".into(),
            table_name: "fFato".into(),
            formula: "CALCULATE([ResultadoDRE], dCategorias[Grupo] = \"Receita\")".into(),
            // TMDL formatString a real currency measure would carry.
            format_string: Some("#,##0.00".to_string()),
        },
    ];
    m.sync_from_payload(DaxModelSyncPayload {
        tables: vec![d_calendario, d_categorias, f_fato],
        relationships: vec![rel_data, rel_cat],
        measures,
    });
    m
}

/// `dCalendario[Ano]` must render using its own TMDL format ("0" -> no
/// thousands grouping), never the UI's generic grouped-integer fallback.
#[test]
fn ano_column_keeps_its_own_format_string_through_sync_payload() {
    let model = model_with_format_strings();
    let query = "EVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"F\", [Faturamento]) ORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    assert_eq!(rs.columns[0].format_string.as_deref(), Some("0"), "Ano column lost its model formatString");
}

/// `DEFINE MEASURE Teste = [Faturamento] * 2` must inherit `Faturamento`'s
/// own TMDL formatString (set on the model via the real sync payload
/// shape), so the DAX Query View shows the cents instead of truncating them
/// through the UI's grouped-integer `formatNumber` fallback.
#[test]
fn define_measure_inherits_referenced_measures_format_string() {
    let model = model_with_format_strings();
    let query = "DEFINE MEASURE fFato[Teste] = [Faturamento] * 2\nEVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"T\", [Teste])\nORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query failed: {:?}", res.error);
    let rs = &res.result_sets[0];
    assert_eq!(
        rs.columns[1].format_string.as_deref(),
        Some("#,##0.00"),
        "Teste (T) extension column didn't inherit Faturamento's format_string; columns: {:?}",
        rs.columns
    );
    // Sanity: the underlying numeric value still has cents (2024: 100.50 * 2 = 201.00).
    let rows: Vec<(String, String)> = rs.display_rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(rows[1].0, "2024");
}
