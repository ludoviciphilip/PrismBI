//! Aggregations: column aggregates, X-iterators (SUMX & co.), RANKX, COUNTROWS,
//! CONCATENATEX and XNPV/XIRR.

use super::*;
use crate::dax::functions::{
    calc_geomean, calc_median, calc_percentile, calc_stdev, calc_variance, calc_xirr, calc_xnpv,
    civil_to_days, parse_flexible_ymd, DatedFlow,
};

/// DAX `RANKX`.
pub(super) fn eval_rankx(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err("RANKX requer (tabela, expressão)".to_string());
    }
    let (table_eval, row_indices, rank_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
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
    // Unlike the X-aggregators, RANKX evaluates its expression with a per-row context
    // transition even for non-measure expressions (e.g. RANKX(ALL(T), SUM(T[v]))),
    // which is what the corpus oracle (stats.rankx.*) encodes. Only columns with model
    // lineage become filters.
    let mut scores = Vec::new();
    for r_idx in row_indices {
        let row_ctx = RowContext { table: table_data, row_idx: r_idx };
        let eff_ctx = apply_lineage_transition(model, &rank_ctx, &row_ctx);
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

/// DAX `SUM`, `AVERAGE`, `AVERAGEA`, `MIN`, `MINA`, `MAX`, `MAXA`, `COUNT`, `COUNTA`, `COUNTBLANK`,
/// `DISTINCTCOUNT`, `DISTINCTCOUNTNOBLANK`, `MEDIAN`, `STDEV.S`, `STDEV.P`, `VAR.S`, `VAR.P`,
/// `PRODUCT`, `GEOMEAN`.
pub(super) fn eval_column_aggregate(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
    let Some(Expr::TableColRef { table, column }) = args.first() else {
        return Err(format!("{} requer referência Tabela[Coluna]", fname));
    };
    eval_column_agg(&fname, model, ctx, table, column)
}

/// DAX `PERCENTILE.INC`, `PERCENTILE.EXC`.
pub(super) fn eval_percentile(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
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

/// DAX `COUNTROWS`.
pub(super) fn eval_countrows(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let Some(first) = args.first() else {
        return Err("COUNTROWS requer expressão de tabela".to_string());
    };
    let (_, indices, _) = eval_table_expr(first, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Number(indices.len() as f64))
}

/// DAX `ISEMPTY`.
pub(super) fn eval_isempty(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let Some(first) = args.first() else {
        return Ok(DaxValue::Boolean(true));
    };
    let (_, indices, _) = eval_table_expr(first, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Boolean(indices.is_empty()))
}

/// DAX `CONTAINSROW`.
pub(super) fn eval_containsrow(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err("CONTAINSROW requer (tabela, valor1, ...)".to_string());
    }
    let (table_eval, row_indices, _) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
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

/// DAX `SUMX`, `AVERAGEX`, `MINX`, `MAXX`, `COUNTX`, `COUNTAX`, `MEDIANX`, `STDEVX.S`, `STDEVX.P`,
/// `VARX.S`, `VARX.P`, `PRODUCTX`, `GEOMEANX`.
pub(super) fn eval_x_aggregate(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err(format!("{} requer (tabela, expressão)", fname));
    }
    let (table_eval, row_indices, iter_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
    let table_data = table_eval.as_ref();
    // Row context only, no implicit context transition: a nested FILTER(T, ...) must
    // still see every row of T (EARLIER pattern); measure references / CALCULATE
    // transition explicitly.
    if fname == "COUNTAX" {
        let mut cnt = 0usize;
        for r_idx in row_indices {
            let mut next_stack = row_stack.to_vec();
            next_stack.push(RowContext { table: table_data, row_idx: r_idx });
            let v = evaluate_ast(&args[1], model, &iter_ctx, &next_stack, vars)?;
            if !v.is_blank() {
                cnt += 1;
            }
        }
        return Ok(DaxValue::Number(cnt as f64));
    }
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
    if vals.is_empty() {
        return Ok(DaxValue::Blank);
    }
    let res = match fname {
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

/// DAX `PERCENTILEX.INC`, `PERCENTILEX.EXC`.
pub(super) fn eval_percentilex(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 3 {
        return Err(format!("{} requer (tabela, expressão, k)", fname));
    }
    let (table_eval, row_indices, iter_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
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

/// DAX `CONCATENATEX`.
pub(super) fn eval_concatenatex(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err("CONCATENATEX requer (tabela, expressão, [delimitador])".to_string());
    }
    let (table_eval, row_indices, iter_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
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

/// DAX `XNPV`, `XIRR`.
pub(super) fn eval_xnpv_xirr(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let min_args = if fname == "XNPV" { 4 } else { 3 };
    if args.len() < min_args {
        return Err(if fname == "XNPV" {
            "XNPV requer (tabela, valores, datas, taxa)".to_string()
        } else {
            "XIRR requer (tabela, valores, datas, [estimativa])".to_string()
        });
    }
    let flows = eval_dated_flows(&fname, args, model, ctx, row_stack, vars)?;
    if flows.is_empty() {
        return Ok(DaxValue::Blank);
    }
    if fname == "XNPV" {
        let rate = evaluate_ast(&args[3], model, ctx, row_stack, vars)?
            .as_f64()
            .ok_or("XNPV: taxa deve ser numérica")?;
        calc_xnpv(rate, &flows).map(DaxValue::Number)
    } else {
        let guess = match args.get(3) {
            None | Some(Expr::Blank) => 0.1,
            Some(e) => evaluate_ast(e, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.1),
        };
        calc_xirr(&flows, guess).map(DaxValue::Number)
    }
}

/// Iterates `args[0]` (a table) evaluating `args[1]` (amount) and `args[2]` (date) per row,
/// for XNPV/XIRR. Rows with a blank amount are skipped; a blank/invalid date is an error.
pub(super) fn eval_dated_flows(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<Vec<DatedFlow>, String> {
    let (table_eval, row_indices, iter_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
    let table_data = table_eval.as_ref();
    let mut flows = Vec::with_capacity(row_indices.len());
    for r_idx in row_indices {
        let mut next_stack = row_stack.to_vec();
        next_stack.push(RowContext { table: table_data, row_idx: r_idx });
        let value = evaluate_ast(&args[1], model, &iter_ctx, &next_stack, vars)?;
        if value.is_blank() {
            continue;
        }
        let amount = value
            .as_f64()
            .ok_or_else(|| format!("{}: valor não numérico '{}'", fname, value.as_display_string()))?;
        let date = evaluate_ast(&args[2], model, &iter_ctx, &next_stack, vars)?;
        let day = match &date {
            DaxValue::Number(n) => n.floor() as i64,
            other => parse_flexible_ymd(&other.as_display_string())
                .map(|(y, m, d)| civil_to_days(y, m, d) as i64)
                .ok_or_else(|| format!("{}: data inválida '{}'", fname, other.as_display_string()))?,
        };
        flows.push((amount, day));
    }
    Ok(flows)
}

pub(super) fn eval_column_agg(
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
