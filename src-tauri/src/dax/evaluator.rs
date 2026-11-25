use crate::dax::ast::{BinaryOp, Expr, UnaryOp};
use crate::dax::functions::{
    add_days, add_months, calc_geomean, calc_median, calc_percentile, calc_stdev, calc_variance,
    civil_to_days, days_in_month, parse_flexible_ymd, parse_iso_ymd, try_eval_scalar_fn,
};
use crate::dax::parser::parse_dax;
use crate::dax::{DaxModel, DaxTableData, DaxValue, FilterContext, RelOverride};
use std::collections::{HashMap, HashSet};

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
                if let Some(cidx) = rc.table.get_col_idx(name) {
                    return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            // 2. Check measure
            if let Some(measure) = model.get_measure(name) {
                let effective_ctx = if let Some(rc) = row_stack.last() {
                    apply_context_transition(ctx, rc)
                } else {
                    ctx.clone()
                };
                let parsed = parse_dax(&measure.formula)?;
                return evaluate_ast(&parsed, model, &effective_ctx, &[], &HashMap::new());
            }
            // 3. Fallback search across row stack
            for rc in row_stack.iter().rev() {
                if let Some(cidx) = rc.table.get_col_idx(name) {
                    return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            Err(format!("Referência '[{}]' não encontrada.", name))
        }
        Expr::TableColRef { table, column } => {
            for rc in row_stack.iter().rev() {
                if rc.table.name.eq_ignore_ascii_case(table) {
                    if let Some(cidx) = rc.table.get_col_idx(column) {
                        return Ok(rc.table.rows[rc.row_idx][cidx].clone());
                    }
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

fn apply_context_transition(ctx: &FilterContext, rc: &RowContext<'_>) -> FilterContext {
    apply_context_transition_cols(ctx, rc, None)
}

/// Like `apply_context_transition`, but when `only_cols` is given (e.g. the iterator source
/// was `VALUES`/`DISTINCT`/`ALL` over a *single* column), only that column's value is used to
/// narrow the context — not every column of the underlying physical table. Without this,
/// transitioning on a representative row of `VALUES(d_Produto[Categoria])` would also pin
/// `d_Produto[ProdutoID]` to that one representative product, collapsing the whole category
/// down to a single row instead of letting the relationship propagate the category filter.
fn apply_context_transition_cols(
    ctx: &FilterContext,
    rc: &RowContext<'_>,
    only_cols: Option<&[String]>,
) -> FilterContext {
    let mut next = ctx.clone();
    match only_cols {
        Some(cols) => {
            for cname in cols {
                if let Some(idx) = rc.table.get_col_idx(cname) {
                    let mut set = HashSet::new();
                    set.insert(rc.table.rows[rc.row_idx][idx].canonical_key());
                    next.set_column_filter(&rc.table.name, cname, set);
                }
            }
        }
        None => {
            for (idx, cname) in rc.table.columns.iter().enumerate() {
                let mut set = HashSet::new();
                set.insert(rc.table.rows[rc.row_idx][idx].canonical_key());
                next.set_column_filter(&rc.table.name, cname, set);
            }
        }
    }
    next
}

/// If `expr` is `VALUES`/`DISTINCT`/`ALL` applied to a single `Table[Column]`, returns that
/// column name so context transition over the resulting rows narrows only that column.
fn single_column_transition_scope(expr: &Expr) -> Option<Vec<String>> {
    if let Expr::Call { name, args } = expr {
        let upper = name.to_uppercase();
        if matches!(upper.as_str(), "VALUES" | "DISTINCT" | "ALL" | "ALLSELECTED") {
            if let Some(Expr::TableColRef { column, .. }) = args.first() {
                return Some(vec![column.clone()]);
            }
        }
    }
    None
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
        "ISBLANK" => {
            let v = eval_arg(args, 0, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Boolean(v.is_blank()))
        }
        "NOT" => {
            let v = eval_arg(args, 0, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Boolean(!v.as_bool()))
        }
        "AND" => {
            let a = eval_arg(args, 0, model, ctx, row_stack, vars)?;
            let b = eval_arg(args, 1, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Boolean(a.as_bool() && b.as_bool()))
        }
        "OR" => {
            let a = eval_arg(args, 0, model, ctx, row_stack, vars)?;
            let b = eval_arg(args, 1, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Boolean(a.as_bool() || b.as_bool()))
        }
        "IF" | "IF.EAGER" => {
            let cond = eval_arg(args, 0, model, ctx, row_stack, vars)?;
            if cond.as_bool() {
                eval_arg(args, 1, model, ctx, row_stack, vars)
            } else if args.len() >= 3 {
                eval_arg(args, 2, model, ctx, row_stack, vars)
            } else {
                Ok(DaxValue::Blank)
            }
        }
        "SWITCH" => {
            if args.len() < 3 {
                return Ok(DaxValue::Blank);
            }
            let target = evaluate_ast(&args[0], model, ctx, row_stack, vars)?;
            let mut idx = 1;
            while idx + 1 < args.len() {
                let cand = evaluate_ast(&args[idx], model, ctx, row_stack, vars)?;
                if target.dax_equals(&cand) {
                    return evaluate_ast(&args[idx + 1], model, ctx, row_stack, vars);
                }
                idx += 2;
            }
            if idx < args.len() {
                evaluate_ast(&args[idx], model, ctx, row_stack, vars)
            } else {
                Ok(DaxValue::Blank)
            }
        }
        "DIVIDE" => {
            let num = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            let den = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            if den.abs() < 1e-12 {
                if args.len() >= 3 {
                    eval_arg(args, 2, model, ctx, row_stack, vars)
                } else {
                    Ok(DaxValue::Blank)
                }
            } else {
                Ok(DaxValue::Number(num / den))
            }
        }
        "COALESCE" => {
            for a in args {
                let v = evaluate_ast(a, model, ctx, row_stack, vars)?;
                if !v.is_blank() {
                    return Ok(v);
                }
            }
            Ok(DaxValue::Blank)
        }
        "IFERROR" => match eval_arg(args, 0, model, ctx, row_stack, vars) {
            Ok(v) if matches!(&v, DaxValue::Number(n) if !n.is_finite()) => {
                eval_arg(args, 1, model, ctx, row_stack, vars)
            }
            Ok(v) => Ok(v),
            Err(_) => eval_arg(args, 1, model, ctx, row_stack, vars),
        },
        "ISERROR" => match evaluate_ast(&args[0], model, ctx, row_stack, vars) {
            Ok(DaxValue::Number(n)) if !n.is_finite() => Ok(DaxValue::Boolean(true)),
            Ok(_) => Ok(DaxValue::Boolean(false)),
            Err(_) => Ok(DaxValue::Boolean(true)),
        },
        "ABS" => Ok(DaxValue::Number(
            eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0).abs(),
        )),
        "ROUND" => {
            let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            let digits = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0) as i32;
            let factor = 10f64.powi(digits);
            Ok(DaxValue::Number((n * factor).round() / factor))
        }
        "INT" => {
            let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            Ok(DaxValue::Number(n.floor()))
        }
        "SQRT" => {
            let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            if n < 0.0 {
                Err("SQRT: número negativo".to_string())
            } else {
                Ok(DaxValue::Number(n.sqrt()))
            }
        }
        "POWER" => {
            let base = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            let exp = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
            Ok(DaxValue::Number(base.powf(exp)))
        }
        "TOTALYTD" | "TOTALMTD" | "TOTALQTD" => {
            if args.len() < 2 {
                return Err(format!("{} requer (expressão, Tabela[Data])", fname));
            }
            let Some(Expr::TableColRef { table, column }) = args.get(1) else {
                return Err(format!("{} requer coluna de data no 2º argumento", fname));
            };
            let mut next_ctx = if let Some(rc) = row_stack.last() {
                apply_context_transition(ctx, rc)
            } else {
                ctx.clone()
            };
            let mode = match fname.as_str() {
                "TOTALMTD" => "MTD",
                "TOTALQTD" => "QTD",
                _ => "YTD",
            };
            let year_end = if fname == "TOTALYTD" {
                args.get(2)
                    .map(|e| evaluate_ast(e, model, ctx, row_stack, vars))
                    .transpose()?
                    .map(|v| parse_fiscal_year_end(&v.as_display_string()))
                    .unwrap_or((12, 31))
            } else {
                (12, 31)
            };
            apply_period_to_date(model, ctx, &mut next_ctx, table, column, mode, year_end);
            for filter_expr in &args[2..] {
                apply_calculate_modifier(filter_expr, model, ctx, &mut next_ctx, row_stack, vars)?;
            }
            evaluate_ast(&args[0], model, &next_ctx, &[], vars)
        }
        "OPENINGBALANCEMONTH" | "CLOSINGBALANCEMONTH"
        | "OPENINGBALANCEQUARTER" | "CLOSINGBALANCEQUARTER"
        | "OPENINGBALANCEYEAR" | "CLOSINGBALANCEYEAR" => {
            if args.len() < 2 {
                return Err(format!("{} requer (expressão, Tabela[Data])", fname));
            }
            let Some(Expr::TableColRef { table, column }) = args.get(1) else {
                return Err(format!("{} requer coluna de data no 2º argumento", fname));
            };
            let mut next_ctx = if let Some(rc) = row_stack.last() {
                apply_context_transition(ctx, rc)
            } else {
                ctx.clone()
            };
            let is_opening = fname.starts_with("OPENING");
            let scope = if fname.ends_with("MONTH") {
                "MONTH"
            } else if fname.ends_with("QUARTER") {
                "QUARTER"
            } else {
                "YEAR"
            };
            apply_period_boundary(model, ctx, &mut next_ctx, table, column, is_opening, scope);
            for filter_expr in &args[2..] {
                apply_calculate_modifier(filter_expr, model, ctx, &mut next_ctx, row_stack, vars)?;
            }
            evaluate_ast(&args[0], model, &next_ctx, &[], vars)
        }
        "FIRSTDATE" | "LASTDATE" | "STARTOFMONTH" | "ENDOFMONTH"
        | "STARTOFQUARTER" | "ENDOFQUARTER" | "STARTOFYEAR" | "ENDOFYEAR" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err(format!("{} requer referência Tabela[Data]", fname));
            };
            let bound_keys = resolve_date_boundary_keys(model, ctx, table, column, &fname);
            if let Some(first) = bound_keys.into_iter().min() {
                // Canonical key may be `t:YYYY-MM-DD`; strip prefix if present
                let clean = first.strip_prefix("t:").unwrap_or(&first).to_string();
                Ok(DaxValue::Text(clean))
            } else {
                Ok(DaxValue::Blank)
            }
        }
        "RANKX" => {
            if args.len() < 2 {
                return Err("RANKX requer (tabela, expressão)".to_string());
            }
            let (table_eval, row_indices, rank_ctx) = eval_table_expr_owned(&args[0], model, ctx, row_stack, vars)?;
            let table_data = table_eval.as_ref();
            // 3rd arg (value) is optional / can be an omitted placeholder (Expr::Blank);
            // when absent, the expression itself (2nd arg) supplies the value to rank.
            let value_expr = match args.get(2) {
                Some(Expr::Blank) | None => &args[1],
                Some(e) => e,
            };
            let target_val = evaluate_ast(value_expr, model, ctx, row_stack, vars)?;
            let Some(target_num) = (if target_val.is_blank() { None } else { target_val.as_f64() }) else {
                return Ok(DaxValue::Blank);
            };
            let ascending = match args.get(3) {
                Some(Expr::Ident(s)) => s.eq_ignore_ascii_case("ASC"),
                Some(Expr::Blank) | None => false,
                Some(ord_expr) => evaluate_ast(ord_expr, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0) == 1.0,
            };
            let dense = match args.get(4) {
                Some(Expr::Ident(s)) => s.eq_ignore_ascii_case("DENSE"),
                Some(Expr::Text(s)) => s.eq_ignore_ascii_case("DENSE"),
                _ => false,
            };
            let transition_scope = single_column_transition_scope(&args[0]);

            let mut scores = Vec::new();
            for r_idx in row_indices {
                let row_ctx = RowContext { table: table_data, row_idx: r_idx };
                let eff_ctx = apply_context_transition_cols(&rank_ctx, &row_ctx, transition_scope.as_deref());
                let mut next_stack = row_stack.to_vec();
                next_stack.push(row_ctx);
                let v = evaluate_ast(&args[1], model, &eff_ctx, &next_stack, vars)?;
                // BLANK is treated as 0 in RANKX comparisons (per dax.guide), not skipped.
                scores.push(if v.is_blank() { 0.0 } else { v.as_f64().unwrap_or(0.0) });
            }

            if dense {
                let mut better: Vec<f64> = scores
                    .iter()
                    .copied()
                    .filter(|&s| if ascending { s < target_num - 1e-9 } else { s > target_num + 1e-9 })
                    .collect();
                better.sort_by(|a, b| a.partial_cmp(b).unwrap());
                better.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                Ok(DaxValue::Number((better.len() + 1) as f64))
            } else {
                let better = scores
                    .iter()
                    .filter(|&&s| if ascending { s < target_num - 1e-9 } else { s > target_num + 1e-9 })
                    .count();
                Ok(DaxValue::Number((better + 1) as f64))
            }
        }
        "SUM" | "AVERAGE" | "AVERAGEA" | "MIN" | "MINA" | "MAX" | "MAXA"
        | "COUNT" | "COUNTA" | "COUNTBLANK" | "DISTINCTCOUNT" | "DISTINCTCOUNTNOBLANK"
        | "MEDIAN" | "STDEV.S" | "STDEV.P" | "VAR.S" | "VAR.P" | "PRODUCT" | "GEOMEAN" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err(format!("{} requer referência Tabela[Coluna]", fname));
            };
            eval_column_agg(&fname, model, ctx, table, column)
        }
        "PERCENTILE.INC" | "PERCENTILE.EXC" => {
            if args.len() < 2 {
                return Err(format!("{} requer (Tabela[Coluna], k)", fname));
            }
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err(format!("{} requer referência Tabela[Coluna]", fname));
            };
            let k = evaluate_ast(&args[1], model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.5);
            let tbl = model
                .get_table(table)
                .ok_or_else(|| format!("Tabela '{}' não encontrada", table))?;
            let cidx = tbl
                .get_col_idx(column)
                .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table, column))?;
            let vis = model.get_visible_row_indices(table, ctx);
            let nums: Vec<f64> = vis
                .iter()
                .filter_map(|&i| {
                    let v = &tbl.rows[i][cidx];
                    if v.is_blank() { None } else { v.as_f64() }
                })
                .collect();
            Ok(calc_percentile(&nums, k, fname == "PERCENTILE.INC")
                .map(DaxValue::Number)
                .unwrap_or(DaxValue::Blank))
        }
        "COUNTROWS" => {
            let Some(first) = args.first() else {
                return Err("COUNTROWS requer expressão de tabela".to_string());
            };
            let (_, indices, _) = eval_table_expr_owned(first, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Number(indices.len() as f64))
        }
        "ISEMPTY" => {
            let Some(first) = args.first() else {
                return Ok(DaxValue::Boolean(true));
            };
            let (_, indices, _) = eval_table_expr_owned(first, model, ctx, row_stack, vars)?;
            Ok(DaxValue::Boolean(indices.is_empty()))
        }
        "CONTAINSROW" => {
            if args.len() < 2 {
                return Err("CONTAINSROW requer (tabela, valor1, ...)".to_string());
            }
            let (table_eval, row_indices, _) = eval_table_expr_owned(&args[0], model, ctx, row_stack, vars)?;
            let table_data = table_eval.as_ref();
            let mut search_keys = Vec::with_capacity(args.len() - 1);
            for a in &args[1..] {
                search_keys.push(evaluate_ast(a, model, ctx, row_stack, vars)?.canonical_key());
            }
            let found = row_indices.iter().any(|&r_idx| {
                let row = &table_data.rows[r_idx];
                if search_keys.len() == 1 {
                    row.first().is_some_and(|cell| cell.canonical_key() == search_keys[0])
                } else {
                    search_keys.len() <= row.len()
                        && search_keys.iter().enumerate().all(|(ci, k)| row[ci].canonical_key() == *k)
                }
            });
            Ok(DaxValue::Boolean(found))
        }
        "SUMX" | "AVERAGEX" | "MINX" | "MAXX" | "COUNTX" | "COUNTAX"
        | "MEDIANX" | "STDEVX.S" | "STDEVX.P" | "VARX.S" | "VARX.P"
        | "PRODUCTX" | "GEOMEANX" => {
            if args.len() < 2 {
                return Err(format!("{} requer (tabela, expressão)", fname));
            }
            let (table_eval, row_indices, iter_ctx) = eval_table_expr_owned(&args[0], model, ctx, row_stack, vars)?;
            let table_data = table_eval.as_ref();
            let transition_scope = single_column_transition_scope(&args[0]);
            if fname == "COUNTAX" {
                let mut cnt = 0usize;
                for r_idx in row_indices {
                    let row_ctx = RowContext { table: table_data, row_idx: r_idx };
                    let eff_ctx = apply_context_transition_cols(&iter_ctx, &row_ctx, transition_scope.as_deref());
                    let mut next_stack = row_stack.to_vec();
                    next_stack.push(row_ctx);
                    let v = evaluate_ast(&args[1], model, &eff_ctx, &next_stack, vars)?;
                    if !v.is_blank() {
                        cnt += 1;
                    }
                }
                return Ok(DaxValue::Number(cnt as f64));
            }
            let mut vals = Vec::new();
            for r_idx in row_indices {
                let row_ctx = RowContext { table: table_data, row_idx: r_idx };
                let eff_ctx = apply_context_transition_cols(&iter_ctx, &row_ctx, transition_scope.as_deref());
                let mut next_stack = row_stack.to_vec();
                next_stack.push(row_ctx);
                let v = evaluate_ast(&args[1], model, &eff_ctx, &next_stack, vars)?;
                if !v.is_blank() {
                    if let Some(n) = v.as_f64() {
                        vals.push(n);
                    }
                }
            }
            if vals.is_empty() {
                return Ok(DaxValue::Blank);
            }
            let res = match fname.as_str() {
                "SUMX" => Some(vals.iter().sum()),
                "AVERAGEX" => Some(vals.iter().sum::<f64>() / vals.len() as f64),
                "MINX" => Some(vals.iter().copied().fold(f64::INFINITY, f64::min)),
                "MAXX" => Some(vals.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
                "COUNTX" => Some(vals.len() as f64),
                "MEDIANX" => calc_median(&vals),
                "STDEVX.S" => calc_stdev(&vals, true),
                "STDEVX.P" => calc_stdev(&vals, false),
                "VARX.S" => calc_variance(&vals, true),
                "VARX.P" => calc_variance(&vals, false),
                "PRODUCTX" => Some(vals.iter().product()),
                "GEOMEANX" => calc_geomean(&vals),
                _ => None,
            };
            Ok(res.map(DaxValue::Number).unwrap_or(DaxValue::Blank))
        }
        "PERCENTILEX.INC" | "PERCENTILEX.EXC" => {
            if args.len() < 3 {
                return Err(format!("{} requer (tabela, expressão, k)", fname));
            }
            let (table_eval, row_indices, iter_ctx) = eval_table_expr_owned(&args[0], model, ctx, row_stack, vars)?;
            let table_data = table_eval.as_ref();
            let k = evaluate_ast(&args[2], model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.5);
            let mut vals = Vec::new();
            for r_idx in row_indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: table_data, row_idx: r_idx });
                let v = evaluate_ast(&args[1], model, &iter_ctx, &next_stack, vars)?;
                if !v.is_blank() {
                    if let Some(n) = v.as_f64() {
                        vals.push(n);
                    }
                }
            }
            Ok(calc_percentile(&vals, k, fname == "PERCENTILEX.INC")
                .map(DaxValue::Number)
                .unwrap_or(DaxValue::Blank))
        }
        "CONCATENATEX" => {
            if args.len() < 2 {
                return Err("CONCATENATEX requer (tabela, expressão, [delimitador])".to_string());
            }
            let (table_eval, row_indices, iter_ctx) = eval_table_expr_owned(&args[0], model, ctx, row_stack, vars)?;
            let table_data = table_eval.as_ref();
            let delim = if args.len() >= 3 {
                evaluate_ast(&args[2], model, ctx, row_stack, vars)?.as_display_string()
            } else {
                String::new()
            };
            let sort_asc = args.get(4).is_some_and(|e| matches!(e, Expr::Ident(s) if s.eq_ignore_ascii_case("ASC")));
            let mut items: Vec<(String, String)> = Vec::new();
            for r_idx in row_indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: table_data, row_idx: r_idx });
                let v = evaluate_ast(&args[1], model, &iter_ctx, &next_stack, vars)?;
                if !v.is_blank() {
                    let text = v.as_display_string();
                    let sort_k = if let Some(ord_e) = args.get(3) {
                        evaluate_ast(ord_e, model, &iter_ctx, &next_stack, vars)?.as_display_string()
                    } else {
                        String::new()
                    };
                    items.push((text, sort_k));
                }
            }
            if args.len() >= 4 {
                items.sort_by(|a, b| {
                    let cmp = a.1.cmp(&b.1);
                    if sort_asc || args.len() < 5 { cmp } else { cmp.reverse() }
                });
            }
            let joined: Vec<String> = items.into_iter().map(|(t, _)| t).collect();
            Ok(DaxValue::Text(joined.join(&delim)))
        }
        "LOOKUPVALUE" => {
            if args.len() < 3 || (args.len() % 2 == 0) {
                // Can be (result, search_col, search_val) [odd len] or with alternateValue at end? Wait:
                // 3 args: res, c1, v1 (odd)
                // 4 args: res, c1, v1, alt (even!)
                // 5 args: res, c1, v1, c2, v2 (odd)
                // 6 args: res, c1, v1, c2, v2, alt (even!)
            }
            eval_lookupvalue(args, model, ctx, row_stack, vars)
        }
        "CALCULATE" => {
            if args.is_empty() {
                return Err("CALCULATE requer ao menos 1 argumento".to_string());
            }
            let mut next_ctx = if let Some(rc) = row_stack.last() {
                apply_context_transition(ctx, rc)
            } else {
                ctx.clone()
            };
            for filter_expr in &args[1..] {
                apply_calculate_modifier(filter_expr, model, ctx, &mut next_ctx, row_stack, vars)?;
            }
            evaluate_ast(&args[0], model, &next_ctx, &[], vars)
        }
        "SELECTEDVALUE" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err("SELECTEDVALUE requer Tabela[Coluna]".to_string());
            };
            let alt = if args.len() >= 2 {
                evaluate_ast(&args[1], model, ctx, row_stack, vars)?
            } else {
                DaxValue::Blank
            };
            eval_selected_value(model, ctx, table, column, alt)
        }
        "HASONEVALUE" | "HASONEFILTER" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Ok(DaxValue::Boolean(false));
            };
            if fname == "HASONEFILTER" {
                let direct_count = ctx
                    .filters
                    .get(&table.to_lowercase())
                    .and_then(|cols| cols.get(&column.to_lowercase()))
                    .map(|s| s.len())
                    .unwrap_or(0);
                Ok(DaxValue::Boolean(direct_count == 1))
            } else {
                let count = count_distinct_visible(model, ctx, table, column);
                Ok(DaxValue::Boolean(count == 1))
            }
        }
        "ISFILTERED" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Ok(DaxValue::Boolean(false));
            };
            let is_direct = ctx
                .filters
                .get(&table.to_lowercase())
                .and_then(|cols| cols.get(&column.to_lowercase()))
                .is_some_and(|s| !s.is_empty());
            Ok(DaxValue::Boolean(is_direct))
        }
        "ISCROSSFILTERED" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Ok(DaxValue::Boolean(false));
            };
            if let Some(tbl) = model.get_table(table) {
                let vis_count = model.get_visible_row_indices(table, ctx).len();
                let direct = ctx.filters.get(&tbl.name.to_lowercase()).is_some_and(|c| !c.is_empty());
                let is_filtered = direct || vis_count < tbl.rows.len();
                let _ = column;
                Ok(DaxValue::Boolean(is_filtered))
            } else {
                Ok(DaxValue::Boolean(false))
            }
        }
        "ISINSCOPE" | "ISATLEVEL" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Ok(DaxValue::Boolean(false));
            };
            let count = count_distinct_visible(model, ctx, table, column);
            let direct = ctx
                .filters
                .get(&table.to_lowercase())
                .and_then(|cols| cols.get(&column.to_lowercase()))
                .is_some();
            Ok(DaxValue::Boolean(direct || count == 1))
        }
        "ISSUBTOTAL" => Ok(DaxValue::Boolean(false)),
        "FIRSTNONBLANK" | "LASTNONBLANK" => {
            if args.len() < 2 {
                return Err(format!("{} requer (Tabela[Coluna], expressão)", fname));
            }
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err(format!("{} requer Tabela[Coluna]", fname));
            };
            let Some(tbl) = model.get_table(table) else {
                return Ok(DaxValue::Blank);
            };
            let Some(cidx) = tbl.get_col_idx(column) else {
                return Ok(DaxValue::Blank);
            };
            let vis = model.get_visible_row_indices(table, ctx);
            let mut distinct_vals: Vec<(String, DaxValue)> = Vec::new();
            let mut seen = HashSet::new();
            for r_idx in vis {
                let v = &tbl.rows[r_idx][cidx];
                if !v.is_blank() && seen.insert(v.canonical_key()) {
                    distinct_vals.push((v.canonical_key(), v.clone()));
                }
            }
            distinct_vals.sort_by(|a, b| a.1.as_display_string().cmp(&b.1.as_display_string()));
            if fname == "LASTNONBLANK" {
                distinct_vals.reverse();
            }
            for (k, v) in distinct_vals {
                let mut test_ctx = ctx.clone();
                let mut set = HashSet::new();
                set.insert(k);
                test_ctx.set_column_filter(&tbl.name, column, set);
                let res = evaluate_ast(&args[1], model, &test_ctx, row_stack, vars)?;
                if !res.is_blank() {
                    return Ok(v);
                }
            }
            Ok(DaxValue::Blank)
        }
        "RELATED" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err("RELATED requer Tabela[Coluna]".to_string());
            };
            if let Some(rc) = row_stack.last() {
                Ok(resolve_related(model, rc, table, column).unwrap_or(DaxValue::Blank))
            } else {
                Ok(DaxValue::Blank)
            }
        }
        "UPPER" => Ok(DaxValue::Text(
            eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().to_uppercase(),
        )),
        "LOWER" => Ok(DaxValue::Text(
            eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().to_lowercase(),
        )),
        "LEN" => Ok(DaxValue::Number(
            eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().chars().count() as f64,
        )),
        "CONCATENATE" => {
            let a = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
            let b = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_display_string();
            Ok(DaxValue::Text(format!("{}{}", a, b)))
        }
        "LEFT" => {
            let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
            let n = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(1.0).max(0.0) as usize;
            Ok(DaxValue::Text(s.chars().take(n).collect()))
        }
        "RIGHT" => {
            let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
            let n = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(1.0).max(0.0) as usize;
            let chars: Vec<char> = s.chars().collect();
            let start = chars.len().saturating_sub(n);
            Ok(DaxValue::Text(chars[start..].iter().collect()))
        }
        "YEAR" | "MONTH" | "DAY" => {
            let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
            if let Some((y, m, d)) = parse_iso_ymd(&s) {
                let val = match fname.as_str() {
                    "YEAR" => y as f64,
                    "MONTH" => m as f64,
                    _ => d as f64,
                };
                Ok(DaxValue::Number(val))
            } else {
                Ok(DaxValue::Blank)
            }
        }
        _ => {
            let mut eval_args = Vec::with_capacity(args.len());
            for a in args {
                eval_args.push(evaluate_ast(a, model, ctx, row_stack, vars)?);
            }
            if let Some(scalar) = try_eval_scalar_fn(&fname, &eval_args)? {
                Ok(scalar)
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

fn extract_table_name(expr: Option<&Expr>) -> Result<String, String> {
    match expr {
        Some(Expr::Ident(t)) => Ok(t.clone()),
        _ => Err("Esperado nome de tabela".to_string()),
    }
}

enum TableEval<'a> {
    Borrowed(&'a DaxTableData),
    Owned(DaxTableData),
}

impl<'a> TableEval<'a> {
    fn as_ref(&self) -> &DaxTableData {
        match self {
            TableEval::Borrowed(t) => t,
            TableEval::Owned(t) => t,
        }
    }
}

fn eval_table_expr_owned<'a>(
    expr: &Expr,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if let Expr::Call { name, args } = expr {
        let upper = name.to_uppercase();
        if matches!(upper.as_str(), "VALUES" | "DISTINCT" | "ALL" | "ALLSELECTED") {
            if let Some(Expr::TableColRef { table, column }) = args.first() {
                // VALUES/DISTINCT/ALL over a *single* column conceptually returns a
                // one-column table. Materialize it as such (instead of returning the
                // underlying physical table + representative row indices) so that any
                // later context transition over its rows only narrows that one column,
                // letting relationships propagate the rest instead of pinning every
                // other column of an arbitrary representative row.
                let tbl = model
                    .get_table(table)
                    .ok_or_else(|| format!("Tabela '{}' não encontrada", table))?;
                let cidx = tbl
                    .get_col_idx(column)
                    .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table, column))?;
                let (source_indices, base_ctx): (Vec<usize>, FilterContext) =
                    if upper == "VALUES" || upper == "DISTINCT" {
                        (model.get_visible_row_indices(table, ctx), ctx.clone())
                    } else {
                        let mut cleared = ctx.clone();
                        cleared.clear_column(&tbl.name, column);
                        ((0..tbl.rows.len()).collect(), cleared)
                    };
                let mut seen = HashSet::new();
                let mut values = Vec::new();
                for r_idx in source_indices {
                    let v = tbl.rows[r_idx][cidx].clone();
                    if seen.insert(v.canonical_key()) {
                        values.push(v);
                    }
                }
                let mut col_index = HashMap::new();
                col_index.insert(column.to_lowercase(), 0);
                let rows: Vec<Vec<DaxValue>> = values.into_iter().map(|v| vec![v]).collect();
                let n = rows.len();
                let synth = DaxTableData {
                    name: tbl.name.clone(),
                    columns: vec![column.clone()],
                    col_index,
                    rows,
                };
                return Ok((TableEval::Owned(synth), (0..n).collect(), base_ctx));
            }
        }
        if name.eq_ignore_ascii_case("GENERATESERIES") {
            let start = args
                .first()
                .map(|a| evaluate_ast(a, model, ctx, row_stack, vars))
                .transpose()?
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let end = args
                .get(1)
                .map(|a| evaluate_ast(a, model, ctx, row_stack, vars))
                .transpose()?
                .and_then(|v| v.as_f64())
                .unwrap_or(start);
            let step = args
                .get(2)
                .map(|a| evaluate_ast(a, model, ctx, row_stack, vars))
                .transpose()?
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            let mut rows = Vec::new();
            if step.abs() > 1e-12 {
                let mut cur = start;
                let max_iters = 100_000usize;
                for _ in 0..max_iters {
                    if (step > 0.0 && cur > end + 1e-9) || (step < 0.0 && cur < end - 1e-9) {
                        break;
                    }
                    rows.push(vec![DaxValue::Number(cur)]);
                    cur += step;
                }
            }
            let mut col_index = HashMap::new();
            col_index.insert("value".to_string(), 0);
            let tbl = DaxTableData {
                name: "GENERATESERIES".to_string(),
                columns: vec!["Value".to_string()],
                col_index,
                rows,
            };
            let indices: Vec<usize> = (0..tbl.rows.len()).collect();
            return Ok((TableEval::Owned(tbl), indices, ctx.clone()));
        }
    }
    let (tbl, indices, base_ctx) = eval_table_expr(expr, model, ctx, row_stack, vars)?;
    Ok((TableEval::Borrowed(tbl), indices, base_ctx))
}

