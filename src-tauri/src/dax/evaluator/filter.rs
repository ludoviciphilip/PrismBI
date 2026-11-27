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
                "REMOVEFILTERS" | "ALL" => {
                    if args.is_empty() {
                        next_ctx.filters.clear();
                        next_ctx.tuple_filters.clear();
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
                "ALLSELECTED" => {
                    // §3 item 3: ALLSELECTED restores the *visual's* starting filter
                    // (the shadow captured once in `build_filter_context`), not the
                    // model-wide unfiltered state — unlike bare ALL/REMOVEFILTERS.
                    let shadow = outer_ctx.shadow_ctx();
                    if args.is_empty() {
                        next_ctx.filters = shadow.filters.clone();
                        next_ctx.tuple_filters = shadow.tuple_filters.clone();
                    } else {
                        for arg in args {
                            match arg {
                                Expr::Ident(t) => {
                                    let tl = t.to_lowercase();
                                    next_ctx.filters.remove(&tl);
                                    next_ctx.tuple_filters.retain(|tf| tf.table != tl);
                                    if let Some(cols) = shadow.filters.get(&tl) {
                                        next_ctx.filters.insert(tl.clone(), cols.clone());
                                    }
                                    next_ctx
                                        .tuple_filters
                                        .extend(shadow.tuple_filters.iter().filter(|tf| tf.table == tl).cloned());
                                }
                                Expr::TableColRef { table, column } => {
                                    let shadow_val = shadow
                                        .filters
                                        .get(&table.to_lowercase())
                                        .and_then(|m| m.get(&column.to_lowercase()))
                                        .cloned();
                                    match shadow_val {
                                        Some(v) => next_ctx.set_column_filter(table, column, v),
                                        None => next_ctx.clear_column(table, column),
                                    }
                                }
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
                        let is_table_filter = matches!(
                            inner,
                            Expr::Call { name, .. } if matches!(
                                name.trim().to_uppercase().as_str(),
                                "FILTER" | "VALUES" | "DISTINCT" | "SUMMARIZE" | "ADDCOLUMNS" | "SELECTCOLUMNS"
                                    | "UNION" | "INTERSECT" | "EXCEPT" | "CROSSJOIN" | "TOPN" | "CALCULATETABLE"
                                    | "RELATEDTABLE" | "TREATAS"
                            )
                        );
                        if is_table_filter {
                            if let Expr::Call { name, args: inner_args } = inner {
                                if name.trim().eq_ignore_ascii_case("TREATAS") {
                                    apply_treatas(inner_args, model, outer_ctx, next_ctx, row_stack, vars, true)?;
                                    return Ok(());
                                }
                            }
                            apply_table_filter(inner, model, outer_ctx, next_ctx, row_stack, vars, true)?;
                            return Ok(());
                        }
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
                    apply_treatas(args, model, outer_ctx, next_ctx, row_stack, vars, false)?;
                    return Ok(());
                }
                "FILTER" => {
                    apply_table_filter(expr, model, outer_ctx, next_ctx, row_stack, vars, false)?;
                    return Ok(());
                }
                // §2.3/§3: a table expression used *directly* as a CALCULATE
                // modifier (without a FILTER wrapper) — SUMMARIZE/VALUES/ADDCOLUMNS
                // used as a filter, CALCULATETABLE, set ops, GENERATESERIES/CROSSJOIN
                // (no-op when the result has no model lineage). This mirrors FILTER's
                // handling: the resulting rows become the filter, grouped into tuple
                // filters whenever more than one of their columns trace back to the
                // same table.
                "VALUES" | "DISTINCT" | "SUMMARIZE" | "ADDCOLUMNS" | "SELECTCOLUMNS" | "UNION" | "INTERSECT"
                | "EXCEPT" | "CROSSJOIN" | "GENERATESERIES" | "TOPN" | "CALCULATETABLE" | "RELATEDTABLE" => {
                    apply_table_filter(expr, model, outer_ctx, next_ctx, row_stack, vars, false)?;
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

/// §2.3: TREATAS's target columns, grouped by table so a multi-column TREATAS
/// onto the same table becomes ONE tuple filter — the combinations that
/// actually occur in the source table — instead of N independent per-column
/// filters that would also let through combinations that never occurred
/// together in the source (classic cross-join-of-filters bug).
pub(super) fn apply_treatas(
    args: &[Expr],
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
    keep: bool,
) -> Result<(), String> {
    if args.len() < 2 {
        return Ok(());
    }
    let (src_tbl, src_indices, _) = eval_table_expr(&args[0], model, outer_ctx, row_stack, vars)?;
    let src_data = src_tbl.as_ref();
    let mut by_table: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for (col_pos, col_expr) in args[1..].iter().enumerate() {
        if let Expr::TableColRef { table, column } = col_expr {
            if model.get_table(table).is_some() {
                by_table.entry(table.clone()).or_default().push((col_pos, column.clone()));
            }
        }
    }
    for (table, cols) in by_table {
        if cols.len() == 1 {
            let (pos, col) = &cols[0];
            let mut allowed: HashSet<ValueKey> =
                src_indices.iter().filter_map(|&i| src_data.rows[i].get(*pos)).map(|c| c.value_key()).collect();
            if keep {
                if let Some(existing) = outer_ctx.filters.get(&table.to_lowercase()).and_then(|m| m.get(&col.to_lowercase())) {
                    allowed = allowed.intersection(existing).cloned().collect();
                }
            }
            next_ctx.set_column_filter(&table, col, allowed);
        } else {
            let col_names: Vec<String> = cols.iter().map(|(_, c)| c.clone()).collect();
            let mut tuples: HashSet<Vec<ValueKey>> = src_indices
                .iter()
                .map(|&i| cols.iter().map(|(pos, _)| src_data.rows[i][*pos].value_key()).collect())
                .collect();
            if keep {
                let tl = table.to_lowercase();
                let cls: Vec<String> = col_names.iter().map(|c| c.to_lowercase()).collect();
                if let Some(existing) = outer_ctx.tuple_filters.iter().find(|tf| tf.table == tl && tf.columns == cls) {
                    tuples = tuples.intersection(&existing.tuples).cloned().collect();
                }
            }
            next_ctx.set_tuple_filter(&table, &col_names, tuples);
        }
    }
    Ok(())
}

/// §2.3/§3: applies a table expression's resulting rows as a filter —
/// FILTER's own argument, or any other table-producing call used *directly*
/// as a CALCULATE modifier (SUMMARIZE/VALUES/ADDCOLUMNS as a filter,
/// CALCULATETABLE, set ops...). A physical table is filtered through its
/// first (key) column; a synthetic table filters every column that has model
/// lineage, grouped by source table so that 2+ lineage columns from the SAME
/// table become one tuple filter instead of independent per-column filters.
/// `keep`: KEEPFILTERS semantics — intersect with any filter already on the
/// exact same (table, column) or (table, columns) signature instead of
/// replacing it.
pub(super) fn apply_table_filter(
    expr: &Expr,
    model: &DaxModel,
    outer_ctx: &FilterContext,
    next_ctx: &mut FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
    keep: bool,
) -> Result<(), String> {
    let (tbl_eval, kept_indices, _) = eval_table_expr(expr, model, outer_ctx, row_stack, vars)?;
    let tbl = tbl_eval.as_ref();
    let filter_cols: Vec<(usize, String, String)> = match &tbl_eval {
        TableEval::Borrowed(_) => {
            tbl.columns.first().map(|c| vec![(0, tbl.name.clone(), c.clone())]).unwrap_or_default()
        }
        TableEval::Owned(_) => {
            (0..tbl.columns.len()).filter_map(|i| column_lineage(model, tbl, i).map(|(t, c)| (i, t, c))).collect()
        }
    };
    let mut by_table: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for (cidx, t, c) in filter_cols {
        by_table.entry(t).or_default().push((cidx, c));
    }
    for (t, cols) in by_table {
        if cols.len() == 1 {
            let (cidx, c) = &cols[0];
            let mut allowed: HashSet<ValueKey> =
                kept_indices.iter().map(|&i| tbl.rows[i][*cidx].value_key()).collect();
            if keep {
                if let Some(existing) = outer_ctx.filters.get(&t.to_lowercase()).and_then(|m| m.get(&c.to_lowercase())) {
                    allowed = allowed.intersection(existing).cloned().collect();
                }
            }
            next_ctx.set_column_filter(&t, c, allowed);
        } else {
            let col_names: Vec<String> = cols.iter().map(|(_, c)| c.clone()).collect();
            let mut tuples: HashSet<Vec<ValueKey>> = kept_indices
                .iter()
                .map(|&i| cols.iter().map(|(cidx, _)| tbl.rows[i][*cidx].value_key()).collect())
                .collect();
            if keep {
                let tl = t.to_lowercase();
                let cls: Vec<String> = col_names.iter().map(|c| c.to_lowercase()).collect();
                if let Some(existing) = outer_ctx.tuple_filters.iter().find(|tf| tf.table == tl && tf.columns == cls) {
                    tuples = tuples.intersection(&existing.tuples).cloned().collect();
                }
            }
            next_ctx.set_tuple_filter(&t, &col_names, tuples);
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

#[cfg(test)]
mod tests {
    use crate::dax::{
        build_filter_context, DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput,
        NativeFilterInput,
    };
    use serde_json::json;

    fn row(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().unwrap().clone()
    }

    /// A tiny star schema (no global state: built fresh in every test),
    /// mirroring tests/dax-corpus/models/vendas.json closely enough to
    /// hand-check every assertion below. `orphan`: adds one f_Vendas row
    /// whose ProdutoID (9) doesn't exist in d_Produto, to exercise the
    /// blank-member tests.
    fn sales_model(orphan: bool) -> DaxModel {
        let mut f_rows = vec![
            row(json!({"VendaID": 1, "ProdutoID": 1, "ClienteID": 1, "Valor": 25.0})),
            row(json!({"VendaID": 2, "ProdutoID": 1, "ClienteID": 2, "Valor": 50.0})),
            row(json!({"VendaID": 3, "ProdutoID": 2, "ClienteID": 1, "Valor": 36.0})),
            row(json!({"VendaID": 4, "ProdutoID": 2, "ClienteID": 2, "Valor": 60.0})),
            row(json!({"VendaID": 5, "ProdutoID": 2, "ClienteID": 2, "Valor": 24.0})),
            row(json!({"VendaID": 6, "ProdutoID": 4, "ClienteID": 1, "Valor": 300.0})),
            row(json!({"VendaID": 7, "ProdutoID": 1, "ClienteID": 1, "Valor": 20.0})),
        ];
        if orphan {
            // ProdutoID 9 doesn't exist in d_Produto below -> orphan FK.
            f_rows.push(row(json!({"VendaID": 8, "ProdutoID": 9, "ClienteID": 1, "Valor": 99.0})));
        }
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    duckdb_table: None,
                    name: "d_Produto".into(),
                    columns: vec!["ProdutoID".into(), "Produto".into(), "Categoria".into()],
                    rows: vec![
                        row(json!({"ProdutoID": 1, "Produto": "Caneta", "Categoria": "Papelaria"})),
                        row(json!({"ProdutoID": 2, "Produto": "Caderno", "Categoria": "Papelaria"})),
                        row(json!({"ProdutoID": 3, "Produto": "Mouse", "Categoria": "Informatica"})),
                        row(json!({"ProdutoID": 4, "Produto": "Teclado", "Categoria": "Informatica"})),
                    ],
                    column_types: vec![],
                    sort_by_columns: Default::default(),
                },
                DaxTableInput {
                    duckdb_table: None,
                    name: "d_Cliente".into(),
                    columns: vec!["ClienteID".into(), "Cliente".into()],
                    rows: vec![
                        row(json!({"ClienteID": 1, "Cliente": "Ana"})),
                        row(json!({"ClienteID": 2, "Cliente": "Bruno"})),
                        row(json!({"ClienteID": 3, "Cliente": "Carla"})),
                    ],
                    column_types: vec![],
                    sort_by_columns: Default::default(),
                },
                DaxTableInput {
                    duckdb_table: None,
                    name: "f_Vendas".into(),
                    // (ProdutoID, ClienteID): (1,1)x2, (1,2), (2,1), (2,2)x2, (4,1) — a
                    // deliberately non-bijective mix so a per-column AND decomposition
                    // of a 2-column filter differs from the true tuple set.
                    columns: vec!["VendaID".into(), "ProdutoID".into(), "ClienteID".into(), "Valor".into()],
                    rows: f_rows,
                    column_types: vec![],
                    sort_by_columns: Default::default(),
                },
            ],
            relationships: vec![
                DaxRelationshipInput {
                    from_table: "f_Vendas".into(),
                    from_column: "ProdutoID".into(),
                    to_table: "d_Produto".into(),
                    to_column: "ProdutoID".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
                DaxRelationshipInput {
                    from_table: "f_Vendas".into(),
                    from_column: "ClienteID".into(),
                    to_table: "d_Cliente".into(),
                    to_column: "ClienteID".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
            ],
            measures: vec![DaxMeasureInput {
                name: "Total".into(),
                table_name: "f_Vendas".into(),
                formula: "SUM(f_Vendas[Valor])".into(),
            }],
        });
        model
    }

    fn val(model: &DaxModel, expr: &str, filters: &[NativeFilterInput]) -> f64 {
        let ctx = build_filter_context(filters, model);
        model.evaluate_dax(expr, &ctx).unwrap().as_f64().unwrap()
    }

    fn filt(table: &str, column: &str, values: &[&str]) -> NativeFilterInput {
        NativeFilterInput {
            table_id: table.into(),
            column: column.into(),
            values: values.iter().map(|v| json!(v)).collect(),
        }
    }

    #[test]
    fn allselected_table_arg_restores_shadow_not_model_total() {
        let model = sales_model(false);
        let slicer = [filt("d_Produto", "Produto", &["Caneta", "Mouse"])];
        // Caneta(25+50+20)=95, Mouse has no rows here -> shadow total = 95.
        let got = val(
            &model,
            "CALCULATE(CALCULATE([Total], ALLSELECTED(d_Produto)), d_Produto[Produto] = \"Caneta\")",
            &slicer,
        );
        let shadow_total = val(&model, "[Total]", &slicer);
        assert_eq!(got, shadow_total, "ALLSELECTED(table) must restore the visual's shadow filter");
        let model_total = val(&model, "[Total]", &[]);
        assert_ne!(got, model_total, "ALLSELECTED must not behave like ALL/REMOVEFILTERS (model-wide total)");
    }

    #[test]
    fn allselected_column_outside_shadow_clears_fully() {
        let model = sales_model(false);
        // Shadow never touched d_Produto[Produto] at all, so ALLSELECTED on
        // that column must behave like a full clear, not a no-op.
        let got = val(&model, "CALCULATE(CALCULATE([Total], ALLSELECTED(d_Produto[Produto])), d_Produto[Produto] = \"Caneta\")", &[]);
        let total_all = val(&model, "[Total]", &[]);
        assert_eq!(got, total_all);
    }

    #[test]
    fn tuple_filter_restricts_to_exact_combinations_not_cross_join() {
        let model = sales_model(false);
        let expr = "CALCULATE([Total], TREATAS(SUMMARIZE(FILTER(f_Vendas, (f_Vendas[ProdutoID] = 1 && f_Vendas[ClienteID] = 1) || (f_Vendas[ProdutoID] = 2 && f_Vendas[ClienteID] = 2)), f_Vendas[ProdutoID], f_Vendas[ClienteID]), f_Vendas[ProdutoID], f_Vendas[ClienteID]))";
        // Rows with (1,1): VendaID 1,7 (25+20=45); rows with (2,2): VendaID 4,5 (60+24=84). Total 129.
        let got = val(&model, expr, &[]);
        assert_eq!(got, 129.0);
        // A naive per-column AND decomposition (ProdutoID∈{1,2}, ClienteID∈{1,2})
        // would additionally admit VendaID 2 (1,2)=50 and VendaID 3 (2,1)=36,
        // landing on 215 instead — the exact bug this filter exists to avoid.
        assert_ne!(got, 215.0);
    }

    #[test]
    fn keepfilters_intersects_tuple_filters_instead_of_replacing() {
        let model = sales_model(false);
        let expr = "CALCULATE(CALCULATE([Total], KEEPFILTERS(TREATAS(SUMMARIZE(FILTER(f_Vendas, (f_Vendas[ProdutoID] = 1 && f_Vendas[ClienteID] = 1) || (f_Vendas[ProdutoID] = 4 && f_Vendas[ClienteID] = 1)), f_Vendas[ProdutoID], f_Vendas[ClienteID]), f_Vendas[ProdutoID], f_Vendas[ClienteID]))), TREATAS(SUMMARIZE(FILTER(f_Vendas, (f_Vendas[ProdutoID] = 1 && f_Vendas[ClienteID] = 1) || (f_Vendas[ProdutoID] = 2 && f_Vendas[ClienteID] = 2)), f_Vendas[ProdutoID], f_Vendas[ClienteID]), f_Vendas[ProdutoID], f_Vendas[ClienteID]))";
        // Outer tuple set = {(1,1),(2,2)}; inner (without KEEPFILTERS) would be
        // {(1,1),(4,1)}. KEEPFILTERS intersects: only (1,1) survives -> 45.
        let got = val(&model, expr, &[]);
        assert_eq!(got, 45.0);
    }

    #[test]
    fn values_table_includes_blank_member_for_orphan_fk() {
        let model = sales_model(true);
        let n_table = val(&model, "COUNTROWS(d_Produto)", &[]);
        let n_values = val(&model, "COUNTROWS(VALUES(d_Produto))", &[]);
        let n_distinct = val(&model, "COUNTROWS(DISTINCT(d_Produto[Produto]))", &[]);
        assert_eq!(n_table, 4.0, "the physical table never gains a blank row");
        assert_eq!(n_values, 5.0, "VALUES(table) must surface the blank member for the orphan FK");
        assert_eq!(n_distinct, 4.0, "DISTINCT never includes the blank member");
    }

    #[test]
    fn values_blank_member_disappears_under_a_direct_filter() {
        let model = sales_model(true);
        let direct = [filt("d_Produto", "Categoria", &["Papelaria"])];
        let got = val(&model, "COUNTROWS(VALUES(d_Produto))", &direct);
        // Papelaria has 2 physical rows and the blank member can never match
        // a direct equality filter, so it must not be counted here.
        assert_eq!(got, 2.0);
    }
}
