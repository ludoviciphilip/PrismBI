//! Table expressions (physical, synthetic and row-filtering) and synthetic-table column lineage.

use super::*;

pub(super) fn extract_table_name(expr: Option<&Expr>) -> Result<String, String> {
    match expr {
        Some(Expr::Ident(t)) => Ok(t.clone()),
        _ => Err("Esperado nome de tabela".to_string()),
    }
}

pub(super) enum TableEval<'a> {
    Borrowed(&'a DaxTableData),
    Owned(DaxTableData),
}

impl<'a> TableEval<'a> {
    pub(super) fn as_ref(&self) -> &DaxTableData {
        match self {
            TableEval::Borrowed(t) => t,
            TableEval::Owned(t) => t,
        }
    }
}

pub(super) fn eval_table_expr<'a>(
    expr: &Expr,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if let Expr::Call { name, args } = expr {
        let upper = name.to_uppercase();
        if matches!(upper.as_str(), "VALUES" | "DISTINCT" | "ALL" | "ALLSELECTED") {
            if args.len() >= 2 && args.iter().all(|a| matches!(a, Expr::TableColRef { .. })) {
                return eval_multicol_table_fn(&upper, args, model, ctx);
            }
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
                    } else if upper == "ALLSELECTED" {
                        // §3 item 3: restore the visual's shadow filter on this
                        // column, not the model-wide unfiltered state.
                        let shadow = ctx.shadow_ctx();
                        let mut restored = ctx.clone();
                        match shadow.filters.get(&tbl.name.to_lowercase()).and_then(|m| m.get(&column.to_lowercase())) {
                            Some(v) => restored.set_column_filter(&tbl.name, column, v.clone()),
                            None => restored.clear_column(&tbl.name, column),
                        }
                        (model.get_visible_row_indices(table, &restored), restored)
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
                // Blank member: VALUES (never ALL/DISTINCT/ALLSELECTED) over a
                // column of a dimension with an orphan FK surfaces one extra
                // BLANK row, unless this column is itself directly filtered
                // (a direct filter can never match the blank member).
                if upper == "VALUES" {
                    let has_direct_filter = ctx
                        .filters
                        .get(&tbl.name.to_lowercase())
                        .and_then(|m| m.get(&column.to_lowercase()))
                        .is_some_and(|s| !s.is_empty());
                    if !has_direct_filter
                        && model.has_orphan_member(table)
                        && seen.insert(DaxValue::Blank.canonical_key())
                    {
                        values.push(DaxValue::Blank);
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
                    col_types: vec![ColumnType::Text],
                    rows,
                    sort_by_columns: HashMap::new(),
                };
                return Ok((TableEval::Owned(synth), (0..n).collect(), base_ctx));
            }
        }
        if matches!(upper.as_str(), "UNION" | "INTERSECT" | "EXCEPT") && args.len() >= 2 {
            let (t1, i1, c1) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let (t2, i2, _) = eval_table_expr(&args[1], model, ctx, row_stack, vars)?;
            let t1 = t1.as_ref();
            let t2 = t2.as_ref();
            let row_key = |t: &DaxTableData, idx: usize| -> String {
                t.rows[idx].iter().map(|v| v.canonical_key()).collect::<Vec<_>>().join("\u{1}")
            };
            let rows: Vec<Vec<DaxValue>> = match upper.as_str() {
                "UNION" => i1
                    .iter()
                    .map(|&i| t1.rows[i].clone())
                    .chain(i2.iter().map(|&i| t2.rows[i].clone()))
                    .collect(),
                "INTERSECT" => {
                    let keys2: HashSet<String> = i2.iter().map(|&i| row_key(t2, i)).collect();
                    i1.iter()
                        .filter(|&&i| keys2.contains(&row_key(t1, i)))
                        .map(|&i| t1.rows[i].clone())
                        .collect()
                }
                _ => {
                    // EXCEPT
                    let keys2: HashSet<String> = i2.iter().map(|&i| row_key(t2, i)).collect();
                    i1.iter()
                        .filter(|&&i| !keys2.contains(&row_key(t1, i)))
                        .map(|&i| t1.rows[i].clone())
                        .collect()
                }
            };
            let n = rows.len();
            // The result keeps the first table's columns, name and therefore lineage, so
            // `T[c]` references and context transition keep working on it when nested.
            let synth = DaxTableData {
                name: t1.name.clone(),
                columns: t1.columns.clone(),
                col_index: t1.col_index.clone(),
                col_types: t1.col_types.clone(),
                rows,
                sort_by_columns: t1.sort_by_columns.clone(),
            };
            return Ok((TableEval::Owned(synth), (0..n).collect(), c1));
        }
        if upper == "CROSSJOIN" && args.len() >= 2 {
            let (t1, i1, c1) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let (t2, i2, _) = eval_table_expr(&args[1], model, ctx, row_stack, vars)?;
            let t1 = t1.as_ref();
            let t2 = t2.as_ref();
            // Columns keep their lineage as `Table[Column]` (already-qualified ones as-is).
            let qualify = |t: &DaxTableData| -> Vec<String> {
                t.columns
                    .iter()
                    .map(|c| if split_qualified_col(c).is_some() { c.clone() } else { qualified_col_name(&t.name, c) })
                    .collect()
            };
            let mut columns = qualify(t1);
            columns.extend(qualify(t2));
            let mut rows = Vec::with_capacity(i1.len() * i2.len());
            for &a in &i1 {
                for &b in &i2 {
                    let mut row = t1.rows[a].clone();
                    row.extend(t2.rows[b].clone());
                    rows.push(row);
                }
            }
            let n = rows.len();
            let synth = synth_table(format!("CROSSJOIN_{}_{}", t1.name, t2.name), columns, rows);
            return Ok((TableEval::Owned(synth), (0..n).collect(), c1));
        }
        if upper == "SUMMARIZE" && !args.is_empty() {
            return eval_summarize(args, model, ctx, row_stack, vars);
        }
        if upper == "SUMMARIZECOLUMNS" {
            return eval_summarizecolumns(args, model, ctx, row_stack, vars);
        }
        if (upper == "ADDCOLUMNS" || upper == "SELECTCOLUMNS") && !args.is_empty() {
            return eval_add_select_columns(upper == "ADDCOLUMNS", args, model, ctx, row_stack, vars);
        }
        if upper == "CALENDAR" {
            return eval_calendar(args, model, ctx, row_stack, vars);
        }
        if upper == "CALENDARAUTO" {
            return eval_calendarauto(args, model, ctx, row_stack, vars);
        }
        if upper == "OFFSET" {
            return super::window::eval_offset(args, model, ctx, row_stack, vars);
        }
        if upper == "INDEX" {
            return super::window::eval_index(args, model, ctx, row_stack, vars);
        }
        if upper == "WINDOW" {
            return super::window::eval_window(args, model, ctx, row_stack, vars);
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
                    rows.push(vec![DaxValue::number_smart(cur)]);
                    cur += step;
                }
            }
            let mut col_index = HashMap::new();
            col_index.insert("value".to_string(), 0);
            let tbl = DaxTableData {
                name: "GENERATESERIES".to_string(),
                columns: vec!["Value".to_string()],
                col_index,
                col_types: vec![ColumnType::Decimal],
                rows,
                sort_by_columns: HashMap::new(),
            };
            let indices: Vec<usize> = (0..tbl.rows.len()).collect();
            return Ok((TableEval::Owned(tbl), indices, ctx.clone()));
        }
    }
    eval_base_table_expr(expr, model, ctx, row_stack, vars)
}