fn eval_table_expr<'a>(
    expr: &Expr,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(&'a DaxTableData, Vec<usize>, FilterContext), String> {
    match expr {
        Expr::Ident(tname) => {
            let tbl = model
                .get_table(tname)
                .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))?;
            let indices = model.get_visible_row_indices(tname, ctx);
            Ok((tbl, indices, ctx.clone()))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("ALL") || name.eq_ignore_ascii_case("ALLSELECTED") => {
            match args.first() {
                Some(Expr::TableColRef { table, column }) => {
                    let tbl = model
                        .get_table(table)
                        .ok_or_else(|| format!("Tabela '{}' não encontrada", table))?;
                    let cidx = tbl
                        .get_col_idx(column)
                        .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table, column))?;
                    let mut cleared_ctx = ctx.clone();
                    cleared_ctx.clear_column(&tbl.name, column);
                    let mut seen = HashSet::new();
                    let mut rep_indices = Vec::new();
                    for (r_idx, row) in tbl.rows.iter().enumerate() {
                        let k = row[cidx].canonical_key();
                        if seen.insert(k) {
                            rep_indices.push(r_idx);
                        }
                    }
                    Ok((tbl, rep_indices, cleared_ctx))
                }
                _ => {
                    let tname = extract_table_name(args.first())?;
                    let tbl = model
                        .get_table(&tname)
                        .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))?;
                    let mut cleared_ctx = ctx.clone();
                    cleared_ctx.clear_table(&tbl.name);
                    for rel in &model.relationships {
                        if rel.is_active && rel.from_table.eq_ignore_ascii_case(&tbl.name) {
                            cleared_ctx.clear_table(&rel.to_table);
                        }
                    }
                    Ok((tbl, (0..tbl.rows.len()).collect(), cleared_ctx))
                }
            }
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("VALUES") || name.eq_ignore_ascii_case("DISTINCT") => {
            match args.first() {
                Some(Expr::TableColRef { table, column }) => {
                    let tbl = model
                        .get_table(table)
                        .ok_or_else(|| format!("Tabela '{}' não encontrada", table))?;
                    let cidx = tbl
                        .get_col_idx(column)
                        .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table, column))?;
                    let visible = model.get_visible_row_indices(table, ctx);
                    let mut seen = HashSet::new();
                    let mut rep_indices = Vec::new();
                    for r_idx in visible {
                        let k = tbl.rows[r_idx][cidx].canonical_key();
                        if seen.insert(k) {
                            rep_indices.push(r_idx);
                        }
                    }
                    Ok((tbl, rep_indices, ctx.clone()))
                }
                _ => {
                    let tname = extract_table_name(args.first())?;
                    let tbl = model
                        .get_table(&tname)
                        .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))?;
                    let indices = model.get_visible_row_indices(&tname, ctx);
                    Ok((tbl, indices, ctx.clone()))
                }
            }
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("TOPN") => {
            if args.len() < 2 {
                return Err("TOPN requer (N, tabela, [ordem])".to_string());
            }
            let top_n = evaluate_ast(&args[0], model, ctx, row_stack, vars)?
                .as_f64()
                .unwrap_or(0.0)
                .max(0.0) as usize;
            let (tbl, indices, base_ctx) = eval_table_expr(&args[1], model, ctx, row_stack, vars)?;
            if top_n == 0 {
                return Ok((tbl, Vec::new(), base_ctx));
            }
            let ascending = args.get(3).is_some_and(|e| matches!(e, Expr::Ident(s) if s.eq_ignore_ascii_case("ASC")));
            if let Some(ord_expr) = args.get(2) {
                let mut scored: Vec<(usize, f64)> = Vec::with_capacity(indices.len());
                for r_idx in indices {
                    let mut next_stack = row_stack.to_vec();
                    next_stack.push(RowContext { table: tbl, row_idx: r_idx });
                    let val = evaluate_ast(ord_expr, model, &base_ctx, &next_stack, vars)?
                        .as_f64()
                        .unwrap_or(f64::NEG_INFINITY);
                    scored.push((r_idx, val));
                }
                scored.sort_by(|a, b| {
                    let ord = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                    if ascending { ord } else { ord.reverse() }
                });
                let kept = scored.into_iter().take(top_n).map(|(i, _)| i).collect();
                Ok((tbl, kept, base_ctx))
            } else {
                Ok((tbl, indices.into_iter().take(top_n).collect(), base_ctx))
            }
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("FILTER") => {
            if args.len() < 2 {
                return Err("FILTER requer (tabela, condição)".to_string());
            }
            let (tbl, indices, base_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let mut kept = Vec::new();
            for r_idx in indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: tbl, row_idx: r_idx });
                let cond = evaluate_ast(&args[1], model, &base_ctx, &next_stack, vars)?;
                if cond.as_bool() {
                    kept.push(r_idx);
                }
            }
            Ok((tbl, kept, base_ctx))
        }
        _ => Err("Expressão de tabela não suportada neste iterador".to_string()),
    }
}

