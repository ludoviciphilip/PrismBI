//! v0.11 Rodada 2, Frente E: evaluates a Visual Calculation DAX formula
//! (e.g. `RUNNINGSUM([Total Vendas])`, `[Total Vendas] - PREVIOUS([Total Vendas])`,
//! as typed in `VisualCalculationModal.tsx`) over the visual's own result
//! grid — one cell per category/row, aligned with
//! `NativeVisualQueryResult::categories` — using the shared DAX parser
//! (`dax::parser::parse_dax`) instead of re-implementing a formula language.
//!
//! Mirrors `src/engine/functions/visualCalculations.ts`
//! (`applyVisualCalculations` in `src/engine/query/planner.ts`)
//! function-for-function: a later calc may reference an earlier one's name
//! because the grid is extended as each calc is applied, exactly like the TS
//! side mutates `visualRows` in place as it iterates `calcs`.

use crate::dax::ast::{BinaryOp, Expr, UnaryOp};
use crate::dax::parser::parse_dax;
use crate::dax::query_planner::{NativeVisualQueryResult, NativeVisualSeries};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{collapse_sum, expand_value, first, last, moving_average, next, previous, running_sum, GridValue};

/// One Visual Calculation to evaluate. `name` becomes the appended series
/// name; `formula` is the DAX expression (same syntax Power BI's visual
/// calculation editor accepts, restricted to the `Expr` grammar already
/// parsed for measures — see `dax::ast::Expr`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVisualCalcRequest {
    pub name: String,
    pub formula: String,
}

type Grid = HashMap<String, Vec<GridValue>>;

/// Appends one series per `calcs` entry to `result.series`, evaluating each
/// formula at every category index. Returns the first formula error
/// (wrapped with the calc's own name) instead of silently zeroing the
/// series, so the caller can decide whether to drop just this calc or fail
/// the whole native query (today: `query_planner` fails the whole request,
/// matching how a bad measure formula fails the whole query elsewhere).
pub fn apply_to_result(
    calcs: &[NativeVisualCalcRequest],
    result: &mut NativeVisualQueryResult,
) -> Result<(), String> {
    if calcs.is_empty() {
        return Ok(());
    }
    let len = result.categories.len();
    let mut grid: Grid = HashMap::new();
    for s in &result.series {
        grid.insert(s.name.clone(), s.data.clone());
    }
    for calc in calcs {
        if calc.name.trim().is_empty() || calc.formula.trim().is_empty() {
            continue;
        }
        let data = eval_formula(&calc.formula, &grid, len)
            .map_err(|e| format!("Cálculo Visual \"{}\": {}", calc.name, e))?;
        grid.insert(calc.name.clone(), data.clone());
        result.series.push(NativeVisualSeries {
            name: calc.name.clone(),
            data,
        });
    }
    Ok(())
}

/// Evaluates `formula` once per grid index `0..len`, returning one value per
/// category — the Rust equivalent of running the TS `Evaluator` once per
/// `visualRowIndex`.
pub fn eval_formula(formula: &str, grid: &Grid, len: usize) -> Result<Vec<GridValue>, String> {
    let expr = parse_dax(formula)?;
    let mut out = Vec::with_capacity(len);
    for idx in 0..len {
        out.push(eval_expr(&expr, grid, idx, len)?);
    }
    Ok(out)
}

fn get_series<'a>(grid: &'a Grid, name: &str) -> Result<&'a Vec<GridValue>, String> {
    grid.get(name)
        .ok_or_else(|| format!("coluna/série '{}' não encontrada no visual", name))
}

fn eval_expr(expr: &Expr, grid: &Grid, idx: usize, len: usize) -> Result<GridValue, String> {
    match expr {
        Expr::Blank => Ok(None),
        Expr::Number(n) => Ok(Some(*n)),
        Expr::Boolean(b) => Ok(Some(if *b { 1.0 } else { 0.0 })),
        Expr::Text(_) => Err("texto não suportado em expressão numérica de Cálculo Visual".to_string()),
        Expr::Ident(name) | Expr::BracketRef(name) => {
            Ok(get_series(grid, name)?.get(idx).copied().flatten())
        }
        Expr::TableColRef { column, .. } => {
            Ok(get_series(grid, column)?.get(idx).copied().flatten())
        }
        Expr::Unary { op, expr } => {
            let v = eval_expr(expr, grid, idx, len)?;
            Ok(match op {
                UnaryOp::Neg => v.map(|x| -x),
                UnaryOp::Pos => v,
                UnaryOp::Not => v.map(|x| if x == 0.0 { 1.0 } else { 0.0 }),
            })
        }
        Expr::Binary { op, left, right } => {
            let l = eval_expr(left, grid, idx, len)?;
            let r = eval_expr(right, grid, idx, len)?;
            apply_binary(op, l, r)
        }
        Expr::Call { name, args } => eval_call(name, args, grid, idx, len),
        Expr::InList { .. } | Expr::VarBlock { .. } => {
            Err("expressão não suportada no avaliador nativo de Cálculo Visual".to_string())
        }
    }
}