/// Builds an in-memory (synthetic) table, indexing column names case-insensitively.
pub(super) fn synth_table(name: String, columns: Vec<String>, rows: Vec<Vec<DaxValue>>) -> DaxTableData {
    let col_index = columns.iter().enumerate().map(|(i, c)| (c.to_lowercase(), i)).collect();
    let col_types = vec![ColumnType::Text; columns.len()];
    DaxTableData { name, columns, col_index, col_types, rows, sort_by_columns: HashMap::new() }
}

/// Column naming convention for synthetic tables whose columns come from *different* model
/// tables (SUMMARIZE group-by columns, CROSSJOIN): the column is stored as `Table[Column]`,
/// which keeps its lineage so `Table[Column]` lookups and context transition still work.
pub(super) fn qualified_col_name(table: &str, column: &str) -> String {
    format!("{}[{}]", table, column)
}

pub(super) fn split_qualified_col(name: &str) -> Option<(&str, &str)> {
    let inner = name.strip_suffix(']')?;
    let open = inner.find('[')?;
    Some((&inner[..open], &inner[open + 1..]))
}

/// Index of `table[column]` within a row-context table: a table named `table` matches the
/// bare column name; any table may also carry it as a qualified `table[column]` column.
pub(super) fn row_col_idx(tbl: &DaxTableData, table: &str, column: &str) -> Option<usize> {
    if tbl.name.eq_ignore_ascii_case(table) {
        if let Some(idx) = tbl.get_col_idx(column) {
            return Some(idx);
        }
    }
    tbl.get_col_idx(&qualified_col_name(table, column))
}

/// Index of a bare `[column]` reference: exact name first, then a qualified `T[column]`.
pub(super) fn bare_col_idx(tbl: &DaxTableData, column: &str) -> Option<usize> {
    tbl.get_col_idx(column).or_else(|| {
        tbl.columns.iter().position(|c| {
            split_qualified_col(c).is_some_and(|(_, bare)| bare.eq_ignore_ascii_case(column))
        })
    })
}