fn apply_calculate_modifier(
    expr: &Expr,
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(), String> {
    match expr {
        Expr::Call { name, args } => {
            let upper = name.trim().to_uppercase();
            match upper.as_str() {
                "REMOVEFILTERS" | "ALL" | "ALLSELECTED" => {
                    if args.is_empty() {
                        next_ctx.filters.clear();
                    } else {
                        for arg in args {
                            match arg {
                                Expr::Ident(t) => next_ctx.clear_table(t),
                                Expr::TableColRef { table, column } => next_ctx.clear_column(table, column),
                                _ => {}
                            }
                        }
                    }
                    return Ok(());
                }
                "USERELATIONSHIP" => {
                    if let (
                        Some(Expr::TableColRef { table: t1, column: c1 }),
                        Some(Expr::TableColRef { table: t2, column: c2 }),
                    ) = (args.first(), args.get(1))
                    {
                        next_ctx.rel_overrides.push(RelOverride {
                            from_table: t1.clone(),
                            from_column: c1.clone(),
                            to_table: t2.clone(),
                            to_column: c2.clone(),
                            activate: Some(true),
                            direction: None,
                        });
                    }
                    return Ok(());
                }
                "CROSSFILTER" => {
                    if let (
                        Some(Expr::TableColRef { table: t1, column: c1 }),
                        Some(Expr::TableColRef { table: t2, column: c2 }),
                        Some(dir_arg),
                    ) = (args.first(), args.get(1), args.get(2))
                    {
                        let raw_dir = match dir_arg {
                            Expr::Ident(s) | Expr::Text(s) => s.to_uppercase(),
                            _ => "BOTH".to_string(),
                        };
                        let mapped = match raw_dir.as_str() {
                            "NONE" => "none",
                            "ONEWAY" | "ONEWAY_LEFTFILTERSRIGHT" | "ONEWAY_RIGHTFILTERSLEFT" | "SINGLE" => "single",
                            _ => "both",
                        };
                        next_ctx.rel_overrides.push(RelOverride {
                            from_table: t1.clone(),
                            from_column: c1.clone(),
                            to_table: t2.clone(),
                            to_column: c2.clone(),
                            activate: None,
                            direction: Some(mapped.to_string()),
                        });
                    }
                    return Ok(());
                }
                "ALLEXCEPT" => {
                    if let Some(Expr::Ident(t)) = args.first() {
                        let keep: Vec<String> = args[1..]
                            .iter()
                            .filter_map(|a| match a {
                                Expr::TableColRef { column, .. } => Some(column.clone()),
                                _ => None,
                            })
                            .collect();
                        next_ctx.clear_except(t, &keep);
                    }
                    return Ok(());
                }
                "KEEPFILTERS" => {
                    if let Some(inner) = args.first() {
                        return apply_calculate_modifier(inner, model, outer_ctx, next_ctx, row_stack, vars);
                    }
                    return Ok(());
                }
                "DATEADD" => {
                    if args.len() >= 3 {
                        if let Expr::TableColRef { table, column } = &args[0] {
                            let shift = evaluate_ast(&args[1], model, outer_ctx, row_stack, vars)?
                                .as_f64()
                                .unwrap_or(0.0) as i32;
                            let interval = match &args[2] {
                                Expr::Ident(s) => s.to_uppercase(),
                                _ => "DAY".to_string(),
                            };
                            apply_date_shift(model, outer_ctx, next_ctx, table, column, shift, &interval);
                        }
                    }
                    return Ok(());
                }
                "PREVIOUSDAY" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_date_shift(model, outer_ctx, next_ctx, table, column, -1, "DAY");
                    }
                    return Ok(());
                }
                "NEXTDAY" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_date_shift(model, outer_ctx, next_ctx, table, column, 1, "DAY");
                    }
                    return Ok(());
                }
                "PREVIOUSMONTH" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, -1, "MONTH");
                    }
                    return Ok(());
                }
                "NEXTMONTH" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, 1, "MONTH");
                    }
                    return Ok(());
                }
                "PREVIOUSQUARTER" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, -1, "QUARTER");
                    }
                    return Ok(());
                }
                "NEXTQUARTER" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, 1, "QUARTER");
                    }
                    return Ok(());
                }
                "PREVIOUSYEAR" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, -1, "YEAR");
                    }
                    return Ok(());
                }
                "NEXTYEAR" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, 1, "YEAR");
                    }
                    return Ok(());
                }
                "SAMEPERIODLASTYEAR" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_date_shift(model, outer_ctx, next_ctx, table, column, -1, "YEAR");
                    }
                    return Ok(());
                }
                "PARALLELPERIOD" => {
                    if args.len() >= 3 {
                        if let Expr::TableColRef { table, column } = &args[0] {
                            let shift = evaluate_ast(&args[1], model, outer_ctx, row_stack, vars)?
                                .as_f64()
                                .unwrap_or(0.0) as i32;
                            let interval = match &args[2] {
                                Expr::Ident(s) => s.to_uppercase(),
                                _ => "MONTH".to_string(),
                            };
                            apply_complete_period_shift(model, outer_ctx, next_ctx, table, column, shift, &interval);
                        }
                    }
                    return Ok(());
                }
                "DATESYTD" | "DATESMTD" | "DATESQTD" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        let mode = match upper.as_str() {
                            "DATESMTD" => "MTD",
                            "DATESQTD" => "QTD",
                            _ => "YTD",
                        };
                        let year_end = if upper == "DATESYTD" {
                            args.get(1)
                                .map(|e| evaluate_ast(e, model, outer_ctx, row_stack, vars))
                                .transpose()?
                                .map(|v| parse_fiscal_year_end(&v.as_display_string()))
                                .unwrap_or((12, 31))
                        } else {
                            (12, 31)
                        };
                        apply_period_to_date(model, outer_ctx, next_ctx, table, column, mode, year_end);
                    }
                    return Ok(());
                }
                "DATESBETWEEN" => {
                    if args.len() >= 3 {
                        if let Expr::TableColRef { table, column } = &args[0] {
                            let start_s = evaluate_ast(&args[1], model, outer_ctx, row_stack, vars)?.as_display_string();
                            let end_s = evaluate_ast(&args[2], model, outer_ctx, row_stack, vars)?.as_display_string();
                            apply_dates_between(model, next_ctx, table, column, &start_s, &end_s);
                        }
                    }
                    return Ok(());
                }
                "DATESINPERIOD" => {
                    if args.len() >= 4 {
                        if let Expr::TableColRef { table, column } = &args[0] {
                            let start_s = evaluate_ast(&args[1], model, outer_ctx, row_stack, vars)?.as_display_string();
                            let count = evaluate_ast(&args[2], model, outer_ctx, row_stack, vars)?.as_f64().unwrap_or(0.0) as i32;
                            let interval = match &args[3] {
                                Expr::Ident(s) => s.to_uppercase(),
                                _ => "DAY".to_string(),
                            };
                            apply_dates_in_period(model, next_ctx, table, column, &start_s, count, &interval);
                        }
                    }
                    return Ok(());
                }
                "FIRSTDATE" | "LASTDATE" | "STARTOFMONTH" | "ENDOFMONTH"
                | "STARTOFQUARTER" | "ENDOFQUARTER" | "STARTOFYEAR" | "ENDOFYEAR" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        let allowed = resolve_date_boundary_keys(model, outer_ctx, table, column, &upper);
                        if let Some(tbl) = model.get_table(table) {
                            next_ctx.clear_table(&tbl.name);
                            next_ctx.set_column_filter(&tbl.name, column, allowed);
                        }
                    }
                    return Ok(());
                }
                "TREATAS" => {
                    if args.len() >= 2 {
                        let (src_tbl, src_indices, _) =
                            eval_table_expr_owned(&args[0], model, outer_ctx, row_stack, vars)?;
                        let src_data = src_tbl.as_ref();
                        for (col_pos, col_expr) in args[1..].iter().enumerate() {
                            if let Expr::TableColRef { table, column } = col_expr {
                                if let Some(target_tbl) = model.get_table(table) {
                                    let mut allowed = HashSet::new();
                                    for &r_idx in &src_indices {
                                        if let Some(cell) = src_data.rows[r_idx].get(col_pos) {
                                            allowed.insert(cell.canonical_key());
                                        }
                                    }
                                    next_ctx.set_column_filter(&target_tbl.name, column, allowed);
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                "FILTER" => {
                    if args.len() >= 2 {
                        let (tbl, kept_indices, _) = eval_table_expr(expr, model, outer_ctx, row_stack, vars)?;
                        if let Some(first_col) = tbl.columns.first() {
                            let allowed: HashSet<String> = kept_indices
                                .iter()
                                .map(|&i| tbl.rows[i][0].canonical_key())
                                .collect();
                            next_ctx.set_column_filter(&tbl.name, first_col, allowed);
                        }
                    }
                    return Ok(());
                }
                "FIRSTNONBLANK" | "LASTNONBLANK" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        let picked = eval_call(&upper, args, model, outer_ctx, row_stack, vars)?;
                        if let Some(tbl) = model.get_table(table) {
                            let mut allowed = HashSet::new();
                            if !picked.is_blank() {
                                allowed.insert(picked.canonical_key());
                            }
                            next_ctx.clear_table(&tbl.name);
                            next_ctx.set_column_filter(&tbl.name, column, allowed);
                        }
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
        // Handle Boolean column predicate: e.g., dCategorias[DRE] == "Sim" or dCategorias[Saída de Caixa]
        _ => {}
    }

    // Collect referenced table from predicate
    if let Some((tbl_name, col_name)) = find_primary_table_col(expr) {
        if let Some(tbl) = model.get_table(&tbl_name) {
            if let Some(cidx) = tbl.get_col_idx(&col_name) {
                let mut allowed = HashSet::new();
                for r_idx in 0..tbl.rows.len() {
                    let rc = RowContext { table: tbl, row_idx: r_idx };
                    let v = evaluate_ast(expr, model, outer_ctx, &[rc], vars)?;
                    if v.as_bool() {
                        allowed.insert(tbl.rows[r_idx][cidx].canonical_key());
                    }
                }
                next_ctx.set_column_filter(&tbl.name, &col_name, allowed);
            }
        }
    }

    Ok(())
}

fn find_primary_table_col(expr: &Expr) -> Option<(String, String)> {
    match expr {
        Expr::TableColRef { table, column } => Some((table.clone(), column.clone())),
        Expr::Binary { left, right, .. } => {
            find_primary_table_col(left).or_else(|| find_primary_table_col(right))
        }
        Expr::InList { expr, .. } => find_primary_table_col(expr),
        Expr::Unary { expr, .. } => find_primary_table_col(expr),
        _ => None,
    }
}

fn collect_table_dates(tbl: &DaxTableData, cidx: usize) -> HashMap<(i32, u32, u32), Vec<String>> {
    let mut map: HashMap<(i32, u32, u32), Vec<String>> = HashMap::new();
    for row in &tbl.rows {
        let s = row[cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            map.entry(ymd).or_default().push(row[cidx].canonical_key());
        }
    }
    map
}

fn apply_date_shift(
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    shift: i32,
    interval: &str,
) {
    let Some(tbl) = model.get_table(table_name) else { return };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return };
    let visible = model.get_visible_row_indices(table_name, outer_ctx);
    let all_dates_map = collect_table_dates(tbl, cidx);

    let mut visible_ymds: Vec<(i32, u32, u32)> = Vec::new();
    for r_idx in visible {
        let s = tbl.rows[r_idx][cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            visible_ymds.push(ymd);
        }
    }

    let mut shifted_keys = HashSet::new();
    let push_ymd_keys = |ymd: (i32, u32, u32), out: &mut HashSet<String>| {
        if let Some(keys) = all_dates_map.get(&ymd) {
            for k in keys {
                out.insert(k.clone());
            }
        }
    };

    if matches!(interval, "YEAR" | "MONTH" | "QUARTER") {
        let delta_months = match interval {
            "YEAR" => shift * 12,
            "QUARTER" => shift * 3,
            _ => shift,
        };

        let mut by_month: HashMap<(i32, u32), HashSet<u32>> = HashMap::new();
        for &(y, m, d) in &visible_ymds {
            by_month.entry((y, m)).or_default().insert(d);
        }

        for ((y, m), days_set) in by_month {
            let dim_src = days_in_month(y, m);
            let is_full_month = days_set.len() as u32 == dim_src
                || (days_set.contains(&1) && days_set.contains(&dim_src));

            let (ny, nm, _) = add_months(y, m, 1, delta_months);
            let dim_dst = days_in_month(ny, nm);

            if is_full_month {
                for d_dst in 1..=dim_dst {
                    push_ymd_keys((ny, nm, d_dst), &mut shifted_keys);
                }
            } else {
                for d in days_set {
                    let (_, _, nd) = add_months(y, m, d, delta_months);
                    push_ymd_keys((ny, nm, nd), &mut shifted_keys);
                }
            }
        }
    } else {
        for (y, m, d) in visible_ymds {
            let shifted = add_days(y, m, d, shift);
            push_ymd_keys(shifted, &mut shifted_keys);
        }
    }

    // In DAX, time-intelligence functions replace any active column filters on the date table
    next_ctx.clear_table(&tbl.name);
    next_ctx.set_column_filter(&tbl.name, col_name, shifted_keys);
}

fn apply_complete_period_shift(
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    shift: i32,
    interval: &str,
) {
    let Some(tbl) = model.get_table(table_name) else { return };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return };
    let visible = model.get_visible_row_indices(table_name, outer_ctx);

    let mut anchor_ymd: Option<(i32, u32, u32)> = None;
    for r_idx in visible {
        let s = tbl.rows[r_idx][cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            anchor_ymd = match anchor_ymd {
                None => Some(ymd),
                Some(cur) => {
                    if shift < 0 {
                        Some(if ymd < cur { ymd } else { cur })
                    } else {
                        Some(if ymd > cur { ymd } else { cur })
                    }
                }
            };
        }
    }

    let mut allowed = HashSet::new();
    if let Some((y, m, _d)) = anchor_ymd {
        let delta_months = match interval {
            "YEAR" => shift * 12,
            "QUARTER" => shift * 3,
            _ => shift,
        };
        let (ty, tm, _) = add_months(y, m, 1, delta_months);
        let target_quarter = (tm - 1) / 3;

        for row in &tbl.rows {
            let s = row[cidx].as_display_string();
            if let Some((ry, rm, _)) = parse_iso_ymd(&s) {
                let in_target = match interval {
                    "YEAR" => ry == ty,
                    "QUARTER" => ry == ty && ((rm - 1) / 3) == target_quarter,
                    _ => ry == ty && rm == tm,
                };
                if in_target {
                    allowed.insert(row[cidx].canonical_key());
                }
            }
        }
    }

    next_ctx.clear_table(&tbl.name);
    next_ctx.set_column_filter(&tbl.name, col_name, allowed);
}

fn parse_fiscal_year_end(s: &str) -> (u32, u32) {
    let parts: Vec<&str> = s.trim().splitn(2, '-').collect();
    if parts.len() == 2 {
        if let (Ok(m), Ok(d)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return (m, d);
            }
        }
    }
    (12, 31)
}

