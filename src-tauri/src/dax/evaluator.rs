use crate::dax::ast::{BinaryOp, Expr, UnaryOp};
use crate::dax::functions::{add_days, parse_iso_ymd, try_eval_scalar_fn};
use crate::dax::parser::parse_dax;
use crate::dax::{DaxModel, DaxTableData, DaxValue, FilterContext, RelOverride};
use std::collections::{HashMap, HashSet};

// `eval_call` below is a routing table; the implementations live in one module per family.
// Every family module does `use super::*`, so the shared core (evaluate_ast, RowContext,
// context transition, ...) and the sibling modules' `pub(super)` items are all in scope.
mod aggregate;
mod filter;
mod row;
mod scalar;
mod table;
mod time;

use filter::*;
use row::*;
use table::*;
use time::*;

#[derive(Debug, Clone)]
pub struct RowContext<'a> {
    pub table: &'a DaxTableData,
    pub row_idx: usize,
}

pub fn evaluate_ast(
    expr: &Expr,
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    match expr {
        Expr::Blank => Ok(DaxValue::Blank),
        Expr::Number(n) => Ok(DaxValue::Number(*n)),
        Expr::Text(s) => Ok(DaxValue::Text(s.clone())),
        Expr::Boolean(b) => Ok(DaxValue::Boolean(*b)),
        Expr::Ident(id) => {
            if let Some(v) = vars.get(&id.to_lowercase()) {
                return Ok(v.clone());
            }
            if id.eq_ignore_ascii_case("BLANK") {
                return Ok(DaxValue::Blank);
            }
            Ok(DaxValue::Text(id.clone()))
        }
        Expr::BracketRef(name) => {
            // 1. Check active row context column first
            if let Some(rc) = row_stack.last() {
                if let Some(cidx) = bare_col_idx(rc.table, name) {
                    return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            // 2. Check measure
            if let Some(measure) = model.get_measure(name) {
                let effective_ctx = apply_context_transition(model, ctx, row_stack);
                let parsed = parse_dax(&measure.formula)?;
                return evaluate_ast(&parsed, model, &effective_ctx, &[], &HashMap::new());
            }
            // 3. Fallback search across row stack
            for rc in row_stack.iter().rev() {
                if let Some(cidx) = bare_col_idx(rc.table, name) {
                    return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            Err(format!("Referência '[{}]' não encontrada.", name))
        }
        Expr::TableColRef { table, column } => {
            for rc in row_stack.iter().rev() {
                if let Some(cidx) = row_col_idx(rc.table, table, column) {
                    return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            // Try RELATED path if in row context of another table
            if let Some(rc) = row_stack.last() {
                if let Some(rel_val) = resolve_related(model, rc, table, column) {
                    return Ok(rel_val);
                }
            }
            // Scalar fallback via SELECTEDVALUE when outside row context
            eval_selected_value(model, ctx, table, column, DaxValue::Blank)
        }
        Expr::Unary { op, expr } => {
            let v = evaluate_ast(expr, model, ctx, row_stack, vars)?;
            match op {
                UnaryOp::Neg => Ok(DaxValue::Number(-(v.as_f64().unwrap_or(0.0)))),
                UnaryOp::Pos => Ok(DaxValue::Number(v.as_f64().unwrap_or(0.0))),
                UnaryOp::Not => Ok(DaxValue::Boolean(!v.as_bool())),
            }
        }
        Expr::Binary { op, left, right } => {
            if matches!(op, BinaryOp::And) {
                let l = evaluate_ast(left, model, ctx, row_stack, vars)?;
                if !l.as_bool() {
                    return Ok(DaxValue::Boolean(false));
                }
                let r = evaluate_ast(right, model, ctx, row_stack, vars)?;
                return Ok(DaxValue::Boolean(r.as_bool()));
            }
            if matches!(op, BinaryOp::Or) {
                let l = evaluate_ast(left, model, ctx, row_stack, vars)?;
                if l.as_bool() {
                    return Ok(DaxValue::Boolean(true));
                }
                let r = evaluate_ast(right, model, ctx, row_stack, vars)?;
                return Ok(DaxValue::Boolean(r.as_bool()));
            }

            let l = evaluate_ast(left, model, ctx, row_stack, vars)?;
            let r = evaluate_ast(right, model, ctx, row_stack, vars)?;
            eval_binary(op, &l, &r)
        }
        Expr::InList { expr, list } => {
            let lhs = evaluate_ast(expr, model, ctx, row_stack, vars)?;
            for item in list {
                let rhs = evaluate_ast(item, model, ctx, row_stack, vars)?;
                if lhs.dax_equals(&rhs) {
                    return Ok(DaxValue::Boolean(true));
                }
            }
            Ok(DaxValue::Boolean(false))
        }
        Expr::VarBlock { bindings, body } => {
            let mut local_vars = vars.clone();
            for (vname, vexpr) in bindings {
                let val = evaluate_ast(vexpr, model, ctx, row_stack, &local_vars)?;
                local_vars.insert(vname.to_lowercase(), val);
            }
            evaluate_ast(body, model, ctx, row_stack, &local_vars)
        }
        Expr::Call { name, args } => eval_call(name, args, model, ctx, row_stack, vars),
    }
}

fn eval_binary(op: &BinaryOp, l: &DaxValue, r: &DaxValue) -> Result<DaxValue, String> {
    // Date ± number arithmetic yields a date (DAX stores dates as a serial day count
    // internally; here dates are ISO text, so shift the civil date directly).
    if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
        let l_date = match l {
            DaxValue::Text(s) => parse_iso_ymd(s),
            _ => None,
        };
        let r_date = match r {
            DaxValue::Text(s) => parse_iso_ymd(s),
            _ => None,
        };
        if let (Some((y, m, d)), None) = (l_date, r_date) {
            let delta = r.as_f64().unwrap_or(0.0).round() as i32;
            let signed = if matches!(op, BinaryOp::Sub) { -delta } else { delta };
            let (ny, nm, nd) = add_days(y, m, d, signed);
            return Ok(DaxValue::Text(format!("{:04}-{:02}-{:02}", ny, nm, nd)));
        }
        if let (None, Some((y, m, d))) = (l_date, r_date) {
            if matches!(op, BinaryOp::Add) {
                let delta = l.as_f64().unwrap_or(0.0).round() as i32;
                let (ny, nm, nd) = add_days(y, m, d, delta);
                return Ok(DaxValue::Text(format!("{:04}-{:02}-{:02}", ny, nm, nd)));
            }
        }
    }
    match op {
        BinaryOp::Add => Ok(DaxValue::Number(l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0))),
        BinaryOp::Sub => Ok(DaxValue::Number(l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0))),
        BinaryOp::Mul => {
            if l.is_blank() || r.is_blank() {
                Ok(DaxValue::Blank)
            } else {
                Ok(DaxValue::Number(l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0)))
            }
        }
        BinaryOp::Div => {
            // DAX: raw division by zero yields Infinity/-Infinity/NaN, same as IEEE754 (it is
            // DIVIDE(...) that safely returns BLANK/alternate). ISERROR/IFERROR treat the
            // resulting non-finite number as an error.
            let den = r.as_f64().unwrap_or(0.0);
            Ok(DaxValue::Number(l.as_f64().unwrap_or(0.0) / den))
        }
        BinaryOp::Concat => Ok(DaxValue::Text(format!(
            "{}{}",
            l.as_display_string(),
            r.as_display_string()
        ))),
        BinaryOp::Eq => Ok(DaxValue::Boolean(l.dax_equals(r))),
        BinaryOp::StrictEq => Ok(DaxValue::Boolean(l.strict_equals(r))),
        BinaryOp::Neq => Ok(DaxValue::Boolean(!l.dax_equals(r))),
        BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
            let ord = compare_dax_values(l, r);
            let res = match op {
                BinaryOp::Lt => ord < 0,
                BinaryOp::Lte => ord <= 0,
                BinaryOp::Gt => ord > 0,
                BinaryOp::Gte => ord >= 0,
                _ => false,
            };
            Ok(DaxValue::Boolean(res))
        }
        BinaryOp::And | BinaryOp::Or => unreachable!(),
    }
}

fn compare_dax_values(l: &DaxValue, r: &DaxValue) -> i8 {
    if let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) {
        if !matches!(l, DaxValue::Text(_)) && !matches!(r, DaxValue::Text(_)) {
            if (a - b).abs() < 1e-9 {
                return 0;
            }
            return if a < b { -1 } else { 1 };
        }
    }
    let sa = l.as_display_string();
    let sb = r.as_display_string();
    match sa.cmp(&sb) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Context transition (CALCULATE, measure references): every active row context on the
/// stack becomes a filter, outermost first so inner rows win on shared columns. Only
/// columns with model lineage are turned into filters (extension columns are skipped).
fn apply_context_transition(model: &DaxModel, ctx: &FilterContext, row_stack: &[RowContext<'_>]) -> FilterContext {
    row_stack
        .iter()
        .fold(ctx.clone(), |acc, rc| apply_lineage_transition(model, &acc, rc))
}

fn eval_call(
    name: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let fname = name.trim().to_uppercase();
    match fname.as_str() {
        "BLANK" => Ok(DaxValue::Blank),
        "TRUE" => Ok(DaxValue::Boolean(true)),
        "FALSE" => Ok(DaxValue::Boolean(false)),
        "ISBLANK" => scalar::eval_isblank(args, model, ctx, row_stack, vars),
        "NOT" => scalar::eval_not(args, model, ctx, row_stack, vars),
        "AND" => scalar::eval_and(args, model, ctx, row_stack, vars),
        "OR" => scalar::eval_or(args, model, ctx, row_stack, vars),
        "IF" | "IF.EAGER" => scalar::eval_if(args, model, ctx, row_stack, vars),
        "SWITCH" => scalar::eval_switch(args, model, ctx, row_stack, vars),
        "DIVIDE" => scalar::eval_divide(args, model, ctx, row_stack, vars),
        "COALESCE" => scalar::eval_coalesce(args, model, ctx, row_stack, vars),
        "IFERROR" => scalar::eval_iferror(args, model, ctx, row_stack, vars),
        "ISERROR" => scalar::eval_iserror(args, model, ctx, row_stack, vars),
        "ABS" => scalar::eval_abs(args, model, ctx, row_stack, vars),
        "ROUND" => scalar::eval_round(args, model, ctx, row_stack, vars),
        "INT" => scalar::eval_int(args, model, ctx, row_stack, vars),
        "SQRT" => scalar::eval_sqrt(args, model, ctx, row_stack, vars),
        "POWER" => scalar::eval_power(args, model, ctx, row_stack, vars),
        "TOTALYTD" | "TOTALMTD" | "TOTALQTD" => {
            time::eval_total_to_date(&fname, args, model, ctx, row_stack, vars)
        }
        "OPENINGBALANCEMONTH" | "CLOSINGBALANCEMONTH"
        | "OPENINGBALANCEQUARTER" | "CLOSINGBALANCEQUARTER"
        | "OPENINGBALANCEYEAR" | "CLOSINGBALANCEYEAR" => {
            time::eval_opening_closing_balance(&fname, args, model, ctx, row_stack, vars)
        }
        "FIRSTDATE" | "LASTDATE" | "STARTOFMONTH" | "ENDOFMONTH"
        | "STARTOFQUARTER" | "ENDOFQUARTER" | "STARTOFYEAR" | "ENDOFYEAR" => {
            time::eval_date_boundary(&fname, args, model, ctx)
        }
        "RANKX" => aggregate::eval_rankx(args, model, ctx, row_stack, vars),
        "SUM" | "AVERAGE" | "AVERAGEA" | "MIN" | "MINA" | "MAX" | "MAXA"
        | "COUNT" | "COUNTA" | "COUNTBLANK" | "DISTINCTCOUNT" | "DISTINCTCOUNTNOBLANK"
        | "MEDIAN" | "STDEV.S" | "STDEV.P" | "VAR.S" | "VAR.P" | "PRODUCT" | "GEOMEAN" => {
            aggregate::eval_column_aggregate(&fname, args, model, ctx)
        }
        "PERCENTILE.INC" | "PERCENTILE.EXC" => {
            aggregate::eval_percentile(&fname, args, model, ctx, row_stack, vars)
        }
        "COUNTROWS" => aggregate::eval_countrows(args, model, ctx, row_stack, vars),
        "ISEMPTY" => aggregate::eval_isempty(args, model, ctx, row_stack, vars),
        "CONTAINSROW" => aggregate::eval_containsrow(args, model, ctx, row_stack, vars),
        "SUMX" | "AVERAGEX" | "MINX" | "MAXX" | "COUNTX" | "COUNTAX"
        | "MEDIANX" | "STDEVX.S" | "STDEVX.P" | "VARX.S" | "VARX.P"
        | "PRODUCTX" | "GEOMEANX" => {
            aggregate::eval_x_aggregate(&fname, args, model, ctx, row_stack, vars)
        }
        "PERCENTILEX.INC" | "PERCENTILEX.EXC" => {
            aggregate::eval_percentilex(&fname, args, model, ctx, row_stack, vars)
        }
        "CONCATENATEX" => aggregate::eval_concatenatex(args, model, ctx, row_stack, vars),
        "LOOKUPVALUE" => filter::eval_lookupvalue(args, model, ctx, row_stack, vars),
        "CALCULATE" => filter::eval_calculate(args, model, ctx, row_stack, vars),
        "SELECTEDVALUE" => filter::eval_selectedvalue(args, model, ctx, row_stack, vars),
        "HASONEVALUE" | "HASONEFILTER" => filter::eval_hasonevalue(&fname, args, model, ctx),
        "ISFILTERED" => filter::eval_isfiltered(args, ctx),
        "ISCROSSFILTERED" => filter::eval_iscrossfiltered(args, model, ctx),
        "ISINSCOPE" | "ISATLEVEL" => filter::eval_isinscope(args, model, ctx),
        "ISSUBTOTAL" => Ok(DaxValue::Boolean(false)),
        "FIRSTNONBLANK" | "LASTNONBLANK" => {
            filter::eval_first_last_nonblank(&fname, args, model, ctx, row_stack, vars)
        }
        "XNPV" | "XIRR" => aggregate::eval_xnpv_xirr(&fname, args, model, ctx, row_stack, vars),
        "EARLIER" | "EARLIEST" => row::eval_earlier(&fname, args, model, ctx, row_stack, vars),
        "RELATED" => row::eval_related(args, model, row_stack),
        "UPPER" => scalar::eval_upper(args, model, ctx, row_stack, vars),
        "LOWER" => scalar::eval_lower(args, model, ctx, row_stack, vars),
        "LEN" => scalar::eval_len(args, model, ctx, row_stack, vars),
        "CONCATENATE" => scalar::eval_concatenate(args, model, ctx, row_stack, vars),
        "LEFT" => scalar::eval_left(args, model, ctx, row_stack, vars),
        "RIGHT" => scalar::eval_right(args, model, ctx, row_stack, vars),
        "YEAR" | "MONTH" | "DAY" => {
            scalar::eval_date_part(&fname, args, model, ctx, row_stack, vars)
        }
        _ => {
            let mut eval_args = Vec::with_capacity(args.len());
            for a in args {
                eval_args.push(evaluate_ast(a, model, ctx, row_stack, vars)?);
            }
            if let Some(value) = try_eval_scalar_fn(&fname, &eval_args)? {
                Ok(value)
            } else {
                Err(format!("Função DAX '{}' ainda não implementada no motor nativo.", fname))
            }
        }
    }
}

fn eval_arg(
    args: &[Expr],
    idx: usize,
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let Some(e) = args.get(idx) else {
        return Ok(DaxValue::Blank);
    };
    evaluate_ast(e, model, ctx, row_stack, vars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dax::{DaxMeasureInput, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
    use crate::engine::DuckDbStore;
    use crate::pbi::pbip::open_pbip_project;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn test_native_rust_dax_calculate_var_switch_and_relationships() {
        let mut model = DaxModel::new();
        let f_rows = vec![
            serde_json::from_value(json!({"Data": "2026-04-05", "Valor": 10000.0, "Categoria": "Frete", "Tipo": "Entrada"})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-04-10", "Valor": 2500.0, "Categoria": "Combustível", "Tipo": "Saída"})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-05-05", "Valor": 15000.0, "Categoria": "Frete", "Tipo": "Entrada"})).unwrap(),
        ];
        let cat_rows = vec![
            serde_json::from_value(json!({"Categoria": "Frete", "Grupo DRE": "Receita Bruta", "DRE": "Sim"})).unwrap(),
            serde_json::from_value(json!({"Categoria": "Combustível", "Grupo DRE": "(-) Custos Variáveis", "DRE": "Sim"})).unwrap(),
        ];
        let est_rows = vec![
            serde_json::from_value(json!({"Linha": "Receita Bruta", "Ordem": 1})).unwrap(),
            serde_json::from_value(json!({"Linha": "Resultado Operacional", "Ordem": 13})).unwrap(),
        ];
        let cal_rows = vec![
            serde_json::from_value(json!({"Data": "2026-04-05", "Mês": 4})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-04-10", "Mês": 4})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-05-05", "Mês": 5})).unwrap(),
        ];

        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput { name: "fLançamentos".into(), columns: vec!["Data".into(), "Valor".into(), "Categoria".into(), "Tipo".into()], rows: f_rows },
                DaxTableInput { name: "dCategorias".into(), columns: vec!["Categoria".into(), "Grupo DRE".into(), "DRE".into()], rows: cat_rows },
                DaxTableInput { name: "dEstrutura".into(), columns: vec!["Linha".into(), "Ordem".into()], rows: est_rows },
                DaxTableInput { name: "dCalendario".into(), columns: vec!["Data".into(), "Mês".into()], rows: cal_rows },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "fLançamentos".into(),
                    from_column: "Categoria".into(),
                    to_table: "dCategorias".into(),
                    to_column: "Categoria".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "fLançamentos".into(),
                    from_column: "Data".into(),
                    to_table: "dCalendario".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
            ],
            measures: vec![
                DaxMeasureInput {
                    name: "Valor Assinado".into(),
                    table_name: "Medidas".into(),
                    formula: "SUMX('fLançamentos', 'fLançamentos'[Valor] * IF('fLançamentos'[Tipo] == \"Entrada\", 1, -1))".into(),
                },
                DaxMeasureInput {
                    name: "Resultado DRE".into(),
                    table_name: "Medidas".into(),
                    formula: "CALCULATE([Valor Assinado], dCategorias[DRE] == \"Sim\")".into(),
                },
                DaxMeasureInput {
                    name: "Faturamento".into(),
                    table_name: "Medidas".into(),
                    formula: "CALCULATE([Resultado DRE], dCategorias[Grupo DRE] = \"Receita Bruta\")".into(),
                },
                DaxMeasureInput {
                    name: "DRE".into(),
                    table_name: "Medidas".into(),
                    formula: "VAR _linha = SELECTEDVALUE(dEstrutura[Linha], \"Resultado Operacional\") VAR _rec = [Faturamento] RETURN SWITCH(_linha, \"Receita Bruta\", _rec, \"Resultado Operacional\", [Resultado DRE])".into(),
                },
            ],
        });

        let ctx = FilterContext::default();
        let fat = model.evaluate_dax("[Faturamento]", &ctx).unwrap().as_f64().unwrap();
        assert!((fat - 25000.0).abs() < 1e-6);

        let dre_total = model.evaluate_dax("[DRE]", &ctx).unwrap().as_f64().unwrap();
        assert!((dre_total - 22500.0).abs() < 1e-6);

        let mut ctx_rec = FilterContext::default();
        ctx_rec.set_column_filter("dEstrutura", "Linha", [DaxValue::Text("Receita Bruta".into()).canonical_key()].into_iter().collect());
        let dre_rec = model.evaluate_dax("[DRE]", &ctx_rec).unwrap().as_f64().unwrap();
        assert!((dre_rec - 25000.0).abs() < 1e-6);
    }

    #[test]
    fn test_native_rust_dax_ytd_mtd_previous_year_and_rankx() {
        let mut model = DaxModel::new();
        let make_cal_row = |d: &str| {
            let mut m = serde_json::Map::new();
            m.insert("Data".into(), serde_json::Value::String(d.into()));
            m
        };
        let make_sale_row = |d: &str, v: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Data".into(), serde_json::Value::String(d.into()));
            m.insert("Valor".into(), serde_json::json!(v));
            m
        };

        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "dCal".into(),
                    columns: vec!["Data".into()],
                    rows: vec![
                        make_cal_row("2025-01-10"),
                        make_cal_row("2025-02-15"),
                        make_cal_row("2026-01-10"),
                        make_cal_row("2026-02-15"),
                        make_cal_row("2026-03-05"),
                    ],
                },
                DaxTableInput {
                    name: "fVendas".into(),
                    columns: vec!["Data".into(), "Valor".into()],
                    rows: vec![
                        make_sale_row("2025-01-10", 100.0),
                        make_sale_row("2025-02-15", 200.0),
                        make_sale_row("2026-01-10", 300.0),
                        make_sale_row("2026-02-15", 400.0),
                        make_sale_row("2026-03-05", 500.0),
                    ],
                },
            ],
            relationships: vec![DaxRelationshipInput {
                from_table: "fVendas".into(),
                from_column: "Data".into(),
                to_table: "dCal".into(),
                to_column: "Data".into(),
                cross_filter_direction: "single".into(),
                is_active: true,
            }],
            measures: vec![
                DaxMeasureInput {
                    name: "Receita".into(),
                    table_name: "fVendas".into(),
                    formula: "SUM(fVendas[Valor])".into(),
                },
                DaxMeasureInput {
                    name: "Receita YTD".into(),
                    table_name: "fVendas".into(),
                    formula: "TOTALYTD([Receita], dCal[Data])".into(),
                },
                DaxMeasureInput {
                    name: "Receita MTD".into(),
                    table_name: "fVendas".into(),
                    formula: "TOTALMTD([Receita], dCal[Data])".into(),
                },
                DaxMeasureInput {
                    name: "Receita Ano Ant".into(),
                    table_name: "fVendas".into(),
                    formula: "CALCULATE([Receita], PREVIOUSYEAR(dCal[Data]))".into(),
                },
                DaxMeasureInput {
                    name: "Receita Mesmo Dia Ano Ant".into(),
                    table_name: "fVendas".into(),
                    formula: "CALCULATE([Receita], SAMEPERIODLASTYEAR(dCal[Data]))".into(),
                },
                DaxMeasureInput {
                    name: "Rank Data".into(),
                    table_name: "fVendas".into(),
                    formula: "RANKX(ALL(fVendas), [Receita])".into(),
                },
            ],
        });

        let mut ctx_feb_2026 = FilterContext::default();
        ctx_feb_2026.set_column_filter(
            "dCal",
            "Data",
            [DaxValue::Text("2026-02-15".into()).canonical_key()]
                .into_iter()
                .collect(),
        );

        let ytd = model.evaluate_dax("[Receita YTD]", &ctx_feb_2026).unwrap().as_f64().unwrap();
        assert!((ytd - 700.0).abs() < 1e-6);

        let mtd = model.evaluate_dax("[Receita MTD]", &ctx_feb_2026).unwrap().as_f64().unwrap();
        assert!((mtd - 400.0).abs() < 1e-6);

        let prev_y = model.evaluate_dax("[Receita Ano Ant]", &ctx_feb_2026).unwrap().as_f64().unwrap();
        assert!((prev_y - 300.0).abs() < 1e-6);

        let same_p = model.evaluate_dax("[Receita Mesmo Dia Ano Ant]", &ctx_feb_2026).unwrap().as_f64().unwrap();
        assert!((same_p - 200.0).abs() < 1e-6);

        let rank = model.evaluate_dax("[Rank Data]", &ctx_feb_2026).unwrap().as_f64().unwrap();
        assert!((rank - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_rhino_pbip_end_to_end() {
        let rhino_path = Path::new("/home/luis/Documents/Power BI/Rhino/Rhino Bip.pbip");
        if !rhino_path.exists() {
            return;
        }
        let db = DuckDbStore::new_in_memory().unwrap();
        let summary = open_pbip_project(rhino_path, Some(&db)).expect("load Rhino Bip.pbip");
        assert!(!summary.loaded_tables.is_empty(), "Rhino deve carregar tabelas reais do Excel/Calendar/DATATABLE");

        let mut model = DaxModel::new();
        let tables_input: Vec<DaxTableInput> = summary
            .loaded_tables
            .iter()
            .map(|t| DaxTableInput {
                name: t.table_name.clone(),
                columns: t.columns.iter().map(|c| c.name.clone()).collect(),
                rows: t.rows.clone(),
            })
            .collect();
        let rels_input: Vec<DaxRelationshipInput> = summary
            .relationships
            .iter()
            .map(|r| DaxRelationshipInput {
                from_table: r.from_table.clone(),
                from_column: r.from_column.clone(),
                to_table: r.to_table.clone(),
                to_column: r.to_column.clone(),
                cross_filter_direction: r.cross_filter_direction.clone(),
                is_active: r.is_active,
            })
            .collect();
        let measures_input: Vec<DaxMeasureInput> = summary
            .tables
            .iter()
            .flat_map(|t| t.measures.iter())
            .map(|m| DaxMeasureInput {
                name: m.name.clone(),
                table_name: m.table_name.clone(),
                formula: m.expression.clone(),
            })
            .collect();

        model.sync_from_payload(DaxModelSyncPayload {
            tables: tables_input,
            relationships: rels_input,
            measures: measures_input,
        });

        let ctx = FilterContext::default();
        let fat = model.evaluate_dax("[Faturamento]", &ctx).expect("eval Faturamento");
        let salarios = model.evaluate_dax("[Salários]", &ctx).expect("eval Salários");
        let dre = model.evaluate_dax("[DRE]", &ctx).expect("eval DRE");
        let entradas = model.evaluate_dax("[Entradas]", &ctx).expect("eval Entradas");
        let saidas = model.evaluate_dax("[Saídas]", &ctx).expect("eval Saídas");

        let all_value_fields: Vec<&str> = summary
            .pages
            .iter()
            .flat_map(|p| p.visuals.iter())
            .flat_map(|v| v.value_fields.iter())
            .map(|s| s.as_str())
            .collect();
        assert!(
            all_value_fields.contains(&"Salários"),
            "Visual multi-card da DRE deve resolver Medidas.Salários em vez de Medidas.Despesas com Pessoal"
        );
        assert!(
            all_value_fields.contains(&"Abertura do Caixa Mês"),
            "Visual da DFC deve resolver Medidas.Abertura do Caixa Mês em vez de Medidas.Abertura do Caixa"
        );

        let mut ctx_set = FilterContext::default();
        ctx_set.set_column_filter(
            "dCalendario",
            "Ano",
            [DaxValue::Text("2026".into()).canonical_key()].into_iter().collect(),
        );
        ctx_set.set_column_filter(
            "dCalendario",
            "Nome do Mês",
            [DaxValue::Text("Setembro".into()).canonical_key()].into_iter().collect(),
        );
        let fat_set = model.evaluate_dax("[Faturamento]", &ctx_set).expect("eval Faturamento Setembro");
        let sal_set = model.evaluate_dax("[Salários]", &ctx_set).expect("eval Salários Setembro");
        let mut ctx_out = FilterContext::default();
        ctx_out.set_column_filter(
            "dCalendario",
            "Nome do Mês",
            [DaxValue::Text("Outubro".into()).canonical_key()].into_iter().collect(),
        );
        let fat_out = model.evaluate_dax("[Faturamento]", &ctx_out).expect("eval Faturamento Outubro");

        let mut ctx_nov_mesano = FilterContext::default();
        ctx_nov_mesano.set_column_filter(
            "dCalendario",
            "Mês/Ano",
            [DaxValue::Text("Nov/2026".into()).canonical_key()].into_iter().collect(),
        );
        let fat_nov = model.evaluate_dax("[Faturamento]", &ctx_nov_mesano).expect("eval Fat Nov");
        let lucro_nov = model.evaluate_dax("[Lucro Atual]", &ctx_nov_mesano).expect("eval Lucro Nov");
        let fat_prev_nov = model.evaluate_dax("[Faturamento Mês Passado]", &ctx_nov_mesano).expect("eval Fat Prev Nov");
        let lucro_prev_nov = model.evaluate_dax("[Lucro Mês Passado]", &ctx_nov_mesano).expect("eval Lucro Prev Nov");

        let mut ctx_out_mesano = FilterContext::default();
        ctx_out_mesano.set_column_filter(
            "dCalendario",
            "Mês/Ano",
            [DaxValue::Text("Out/2026".into()).canonical_key()].into_iter().collect(),
        );
        let fat_out_m = model.evaluate_dax("[Faturamento]", &ctx_out_mesano).expect("eval Fat Out");
        let lucro_out_m = model.evaluate_dax("[Lucro Atual]", &ctx_out_mesano).expect("eval Lucro Out");
        let fat_prev_out = model.evaluate_dax("[Faturamento Mês Passado]", &ctx_out_mesano).expect("eval Fat Prev Out");
        let lucro_prev_out = model.evaluate_dax("[Lucro Mês Passado]", &ctx_out_mesano).expect("eval Lucro Prev Out");

        println!(
            "RHINO REAL -> Faturamento={:?}, Salarios={:?}, FatSet={:?}, SalariosSet={:?}, FatOut={:?}, DRE={:?}, Entradas={:?}, Saidas={:?}, LoadedTables={:?}",
            fat,
            salarios,
            fat_set,
            sal_set,
            fat_out,
            dre,
            entradas,
            saidas,
            summary.loaded_tables.iter().map(|t| (&t.table_name, t.total_rows)).collect::<Vec<_>>()
        );
        println!(
            "RHINO TIME INTELLIGENCE -> Nov(Fat={:?}, Lucro={:?}, FatPrev={:?}, LucroPrev={:?}) | Out(Fat={:?}, Lucro={:?}, FatPrev={:?}, LucroPrev={:?})",
            fat_nov, lucro_nov, fat_prev_nov, lucro_prev_nov,
            fat_out_m, lucro_out_m, fat_prev_out, lucro_prev_out
        );
        assert!(fat.as_f64().unwrap_or(0.0) > 0.0);
        assert!(salarios.as_f64().unwrap_or(0.0).abs() > 0.0);
        assert!(
            fat_set.as_f64().unwrap_or(0.0) > 50_000.0 || fat_out.as_f64().unwrap_or(0.0) > 50_000.0,
            "Faturamento mensal deve considerar o mês completo, não apenas o dia 1"
        );
        assert!(
            (fat_prev_nov.as_f64().unwrap_or(0.0) - fat_out_m.as_f64().unwrap_or(0.0)).abs() < 1e-2,
            "Faturamento Mês Passado em Nov/2026 deve igualar Faturamento de Out/2026 (incluindo dia 31)"
        );
        assert!(
            (lucro_prev_nov.as_f64().unwrap_or(0.0) - lucro_out_m.as_f64().unwrap_or(0.0)).abs() < 1e-2,
            "Lucro Mês Passado em Nov/2026 deve igualar Lucro Atual de Out/2026 (incluindo dia 31)"
        );
        assert!(
            (fat_prev_out.as_f64().unwrap_or(0.0) - fat_set.as_f64().unwrap_or(0.0)).abs() < 1e-2,
            "Faturamento Mês Passado em Out/2026 deve igualar Faturamento de Set/2026"
        );
    }

    #[test]
    fn test_dateadd_full_month_expansion_nov_to_oct_clears_mes_ano_filter() {
        let mut model = DaxModel::new();
        let mut cal_rows = Vec::new();
        for d in 1..=31 {
            cal_rows.push(
                serde_json::from_value(json!({
                    "Data": format!("2026-10-{:02}", d),
                    "Mês/Ano": "Out/2026"
                }))
                .unwrap(),
            );
        }
        for d in 1..=30 {
            cal_rows.push(
                serde_json::from_value(json!({
                    "Data": format!("2026-11-{:02}", d),
                    "Mês/Ano": "Nov/2026"
                }))
                .unwrap(),
            );
        }

        let fact_rows = vec![
            serde_json::from_value(json!({"Data": "2026-10-01", "Valor": 1000.0})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-10-31", "Valor": 6700.0})).unwrap(),
            serde_json::from_value(json!({"Data": "2026-11-15", "Valor": 5000.0})).unwrap(),
        ];

        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "dCalendario".into(),
                    columns: vec!["Data".into(), "Mês/Ano".into()],
                    rows: cal_rows,
                },
                DaxTableInput {
                    name: "fLanc".into(),
                    columns: vec!["Data".into(), "Valor".into()],
                    rows: fact_rows,
                },
            ],
            relationships: vec![DaxRelationshipInput {
                from_table: "fLanc".into(),
                from_column: "Data".into(),
                to_table: "dCalendario".into(),
                to_column: "Data".into(),
                cross_filter_direction: "single".into(),
                is_active: true,
            }],
            measures: vec![
                DaxMeasureInput {
                    name: "Fat".into(),
                    table_name: "fLanc".into(),
                    formula: "SUM(fLanc[Valor])".into(),
                },
                DaxMeasureInput {
                    name: "Fat Mes Passado DateAdd".into(),
                    table_name: "fLanc".into(),
                    formula: "CALCULATE([Fat], DATEADD(dCalendario[Data], -1, MONTH))".into(),
                },
                DaxMeasureInput {
                    name: "Fat Mes Passado Prev".into(),
                    table_name: "fLanc".into(),
                    formula: "CALCULATE([Fat], PREVIOUSMONTH(dCalendario[Data]))".into(),
                },
            ],
        });

        let mut ctx_nov = FilterContext::default();
        ctx_nov.set_column_filter(
            "dCalendario",
            "Mês/Ano",
            [DaxValue::Text("Nov/2026".into()).canonical_key()].into_iter().collect(),
        );

        let prev_dateadd = model.evaluate_dax("[Fat Mes Passado DateAdd]", &ctx_nov).unwrap().as_f64().unwrap();
        let prev_month = model.evaluate_dax("[Fat Mes Passado Prev]", &ctx_nov).unwrap().as_f64().unwrap();
        assert!(
            (prev_dateadd - 7700.0).abs() < 1e-6,
            "DATEADD -1 MONTH em Nov/2026 sem REMOVEFILTERS explícito deve limpar Mês/Ano e incluir dia 31/10 (esperado 7700, obtido {})",
            prev_dateadd
        );
        assert!(
            (prev_month - 7700.0).abs() < 1e-6,
            "PREVIOUSMONTH em Nov/2026 deve retornar todo Out/2026 incluindo dia 31/10 (esperado 7700, obtido {})",
            prev_month
        );
    }

    #[test]
    fn test_bidirectional_relationship_no_bounce_back_and_numeric_string_filter() {
        let mut model = DaxModel::new();
        let cal_row = |d: &str, ano: i64, mes: i64| {
            let mut m = serde_json::Map::new();
            m.insert("Data".into(), serde_json::Value::String(d.into()));
            m.insert("Ano".into(), serde_json::json!(ano));
            m.insert("Mes".into(), serde_json::json!(mes));
            m
        };
        let sale_row = |d: &str, v: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Data".into(), serde_json::Value::String(d.into()));
            m.insert("Valor".into(), serde_json::json!(v));
            m
        };
        let open_row = |d: &str, v: f64| {
            let mut m = serde_json::Map::new();
            m.insert("Data".into(), serde_json::Value::String(d.into()));
            m.insert("Saldo".into(), serde_json::json!(v));
            m
        };

        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "dCal".into(),
                    columns: vec!["Data".into(), "Ano".into(), "Mes".into()],
                    rows: vec![
                        cal_row("2026-03-01", 2026, 3),
                        cal_row("2026-03-10", 2026, 3),
                        cal_row("2026-03-20", 2026, 3),
                    ],
                },
                DaxTableInput {
                    name: "fVendas".into(),
                    columns: vec!["Data".into(), "Valor".into()],
                    rows: vec![
                        sale_row("2026-03-01", 3117.20),
                        sale_row("2026-03-10", 60000.00),
                        sale_row("2026-03-20", 65606.32),
                    ],
                },
                DaxTableInput {
                    name: "fAberturas".into(),
                    columns: vec!["Data".into(), "Saldo".into()],
                    // Only day 1 of month exists in fAberturas
                    rows: vec![open_row("2026-03-01", 10000.0)],
                },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "fVendas".into(),
                    from_column: "Data".into(),
                    to_table: "dCal".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "dCal".into(),
                    from_column: "Data".into(),
                    to_table: "fAberturas".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "both".into(),
                    is_active: true,
                },
            ],
            measures: vec![DaxMeasureInput {
                name: "Fat".into(),
                table_name: "fVendas".into(),
                formula: "SUM(fVendas[Valor])".into(),
            }],
        });

        let mut ctx = FilterContext::default();
        ctx.set_column_filter(
            "dCal",
            "Ano",
            [DaxValue::Text("2026".into()).canonical_key()].into_iter().collect(),
        );
        ctx.set_column_filter(
            "dCal",
            "Mes",
            [DaxValue::Text("3".into()).canonical_key()].into_iter().collect(),
        );

        let total = model.evaluate_dax("[Fat]", &ctx).unwrap().as_f64().unwrap();
        assert!((total - 128723.52).abs() < 1e-6);
    }

    #[test]
    fn test_dax_v06_stats_lookup_topn_time_intel_and_userelationship() {
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "dCal".into(),
                    columns: vec!["Data".into(), "Trimestre".into()],
                    rows: vec![
                        serde_json::from_value(json!({"Data": "2026-01-10", "Trimestre": "T1"})).unwrap(),
                        serde_json::from_value(json!({"Data": "2026-02-15", "Trimestre": "T1"})).unwrap(),
                        serde_json::from_value(json!({"Data": "2026-04-05", "Trimestre": "T2"})).unwrap(),
                        serde_json::from_value(json!({"Data": "2026-04-30", "Trimestre": "T2"})).unwrap(),
                    ],
                },
                DaxTableInput {
                    name: "dClientes".into(),
                    columns: vec!["Id".into(), "Nome".into()],
                    rows: vec![
                        serde_json::from_value(json!({"Id": 1, "Nome": "Alpha"})).unwrap(),
                        serde_json::from_value(json!({"Id": 2, "Nome": "Beta"})).unwrap(),
                    ],
                },
                DaxTableInput {
                    name: "fPedidos".into(),
                    columns: vec!["DataPedido".into(), "DataEntrega".into(), "ClienteId".into(), "Valor".into()],
                    rows: vec![
                        serde_json::from_value(json!({"DataPedido": "2026-01-10", "DataEntrega": "2026-04-05", "ClienteId": 1, "Valor": 100.0})).unwrap(),
                        serde_json::from_value(json!({"DataPedido": "2026-02-15", "DataEntrega": "2026-04-30", "ClienteId": 2, "Valor": 300.0})).unwrap(),
                        serde_json::from_value(json!({"DataPedido": "2026-04-05", "DataEntrega": "2026-04-30", "ClienteId": 1, "Valor": 500.0})).unwrap(),
                        serde_json::from_value(json!({"DataPedido": "2026-04-30", "DataEntrega": "2026-04-30", "ClienteId": 2, "Valor": 700.0})).unwrap(),
                    ],
                },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "fPedidos".into(),
                    from_column: "DataPedido".into(),
                    to_table: "dCal".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "fPedidos".into(),
                    from_column: "DataEntrega".into(),
                    to_table: "dCal".into(),
                    to_column: "Data".into(),
                    cross_filter_direction: "single".into(),
                    is_active: false,
                },
            ],
            measures: vec![
                DaxMeasureInput {
                    name: "Total".into(),
                    table_name: "fPedidos".into(),
                    formula: "SUM(fPedidos[Valor])".into(),
                },
                DaxMeasureInput {
                    name: "QTD T2".into(),
                    table_name: "fPedidos".into(),
                    formula: "TOTALQTD([Total], dCal[Data])".into(),
                },
            ],
        });

        let ctx = FilterContext::default();

        // Stats + Lookup + TopN + Values + Concat
        let median = model.evaluate_dax("MEDIAN(fPedidos[Valor])", &ctx).unwrap().as_f64().unwrap();
        assert!((median - 400.0).abs() < 1e-6);
        let p25 = model.evaluate_dax("PERCENTILE.INC(fPedidos[Valor], 0.5)", &ctx).unwrap().as_f64().unwrap();
        assert!((p25 - 400.0).abs() < 1e-6);
        let stdev = model.evaluate_dax("STDEV.S(fPedidos[Valor])", &ctx).unwrap().as_f64().unwrap();
        assert!(stdev > 200.0);

        let cli = model.evaluate_dax("LOOKUPVALUE(dClientes[Nome], dClientes[Id], 2)", &ctx).unwrap().as_display_string();
        assert_eq!(cli, "Beta");

        let top2 = model.evaluate_dax("SUMX(TOPN(2, fPedidos, fPedidos[Valor], DESC), fPedidos[Valor])", &ctx).unwrap().as_f64().unwrap();
        assert!((top2 - 1200.0).abs() < 1e-6);

        let distinct_cli = model.evaluate_dax("COUNTROWS(VALUES(fPedidos[ClienteId]))", &ctx).unwrap().as_f64().unwrap();
        assert!((distinct_cli - 2.0).abs() < 1e-6);

        let concat = model.evaluate_dax("CONCATENATEX(dClientes, dClientes[Nome], \", \", dClientes[Id], ASC)", &ctx).unwrap().as_display_string();
        assert_eq!(concat, "Alpha, Beta");

        // Time intelligence: DATESBETWEEN & CLOSINGBALANCEMONTH
        let between = model
            .evaluate_dax("CALCULATE([Total], DATESBETWEEN(dCal[Data], \"2026-01-01\", \"2026-03-01\"))", &ctx)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((between - 400.0).abs() < 1e-6);

        let mut ctx_apr = FilterContext::default();
        ctx_apr.set_column_filter("dCal", "Trimestre", [DaxValue::Text("T2".into()).canonical_key()].into_iter().collect());
        let qtd = model.evaluate_dax("[QTD T2]", &ctx_apr).unwrap().as_f64().unwrap();
        assert!((qtd - 1200.0).abs() < 1e-6);
        let close_m = model.evaluate_dax("CLOSINGBALANCEMONTH([Total], dCal[Data])", &ctx_apr).unwrap().as_f64().unwrap();
        assert!((close_m - 700.0).abs() < 1e-6);

        // USERELATIONSHIP activates delivery date instead of order date (100 + 300 + 500 + 700 = 1600 all delivered in T2!)
        let by_delivery = model
            .evaluate_dax("CALCULATE([Total], USERELATIONSHIP(fPedidos[DataEntrega], dCal[Data]))", &ctx_apr)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((by_delivery - 1600.0).abs() < 1e-6);
    }

    fn rows(v: serde_json::Value) -> Vec<serde_json::Map<String, serde_json::Value>> {
        v.as_array().unwrap().iter().map(|r| r.as_object().unwrap().clone()).collect()
    }

    fn rel(from: &str, from_col: &str, to: &str, to_col: &str) -> DaxRelationshipInput {
        DaxRelationshipInput {
            from_table: from.into(),
            from_column: from_col.into(),
            to_table: to.into(),
            to_column: to_col.into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }
    }

    /// fact f (4 sales) -> dim p (3 products) -> sub-dim g (2 groups).
    fn table_fns_model() -> DaxModel {
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "f".into(),
                    columns: vec!["ProdID".into(), "Valor".into()],
                    rows: rows(json!([
                        {"ProdID": 1, "Valor": 10}, {"ProdID": 1, "Valor": 5},
                        {"ProdID": 2, "Valor": 20}, {"ProdID": 3, "Valor": 40}
                    ])),
                },
                DaxTableInput {
                    name: "p".into(),
                    columns: vec!["ProdID".into(), "Cat".into(), "GrupoID".into(), "Preco".into()],
                    rows: rows(json!([
                        {"ProdID": 1, "Cat": "A", "GrupoID": 1, "Preco": 3},
                        {"ProdID": 2, "Cat": "A", "GrupoID": 1, "Preco": 1},
                        {"ProdID": 3, "Cat": "B", "GrupoID": 2, "Preco": 2}
                    ])),
                },
                DaxTableInput {
                    name: "g".into(),
                    columns: vec!["GrupoID".into(), "Grupo".into()],
                    rows: rows(json!([{"GrupoID": 1, "Grupo": "G1"}, {"GrupoID": 2, "Grupo": "G2"}])),
                },
            ],
            relationships: vec![rel("f", "ProdID", "p", "ProdID"), rel("p", "GrupoID", "g", "GrupoID")],
            measures: vec![DaxMeasureInput {
                name: "Total".into(),
                table_name: "f".into(),
                formula: "SUM(f[Valor])".into(),
            }],
        });
        model
    }

    fn eval_num(model: &DaxModel, expr: &str) -> f64 {
        match model.evaluate_dax(expr, &FilterContext::default()) {
            Ok(v) => v.as_f64().unwrap_or_else(|| panic!("{expr} -> {v:?} (not a number)")),
            Err(e) => panic!("{expr} -> error: {e}"),
        }
    }

    #[test]
    fn test_summarize_addcolumns_selectcolumns_owned_tables() {
        let model = table_fns_model();
        // Group-by through one relationship hop (f -> p) and through two hops (f -> p -> g).
        assert_eq!(eval_num(&model, "COUNTROWS(SUMMARIZE(f, p[Cat]))"), 2.0);
        assert_eq!(eval_num(&model, "COUNTROWS(SUMMARIZE(f, g[Grupo], p[Cat]))"), 2.0);
        assert_eq!(eval_num(&model, "COUNTROWS(SUMMARIZE(f, f[ProdID], p[Cat]))"), 3.0);
        // Extension columns are evaluated in the group's filter context.
        assert_eq!(
            eval_num(&model, "MAXX(SUMMARIZE(f, p[Cat], \"V\", SUM(f[Valor])), [V])"),
            40.0
        );
        assert_eq!(
            eval_num(&model, "SUMX(SUMMARIZE(f, p[Cat], \"V\", [Total]), [V])"),
            75.0
        );
        // Group-by column values resolve in row context, both qualified and bare.
        assert_eq!(
            model
                .evaluate_dax(
                    "CONCATENATEX(SUMMARIZE(f, p[Cat]), p[Cat] & [Cat], \",\", p[Cat], ASC)",
                    &FilterContext::default()
                )
                .unwrap(),
            DaxValue::Text("AA,BB".into())
        );
        // SUMMARIZE respects the outer filter context of its base table.
        assert_eq!(eval_num(&model, "CALCULATE(COUNTROWS(SUMMARIZE(f, p[Cat])), p[Cat] = \"B\")"), 1.0);

        // ADDCOLUMNS keeps row count and base columns; measure refs transition per row.
        assert_eq!(eval_num(&model, "COUNTROWS(ADDCOLUMNS(VALUES(p[Cat]), \"T\", [Total]))"), 2.0);
        assert_eq!(eval_num(&model, "MAXX(ADDCOLUMNS(VALUES(p[Cat]), \"T\", [Total]), [T])"), 40.0);
        assert_eq!(
            eval_num(&model, "SUMX(ADDCOLUMNS(p, \"Dobro\", p[Preco] * 2), [Dobro] + p[Preco])"),
            18.0
        );
        // SELECTCOLUMNS projects without removing duplicates.
        assert_eq!(eval_num(&model, "COUNTROWS(SELECTCOLUMNS(p, \"C\", p[Cat]))"), 3.0);
        assert_eq!(
            model
                .evaluate_dax(
                    "CONCATENATEX(SELECTCOLUMNS(p, \"C\", p[Cat]), [C], \"\")",
                    &FilterContext::default()
                )
                .unwrap(),
            DaxValue::Text("AAB".into())
        );
        // Malformed extension pairs are errors, not panics.
        assert!(model.evaluate_dax("COUNTROWS(ADDCOLUMNS(p, \"X\"))", &FilterContext::default()).is_err());
        assert!(model.evaluate_dax("COUNTROWS(SUMMARIZE(f, p[Nope]))", &FilterContext::default()).is_err());
    }

    #[test]
    fn test_filter_topn_compose_with_synthetic_tables() {
        let model = table_fns_model();
        // FILTER(VALUES(col)) yields the one-column synthetic table set operations compare against.
        assert_eq!(
            eval_num(&model, "COUNTROWS(EXCEPT(VALUES(p[Cat]), FILTER(VALUES(p[Cat]), p[Cat] = \"A\")))"),
            1.0
        );
        assert_eq!(
            eval_num(&model, "COUNTROWS(INTERSECT(VALUES(p[Cat]), FILTER(ALL(p[Cat]), p[Cat] <> \"A\")))"),
            1.0
        );
        // FILTER / TOPN over ADDCOLUMNS / SUMMARIZE / UNION.
        assert_eq!(
            eval_num(&model, "COUNTROWS(FILTER(ADDCOLUMNS(VALUES(p[Cat]), \"T\", [Total]), [T] > 35))"),
            1.0
        );
        assert_eq!(
            eval_num(&model, "MAXX(TOPN(1, SUMMARIZE(f, p[Cat], \"V\", [Total]), [V], ASC), [V])"),
            35.0
        );
        assert_eq!(
            eval_num(&model, "COUNTROWS(FILTER(UNION(VALUES(p[Cat]), VALUES(p[Cat])), p[Cat] = \"B\"))"),
            2.0
        );
        assert_eq!(eval_num(&model, "COUNTROWS(TOPN(2, FILTER(p, p[Preco] > 1), p[Preco]))"), 2.0);
        // Synthetic FILTER as a CALCULATE modifier filters through the column lineage
        // (category B -> product 3 -> 40), physical FILTER through the key column.
        assert_eq!(eval_num(&model, "CALCULATE([Total], FILTER(VALUES(p[Cat]), p[Cat] = \"B\"))"), 40.0);
        assert_eq!(eval_num(&model, "CALCULATE([Total], FILTER(p, p[Preco] >= 2))"), 55.0);
        // CALCULATETABLE over a synthetic table.
        assert_eq!(
            eval_num(&model, "COUNTROWS(CALCULATETABLE(SUMMARIZE(f, p[Cat]), p[Cat] = \"A\"))"),
            1.0
        );
    }

    #[test]
    fn test_earlier_earliest_row_context_stack() {
        let model = table_fns_model();
        let is_err = |e: &str| model.evaluate_dax(e, &FilterContext::default()).is_err();
        // p[Preco] = 3, 1, 2. Classic rank-by-count: 3 + 1 + 2.
        assert_eq!(eval_num(&model, "SUMX(p, COUNTROWS(FILTER(p, p[Preco] <= EARLIER(p[Preco]))))"), 6.0);

        // Three nested row contexts on p: outer {3,2}, middle {1}, inner {3,1,2}.
        let nested = |inner: &str| {
            format!(
                "SUMX(FILTER(p, p[Preco] >= 2), SUMX(FILTER(p, p[Preco] <= 1), SUMX(p, {})))",
                inner
            )
        };
        assert_eq!(eval_num(&model, &nested("p[Preco]")), 12.0); // innermost: 2 * (3+1+2)
        assert_eq!(eval_num(&model, &nested("EARLIER(p[Preco])")), 6.0); // middle: 2 * 3 * 1
        assert_eq!(eval_num(&model, &nested("EARLIER(p[Preco], 1)")), 6.0);
        assert_eq!(eval_num(&model, &nested("EARLIER(p[Preco], 2)")), 15.0); // outer: (3+2) * 3
        assert_eq!(eval_num(&model, &nested("EARLIEST(p[Preco])")), 15.0);
        assert!(is_err(&nested("EARLIER(p[Preco], 3)")));
        assert!(is_err(&nested("EARLIER(p[Preco], 0)")));

        // With two levels EARLIEST == EARLIER: 3 * (3+1+2).
        assert_eq!(eval_num(&model, "SUMX(p, SUMX(p, EARLIEST(p[Preco])))"), 18.0);
        // Only row contexts containing the column count as levels.
        assert!(is_err("SUMX(p, SUMX(g, EARLIER(p[Preco])))"));
        assert_eq!(eval_num(&model, "SUMX(p, SUMX(g, SUMX(p, EARLIER(p[Preco]))))"), 36.0);
        // No outer row context at all.
        assert!(is_err("SUMX(p, EARLIER(p[Preco]))"));
        assert!(is_err("EARLIEST(p[Preco])"));

        // ADDCOLUMNS pushes a row context too, and EARLIER works on extension columns.
        assert_eq!(
            eval_num(&model, "MAXX(ADDCOLUMNS(p, \"R\", COUNTROWS(FILTER(p, p[Preco] >= EARLIER(p[Preco])))), [R])"),
            3.0
        );
        assert_eq!(
            eval_num(
                &model,
                "SUMX(ADDCOLUMNS(p, \"D\", p[Preco] * 10), \
                 COUNTROWS(FILTER(ADDCOLUMNS(p, \"D\", p[Preco] * 10), [D] < EARLIER([D]))))"
            ),
            3.0
        );
    }

    #[test]
    fn test_iterators_row_context_vs_context_transition() {
        let model = table_fns_model();
        // Iterators add a row context only; the filter context is unchanged...
        assert_eq!(eval_num(&model, "SUMX(p, COUNTROWS(p))"), 9.0);
        // ...until CALCULATE / a measure reference transitions it.
        assert_eq!(eval_num(&model, "SUMX(p, CALCULATE(COUNTROWS(p)))"), 3.0);
        // Context transition applies *every* row context on the stack, not only the innermost:
        // (A,1)=15, (A,2)=20, (B,3)=40, the other (Cat, ProdID) pairs are blank. Innermost-only
        // transition would give 150.
        assert_eq!(eval_num(&model, "SUMX(VALUES(p[Cat]), SUMX(VALUES(p[ProdID]), [Total]))"), 75.0);
    }

    #[test]
    fn test_dax_v07_rust_expanded_coverage() {
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "fVendas".into(),
                    columns: vec!["Cat".into(), "Valor".into()],
                    rows: vec![
                        serde_json::json!({"Cat": "A", "Valor": 100}).as_object().unwrap().clone(),
                        serde_json::json!({"Cat": "B", "Valor": 250}).as_object().unwrap().clone(),
                        serde_json::json!({"Cat": "C", "Valor": 400}).as_object().unwrap().clone(),
                    ],
                },
                DaxTableInput {
                    name: "dGrupo".into(),
                    columns: vec!["Nome".into()],
                    rows: vec![
                        serde_json::json!({"Nome": "A"}).as_object().unwrap().clone(),
                        serde_json::json!({"Nome": "C"}).as_object().unwrap().clone(),
                    ],
                },
            ],
            relationships: vec![],
            measures: vec![DaxMeasureInput {
                name: "Total".into(),
                table_name: "fVendas".into(),
                formula: "SUM(fVendas[Valor])".into(),
            }],
        });

        let ctx = FilterContext::default();

        // TREATAS applies virtual filter from unrelated table dGrupo[Nome] -> fVendas[Cat] (100 + 400 = 500)
        let treatas_val = model
            .evaluate_dax("CALCULATE([Total], TREATAS(VALUES(dGrupo[Nome]), fVendas[Cat]))", &ctx)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((treatas_val - 500.0).abs() < 1e-6);

        // GENERATESERIES + SUMX: 1 + 2 + 3 + 4 + 5 = 15, COUNTROWS = 5
        let gs_count = model.evaluate_dax("COUNTROWS(GENERATESERIES(1, 5, 1))", &ctx).unwrap().as_f64().unwrap();
        assert!((gs_count - 5.0).abs() < 1e-6);
        let gs_sum = model.evaluate_dax("SUMX(GENERATESERIES(1, 5, 1), [Value])", &ctx).unwrap().as_f64().unwrap();
        assert!((gs_sum - 15.0).abs() < 1e-6);

        // CONTAINSROW
        assert_eq!(
            model.evaluate_dax("CONTAINSROW(VALUES(dGrupo[Nome]), \"A\")", &ctx).unwrap(),
            DaxValue::Boolean(true)
        );
        assert_eq!(
            model.evaluate_dax("CONTAINSROW(VALUES(dGrupo[Nome]), \"Z\")", &ctx).unwrap(),
            DaxValue::Boolean(false)
        );

        // Type/conversion, math/bitwise, parent-child & financial via evaluator
        assert_eq!(
            model.evaluate_dax("COALESCE(BLANK(), 42)", &ctx).unwrap().as_f64(),
            Some(42.0)
        );
        assert_eq!(
            model.evaluate_dax("COMBIN(6, 2)", &ctx).unwrap().as_f64(),
            Some(15.0)
        );
        assert_eq!(
            model.evaluate_dax("PATHLENGTH(\"1|4|9|12\")", &ctx).unwrap().as_f64(),
            Some(4.0)
        );
        assert_eq!(
            model.evaluate_dax("SLN(5000, 500, 9)", &ctx).unwrap().as_f64(),
            Some(500.0)
        );
    }
}
