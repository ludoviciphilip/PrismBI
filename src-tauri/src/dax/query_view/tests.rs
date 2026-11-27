//! Rust tests for `dax_query_execute`'s building blocks (`qexec`/`qparser`)
//! and `dax_validate` (`qvalidate`). Values are derived by hand from the
//! tiny model below (see each test's comment for the arithmetic), never
//! copied from a prior run of this same code, per `tests/dax-corpus/README.md`'s
//! rule even though this corpus lives inside the lib rather than the
//! shared JSON corpus (DEFINE/EVALUATE/ORDER BY/START AT have no
//! equivalent slot in that format yet — see the final report).

use super::qexec::{execute_query, DaxQueryOptions};
use super::qvalidate::{validate_expression, ValidateContext};
use crate::dax::{DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use serde_json::json;

/// Produto(1,"Caneta","Papelaria") (2,"Caderno","Papelaria") (3,"Mouse","Eletrônicos")
/// Vendas: (Produto 1, 10, 2) (Produto 1, 5, 1) (Produto 2, 20, 1) (Produto 3, 100, 1)
/// Total Vendas = SUM(Vendas[Valor]) = 10+5+20+100 = 135.
fn base_model() -> DaxModel {
    let mut model = DaxModel::new();
    let produto = DaxTableInput { column_format_strings: Default::default(),
        duckdb_table: None,
        name: "Produto".into(),
        columns: vec!["ProdutoID".into(), "Nome".into(), "Categoria".into()],
        rows: vec![
            json!({"ProdutoID": 1, "Nome": "Caneta", "Categoria": "Papelaria"}).as_object().unwrap().clone(),
            json!({"ProdutoID": 2, "Nome": "Caderno", "Categoria": "Papelaria"}).as_object().unwrap().clone(),
            json!({"ProdutoID": 3, "Nome": "Mouse", "Categoria": "Eletrônicos"}).as_object().unwrap().clone(),
        ],
        column_types: vec![],
        sort_by_columns: Default::default(),
        is_calculated: false,
        expression: None,
        calculated_columns: Default::default(),
    };
    let vendas = DaxTableInput { column_format_strings: Default::default(),
        duckdb_table: None,
        name: "Vendas".into(),
        columns: vec!["ProdutoID".into(), "Valor".into(), "Qtd".into()],
        rows: vec![
            json!({"ProdutoID": 1, "Valor": 10, "Qtd": 2}).as_object().unwrap().clone(),
            json!({"ProdutoID": 1, "Valor": 5, "Qtd": 1}).as_object().unwrap().clone(),
            json!({"ProdutoID": 2, "Valor": 20, "Qtd": 1}).as_object().unwrap().clone(),
            json!({"ProdutoID": 3, "Valor": 100, "Qtd": 1}).as_object().unwrap().clone(),
        ],
        column_types: vec![],
        sort_by_columns: Default::default(),
        is_calculated: false,
        expression: None,
        calculated_columns: Default::default(),
    };
    let rel = DaxRelationshipInput {
        from_table: "Vendas".into(),
        from_column: "ProdutoID".into(),
        to_table: "Produto".into(),
        to_column: "ProdutoID".into(),
        cross_filter_direction: "single".into(),
        is_active: true,
    };
    let measure = DaxMeasureInput { name: "Total Vendas".into(), table_name: "Vendas".into(), formula: "SUM(Vendas[Valor])".into() };
    model.sync_from_payload(DaxModelSyncPayload { tables: vec![produto, vendas], relationships: vec![rel], measures: vec![measure] });
    model
}

fn run(model: &DaxModel, q: &str) -> super::qexec::DaxQueryExecuteResult {
    execute_query(model, q, &DaxQueryOptions::default())
}

#[test]
fn evaluate_row_literal() {
    let model = base_model();
    let r = run(&model, "EVALUATE ROW(\"X\", 1+2)");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets.len(), 1);
    assert_eq!(r.result_sets[0].display_rows, vec![vec!["3".to_string()]]);
}

#[test]
fn evaluate_values_order_by_asc() {
    let model = base_model();
    let r = run(&model, "EVALUATE VALUES(Produto[Categoria]) ORDER BY [Categoria] ASC");
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.display_rows, vec![vec!["Eletrônicos".to_string()], vec!["Papelaria".to_string()]]);
}