/// Fiscal year "label" (the calendar year in which the fiscal year ends) for a date,
/// given a fiscal year_end (month, day). Dates after the fiscal year end belong to the
/// fiscal year ending the following calendar year.
fn fiscal_end_year(y: i32, m: u32, d: u32, year_end: (u32, u32)) -> i32 {
    if (m, d) <= year_end {
        y
    } else {
        y + 1
    }
}

fn apply_period_to_date(
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    mode: &str, // "MTD" | "QTD" | "YTD"
    year_end: (u32, u32),
) {
    let Some(tbl) = model.get_table(table_name) else { return };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return };
    let visible = model.get_visible_row_indices(table_name, outer_ctx);

    let mut max_ymd: Option<(i32, u32, u32)> = None;
    for r_idx in visible {
        let s = tbl.rows[r_idx][cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            if max_ymd.is_none_or(|cur| ymd > cur) {
                max_ymd = Some(ymd);
            }
        }
    }

    let Some((max_y, max_m, max_d)) = max_ymd else {
        return;
    };
    let max_q = (max_m - 1) / 3;
    let max_fy = fiscal_end_year(max_y, max_m, max_d, year_end);

    let mut allowed = HashSet::new();
    for row in &tbl.rows {
        let s = row[cidx].as_display_string();
        if let Some((y, m, d)) = parse_iso_ymd(&s) {
            let in_period = match mode {
                "MTD" => y == max_y && m == max_m && d <= max_d,
                "QTD" => y == max_y && ((m - 1) / 3) == max_q && (m, d) <= (max_m, max_d),
                _ => {
                    fiscal_end_year(y, m, d, year_end) == max_fy && (y, m, d) <= (max_y, max_m, max_d)
                }
            };
            if in_period {
                allowed.insert(row[cidx].canonical_key());
            }
        }
    }
    next_ctx.clear_table(&tbl.name);
    next_ctx.set_column_filter(&tbl.name, col_name, allowed);
}

