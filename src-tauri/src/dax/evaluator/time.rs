//! Time intelligence: TOTALxTD, opening/closing balances, date boundaries and the date-filter helpers.

use super::*;
use crate::dax::functions::{
    add_days, add_months, civil_to_days, days_in_month, parse_flexible_ymd, parse_iso_ymd,
};

/// DAX `TOTALYTD`, `TOTALMTD`, `TOTALQTD`.
pub(super) fn eval_total_to_date(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err(format!("{} requer (expressão, Tabela[Data])", fname));
    }
    let Some(Expr::TableColRef { table, column }) = args.get(1) else {
        return Err(format!("{} requer coluna de data no 2º argumento", fname));
    };
    let mut next_ctx = apply_context_transition(model, ctx, row_stack);
    let mode = match fname {
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

/// DAX `OPENINGBALANCEMONTH`, `CLOSINGBALANCEMONTH`, `OPENINGBALANCEQUARTER`,
/// `CLOSINGBALANCEQUARTER`, `OPENINGBALANCEYEAR`, `CLOSINGBALANCEYEAR`.
pub(super) fn eval_opening_closing_balance(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 2 {
        return Err(format!("{} requer (expressão, Tabela[Data])", fname));
    }
    let Some(Expr::TableColRef { table, column }) = args.get(1) else {
        return Err(format!("{} requer coluna de data no 2º argumento", fname));
    };
    let mut next_ctx = apply_context_transition(model, ctx, row_stack);
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

/// DAX `FIRSTDATE`, `LASTDATE`, `STARTOFMONTH`, `ENDOFMONTH`, `STARTOFQUARTER`, `ENDOFQUARTER`,
/// `STARTOFYEAR`, `ENDOFYEAR`.
pub(super) fn eval_date_boundary(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
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

pub(super) fn collect_table_dates(tbl: &DaxTableData, cidx: usize) -> HashMap<(i32, u32, u32), Vec<String>> {
    let mut map: HashMap<(i32, u32, u32), Vec<String>> = HashMap::new();
    for row in &tbl.rows {
        let s = row[cidx].as_display_string();
        if let Some(ymd) = parse_iso_ymd(&s) {
            map.entry(ymd).or_default().push(row[cidx].canonical_key());
        }
    }
    map
}

pub(super) fn apply_date_shift(
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

pub(super) fn apply_complete_period_shift(
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

pub(super) fn parse_fiscal_year_end(s: &str) -> (u32, u32) {
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
pub(super) fn fiscal_end_year(y: i32, m: u32, d: u32, year_end: (u32, u32)) -> i32 {
    if (m, d) <= year_end {
        y
    } else {
        y + 1
    }
}

pub(super) fn apply_period_to_date(
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

pub(super) fn resolve_date_boundary_keys(
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

pub(super) fn apply_period_boundary(
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

pub(super) fn apply_dates_between(
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

pub(super) fn apply_dates_in_period(
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
