//! Filter context: CALCULATE and its modifiers, filter introspection (ISFILTERED, HASONEVALUE...), LOOKUPVALUE.

use super::*;

/// DAX `CALCULATE`.
pub(super) fn eval_calculate(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.is_empty() {
        return Err("CALCULATE requer ao menos 1 argumento".to_string());
    }
    let mut next_ctx = apply_context_transition(model, ctx, row_stack);
    for filter_expr in &args[1..] {
        apply_calculate_modifier(filter_expr, model, ctx, &mut next_ctx, row_stack, vars)?;
    }
    evaluate_ast(&args[0], model, &next_ctx, &[], vars)
}

/// DAX `SELECTEDVALUE`.
pub(super) fn eval_selectedvalue(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
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

/// DAX `HASONEVALUE`, `HASONEFILTER`.
pub(super) fn eval_hasonevalue(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
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

/// DAX `ISFILTERED`.
pub(super) fn eval_isfiltered(
    args: &[Expr],
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
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

/// DAX `ISCROSSFILTERED`.
pub(super) fn eval_iscrossfiltered(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
    let table_name = match args.first() {
        Some(Expr::TableColRef { table, .. }) => Some(table.clone()),
        Some(Expr::Ident(t)) => Some(t.clone()),
        _ => None,
    };
    let Some(table) = table_name else {
        return Ok(DaxValue::Boolean(false));
    };
    if let Some(tbl) = model.get_table(&table) {
        let vis_count = model.get_visible_row_indices(&table, ctx).len();
        let direct = ctx.filters.get(&tbl.name.to_lowercase()).is_some_and(|c| c.values().any(|s| !s.is_empty()));
        let is_filtered = direct || vis_count < tbl.rows.len();
        Ok(DaxValue::Boolean(is_filtered))
    } else {
        Ok(DaxValue::Boolean(false))
    }
}

/// DAX `ISINSCOPE`, `ISATLEVEL`.
pub(super) fn eval_isinscope(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
) -> Result<DaxValue, String> {
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

/// DAX `FIRSTNONBLANK`, `LASTNONBLANK`.
pub(super) fn eval_first_last_nonblank(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
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
    let mut distinct_vals: Vec<(ValueKey, DaxValue)> = Vec::new();
    let mut seen = HashSet::new();
    for r_idx in vis {
        let v = &tbl.rows[r_idx][cidx];
        if !v.is_blank() && seen.insert(v.value_key()) {
            distinct_vals.push((v.value_key(), v.clone()));
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

pub(super) fn apply_calculate_modifier(
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
                        if let Some((tbl_name, col_name)) = find_primary_table_col(inner) {
                            if let Some(tbl) = model.get_table(&tbl_name) {
                                if let Some(cidx) = tbl.get_col_idx(&col_name) {
                                    let mut allowed = HashSet::new();
                                    for r_idx in 0..tbl.rows.len() {
                                        let rc = RowContext { table: tbl, row_idx: r_idx };
                                        let v = evaluate_ast(inner, model, outer_ctx, std::slice::from_ref(&rc), vars)?;
                                        if v.as_bool() {
                                            allowed.insert(tbl.rows[r_idx][cidx].value_key());
                                        }
                                    }
                                    // KEEPFILTERS intersects with any existing filter on the same
                                    // column instead of replacing it (unlike a bare CALCULATE filter).
                                    if let Some(existing) = outer_ctx
                                        .filters
                                        .get(&tbl.name.to_lowercase())
                                        .and_then(|m| m.get(&col_name.to_lowercase()))
                                    {
                                        allowed = allowed.intersection(existing).cloned().collect();
                                    }
                                    next_ctx.set_column_filter(&tbl.name, &col_name, allowed);
                                    return Ok(());
                                }
                            }
                        }
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
                            eval_table_expr(&args[0], model, outer_ctx, row_stack, vars)?;
                        let src_data = src_tbl.as_ref();
                        for (col_pos, col_expr) in args[1..].iter().enumerate() {
                            if let Expr::TableColRef { table, column } = col_expr {
                                if let Some(target_tbl) = model.get_table(table) {
                                    let mut allowed = HashSet::new();
                                    for &r_idx in &src_indices {
                                        if let Some(cell) = src_data.rows[r_idx].get(col_pos) {
                                            allowed.insert(cell.value_key());
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
                        let (tbl_eval, kept_indices, _) = eval_table_expr(expr, model, outer_ctx, row_stack, vars)?;
                        let tbl = tbl_eval.as_ref();
                        // A physical table is filtered through its first (key) column; a
                        // synthetic table (e.g. FILTER(VALUES(T[c]), ...), FILTER(SUMMARIZE(..)))
                        // filters every column that has model lineage.
                        let filter_cols: Vec<(usize, String, String)> = match &tbl_eval {
                            TableEval::Borrowed(_) => tbl
                                .columns
                                .first()
                                .map(|c| vec![(0, tbl.name.clone(), c.clone())])
                                .unwrap_or_default(),
                            TableEval::Owned(_) => (0..tbl.columns.len())
                                .filter_map(|i| column_lineage(model, tbl, i).map(|(t, c)| (i, t, c)))
                                .collect(),
                        };
                        for (cidx, t, c) in filter_cols {
                            let allowed: HashSet<ValueKey> =
                                kept_indices.iter().map(|&i| tbl.rows[i][cidx].value_key()).collect();
                            next_ctx.set_column_filter(&t, &c, allowed);
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
                                allowed.insert(picked.value_key());
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
                        allowed.insert(tbl.rows[r_idx][cidx].value_key());
                    }
                }
                next_ctx.set_column_filter(&tbl.name, &col_name, allowed);
            }
        }
    }

    Ok(())
}

pub(super) fn find_primary_table_col(expr: &Expr) -> Option<(String, String)> {
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

pub(super) fn eval_lookupvalue(
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
                    if has_alternate {
                        return Ok(alternate);
                    }
                    return Err("LOOKUPVALUE: múltiplas linhas correspondentes com valores diferentes".to_string());
                }
            } else {
                found = Some(candidate);
            }
        }
    }

    Ok(found.unwrap_or(alternate))
}

pub(super) fn eval_selected_value(
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
        distinct_keys.insert(val.value_key());
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

pub(super) fn count_distinct_visible(
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
        .map(|i| tbl.rows[i][cidx].value_key())
        .collect::<HashSet<_>>()
        .len()
}