fn resolve_date_boundary_keys(
    model: &DaxModel,
    ctx: &FilterContext,
    table_name: &str,
    col_name: &str,
    func_name: &str,
) -> HashSet<String> {
    let mut allowed = HashSet::new();
    let Some(tbl) = model.get_table(table_name) else { return allowed };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return allowed };
    let visible = model.get_visible_row_indices(table_name, ctx);

    let mut min_ymd: Option<(i32, u32, u32)> = None;
    let mut max_ymd: Option<(i32, u32, u32)> = None;
    for r_idx in visible {
        let s = tbl.rows[r_idx][cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            if min_ymd.is_none_or(|cur| ymd < cur) {
                min_ymd = Some(ymd);
            }
            if max_ymd.is_none_or(|cur| ymd > cur) {
                max_ymd = Some(ymd);
            }
        }
    }

    let is_end = func_name.starts_with("ENDOF") || func_name == "LASTDATE";
    let target_ymd = match func_name {
        "FIRSTDATE" => min_ymd,
        "LASTDATE" => max_ymd,
        "STARTOFMONTH" | "ENDOFMONTH" => max_ymd.and_then(|(y, m, _)| {
            let candidates: Vec<(i32, u32, u32)> = tbl
                .rows
                .iter()
                .filter_map(|r| parse_iso_ymd(&r[cidx].as_display_string()))
                .filter(|&(ry, rm, _)| ry == y && rm == m)
                .collect();
            if is_end { candidates.into_iter().max() } else { candidates.into_iter().min() }
        }),
        "STARTOFQUARTER" | "ENDOFQUARTER" => max_ymd.and_then(|(y, m, _)| {
            let q = (m - 1) / 3;
            let candidates: Vec<(i32, u32, u32)> = tbl
                .rows
                .iter()
                .filter_map(|r| parse_iso_ymd(&r[cidx].as_display_string()))
                .filter(|&(ry, rm, _)| ry == y && ((rm - 1) / 3) == q)
                .collect();
            if is_end { candidates.into_iter().max() } else { candidates.into_iter().min() }
        }),
        "STARTOFYEAR" | "ENDOFYEAR" => max_ymd.and_then(|(y, _, _)| {
            let candidates: Vec<(i32, u32, u32)> = tbl
                .rows
                .iter()
                .filter_map(|r| parse_iso_ymd(&r[cidx].as_display_string()))
                .filter(|&(ry, _, _)| ry == y)
                .collect();
            if is_end { candidates.into_iter().max() } else { candidates.into_iter().min() }
        }),
        _ => None,
    };

    if let Some(t) = target_ymd {
        for row in &tbl.rows {
            let s = row[cidx].as_display_string();
            if parse_iso_ymd(&s) == Some(t) {
                allowed.insert(row[cidx].canonical_key());
            }
        }
    }
    allowed
}