/// Arithmetic BLANK rules mirroring `DaxValue::dax_add/dax_sub/dax_mul/dax_div`
/// (`src-tauri/src/dax/mod.rs`) and the TS `daxAdd/daxSubtract/daxMultiply/daxDivide`
/// (`src/engine/expr/types.ts`): +/- treat a lone BLANK as 0 (BLANK op BLANK stays
/// BLANK); * and / propagate BLANK; division by zero degrades to BLANK here
/// instead of throwing (a visual calc should never crash the whole visual).
fn apply_binary(op: &BinaryOp, l: GridValue, r: GridValue) -> Result<GridValue, String> {
    use BinaryOp::*;
    Ok(match op {
        Add => match (l, r) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        },
        Sub => match (l, r) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) - b.unwrap_or(0.0)),
        },
        Mul => match (l, r) {
            (Some(a), Some(b)) => Some(a * b),
            _ => None,
        },
        Div => match (l, r) {
            (Some(a), Some(b)) if b != 0.0 => Some(a / b),
            _ => None,
        },
        Eq | StrictEq => Some(if numeric_eq(l, r) { 1.0 } else { 0.0 }),
        Neq => Some(if numeric_eq(l, r) { 0.0 } else { 1.0 }),
        Lt => Some(if l.unwrap_or(0.0) < r.unwrap_or(0.0) { 1.0 } else { 0.0 }),
        Lte => Some(if l.unwrap_or(0.0) <= r.unwrap_or(0.0) { 1.0 } else { 0.0 }),
        Gt => Some(if l.unwrap_or(0.0) > r.unwrap_or(0.0) { 1.0 } else { 0.0 }),
        Gte => Some(if l.unwrap_or(0.0) >= r.unwrap_or(0.0) { 1.0 } else { 0.0 }),
        And => Some(if l.unwrap_or(0.0) != 0.0 && r.unwrap_or(0.0) != 0.0 { 1.0 } else { 0.0 }),
        Or => Some(if l.unwrap_or(0.0) != 0.0 || r.unwrap_or(0.0) != 0.0 { 1.0 } else { 0.0 }),
        Concat => return Err("operador & não suportado em Cálculo Visual numérico".to_string()),
    })
}

fn numeric_eq(l: GridValue, r: GridValue) -> bool {
    match (l, r) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn col_name_arg(name: &str, args: &[Expr], pos: usize) -> Result<String, String> {
    match args.get(pos) {
        Some(Expr::BracketRef(n)) | Some(Expr::Ident(n)) => Ok(n.clone()),
        Some(Expr::TableColRef { column, .. }) => Ok(column.clone()),
        _ => Err(format!(
            "argumento {} de {} deve ser uma referência de coluna/medida",
            pos + 1,
            name
        )),
    }
}

fn eval_call(name: &str, args: &[Expr], grid: &Grid, idx: usize, len: usize) -> Result<GridValue, String> {
    let upper = name.to_uppercase();
    let num_arg = |pos: usize, default: f64| -> Result<f64, String> {
        match args.get(pos) {
            None => Ok(default),
            Some(a) => Ok(eval_expr(a, grid, idx, len)?.unwrap_or(default)),
        }
    };

    match upper.as_str() {
        "RUNNINGSUM" => {
            let col = col_name_arg(&upper, args, 0)?;
            Ok(running_sum(get_series(grid, &col)?, idx))
        }
        "MOVINGAVERAGE" => {
            let col = col_name_arg(&upper, args, 0)?;
            let window = num_arg(1, 1.0)?.max(1.0) as usize;
            Ok(moving_average(get_series(grid, &col)?, idx, window))
        }
        "PREVIOUS" => {
            let col = col_name_arg(&upper, args, 0)?;
            let offset = num_arg(1, 1.0)?.max(1.0) as usize;
            Ok(previous(get_series(grid, &col)?, idx, offset))
        }
        "NEXT" => {
            let col = col_name_arg(&upper, args, 0)?;
            let offset = num_arg(1, 1.0)?.max(1.0) as usize;
            Ok(next(get_series(grid, &col)?, idx, offset))
        }
        "FIRST" => {
            let col = col_name_arg(&upper, args, 0)?;
            Ok(first(get_series(grid, &col)?))
        }
        "LAST" => {
            let col = col_name_arg(&upper, args, 0)?;
            Ok(last(get_series(grid, &col)?))
        }
        "COLLAPSE" | "COLLAPSEALL" => {
            let col = col_name_arg(&upper, args, 0)?;
            Ok(Some(collapse_sum(get_series(grid, &col)?)))
        }
        "EXPAND" | "EXPANDALL" => {
            let col = col_name_arg(&upper, args, 0)?;
            Ok(expand_value(get_series(grid, &col)?, idx))
        }
        "DIVIDE" => {
            let a = args.get(0).map(|e| eval_expr(e, grid, idx, len)).transpose()?.flatten();
            let b = args.get(1).map(|e| eval_expr(e, grid, idx, len)).transpose()?.flatten();
            let alt = args.get(2).map(|e| eval_expr(e, grid, idx, len)).transpose()?.flatten();
            match (a, b) {
                (Some(x), Some(y)) if y != 0.0 => Ok(Some(x / y)),
                _ => Ok(alt),
            }
        }
        "ABS" => Ok(args
            .get(0)
            .map(|e| eval_expr(e, grid, idx, len))
            .transpose()?
            .flatten()
            .map(f64::abs)),
        "ROUND" => {
            let v = args.get(0).map(|e| eval_expr(e, grid, idx, len)).transpose()?.flatten();
            let digits = num_arg(1, 0.0)?;
            Ok(v.map(|x| {
                let factor = 10f64.powf(digits);
                (x * factor).round() / factor
            }))
        }
        _ => Err(format!("função '{}' não suportada no avaliador nativo de Cálculo Visual", name)),
    }
}

#[cfg(test)]
#[path = "tests_formula.rs"]
mod tests_formula;