/// The model `(table, column)` a row-context column originates from, if any. Extension
/// columns (ADDCOLUMNS/SUMMARIZE "name" pairs, SELECTCOLUMNS) have no lineage.
pub(super) fn column_lineage(model: &DaxModel, tbl: &DaxTableData, idx: usize) -> Option<(String, String)> {
    let col = tbl.columns.get(idx)?;
    let (t, c) = split_qualified_col(col).unwrap_or((tbl.name.as_str(), col.as_str()));
    let mt = model.get_table(t)?;
    let cidx = mt.get_col_idx(c)?;
    Some((mt.name.clone(), mt.columns[cidx].clone()))
}

/// Context transition for one row, narrowing only columns that have model lineage.
pub(super) fn apply_lineage_transition(model: &DaxModel, ctx: &FilterContext, rc: &RowContext<'_>) -> FilterContext {
    let mut next = ctx.clone();
    for idx in 0..rc.table.columns.len() {
        if let Some((t, c)) = column_lineage(model, rc.table, idx) {
            let mut set = HashSet::new();
            set.insert(rc.table.rows[rc.row_idx][idx].value_key());
            next.set_column_filter(&t, &c, set);
        }
    }
    next
}

/// Parses trailing `"name", expression` pairs (SUMMARIZE / ADDCOLUMNS / SELECTCOLUMNS).
pub(super) fn parse_name_expr_pairs<'e>(fname: &str, pairs: &'e [Expr]) -> Result<Vec<(String, &'e Expr)>, String> {
    if pairs.len() % 2 != 0 {
        return Err(format!("{}: esperados pares (\"nome\", expressão)", fname));
    }
    pairs
        .chunks(2)
        .map(|pair| match &pair[0] {
            Expr::Text(n) => Ok((n.clone(), &pair[1])),
            _ => Err(format!("{}: nome de coluna deve ser texto entre aspas", fname)),
        })
        .collect()
}

/// SUMMARIZE(table, groupBy_col..., ["name", expr]...): distinct combinations of the group-by
/// columns over the visible rows of `table` (group-by columns may live in tables reached via
/// many-to-one relationships). Extension columns are evaluated once per group, in the filter
/// context of that group (context transition over the group-by values).
pub(super) fn eval_summarize<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let (base, base_indices, base_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
    let base_t = base.as_ref();
    let mut group_cols: Vec<(String, String)> = Vec::new();
    let mut pos = 1;
    while let Some(Expr::TableColRef { table, column }) = args.get(pos) {
        group_cols.push((table.clone(), column.clone()));
        pos += 1;
    }
    let extensions = parse_name_expr_pairs("SUMMARIZE", &args[pos..])?;

    // Per group-by column: Some(idx) when it's a column of the base table itself, None when it
    // must be fetched through relationships (RELATED-style lookup).
    let mut sources = Vec::with_capacity(group_cols.len());
    for (t, c) in &group_cols {
        let direct = row_col_idx(base_t, t, c);
        if direct.is_none() && model.get_table(t).and_then(|mt| mt.get_col_idx(c)).is_none() {
            return Err(format!("SUMMARIZE: coluna '{}[{}]' não encontrada", t, c));
        }
        sources.push(direct);
    }

    let mut seen = HashSet::new();
    let mut group_rows: Vec<Vec<DaxValue>> = Vec::new();
    for &r_idx in &base_indices {
        let rc = RowContext { table: base_t, row_idx: r_idx };
        let row: Vec<DaxValue> = group_cols
            .iter()
            .zip(&sources)
            .map(|((t, c), src)| match src {
                Some(cidx) => base_t.rows[r_idx][*cidx].clone(),
                None => resolve_related(model, &rc, t, c).unwrap_or(DaxValue::Blank),
            })
            .collect();
        let key = row.iter().map(|v| v.canonical_key()).collect::<Vec<_>>().join("\u{1}");
        if seen.insert(key) {
            group_rows.push(row);
        }
    }

    let mut columns: Vec<String> = group_cols.iter().map(|(t, c)| qualified_col_name(t, c)).collect();
    let mut groups = synth_table("SUMMARIZE".to_string(), columns.clone(), group_rows);
    if !extensions.is_empty() {
        let mut ext_values: Vec<Vec<DaxValue>> = Vec::with_capacity(groups.rows.len());
        for g_idx in 0..groups.rows.len() {
            let rc = RowContext { table: &groups, row_idx: g_idx };
            let group_ctx = apply_lineage_transition(model, &base_ctx, &rc);
            let mut stack = row_stack.to_vec();
            stack.push(rc);
            let mut vals = Vec::with_capacity(extensions.len());
            for (_, e) in &extensions {
                vals.push(evaluate_ast(e, model, &group_ctx, &stack, vars)?);
            }
            ext_values.push(vals);
        }
        columns.extend(extensions.iter().map(|(n, _)| n.clone()));
        let rows = groups.rows.into_iter().zip(ext_values).map(|(mut g, e)| {
            g.extend(e);
            g
        });
        groups = synth_table("SUMMARIZE".to_string(), columns, rows.collect());
    }
    let n = groups.rows.len();
    Ok((TableEval::Owned(groups), (0..n).collect(), base_ctx))
}