fn apply_period_boundary(
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    is_opening: bool,
    scope: &str,
) {
    let fname = match (is_opening, scope) {
        (true, "MONTH") => "STARTOFMONTH",
        (false, "MONTH") => "ENDOFMONTH",
        (true, "QUARTER") => "STARTOFQUARTER",
        (false, "QUARTER") => "ENDOFQUARTER",
        (true, _) => "STARTOFYEAR",
        (false, _) => "ENDOFYEAR",
    };
    let mut keys = HashSet::new();
    if is_opening {
        // Opening balance evaluates at the closing date of the immediately preceding period
        if let Some(tbl) = model.get_table(table_name) {
            if let Some(cidx) = tbl.get_col_idx(col_name) {
                let visible = model.get_visible_row_indices(table_name, outer_ctx);
                let min_cur = visible
                    .into_iter()
                    .filter_map(|i| parse_iso_ymd(&tbl.rows[i][cidx].as_display_string()))
                    .min();
                if let Some((y, m, d)) = min_cur {
                    let prev_day = add_days(y, m, d, -1);
                    for row in &tbl.rows {
                        if parse_iso_ymd(&row[cidx].as_display_string()) == Some(prev_day) {
                            keys.insert(row[cidx].canonical_key());
                        }
                    }
                }
            }
        }
        if keys.is_empty() {
            keys = resolve_date_boundary_keys(model, outer_ctx, table_name, col_name, fname);
        }
    } else {
        keys = resolve_date_boundary_keys(model, outer_ctx, table_name, col_name, fname);
    }
    if let Some(tbl) = model.get_table(table_name) {
        next_ctx.clear_table(&tbl.name);
        next_ctx.set_column_filter(&tbl.name, col_name, keys);
    }
}

