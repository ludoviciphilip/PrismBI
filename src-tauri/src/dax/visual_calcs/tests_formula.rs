//! Hand-derived parity tests for `formula::eval_formula` against the
//! `VisualCalculationModal.tsx` examples (`RUNNINGSUM([Total Vendas])` and
//! `[Total Vendas] - PREVIOUS([Total Vendas])`), plus the same cases run
//! through the TS port of `applyVisualCalculations`
//! (`src/engine/__tests__/v06_window_ranking_visual_calcs.test.ts`) to keep
//! both engines in lockstep. Expected values are derived by hand, never
//! copied from this module's own output.

use super::*;
use std::collections::HashMap;

fn grid_of(series: &[(&str, &[GridValue])]) -> HashMap<String, Vec<GridValue>> {
    series
        .iter()
        .map(|(name, data)| (name.to_string(), data.to_vec()))
        .collect()
}

#[test]
fn runningsum_matches_hand_derived_cumulative_total() {
    // [Total Vendas] por mês: 100, 200, 150, 50 -> acumulado: 100, 300, 450, 500.
    let grid = grid_of(&[("Total Vendas", &[Some(100.0), Some(200.0), Some(150.0), Some(50.0)])]);
    let out = eval_formula("RUNNINGSUM([Total Vendas])", &grid, 4).unwrap();
    assert_eq!(out, vec![Some(100.0), Some(300.0), Some(450.0), Some(500.0)]);
}

#[test]
fn runningsum_skips_blank_cells_without_resetting() {
    // 100, BLANK, 50 -> acumulado: 100, 100 (BLANK não soma nem reseta), 150.
    let grid = grid_of(&[("Total Vendas", &[Some(100.0), None, Some(50.0)])]);
    let out = eval_formula("RUNNINGSUM([Total Vendas])", &grid, 3).unwrap();
    assert_eq!(out, vec![Some(100.0), Some(100.0), Some(150.0)]);
}

#[test]
fn previous_minus_matches_hand_derived_delta() {
    // 100, 300, 450, 500 -> deltas (atual - anterior): BLANK, 200, 150, 50.
    let grid = grid_of(&[("Total Vendas", &[Some(100.0), Some(300.0), Some(450.0), Some(500.0)])]);
    let out = eval_formula("[Total Vendas] - PREVIOUS([Total Vendas])", &grid, 4).unwrap();
    // DAX arithmetic: BLANK - 100 => 0 - 100 = -100 for index 0? No: here it's
    // [Total Vendas] (100) - PREVIOUS(...) (BLANK) = 100 - 0 = 100 on row 0,
    // matching the `Sub` BLANK rule (lone BLANK treated as 0).
    assert_eq!(out, vec![Some(100.0), Some(200.0), Some(150.0), Some(50.0)]);
}

#[test]
fn previous_returns_blank_before_grid_start() {
    let grid = grid_of(&[("Total Vendas", &[Some(10.0), Some(20.0)])]);
    let out = eval_formula("PREVIOUS([Total Vendas])", &grid, 2).unwrap();
    assert_eq!(out, vec![None, Some(10.0)]);
}

#[test]
fn next_returns_blank_past_grid_end() {
    let grid = grid_of(&[("Total Vendas", &[Some(10.0), Some(20.0)])]);
    let out = eval_formula("NEXT([Total Vendas])", &grid, 2).unwrap();
    assert_eq!(out, vec![Some(20.0), None]);
}

#[test]
fn movingaverage_matches_hand_derived_3_point_window() {
    // 10, 20, 30, 40 -> janela 2: 10, (10+20)/2=15, (20+30)/2=25, (30+40)/2=35.
    let grid = grid_of(&[("Total Vendas", &[Some(10.0), Some(20.0), Some(30.0), Some(40.0)])]);
    let out = eval_formula("MOVINGAVERAGE([Total Vendas], 2)", &grid, 4).unwrap();
    assert_eq!(out, vec![Some(10.0), Some(15.0), Some(25.0), Some(35.0)]);
}

#[test]
fn collapse_sums_every_cell_regardless_of_index() {
    let grid = grid_of(&[("Total Vendas", &[Some(10.0), Some(20.0), Some(30.0)])]);
    let out = eval_formula("[Total Vendas] / COLLAPSEALL([Total Vendas])", &grid, 3).unwrap();
    // 10/60, 20/60, 30/60.
    assert_eq!(out, vec![Some(10.0 / 60.0), Some(20.0 / 60.0), Some(30.0 / 60.0)]);
}

#[test]
fn divide_by_zero_degrades_to_blank_not_panic() {
    let grid = grid_of(&[("A", &[Some(10.0)]), ("B", &[Some(0.0)])]);
    let out = eval_formula("[A] / [B]", &grid, 1).unwrap();
    assert_eq!(out, vec![None]);
}

#[test]
fn unknown_series_name_is_a_clear_error_not_a_panic() {
    let grid = grid_of(&[("Total Vendas", &[Some(10.0)])]);
    let err = eval_formula("RUNNINGSUM([Não Existe])", &grid, 1).unwrap_err();
    assert!(err.contains("Não Existe"), "erro inesperado: {err}");
}

#[test]
fn apply_to_result_appends_series_and_lets_later_calc_reference_earlier_one() {
    use crate::dax::query_planner::{NativeVisualQueryResult, NativeVisualSeries};

    let mut result = NativeVisualQueryResult {
        categories: vec!["Jan".into(), "Fev".into(), "Mar".into()],
        series: vec![NativeVisualSeries {
            name: "Total Vendas".into(),
            data: vec![Some(100.0), Some(200.0), Some(150.0)],
        }],
        summary_value: None,
        query_time_ms: 0.0,
        engine: "rust-native".into(),
        category_keys: Vec::new(),
        matrix: None,
    };

    let calcs = vec![
        NativeVisualCalcRequest {
            name: "Acumulado".into(),
            formula: "RUNNINGSUM([Total Vendas])".into(),
        },
        NativeVisualCalcRequest {
            name: "Delta do Acumulado".into(),
            formula: "[Acumulado] - PREVIOUS([Acumulado])".into(),
        },
    ];

    apply_to_result(&calcs, &mut result).unwrap();

    assert_eq!(result.series.len(), 3);
    assert_eq!(result.series[1].name, "Acumulado");
    assert_eq!(result.series[1].data, vec![Some(100.0), Some(300.0), Some(450.0)]);
    assert_eq!(result.series[2].name, "Delta do Acumulado");
    // Deltas do acumulado: 100 (BLANK tratado como 0), 200, 150.
    assert_eq!(result.series[2].data, vec![Some(100.0), Some(200.0), Some(150.0)]);
}
