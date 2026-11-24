use crate::dax::ast::{BinaryOp, Expr, UnaryOp};
use crate::dax::parser::parse_dax;
use crate::dax::{DaxModel, DaxTableData, DaxValue, FilterContext};
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
            let den = r.as_f64().unwrap_or(0.0);
            if den == 0.0 {
                Ok(DaxValue::Blank)
            } else {
                Ok(DaxValue::Number(l.as_f64().unwrap_or(0.0) / den))
            }
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
    let mut next = ctx.clone();
    for (idx, cname) in rc.table.columns.iter().enumerate() {
        let mut set = HashSet::new();
        set.insert(rc.table.rows[rc.row_idx][idx].canonical_key());
        next.set_column_filter(&rc.table.name, cname, set);
    }
    next
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
        "IF" => {
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
            Ok(v) => Ok(v),
            Err(_) => eval_arg(args, 1, model, ctx, row_stack, vars),
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
        "SUM" | "AVERAGE" | "MIN" | "MAX" | "COUNT" | "COUNTA" | "DISTINCTCOUNT" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Err(format!("{} requer referência Tabela[Coluna]", fname));
            };
            eval_column_agg(&fname, model, ctx, table, column)
        }
        "COUNTROWS" => {
            let tname = extract_table_name(args.first())?;
            let vis = model.get_visible_row_indices(&tname, ctx);
            Ok(DaxValue::Number(vis.len() as f64))
        }
        "SUMX" | "AVERAGEX" | "MINX" | "MAXX" | "COUNTX" => {
            if args.len() < 2 {
                return Err(format!("{} requer (tabela, expressão)", fname));
            }
            let (table_data, row_indices) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let mut vals = Vec::new();
            for r_idx in row_indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext {
                    table: table_data,
                    row_idx: r_idx,
                });
                let v = evaluate_ast(&args[1], model, ctx, &next_stack, vars)?;
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
                "SUMX" => vals.iter().sum(),
                "AVERAGEX" => vals.iter().sum::<f64>() / vals.len() as f64,
                "MINX" => vals.iter().copied().fold(f64::INFINITY, f64::min),
                "MAXX" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                "COUNTX" => vals.len() as f64,
                _ => 0.0,
            };
            Ok(DaxValue::Number(res))
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
        "HASONEVALUE" => {
            let Some(Expr::TableColRef { table, column }) = args.first() else {
                return Ok(DaxValue::Boolean(false));
            };
            let count = count_distinct_visible(model, ctx, table, column);
            Ok(DaxValue::Boolean(count == 1))
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
        _ => Err(format!("Função DAX '{}' ainda não implementada no motor nativo.", fname)),
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

fn eval_table_expr<'a>(
    expr: &Expr,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(&'a DaxTableData, Vec<usize>), String> {
    match expr {
        Expr::Ident(tname) => {
            let tbl = model
                .get_table(tname)
                .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))?;
            let indices = model.get_visible_row_indices(tname, ctx);
            Ok((tbl, indices))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("ALL") => {
            let tname = extract_table_name(args.first())?;
            let tbl = model
                .get_table(&tname)
                .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))?;
            Ok((tbl, (0..tbl.rows.len()).collect()))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("FILTER") => {
            if args.len() < 2 {
                return Err("FILTER requer (tabela, condição)".to_string());
            }
            let (tbl, indices) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let mut kept = Vec::new();
            for r_idx in indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: tbl, row_idx: r_idx });
                let cond = evaluate_ast(&args[1], model, ctx, &next_stack, vars)?;
                if cond.as_bool() {
                    kept.push(r_idx);
                }
            }
            Ok((tbl, kept))
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
                "REMOVEFILTERS" | "ALL" => {
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
                "PREVIOUSMONTH" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_date_shift(model, outer_ctx, next_ctx, table, column, -1, "MONTH");
                    }
                    return Ok(());
                }
                "SAMEPERIODLASTYEAR" => {
                    if let Some(Expr::TableColRef { table, column }) = args.first() {
                        apply_date_shift(model, outer_ctx, next_ctx, table, column, -1, "YEAR");
                    }
                    return Ok(());
                }
                "FILTER" => {
                    if args.len() >= 2 {
                        let (tbl, kept_indices) = eval_table_expr(expr, model, outer_ctx, row_stack, vars)?;
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

    let mut shifted_keys = HashSet::new();
    for r_idx in visible {
        let s = tbl.rows[r_idx][cidx].as_display_string();
        if let Some((y, m, d)) = parse_iso_ymd(&s) {
            let (ny, nm, nd) = match interval {
                "YEAR" => (y + shift, m, d),
                "MONTH" => add_months(y, m, d, shift),
                "QUARTER" => add_months(y, m, d, shift * 3),
                _ => add_days(y, m, d, shift),
            };
            let iso = format!("{:04}-{:02}-{:02}", ny, nm, nd);
            shifted_keys.insert(DaxValue::Text(iso).canonical_key());
        }
    }
    next_ctx.set_column_filter(&tbl.name, col_name, shifted_keys);
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
    if func == "DISTINCTCOUNT" {
        let distinct: HashSet<String> = visible
            .iter()
            .filter(|&&i| !tbl.rows[i][cidx].is_blank())
            .map(|&i| tbl.rows[i][cidx].canonical_key())
            .collect();
        return Ok(DaxValue::Number(distinct.len() as f64));
    }

    let nums: Vec<f64> = visible
        .iter()
        .filter_map(|&i| {
            let v = &tbl.rows[i][cidx];
            if v.is_blank() { None } else { v.as_f64() }
        })
        .collect();

    if nums.is_empty() {
        return Ok(DaxValue::Blank);
    }
    let out = match func {
        "SUM" => nums.iter().sum(),
        "AVERAGE" => nums.iter().sum::<f64>() / nums.len() as f64,
        "MIN" => nums.iter().copied().fold(f64::INFINITY, f64::min),
        "MAX" => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => 0.0,
    };
    Ok(DaxValue::Number(out))
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

fn parse_iso_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let date_part = s.split('T').next()?.trim();
    let mut parts = date_part.split('-');
    let y = parts.next()?.parse::<i32>().ok()?;
    let m = parts.next()?.parse::<u32>().ok()?;
    let d = parts.next()?.parse::<u32>().ok()?;
    Some((y, m, d))
}

fn add_months(y: i32, m: u32, d: u32, delta_months: i32) -> (i32, u32, u32) {
    let total_m = y * 12 + (m as i32 - 1) + delta_months;
    let ny = total_m.div_euclid(12);
    let nm = (total_m.rem_euclid(12) + 1) as u32;
    let max_d = days_in_month(ny, nm);
    (ny, nm, d.min(max_d))
}

fn add_days(y: i32, m: u32, d: u32, delta_days: i32) -> (i32, u32, u32) {
    let mut cur_y = y;
    let mut cur_m = m;
    let mut cur_d = d as i32 + delta_days;
    while cur_d > days_in_month(cur_y, cur_m) as i32 {
        cur_d -= days_in_month(cur_y, cur_m) as i32;
        cur_m += 1;
        if cur_m > 12 {
            cur_m = 1;
            cur_y += 1;
        }
    }
    while cur_d < 1 {
        if cur_m == 1 {
            cur_m = 12;
            cur_y -= 1;
        } else {
            cur_m -= 1;
        }
        cur_d += days_in_month(cur_y, cur_m) as i32;
    }
    (cur_y, cur_m, cur_d as u32)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
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
        let dre = model.evaluate_dax("[DRE]", &ctx).expect("eval DRE");
        let entradas = model.evaluate_dax("[Entradas]", &ctx).expect("eval Entradas");
        let saidas = model.evaluate_dax("[Saídas]", &ctx).expect("eval Saídas");
        println!(
            "RHINO REAL -> Faturamento={:?}, DRE={:?}, Entradas={:?}, Saidas={:?}, LoadedTables={:?}",
            fat,
            dre,
            entradas,
            saidas,
            summary.loaded_tables.iter().map(|t| (&t.table_name, t.total_rows)).collect::<Vec<_>>()
        );
        assert!(fat.as_f64().unwrap_or(0.0) > 0.0);
    }
}