fn apply_dates_between(
    model: &DaxModel,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    start_str: &str,
    end_str: &str,
) {
    let Some(tbl) = model.get_table(table_name) else { return };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return };
    let start_days = parse_flexible_ymd(start_str).map(|(y, m, d)| civil_to_days(y, m, d));
    let end_days = parse_flexible_ymd(end_str).map(|(y, m, d)| civil_to_days(y, m, d));

    let mut allowed = HashSet::new();
    for row in &tbl.rows {
        let s = row[cidx].as_display_string();
        if let Some((y, m, d)) = parse_iso_ymd(&s) {
            let cur = civil_to_days(y, m, d);
            let ge_start = start_days.is_none_or(|sd| cur >= sd);
            let le_end = end_days.is_none_or(|ed| cur <= ed);
            if ge_start && le_end {
                allowed.insert(row[cidx].canonical_key());
            }
        }
    }
    next_ctx.clear_table(&tbl.name);
    next_ctx.set_column_filter(&tbl.name, col_name, allowed);
}

fn apply_dates_in_period(
    model: &DaxModel,
    next_ctx: &mut FilterContext,
    table_name: &str,
    col_name: &str,
    anchor_str: &str,
    count: i32,
    interval: &str,
) {
    let Some((ay, am, ad)) = parse_flexible_ymd(anchor_str) else { return };
    let (start_d, end_d) = if count == 0 {
        (civil_to_days(ay, am, ad), civil_to_days(ay, am, ad) - 1)
    } else {
        let shifted = match interval {
            "YEAR" => add_months(ay, am, ad, count * 12),
            "QUARTER" => add_months(ay, am, ad, count * 3),
            "MONTH" => add_months(ay, am, ad, count),
            _ => add_days(ay, am, ad, count),
        };
        let a_days = civil_to_days(ay, am, ad);
        let s_days = civil_to_days(shifted.0, shifted.1, shifted.2);
        if count > 0 {
            (a_days, s_days - 1)
        } else {
            (s_days + 1, a_days)
        }
    };

    let Some(tbl) = model.get_table(table_name) else { return };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return };
    let mut allowed = HashSet::new();
    for row in &tbl.rows {
        let s = row[cidx].as_display_string();
        if let Some((y, m, d)) = parse_iso_ymd(&s) {
            let cur = civil_to_days(y, m, d);
            if cur >= start_d && cur <= end_d {
                allowed.insert(row[cidx].canonical_key());
            }
        }
    }
    next_ctx.clear_table(&tbl.name);
    next_ctx.set_column_filter(&tbl.name, col_name, allowed);
}