/// ADDCOLUMNS(table, "name", expr, ...) keeps every row/column of `table` and appends the
/// computed columns; SELECTCOLUMNS(table, "name", expr, ...) keeps only the computed columns
/// (no de-duplication). Each expression is evaluated in the row context of the source row.
pub(super) fn eval_add_select_columns<'a>(
    keep_base: bool,
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let fname = if keep_base { "ADDCOLUMNS" } else { "SELECTCOLUMNS" };
    let (base, base_indices, base_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
    let base_t = base.as_ref();
    let extensions = parse_name_expr_pairs(fname, &args[1..])?;

    let mut columns = if keep_base { base_t.columns.clone() } else { Vec::new() };
    for (n, _) in &extensions {
        if columns.iter().any(|c| c.eq_ignore_ascii_case(n)) {
            return Err(format!("{}: coluna '{}' duplicada", fname, n));
        }
        columns.push(n.clone());
    }
    let mut rows = Vec::with_capacity(base_indices.len());
    for &r_idx in &base_indices {
        let mut row = if keep_base { base_t.rows[r_idx].clone() } else { Vec::new() };
        let mut stack = row_stack.to_vec();
        stack.push(RowContext { table: base_t, row_idx: r_idx });
        for (_, e) in &extensions {
            row.push(evaluate_ast(e, model, &base_ctx, &stack, vars)?);
        }
        rows.push(row);
    }
    // ADDCOLUMNS keeps the base name so base columns keep their lineage.
    let name = if keep_base { base_t.name.clone() } else { "SELECTCOLUMNS".to_string() };
    let n = rows.len();
    Ok((TableEval::Owned(synth_table(name, columns, rows)), (0..n).collect(), base_ctx))
}