#[test]
fn evaluate_values_order_by_desc() {
    let model = base_model();
    let r = run(&model, "EVALUATE VALUES(Produto[Categoria]) ORDER BY [Categoria] DESC");
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.display_rows, vec![vec!["Papelaria".to_string()], vec!["Eletrônicos".to_string()]]);
}

#[test]
fn evaluate_order_by_start_at_skips_earlier_rows() {
    let model = base_model();
    let r = run(&model, "EVALUATE VALUES(Produto[Categoria]) ORDER BY [Categoria] ASC START AT \"Papelaria\"");
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.display_rows, vec![vec!["Papelaria".to_string()]]);
}

#[test]
fn define_measure_overrides_model_measure_only_for_this_query() {
    let model = base_model();
    // Total Vendas (model) = 135; the scoped override doubles it = 270.
    let r = run(&model, "DEFINE MEASURE Vendas[Total Scoped] = [Total Vendas] * 2 EVALUATE ROW(\"T\", [Total Scoped])");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets[0].display_rows, vec![vec!["270".to_string()]]);
    // The model's own measure is untouched outside the query.
    let ctx = crate::dax::FilterContext::default();
    assert_eq!(model.evaluate_dax("[Total Vendas]", &ctx).unwrap().as_f64(), Some(135.0));
}

#[test]
fn define_var_visible_in_evaluate() {
    let model = base_model();
    let r = run(&model, "DEFINE VAR Fator = 10 EVALUATE ROW(\"R\", Fator * 3)");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets[0].display_rows, vec![vec!["30".to_string()]]);
}

#[test]
fn summarizecolumns_groups_by_category_through_relationship() {
    let model = base_model();
    // Papelaria (Caneta+Caderno) sales = 10+5+20 = 35; Eletrônicos (Mouse) = 100.
    let r = run(
        &model,
        "EVALUATE SUMMARIZECOLUMNS(Produto[Categoria], \"Soma\", [Total Vendas]) ORDER BY Produto[Categoria] ASC",
    );
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.rows.len(), 2);
    assert_eq!(rs.display_rows[0], vec!["Eletrônicos".to_string(), "100".to_string()]);
    assert_eq!(rs.display_rows[1], vec!["Papelaria".to_string(), "35".to_string()]);
}

#[test]
fn topn_desc_picks_the_largest_group() {
    let model = base_model();
    let r = run(
        &model,
        "EVALUATE TOPN(1, SUMMARIZECOLUMNS(Produto[Categoria], \"Soma\", [Total Vendas]), [Soma], DESC)",
    );
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets[0].display_rows, vec![vec!["Eletrônicos".to_string(), "100".to_string()]]);
}

#[test]
fn filter_keeps_rows_matching_predicate() {
    let model = base_model();
    // Valor > 15 keeps the 20 and 100 rows (2 of 4).
    let r = run(&model, "EVALUATE FILTER(Vendas, Vendas[Valor] > 15)");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets[0].rows.len(), 2);
}

#[test]
fn addcolumns_adds_a_literal_column_to_every_row() {
    let model = base_model();
    let r = run(&model, "EVALUATE ADDCOLUMNS(Produto, \"Dobro\", 1 + 1)");
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.rows.len(), 3);
    assert!(rs.columns.iter().any(|c| c.name == "Dobro"));
    assert!(rs.display_rows.iter().all(|r| r.last().unwrap() == "2"));
}

#[test]
fn datatable_literal_builds_a_two_row_table() {
    let model = base_model();
    let r = run(&model, "EVALUATE DATATABLE(\"Nome\", STRING, \"Valor\", INTEGER, {{\"A\",1},{\"B\",2}})");
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.rows.len(), 2);
    assert_eq!(rs.display_rows[0], vec!["A".to_string(), "1".to_string()]);
    assert_eq!(rs.display_rows[1], vec!["B".to_string(), "2".to_string()]);
}

#[test]
fn multiple_evaluate_statements_each_become_a_result_set() {
    let model = base_model();
    let r = run(&model, "EVALUATE ROW(\"A\", 1) EVALUATE ROW(\"B\", 2)");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.result_sets.len(), 2);
    assert_eq!(r.result_sets[0].display_rows, vec![vec!["1".to_string()]]);
    assert_eq!(r.result_sets[1].display_rows, vec![vec!["2".to_string()]]);
}