fn eval_lookupvalue(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let Some(Expr::TableColRef { table: res_table, column: res_col }) = args.first() else {
        return Err("LOOKUPVALUE requer Tabela[ColunaResultado] no 1º argumento".to_string());
    };
    let tbl = model
        .get_table(res_table)
        .ok_or_else(|| format!("Tabela '{}' não encontrada em LOOKUPVALUE", res_table))?;
    let res_cidx = tbl
        .get_col_idx(res_col)
        .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada em LOOKUPVALUE", res_table, res_col))?;

    let has_alternate = args.len() >= 4 && args.len() % 2 == 0;
    let pairs_end = if has_alternate { args.len() - 1 } else { args.len() };
    let alternate = if has_alternate {
        evaluate_ast(&args[args.len() - 1], model, ctx, row_stack, vars)?
    } else {
        DaxValue::Blank
    };

    let mut criteria: Vec<(usize, DaxValue)> = Vec::new();
    let mut i = 1;
    while i + 1 < pairs_end {
        let Some(Expr::TableColRef { column: scol, .. }) = args.get(i) else {
            return Err("LOOKUPVALUE espera pares (Tabela[ColunaBusca], ValorBusca)".to_string());
        };
        let scidx = tbl
            .get_col_idx(scol)
            .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", tbl.name, scol))?;
        let target_val = evaluate_ast(&args[i + 1], model, ctx, row_stack, vars)?;
        criteria.push((scidx, target_val));
        i += 2;
    }

    let mut found: Option<DaxValue> = None;
    for row in &tbl.rows {
        let matches = criteria.iter().all(|(cidx, expected)| row[*cidx].dax_equals(expected));
        if matches {
            let candidate = row[res_cidx].clone();
            if let Some(ref prev) = found {
                if !prev.dax_equals(&candidate) {
                    return Ok(alternate);
                }
            } else {
                found = Some(candidate);
            }
        }
    }

    Ok(found.unwrap_or(alternate))
}

fn eval_column_agg(
    func: &str,
    model: &DaxModel,
    ctx: &FilterContext,
    table_name: &str,
    col_name: &str,
) -> Result<DaxValue, String> {
    let tbl = model
        .get_table(table_name)
        .ok_or_else(|| format!("Tabela '{}' não encontrada", table_name))?;
    let cidx = tbl
        .get_col_idx(col_name)
        .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table_name, col_name))?;
    let visible = model.get_visible_row_indices(table_name, ctx);

    if func == "COUNT" || func == "COUNTA" {
        let cnt = visible.iter().filter(|&&i| !tbl.rows[i][cidx].is_blank()).count();
        return Ok(DaxValue::Number(cnt as f64));
    }
    if func == "COUNTBLANK" {
        let cnt = visible.iter().filter(|&&i| tbl.rows[i][cidx].is_blank()).count();
        return Ok(DaxValue::Number(cnt as f64));
    }
    if func == "DISTINCTCOUNT" {
        let distinct: HashSet<String> = visible
            .iter()
            .map(|&i| tbl.rows[i][cidx].canonical_key())
            .collect();
        return Ok(DaxValue::Number(distinct.len() as f64));
    }
    if func == "DISTINCTCOUNTNOBLANK" {
        let distinct: HashSet<String> = visible
            .iter()
            .filter(|&&i| !tbl.rows[i][cidx].is_blank())
            .map(|&i| tbl.rows[i][cidx].canonical_key())
            .collect();
        return Ok(DaxValue::Number(distinct.len() as f64));
    }

    // MIN/MAX compare chronologically for date-typed (ISO text) columns and return the
    // original value (not coerced to Number). MINA/MAXA keep numeric boolean coercion below.
    if matches!(func, "MIN" | "MAX") {
        let values: Vec<&DaxValue> = visible
            .iter()
            .map(|&i| &tbl.rows[i][cidx])
            .filter(|v| !v.is_blank())
            .collect();
        if values.is_empty() {
            return Ok(DaxValue::Blank);
        }
        let want_min = func == "MIN";
        let mut best = values[0];
        for v in &values[1..] {
            let ord = compare_dax_values(v, best);
            if (want_min && ord < 0) || (!want_min && ord > 0) {
                best = v;
            }
        }
        return Ok(best.clone());
    }

    let nums: Vec<f64> = visible
        .iter()
        .filter_map(|&i| {
            let v = &tbl.rows[i][cidx];
            if v.is_blank() {
                None
            } else if matches!(func, "AVERAGEA" | "MINA" | "MAXA") {
                match v {
                    DaxValue::Number(n) => Some(*n),
                    DaxValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
                    DaxValue::Text(_) => Some(0.0),
                    DaxValue::Blank => None,
                }
            } else {
                v.as_f64()
            }
        })
        .collect();

    if nums.is_empty() {
        return Ok(DaxValue::Blank);
    }
    let out = match func {
        "SUM" => Some(nums.iter().sum()),
        "AVERAGE" | "AVERAGEA" => Some(nums.iter().sum::<f64>() / nums.len() as f64),
        "MINA" => Some(nums.iter().copied().fold(f64::INFINITY, f64::min)),
        "MAXA" => Some(nums.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "MEDIAN" => calc_median(&nums),
        "STDEV.S" => calc_stdev(&nums, true),
        "STDEV.P" => calc_stdev(&nums, false),
        "VAR.S" => calc_variance(&nums, true),
        "VAR.P" => calc_variance(&nums, false),
        "PRODUCT" => Some(nums.iter().product()),
        "GEOMEAN" => calc_geomean(&nums),
        _ => None,
    };
    Ok(out.map(DaxValue::Number).unwrap_or(DaxValue::Blank))
}

fn eval_selected_value(
    model: &DaxModel,
    ctx: &FilterContext,
    table_name: &str,
    col_name: &str,
    alternate: DaxValue,
) -> Result<DaxValue, String> {
    let Some(tbl) = model.get_table(table_name) else {
        return Ok(alternate);
    };
    let Some(cidx) = tbl.get_col_idx(col_name) else {
        return Ok(alternate);
    };
    let visible = model.get_visible_row_indices(table_name, ctx);
    let mut distinct_keys = HashSet::new();
    let mut first_val: Option<DaxValue> = None;

    for r_idx in visible {
        let val = &tbl.rows[r_idx][cidx];
        if val.is_blank() {
            continue;
        }
        distinct_keys.insert(val.canonical_key());
        if first_val.is_none() {
            first_val = Some(val.clone());
        }
        if distinct_keys.len() > 1 {
            return Ok(alternate);
        }
    }

    if distinct_keys.len() == 1 {
        Ok(first_val.unwrap_or(alternate))
    } else {
        Ok(alternate)
    }
}

fn count_distinct_visible(
    model: &DaxModel,
    ctx: &FilterContext,
    table_name: &str,
    col_name: &str,
) -> usize {
    let Some(tbl) = model.get_table(table_name) else { return 0 };
    let Some(cidx) = tbl.get_col_idx(col_name) else { return 0 };
    model
        .get_visible_row_indices(table_name, ctx)
        .into_iter()
        .filter(|&i| !tbl.rows[i][cidx].is_blank())
        .map(|i| tbl.rows[i][cidx].canonical_key())
        .collect::<HashSet<_>>()
        .len()
}

fn resolve_related(
    model: &DaxModel,
    rc: &RowContext<'_>,
    target_table: &str,
    target_col: &str,
) -> Option<DaxValue> {
    for rel in &model.relationships {
        if !rel.is_active {
            continue;
        }
        if rel.from_table.eq_ignore_ascii_case(&rc.table.name)
            && rel.to_table.eq_ignore_ascii_case(target_table)
        {
            let fk_idx = rc.table.get_col_idx(&rel.from_column)?;
            let fk_key = rc.table.rows[rc.row_idx][fk_idx].canonical_key();
            let to_tbl = model.get_table(target_table)?;
            let pk_idx = to_tbl.get_col_idx(&rel.to_column)?;
            let val_idx = to_tbl.get_col_idx(target_col)?;
            for row in &to_tbl.rows {
                if row[pk_idx].canonical_key() == fk_key {
                    return Some(row[val_idx].clone());
                }
            }
        }
    }
    None
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