/// Physical tables and the row-filtering table functions (FILTER/TOPN/CALCULATETABLE/
/// RELATEDTABLE). Physical tables are borrowed from the model; FILTER/TOPN keep whatever
/// table their input evaluated to (borrowed or synthetic) and only narrow its row indices,
/// so they compose with every other table expression.
pub(super) fn eval_base_table_expr<'a>(
    expr: &Expr,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let model_table = |tname: &str| {
        model
            .get_table(tname)
            .ok_or_else(|| format!("Tabela '{}' não encontrada", tname))
    };
    match expr {
        Expr::Ident(tname) => {
            let tbl = model_table(tname)?;
            let indices = model.get_visible_row_indices(tname, ctx);
            Ok((TableEval::Borrowed(tbl), indices, ctx.clone()))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("RELATEDTABLE") => {
            let tname = extract_table_name(args.first())?;
            let tbl = model_table(&tname)?;
            let Some(rc) = row_stack.last() else {
                let indices = model.get_visible_row_indices(&tname, ctx);
                return Ok((TableEval::Borrowed(tbl), indices, ctx.clone()));
            };
            // Find the active relationship where the current row's table is the "one"
            // side (rel.to_table) and the target is the "many" side (rel.from_table).
            let rel = model.relationships.iter().find(|r| {
                r.is_active
                    && r.to_table.eq_ignore_ascii_case(&rc.table.name)
                    && r.from_table.eq_ignore_ascii_case(&tname)
            });
            let indices = if let Some(rel) = rel {
                match (rc.table.get_col_idx(&rel.to_column), tbl.get_col_idx(&rel.from_column)) {
                    (Some(to_cidx), Some(from_cidx)) => {
                        let key = rc.table.rows[rc.row_idx][to_cidx].canonical_key();
                        model
                            .get_visible_row_indices(&tname, ctx)
                            .into_iter()
                            .filter(|&i| tbl.rows[i][from_cidx].canonical_key() == key)
                            .collect()
                    }
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            Ok((TableEval::Borrowed(tbl), indices, ctx.clone()))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("CALCULATETABLE") => {
            if args.is_empty() {
                return Err("CALCULATETABLE requer ao menos 1 argumento".to_string());
            }
            let mut next_ctx = apply_context_transition(model, ctx, row_stack);
            for filter_expr in &args[1..] {
                apply_calculate_modifier(filter_expr, model, ctx, &mut next_ctx, row_stack, vars)?;
            }
            eval_table_expr(&args[0], model, &next_ctx, &[], vars)
        }
        // ALL/VALUES/DISTINCT over a single column are materialized in `eval_table_expr`;
        // here only the whole-table forms remain.
        Expr::Call { name, args } if name.eq_ignore_ascii_case("ALL") || name.eq_ignore_ascii_case("ALLSELECTED") => {
            let tname = extract_table_name(args.first())?;
            let tbl = model_table(&tname)?;
            if name.eq_ignore_ascii_case("ALLSELECTED") {
                let shadow = ctx.shadow_ctx();
                let mut restored = ctx.clone();
                let tl = tbl.name.to_lowercase();
                match shadow.filters.get(&tl) {
                    Some(cols) => {
                        restored.filters.insert(tl.clone(), cols.clone());
                    }
                    None => {
                        restored.filters.remove(&tl);
                    }
                }
                restored.tuple_filters.retain(|tf| tf.table != tl);
                restored
                    .tuple_filters
                    .extend(shadow.tuple_filters.iter().filter(|tf| tf.table == tl).cloned());
                return Ok((TableEval::Borrowed(tbl), (0..tbl.rows.len()).collect(), restored));
            }
            let mut cleared_ctx = ctx.clone();
            cleared_ctx.clear_table(&tbl.name);
            for rel in &model.relationships {
                if rel.is_active && rel.from_table.eq_ignore_ascii_case(&tbl.name) {
                    cleared_ctx.clear_table(&rel.to_table);
                }
            }
            Ok((TableEval::Borrowed(tbl), (0..tbl.rows.len()).collect(), cleared_ctx))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("VALUES") || name.eq_ignore_ascii_case("DISTINCT") => {
            let tname = extract_table_name(args.first())?;
            let tbl = model_table(&tname)?;
            let indices = model.get_visible_row_indices(&tname, ctx);
            // Blank member (VALUES only, see `has_orphan_member`): append one
            // synthetic all-BLANK row, unless the table itself is directly
            // filtered (a direct filter on any of its columns can never match it).
            if name.eq_ignore_ascii_case("VALUES") {
                let has_direct_filter = ctx
                    .filters
                    .get(&tbl.name.to_lowercase())
                    .is_some_and(|m| m.values().any(|s| !s.is_empty()));
                if !has_direct_filter && model.has_orphan_member(&tbl.name) {
                    let mut rows: Vec<Vec<DaxValue>> = indices.iter().map(|&i| tbl.rows[i].clone()).collect();
                    rows.push(vec![DaxValue::Blank; tbl.columns.len()]);
                    let n = rows.len();
                    let synth = DaxTableData {
                        name: tbl.name.clone(),
                        columns: tbl.columns.clone(),
                        col_index: tbl.col_index.clone(),
                        col_types: tbl.col_types.clone(),
                        rows,
                        sort_by_columns: tbl.sort_by_columns.clone(),
                    };
                    return Ok((TableEval::Owned(synth), (0..n).collect(), ctx.clone()));
                }
            }
            Ok((TableEval::Borrowed(tbl), indices, ctx.clone()))
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
            let Some(ord_expr) = args.get(2) else {
                return Ok((tbl, indices.into_iter().take(top_n).collect(), base_ctx));
            };
            let mut scored: Vec<(usize, DaxValue)> = Vec::with_capacity(indices.len());
            for r_idx in indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: tbl.as_ref(), row_idx: r_idx });
                let val = evaluate_ast(ord_expr, model, &base_ctx, &next_stack, vars)?;
                scored.push((r_idx, val));
            }
            scored.sort_by(|a, b| {
                let ord = compare_dax_values(&a.1, &b.1).cmp(&0);
                if ascending { ord } else { ord.reverse() }
            });
            // TOPN includes ties at the N-th boundary value, so the result can have
            // more than N rows (per the official TOPN semantics).
            let kept: Vec<usize> = if scored.len() > top_n {
                let boundary = scored[top_n - 1].1.clone();
                scored
                    .into_iter()
                    .enumerate()
                    .filter(|(i, (_, v))| *i < top_n || v.dax_equals(&boundary))
                    .map(|(_, (idx, _))| idx)
                    .collect()
            } else {
                scored.into_iter().map(|(i, _)| i).collect()
            };
            Ok((tbl, kept, base_ctx))
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("FILTER") => {
            if args.len() < 2 {
                return Err("FILTER requer (tabela, condição)".to_string());
            }
            let (tbl, indices, base_ctx) = eval_table_expr(&args[0], model, ctx, row_stack, vars)?;
            let mut kept = Vec::new();
            for r_idx in indices {
                let mut next_stack = row_stack.to_vec();
                next_stack.push(RowContext { table: tbl.as_ref(), row_idx: r_idx });
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

/// Multi-column `VALUES`/`DISTINCT`/`ALL`/`ALLSELECTED` over columns of the same model table.
fn eval_multicol_table_fn<'a>(
    upper: &str,
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let mut cols: Vec<String> = Vec::with_capacity(args.len());
    let mut table_name: Option<String> = None;
    for a in args {
        let Expr::TableColRef { table, column } = a else {
            return Err(format!("{}: esperado Tabela[Coluna]", upper));
        };
        match &table_name {
            None => table_name = Some(table.clone()),
            Some(first) if !first.eq_ignore_ascii_case(table) => {
                return Err(format!("{}: todas as colunas devem ser da mesma tabela", upper));
            }
            _ => {}
        }
        cols.push(column.clone());
    }
    let table_name = table_name.unwrap_or_default();
    let tbl = model
        .get_table(&table_name)
        .ok_or_else(|| format!("Tabela '{}' não encontrada", table_name))?;
    let mut cidxs = Vec::with_capacity(cols.len());
    let mut col_types = Vec::with_capacity(cols.len());
    for c in &cols {
        let idx = tbl
            .get_col_idx(c)
            .ok_or_else(|| format!("Coluna '{}[{}]' não encontrada", table_name, c))?;
        cidxs.push(idx);
        col_types.push(tbl.col_types.get(idx).copied().unwrap_or(ColumnType::Text));
    }

    let (source_indices, base_ctx): (Vec<usize>, FilterContext) =
        if upper == "VALUES" || upper == "DISTINCT" {
            (model.get_visible_row_indices(&tbl.name, ctx), ctx.clone())
        } else if upper == "ALLSELECTED" {
            let shadow = ctx.shadow_ctx();
            let mut restored = ctx.clone();
            let tl = tbl.name.to_lowercase();
            for c in &cols {
                match shadow.filters.get(&tl).and_then(|m| m.get(&c.to_lowercase())) {
                    Some(v) => restored.set_column_filter(&tbl.name, c, v.clone()),
                    None => restored.clear_column(&tbl.name, c),
                }
            }
            (model.get_visible_row_indices(&tbl.name, &restored), restored)
        } else {
            let mut cleared = ctx.clone();
            for c in &cols {
                cleared.clear_column(&tbl.name, c);
            }
            ((0..tbl.rows.len()).collect(), cleared)
        };

    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for r_idx in source_indices {
        let vals: Vec<DaxValue> = cidxs.iter().map(|&ci| tbl.rows[r_idx][ci].clone()).collect();
        let key = vals
            .iter()
            .map(|v| v.canonical_key())
            .collect::<Vec<_>>()
            .join("\u{1}");
        if seen.insert(key) {
            rows.push(vals);
        }
    }

    let qual_cols: Vec<String> = cols
        .iter()
        .map(|c| qualified_col_name(&tbl.name, c))
        .collect();
    let col_index = qual_cols
        .iter()
        .enumerate()
        .map(|(i, c)| (c.to_lowercase(), i))
        .collect();
    let n = rows.len();
    let synth = DaxTableData {
        name: tbl.name.clone(),
        columns: qual_cols,
        col_index,
        col_types,
        rows,
        sort_by_columns: HashMap::new(),
    };
    Ok((TableEval::Owned(synth), (0..n).collect(), base_ctx))
}

fn build_calendar_range(name: &str, range: Option<(i64, i64)>) -> Result<DaxTableData, String> {
    let mut col_index = HashMap::new();
    col_index.insert("date".to_string(), 0);
    let rows = match range {
        None => Vec::new(),
        Some((start, end)) => {
            if start > end {
                return Err(format!("{}: data inicial maior que data final", name));
            }
            let len = (end - start + 1).clamp(0, 100_000) as usize;
            (0..len)
                .map(|offset| vec![DaxValue::DateTime((start + offset as i64) as f64)])
                .collect()
        }
    };
    Ok(DaxTableData {
        name: name.to_string(),
        columns: vec!["Date".to_string()],
        col_index,
        col_types: vec![ColumnType::Date],
        rows,
        sort_by_columns: HashMap::new(),
    })
}

/// `CALENDAR(start_date, end_date)`
fn eval_calendar<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if args.len() < 2 {
        return Err("CALENDAR requer (start_date, end_date)".to_string());
    }
    let start_s = evaluate_ast(&args[0], model, ctx, row_stack, vars)?.as_date_serial();
    let end_s = evaluate_ast(&args[1], model, ctx, row_stack, vars)?.as_date_serial();
    let range = match (start_s, end_s) {
        (Some(s), Some(e)) if s.is_finite() && e.is_finite() => {
            Some((s.floor() as i64, e.floor() as i64))
        }
        _ => None,
    };
    let tbl = build_calendar_range("CALENDAR", range)?;
    let n = tbl.rows.len();
    Ok((TableEval::Owned(tbl), (0..n).collect(), ctx.clone()))
}

/// `CALENDARAUTO([fiscal_year_end_month])`
fn eval_calendarauto<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let fm: u32 = match args.first() {
        None | Some(Expr::Blank) => 12,
        Some(e) => {
            let v = evaluate_ast(e, model, ctx, row_stack, vars)?;
            if v.is_blank() {
                12
            } else {
                let n = v.as_f64().unwrap_or(0.0).round() as i64;
                if !(1..=12).contains(&n) {
                    return Err("CALENDARAUTO: mês fiscal deve estar entre 1 e 12".to_string());
                }
                n as u32
            }
        }
    };

    let mut min_day: Option<i64> = None;
    let mut max_day: Option<i64> = None;
    for tbl in model.tables.values() {
        for (cidx, cty) in tbl.col_types.iter().enumerate() {
            if *cty != ColumnType::Date {
                continue;
            }
            for row in &tbl.rows {
                if let Some(s) = row[cidx].as_date_serial().filter(|x| x.is_finite()) {
                    let d = s.floor() as i64;
                    min_day = Some(min_day.map_or(d, |m| m.min(d)));
                    max_day = Some(max_day.map_or(d, |m| m.max(d)));
                }
            }
        }
    }

    let range = match (min_day, max_day) {
        (Some(mn), Some(mx)) => {
            let (min_y, min_m, _) = functions::serial_to_civil(mn as f64);
            let (max_y, max_m, _) = functions::serial_to_civil(mx as f64);
            if fm == 12 {
                let start = functions::civil_to_serial(min_y, 1, 1) as i64;
                let end = functions::civil_to_serial(max_y, 12, 31) as i64;
                Some((start, end))
            } else {
                let start_y = if min_m <= fm { min_y - 1 } else { min_y };
                let start = functions::civil_to_serial(start_y, fm + 1, 1) as i64;
                let end_y = if max_m <= fm { max_y } else { max_y + 1 };
                let end = (functions::civil_to_serial(end_y, fm + 1, 1) as i64) - 1;
                Some((start, end))
            }
        }
        _ => None,
    };

    let tbl = build_calendar_range("CALENDARAUTO", range)?;
    let n = tbl.rows.len();
    Ok((TableEval::Owned(tbl), (0..n).collect(), ctx.clone()))
}

/// `SUMMARIZECOLUMNS(groupBy_col..., [filterTable...], ["name", expr]...)`
fn eval_summarizecolumns<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    let mut g_end = 0;
    let mut group_cols: Vec<(String, String)> = Vec::new();
    while let Some(Expr::TableColRef { table, column }) = args.get(g_end) {
        group_cols.push((table.clone(), column.clone()));
        g_end += 1;
    }

    // Trailing extension pairs `("name", expr)`.
    let ext_start = (g_end..=args.len())
        .find(|&i| {
            (args.len() - i) % 2 == 0
                && (i..args.len())
                    .step_by(2)
                    .all(|k| matches!(args[k], Expr::Text(_)))
        })
        .unwrap_or(args.len());

    let filter_args = &args[g_end..ext_start];
    let extensions = parse_name_expr_pairs("SUMMARIZECOLUMNS", &args[ext_start..])?;

    let mut eval_ctx = ctx.clone();
    for f in filter_args {
        apply_calculate_modifier(f, model, ctx, &mut eval_ctx, row_stack, vars)?;
    }

    if group_cols.is_empty() {
        let col_names: Vec<String> = extensions.iter().map(|(n, _)| n.clone()).collect();
        if extensions.is_empty() {
            return Ok((
                TableEval::Owned(synth_table("SUMMARIZECOLUMNS".into(), col_names, Vec::new())),
                Vec::new(),
                eval_ctx,
            ));
        }
        let mut vals = Vec::with_capacity(extensions.len());
        for (_, e) in &extensions {
            vals.push(evaluate_ast(e, model, &eval_ctx, row_stack, vars)?);
        }
        let rows = if vals.iter().all(DaxValue::is_blank) {
            Vec::new()
        } else {
            vec![vals]
        };
        let n = rows.len();
        return Ok((
            TableEval::Owned(synth_table("SUMMARIZECOLUMNS".into(), col_names, rows)),
            (0..n).collect(),
            eval_ctx,
        ));
    }

    // Group requested columns by source table to form distinct visible combinations per table,
    // then take the Cartesian product across distinct tables mapped back to original column order.
    let mut table_order: Vec<String> = Vec::new();
    let mut by_table: HashMap<String, Vec<(usize, usize)>> = HashMap::new(); // table_lower -> [(orig_pos, tbl_cidx)]
    let mut group_types = Vec::with_capacity(group_cols.len());
    for (pos, (t, c)) in group_cols.iter().enumerate() {
        let mt = model
            .get_table(t)
            .ok_or_else(|| format!("SUMMARIZECOLUMNS: tabela '{}' não encontrada", t))?;
        let cidx = mt
            .get_col_idx(c)
            .ok_or_else(|| format!("SUMMARIZECOLUMNS: coluna '{}[{}]' não encontrada", t, c))?;
        group_types.push(mt.col_types.get(cidx).copied().unwrap_or(ColumnType::Text));
        let tl = mt.name.to_lowercase();
        if !by_table.contains_key(&tl) {
            table_order.push(mt.name.clone());
        }
        by_table.entry(tl).or_default().push((pos, cidx));
    }

    let mut per_table_tuples: Vec<(Vec<usize>, Vec<Vec<DaxValue>>)> =
        Vec::with_capacity(table_order.len());
    for tname in &table_order {
        let tbl = model.get_table(tname).unwrap();
        let cols_spec = &by_table[&tname.to_lowercase()];
        let orig_positions: Vec<usize> = cols_spec.iter().map(|&(p, _)| p).collect();
        let cidxs: Vec<usize> = cols_spec.iter().map(|&(_, ci)| ci).collect();
        let mut seen = HashSet::new();
        let mut tuples = Vec::new();
        for r_idx in model.get_visible_row_indices(tname, &eval_ctx) {
            let tup: Vec<DaxValue> = cidxs.iter().map(|&ci| tbl.rows[r_idx][ci].clone()).collect();
            let key = tup
                .iter()
                .map(|v| v.canonical_key())
                .collect::<Vec<_>>()
                .join("\u{1}");
            if seen.insert(key) {
                tuples.push(tup);
            }
        }
        per_table_tuples.push((orig_positions, tuples));
    }

    let max_combos = 100_000usize;
    let mut combos: Vec<Vec<DaxValue>> = vec![vec![DaxValue::Blank; group_cols.len()]];
    for (orig_positions, tuples) in &per_table_tuples {
        if tuples.is_empty() {
            combos.clear();
            break;
        }
        let mut next = Vec::with_capacity((combos.len() * tuples.len()).min(max_combos));
        'outer: for base_row in &combos {
            for tup in tuples {
                if next.len() >= max_combos {
                    break 'outer;
                }
                let mut r = base_row.clone();
                for (&pos, val) in orig_positions.iter().zip(tup.iter()) {
                    r[pos] = val.clone();
                }
                next.push(r);
            }
        }
        combos = next;
    }

    let mut columns: Vec<String> = group_cols
        .iter()
        .map(|(t, c)| qualified_col_name(t, c))
        .collect();
    let mut groups = DaxTableData {
        name: "SUMMARIZECOLUMNS".to_string(),
        columns: columns.clone(),
        col_index: columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.to_lowercase(), i))
            .collect(),
        col_types: group_types.clone(),
        rows: combos,
        sort_by_columns: HashMap::new(),
    };

    if !extensions.is_empty() {
        let mut kept_rows = Vec::with_capacity(groups.rows.len());
        for g_idx in 0..groups.rows.len() {
            let rc = RowContext {
                table: &groups,
                row_idx: g_idx,
            };
            let group_ctx = apply_lineage_transition(model, &eval_ctx, &rc);
            let mut stack = row_stack.to_vec();
            stack.push(rc);
            let mut ext_vals = Vec::with_capacity(extensions.len());
            for (_, e) in &extensions {
                ext_vals.push(evaluate_ast(e, model, &group_ctx, &stack, vars)?);
            }
            if ext_vals.iter().any(|v| !v.is_blank()) {
                let mut full_row = groups.rows[g_idx].clone();
                full_row.extend(ext_vals);
                kept_rows.push(full_row);
            }
        }
        columns.extend(extensions.iter().map(|(n, _)| n.clone()));
        group_types.extend(vec![ColumnType::Decimal; extensions.len()]);
        groups = DaxTableData {
            name: "SUMMARIZECOLUMNS".to_string(),
            col_index: columns
                .iter()
                .enumerate()
                .map(|(i, c)| (c.to_lowercase(), i))
                .collect(),
            columns,
            col_types: group_types,
            rows: kept_rows,
            sort_by_columns: HashMap::new(),
        };
    }

    let n = groups.rows.len();
    Ok((TableEval::Owned(groups), (0..n).collect(), eval_ctx))
}