#[test]
fn row_limit_truncates_and_reports_total() {
    let model = base_model();
    let r = execute_query(&model, "EVALUATE Vendas", &DaxQueryOptions { row_limit: Some(2) });
    assert!(r.error.is_none(), "{:?}", r.error);
    let rs = &r.result_sets[0];
    assert_eq!(rs.rows.len(), 2);
    assert_eq!(rs.row_count, 4);
    assert!(rs.truncated);
}

#[test]
fn syntax_error_reports_a_position_inside_the_query_text() {
    let model = base_model();
    let r = run(&model, "EVALUATE ROW(\"X\", 1 +)");
    let err = r.error.expect("expected a parse error");
    assert!(err.line >= 1 && err.column >= 1);
}

#[test]
fn unknown_table_reports_runtime_error_not_panic() {
    let model = base_model();
    let r = run(&model, "EVALUATE TabelaFake");
    let err = r.error.expect("expected a runtime error for an unknown table");
    assert!(err.message.contains("não encontrada"), "{}", err.message);
}

// --- dax_validate ---------------------------------------------------------

#[test]
fn validate_ok_measure_returns_type_and_sample_value() {
    let model = base_model();
    // [Total Vendas]*2, unfiltered, = 135*2 = 270.
    let v = validate_expression(&model, "[Total Vendas] * 2", ValidateContext::Measure, None);
    assert!(v.ok, "{:?}", v.diagnostics);
    assert_eq!(v.sample_value, Some(json!(270)));
}

#[test]
fn validate_unknown_function_flagged_with_position() {
    let model = base_model();
    let v = validate_expression(&model, "NAOEXISTEFUNC(1,2)", ValidateContext::Measure, None);
    assert!(!v.ok);
    assert_eq!(v.diagnostics.len(), 1);
    assert!(v.diagnostics[0].message.contains("ainda não implementada"), "{}", v.diagnostics[0].message);
    assert_eq!((v.diagnostics[0].line, v.diagnostics[0].column), (1, 1));
}

#[test]
fn validate_unknown_bracket_reference_flagged() {
    let model = base_model();
    let v = validate_expression(&model, "[MedidaQueNaoExiste]", ValidateContext::Measure, None);
    assert!(!v.ok);
    assert!(v.diagnostics[0].message.contains("não encontrada"), "{}", v.diagnostics[0].message);
}

#[test]
fn validate_wrong_arity_flagged_without_evaluating() {
    let model = base_model();
    let v = validate_expression(&model, "RELATED(Produto[Nome], Produto[Categoria])", ValidateContext::CalculatedColumn, Some("Vendas"));
    assert!(!v.ok);
    assert!(v.diagnostics.iter().any(|d| d.message.contains("RELATED")), "{:?}", v.diagnostics);
}

#[test]
fn validate_circular_measure_reference_detected() {
    let mut model = base_model();
    model.measures.insert("a".into(), DaxMeasureInput { name: "A".into(), table_name: "Vendas".into(), formula: "[B] + 1".into() });
    model.measures.insert("b".into(), DaxMeasureInput { name: "B".into(), table_name: "Vendas".into(), formula: "[A] + 1".into() });
    let v = validate_expression(&model, "[A] * 2", ValidateContext::Measure, None);
    assert!(!v.ok);
    assert!(v.diagnostics.iter().any(|d| d.message.contains("circular")), "{:?}", v.diagnostics);
}

#[test]
fn validate_calculated_table_returns_sample_rows() {
    let model = base_model();
    let v = validate_expression(&model, "VALUES(Produto[Categoria])", ValidateContext::CalculatedTable, None);
    assert!(v.ok, "{:?}", v.diagnostics);
    assert_eq!(v.result_type, Some("table"));
    let sample = v.sample_value.unwrap();
    assert_eq!(sample.as_array().unwrap().len(), 2);
}

#[test]
fn validate_calculated_column_with_row_context() {
    let model = base_model();
    // Row 0 of Produto is Caneta/Papelaria: LEN("Papelaria") = 9.
    let v = validate_expression(&model, "LEN(Produto[Categoria])", ValidateContext::CalculatedColumn, Some("Produto"));
    assert!(v.ok, "{:?}", v.diagnostics);
    assert_eq!(v.sample_value, Some(json!(9)));
}
