//! Window functions in Rust: OFFSET, INDEX, WINDOW (table-valued), RANK, ROWNUMBER (scalar).
//!
//! Clause parsing (`ORDERBY`, `PARTITIONBY`, `MATCHBY`, `BLANKS`, `REL`/`ABS`, `SKIP`/`DENSE`)
//! follows DAX semantics cleanly on top of `eval_table_expr` and `DaxTableData` lineage.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlanksMode {
    Default,
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelAbs {
    Rel,
    Abs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankTies {
    Skip,
    Dense,
}

struct OrderKeySpec<'a> {
    expr: &'a Expr,
    asc: bool,
}

struct WindowClauses<'a> {
    relation: Option<&'a Expr>,
    order_by: Vec<OrderKeySpec<'a>>,
    blanks: BlanksMode,
    partition_by: Vec<&'a Expr>,
    match_by: Vec<&'a Expr>,
}

fn with_rows_like(tbl: &DaxTableData, rows: Vec<Vec<DaxValue>>) -> DaxTableData {
    DaxTableData {
        name: tbl.name.clone(),
        columns: tbl.columns.clone(),
        col_index: tbl.col_index.clone(),
        col_types: tbl.col_types.clone(),
        rows,
        sort_by_columns: tbl.sort_by_columns.clone(),
    }
}

fn parse_orderby_clause(expr: &Expr) -> Result<Vec<OrderKeySpec<'_>>, String> {
    let Expr::Call { name, args } = expr else {
        return Err("Esperado ORDERBY(...)".to_string());
    };
    if !name.eq_ignore_ascii_case("ORDERBY") {
        return Err(format!("Esperado ORDERBY(...), encontrado {}", name));
    }
    let mut specs = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let col_expr = &args[i];
        let mut asc = true;
        if let Some(Expr::Ident(dir)) = args.get(i + 1) {
            if dir.eq_ignore_ascii_case("ASC") {
                asc = true;
                i += 2;
            } else if dir.eq_ignore_ascii_case("DESC") {
                asc = false;
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
        specs.push(OrderKeySpec { expr: col_expr, asc });
    }
    Ok(specs)
}

fn parse_col_clause<'a>(expr: &'a Expr, expected_name: &str) -> Result<Vec<&'a Expr>, String> {
    let Expr::Call { name, args } = expr else {
        return Err(format!("Esperado {}(...)", expected_name));
    };
    if !name.eq_ignore_ascii_case(expected_name) {
        return Err(format!("Esperado {}(...), encontrado {}", expected_name, name));
    }
    Ok(args.iter().collect())
}

fn parse_blanks_token(expr: &Expr) -> Option<BlanksMode> {
    match expr {
        Expr::Blank => Some(BlanksMode::Default),
        Expr::Ident(s) => {
            if s.eq_ignore_ascii_case("DEFAULT") || s.eq_ignore_ascii_case("KEEP") {
                Some(BlanksMode::Default)
            } else if s.eq_ignore_ascii_case("FIRST") {
                Some(BlanksMode::First)
            } else if s.eq_ignore_ascii_case("LAST") {
                Some(BlanksMode::Last)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parses `[relation], [orderBy], [blanks], [partitionBy], [matchBy]` regardless of omitted
/// intermediate placeholders (`ORDERBY(...)`, `PARTITIONBY(...)`, `MATCHBY(...)`, `BLANKS`).
fn parse_window_clauses<'a>(args: &'a [Expr]) -> Result<WindowClauses<'a>, String> {
    let mut out = WindowClauses {
        relation: None,
        order_by: Vec::new(),
        blanks: BlanksMode::Default,
        partition_by: Vec::new(),
        match_by: Vec::new(),
    };
    if args.is_empty() {
        return Ok(out);
    }

    let mut idx = 0;
    let first_is_clause = matches!(
        &args[0],
        Expr::Call { name, .. }
            if name.eq_ignore_ascii_case("ORDERBY")
                || name.eq_ignore_ascii_case("PARTITIONBY")
                || name.eq_ignore_ascii_case("MATCHBY")
    ) || parse_blanks_token(&args[0]).is_some();

    if !first_is_clause {
        if !matches!(args[0], Expr::Blank) {
            out.relation = Some(&args[0]);
        }
        idx = 1;
    }

    for arg in &args[idx..] {
        if matches!(arg, Expr::Blank) {
            continue;
        }
        if let Expr::Call { name, .. } = arg {
            if name.eq_ignore_ascii_case("ORDERBY") {
                out.order_by = parse_orderby_clause(arg)?;
                continue;
            }
            if name.eq_ignore_ascii_case("PARTITIONBY") {
                out.partition_by = parse_col_clause(arg, "PARTITIONBY")?;
                continue;
            }
            if name.eq_ignore_ascii_case("MATCHBY") {
                out.match_by = parse_col_clause(arg, "MATCHBY")?;
                continue;
            }
        }
        if let Some(bm) = parse_blanks_token(arg) {
            out.blanks = bm;
            continue;
        }
        return Err("Argumento inválido em função de janela".to_string());
    }
    Ok(out)
}

fn find_first_col_ref(expr: &Expr) -> Option<(String, String)> {
    match expr {
        Expr::TableColRef { table, column } => Some((table.clone(), column.clone())),
        Expr::Call { args, .. } => args.iter().find_map(find_first_col_ref),
        Expr::Binary { left, right, .. } => {
            find_first_col_ref(left).or_else(|| find_first_col_ref(right))
        }
        Expr::Unary { expr, .. } | Expr::InList { expr, .. } => find_first_col_ref(expr),
        _ => None,
    }
}

/// Evaluates `relation` or synthesizes an implicit 1-column `ALLSELECTED(T[C])` relation when omitted.
fn eval_window_relation<'a>(
    clauses: &WindowClauses<'a>,
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if let Some(rel_expr) = clauses.relation {
        return eval_table_expr(rel_expr, model, ctx, row_stack, vars);
    }
    let col_ref = clauses
        .order_by
        .iter()
        .find_map(|o| find_first_col_ref(o.expr))
        .or_else(|| clauses.partition_by.iter().find_map(|p| find_first_col_ref(p)))
        .or_else(|| clauses.match_by.iter().find_map(|m| find_first_col_ref(m)))
        .ok_or_else(|| "Função de janela requer <relation> ou referência Tabela[Coluna]".to_string())?;
    let synthetic_call = Expr::Call {
        name: "ALLSELECTED".to_string(),
        args: vec![Expr::TableColRef {
            table: col_ref.0,
            column: col_ref.1,
        }],
    };
    eval_table_expr(&synthetic_call, model, ctx, row_stack, vars)
}

fn resolve_expr_in_row(
    expr: &Expr,
    rel_tbl: &DaxTableData,
    r_idx: usize,
    model: &DaxModel,
    rel_ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let mut stack = row_stack.to_vec();
    stack.push(RowContext {
        table: rel_tbl,
        row_idx: r_idx,
    });
    let transitioned = apply_context_transition(model, rel_ctx, &stack);
    evaluate_ast(expr, model, &transitioned, &stack, vars)
}

/// Resolves the single current value of a table column `table[column]` from `row_stack` first,
/// or from `ctx` when the visible rows contain exactly one distinct value (including BLANK).
fn resolve_single_col_value(
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    table: &str,
    column: &str,
) -> Option<DaxValue> {
    for rc in row_stack.iter().rev() {
        if let Some(cidx) = row_col_idx(rc.table, table, column) {
            return Some(rc.table.rows[rc.row_idx][cidx].clone());
        }
    }
    let tbl = model.get_table(table)?;
    let cidx = tbl.get_col_idx(column)?;
    let visible = model.get_visible_row_indices(table, ctx);
    let mut distinct_key: Option<ValueKey> = None;
    let mut first_val: Option<DaxValue> = None;
    for r_idx in visible {
        let v = &tbl.rows[r_idx][cidx];
        let k = v.value_key();
        match &distinct_key {
            None => {
                distinct_key = Some(k);
                first_val = Some(v.clone());
            }
            Some(prev) if prev != &k => return None,
            _ => {}
        }
    }
    first_val
}

/// Resolves the current value for each column `i` of `rel_tbl` when uniquely pinned in `(ctx, row_stack)`.
fn resolve_relation_current_values(
    rel_tbl: &DaxTableData,
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
) -> Vec<Option<DaxValue>> {
    (0..rel_tbl.columns.len())
        .map(|i| {
            let col_name = &rel_tbl.columns[i];
            for rc in row_stack.iter().rev() {
                if let Some((t, c)) = split_qualified_col(col_name) {
                    if let Some(cidx) = row_col_idx(rc.table, t, c) {
                        return Some(rc.table.rows[rc.row_idx][cidx].clone());
                    }
                } else if let Some(cidx) = row_col_idx(rc.table, &rel_tbl.name, col_name) {
                    return Some(rc.table.rows[rc.row_idx][cidx].clone());
                } else if let Some(cidx) = bare_col_idx(rc.table, col_name) {
                    return Some(rc.table.rows[rc.row_idx][cidx].clone());
                }
            }
            let (t, c) = column_lineage(model, rel_tbl, i)?;
            resolve_single_col_value(model, ctx, row_stack, &t, &c)
        })
        .collect()
}

fn resolve_current_partition_key(
    partition_by: &[&Expr],
    rel_tbl: &DaxTableData,
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<Option<Vec<ValueKey>>, String> {
    if partition_by.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let mut keys = Vec::with_capacity(partition_by.len());
    for expr in partition_by {
        if let Expr::TableColRef { table, column } = expr {
            let Some(val) = resolve_single_col_value(model, ctx, row_stack, table, column) else {
                // If it exists in `rel_tbl` via bare/qualified name on stack, `resolve_single_col_value`
                // handles `row_stack` and single-value model context; otherwise there is no unique partition.
                return Ok(None);
            };
            keys.push(val.value_key());
        } else {
            let val = evaluate_ast(expr, model, ctx, row_stack, vars)?;
            if val.is_blank() && row_stack.is_empty() {
                let _ = rel_tbl;
            }
            keys.push(val.value_key());
        }
    }
    Ok(Some(keys))
}

fn compare_values_with_blanks(l: &DaxValue, r: &DaxValue, blanks: BlanksMode, asc: bool) -> i8 {
    let lb = l.is_blank();
    let rb = r.is_blank();
    if lb && rb {
        return 0;
    }
    if lb || rb {
        return match blanks {
            BlanksMode::First => {
                if lb {
                    -1
                } else {
                    1
                }
            }
            BlanksMode::Last => {
                if lb {
                    1
                } else {
                    -1
                }
            }
            BlanksMode::Default => {
                let raw = if lb { -1 } else { 1 };
                if asc {
                    raw
                } else {
                    -raw
                }
            }
        };
    }
    let raw = compare_dax_values(l, r);
    if asc {
        raw
    } else {
        -raw
    }
}

struct PreparedPartition {
    /// `(orig_row_idx, order_keys)` sorted deterministically within the active partition.
    rows: Vec<(usize, Vec<DaxValue>)>,
    /// Index into `rows` matching the current row, if determinable.
    current_pos: Option<usize>,
}

fn prepare_partition(
    clauses: &WindowClauses<'_>,
    rel_tbl: &DaxTableData,
    rel_indices: &[usize],
    rel_ctx: &FilterContext,
    model: &DaxModel,
    outer_ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
    need_current_row: bool,
) -> Result<PreparedPartition, String> {
    let cur_part_key = if clauses.partition_by.is_empty() {
        Some(Vec::new())
    } else {
        resolve_current_partition_key(
            &clauses.partition_by,
            rel_tbl,
            model,
            outer_ctx,
            row_stack,
            vars,
        )?
    };
    let Some(cur_part_key) = cur_part_key else {
        return Ok(PreparedPartition {
            rows: Vec::new(),
            current_pos: None,
        });
    };

    // Determine effective ORDERBY columns: explicit clauses, or default to non-partition columns asc.
    let mut partition_rows = Vec::new();
    for &r_idx in rel_indices {
        if !clauses.partition_by.is_empty() {
            let mut row_pk = Vec::with_capacity(clauses.partition_by.len());
            for pe in &clauses.partition_by {
                let pv = resolve_expr_in_row(pe, rel_tbl, r_idx, model, rel_ctx, row_stack, vars)?;
                row_pk.push(pv.value_key());
            }
            if row_pk != cur_part_key {
                continue;
            }
        }
        let mut order_vals = Vec::with_capacity(clauses.order_by.len());
        for o in &clauses.order_by {
            order_vals.push(resolve_expr_in_row(
                o.expr, rel_tbl, r_idx, model, rel_ctx, row_stack, vars,
            )?);
        }
        partition_rows.push((r_idx, order_vals));
    }

    let order_dirs: Vec<bool> = clauses.order_by.iter().map(|o| o.asc).collect();
    let blanks = clauses.blanks;
    partition_rows.sort_by(|(ia, ka), (ib, kb)| {
        for ((va, vb), &asc) in ka.iter().zip(kb.iter()).zip(order_dirs.iter()) {
            let c = compare_values_with_blanks(va, vb, blanks, asc);
            if c != 0 {
                return if c < 0 {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
        }
        // Deterministic tie-breaker across all columns of `rel_tbl` ascending.
        for cidx in 0..rel_tbl.columns.len() {
            let c = compare_values_with_blanks(
                &rel_tbl.rows[*ia][cidx],
                &rel_tbl.rows[*ib][cidx],
                BlanksMode::Default,
                true,
            );
            if c != 0 {
                return if c < 0 {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
        }
        ia.cmp(ib)
    });

    if !need_current_row {
        return Ok(PreparedPartition {
            rows: partition_rows,
            current_pos: None,
        });
    }

    // Match current row using `MATCHBY` expressions if supplied, otherwise all resolved columns of `rel_tbl`.
    let current_pos = if !clauses.match_by.is_empty() {
        let mut target_vals = Vec::with_capacity(clauses.match_by.len());
        let mut resolved_all = true;
        for me in &clauses.match_by {
            if let Expr::TableColRef { table, column } = me {
                match resolve_single_col_value(model, outer_ctx, row_stack, table, column) {
                    Some(v) => target_vals.push(v),
                    None => {
                        resolved_all = false;
                        break;
                    }
                }
            } else {
                target_vals.push(evaluate_ast(me, model, outer_ctx, row_stack, vars)?);
            }
        }
        if !resolved_all || target_vals.is_empty() {
            None
        } else {
            let mut found = None;
            for (pos, (r_idx, _)) in partition_rows.iter().enumerate() {
                let mut matches = true;
                for (me, tv) in clauses.match_by.iter().zip(target_vals.iter()) {
                    let rv =
                        resolve_expr_in_row(me, rel_tbl, *r_idx, model, rel_ctx, row_stack, vars)?;
                    if !rv.dax_equals(tv) {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    found = Some(pos);
                    break;
                }
            }
            found
        }
    } else {
        let cur_vals = resolve_relation_current_values(rel_tbl, model, outer_ctx, row_stack);
        let active_cols: Vec<(usize, &DaxValue)> = cur_vals
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|dv| (i, dv)))
            .collect();
        if active_cols.is_empty() {
            None
        } else {
            partition_rows.iter().position(|(r_idx, _)| {
                active_cols
                    .iter()
                    .all(|&(cidx, tv)| rel_tbl.rows[*r_idx][cidx].dax_equals(tv))
            })
        }
    };

    Ok(PreparedPartition {
        rows: partition_rows,
        current_pos,
    })
}

/// `OFFSET(delta, [relation], [orderBy], [blanks], [partitionBy], [matchBy])`
pub(super) fn eval_offset<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if args.is_empty() {
        return Err("OFFSET requer argumento <delta>".to_string());
    }
    let delta = evaluate_ast(&args[0], model, ctx, row_stack, vars)?
        .as_f64()
        .unwrap_or(0.0)
        .round() as isize;
    let clauses = parse_window_clauses(&args[1..])?;
    let (rel_eval, rel_indices, rel_ctx) =
        eval_window_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = rel_eval.as_ref();
    let prep = prepare_partition(
        &clauses,
        rel_tbl,
        &rel_indices,
        &rel_ctx,
        model,
        ctx,
        row_stack,
        vars,
        true,
    )?;

    let out_rows = match prep.current_pos {
        Some(pos) => {
            let target = pos as isize + delta;
            if target >= 0 && (target as usize) < prep.rows.len() {
                let (r_idx, _) = prep.rows[target as usize];
                vec![rel_tbl.rows[r_idx].clone()]
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    };
    let n = out_rows.len();
    Ok((
        TableEval::Owned(with_rows_like(rel_tbl, out_rows)),
        (0..n).collect(),
        rel_ctx,
    ))
}

/// `INDEX(position, [relation], [orderBy], [blanks], [partitionBy], [matchBy])`
pub(super) fn eval_index<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if args.is_empty() {
        return Err("INDEX requer argumento <position>".to_string());
    }
    let pos_1 = evaluate_ast(&args[0], model, ctx, row_stack, vars)?
        .as_f64()
        .unwrap_or(0.0)
        .round() as isize;
    let clauses = parse_window_clauses(&args[1..])?;
    let (rel_eval, rel_indices, rel_ctx) =
        eval_window_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = rel_eval.as_ref();
    let prep = prepare_partition(
        &clauses,
        rel_tbl,
        &rel_indices,
        &rel_ctx,
        model,
        ctx,
        row_stack,
        vars,
        false,
    )?;

    let len = prep.rows.len() as isize;
    let idx_opt = if pos_1 > 0 && pos_1 <= len {
        Some((pos_1 - 1) as usize)
    } else if pos_1 < 0 && len + pos_1 >= 0 {
        Some((len + pos_1) as usize)
    } else {
        None
    };

    let out_rows = match idx_opt {
        Some(idx) => vec![rel_tbl.rows[prep.rows[idx].0].clone()],
        None => Vec::new(),
    };
    let n = out_rows.len();
    Ok((
        TableEval::Owned(with_rows_like(rel_tbl, out_rows)),
        (0..n).collect(),
        rel_ctx,
    ))
}

fn parse_rel_abs_opt(expr: Option<&Expr>) -> Option<RelAbs> {
    match expr {
        Some(Expr::Ident(s)) if s.eq_ignore_ascii_case("ABS") => Some(RelAbs::Abs),
        Some(Expr::Ident(s)) if s.eq_ignore_ascii_case("REL") => Some(RelAbs::Rel),
        _ => None,
    }
}

/// `WINDOW(from, [from_type], to, [to_type], [relation], [orderBy], [blanks], [partitionBy], [matchBy])`
pub(super) fn eval_window<'a>(
    args: &[Expr],
    model: &'a DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<(TableEval<'a>, Vec<usize>, FilterContext), String> {
    if args.len() < 2 {
        return Err("WINDOW requer pelo menos argumentos <from> e <to>".to_string());
    }
    let from_num = evaluate_ast(&args[0], model, ctx, row_stack, vars)?
        .as_f64()
        .unwrap_or(0.0)
        .round() as isize;
    let mut cursor = 1;
    let from_type = if let Some(ra) = parse_rel_abs_opt(args.get(cursor)) {
        cursor += 1;
        ra
    } else if matches!(args.get(cursor), Some(Expr::Blank)) {
        cursor += 1;
        RelAbs::Rel
    } else {
        RelAbs::Rel
    };

    let Some(to_expr) = args.get(cursor) else {
        return Err("WINDOW requer argumento <to>".to_string());
    };
    let to_num = evaluate_ast(to_expr, model, ctx, row_stack, vars)?
        .as_f64()
        .unwrap_or(0.0)
        .round() as isize;
    cursor += 1;

    let to_type = if let Some(ra) = parse_rel_abs_opt(args.get(cursor)) {
        cursor += 1;
        ra
    } else if matches!(args.get(cursor), Some(Expr::Blank)) {
        cursor += 1;
        RelAbs::Rel
    } else {
        RelAbs::Rel
    };

    let clauses = parse_window_clauses(&args[cursor..])?;
    let (rel_eval, rel_indices, rel_ctx) =
        eval_window_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = rel_eval.as_ref();
    let need_cur = matches!(from_type, RelAbs::Rel) || matches!(to_type, RelAbs::Rel);
    let prep = prepare_partition(
        &clauses,
        rel_tbl,
        &rel_indices,
        &rel_ctx,
        model,
        ctx,
        row_stack,
        vars,
        need_cur,
    )?;

    let len = prep.rows.len() as isize;
    let out_rows = if len == 0 || (need_cur && prep.current_pos.is_none()) {
        Vec::new()
    } else {
        let resolve_bound = |num: isize, kind: RelAbs| -> Option<isize> {
            match kind {
                RelAbs::Rel => prep.current_pos.map(|p| p as isize + num),
                RelAbs::Abs => {
                    if num > 0 {
                        Some(num - 1)
                    } else if num < 0 {
                        Some(len + num)
                    } else {
                        Some(0)
                    }
                }
            }
        };
        let s_idx = resolve_bound(from_num, from_type);
        let e_idx = resolve_bound(to_num, to_type);
        match (s_idx, e_idx) {
            (Some(s), Some(e)) => {
                let start = s.max(0);
                let end = e.min(len - 1);
                if start <= end && start < len && end >= 0 {
                    (start as usize..=end as usize)
                        .map(|i| rel_tbl.rows[prep.rows[i].0].clone())
                        .collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    };

    let n = out_rows.len();
    Ok((
        TableEval::Owned(with_rows_like(rel_tbl, out_rows)),
        (0..n).collect(),
        rel_ctx,
    ))
}

/// `RANK([ties], [relation], [orderBy], [blanks], [partitionBy], [matchBy])`
pub(super) fn eval_rank(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let (ties, rest) = match args.first() {
        Some(Expr::Ident(s)) if s.eq_ignore_ascii_case("DENSE") => (RankTies::Dense, &args[1..]),
        Some(Expr::Ident(s)) if s.eq_ignore_ascii_case("SKIP") => (RankTies::Skip, &args[1..]),
        Some(Expr::Blank) => (RankTies::Skip, &args[1..]),
        _ => (RankTies::Skip, args),
    };
    let clauses = parse_window_clauses(rest)?;
    let (rel_eval, rel_indices, rel_ctx) =
        eval_window_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = rel_eval.as_ref();
    let prep = prepare_partition(
        &clauses,
        rel_tbl,
        &rel_indices,
        &rel_ctx,
        model,
        ctx,
        row_stack,
        vars,
        true,
    )?;

    let Some(pos) = prep.current_pos else {
        return Ok(DaxValue::Blank);
    };
    let mut skip_rank = 1i64;
    let mut dense_rank = 1i64;
    for i in 1..=pos {
        let prev_k = &prep.rows[i - 1].1;
        let cur_k = &prep.rows[i].1;
        let eq = prev_k.len() == cur_k.len()
            && prev_k
                .iter()
                .zip(cur_k.iter())
                .all(|(a, b)| compare_dax_values(a, b) == 0);
        if !eq {
            skip_rank = (i + 1) as i64;
            dense_rank += 1;
        }
    }
    Ok(DaxValue::Integer(match ties {
        RankTies::Dense => dense_rank,
        RankTies::Skip => skip_rank,
    }))
}

/// `ROWNUMBER([relation], [orderBy], [blanks], [partitionBy], [matchBy])`
pub(super) fn eval_rownumber(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let clauses = parse_window_clauses(args)?;
    let (rel_eval, rel_indices, rel_ctx) =
        eval_window_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = rel_eval.as_ref();
    let prep = prepare_partition(
        &clauses,
        rel_tbl,
        &rel_indices,
        &rel_ctx,
        model,
        ctx,
        row_stack,
        vars,
        true,
    )?;
    match prep.current_pos {
        Some(pos) => Ok(DaxValue::Integer((pos + 1) as i64)),
        None => Ok(DaxValue::Blank),
    }
}

#[cfg(test)]
mod tests {
    use crate::dax::{
        build_filter_context, DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput,
        DaxTableInput, DaxValue, FilterContext, NativeFilterInput,
    };
    use serde_json::json;

    fn row(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().unwrap().clone()
    }

    fn window_test_model() -> DaxModel {
        let mut model = DaxModel::new();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![
                DaxTableInput {
                    name: "d_Produto".into(),
                    columns: vec!["ProdutoID".into(), "Produto".into(), "Categoria".into(), "Preco".into()],
                    rows: vec![
                        row(json!({"ProdutoID": 1, "Produto": "Caneta", "Categoria": "Papelaria", "Preco": 10.0})),
                        row(json!({"ProdutoID": 2, "Produto": "Caderno", "Categoria": "Papelaria", "Preco": 20.0})),
                        row(json!({"ProdutoID": 3, "Produto": "Mouse", "Categoria": "Informatica", "Preco": 80.0})),
                        row(json!({"ProdutoID": 4, "Produto": "Teclado", "Categoria": "Informatica", "Preco": 80.0})),
                        row(json!({"ProdutoID": 5, "Produto": "Monitor", "Categoria": "Informatica", "Preco": 300.0})),
                    ],
                    duckdb_table: None,
                    column_types: vec![],
                    sort_by_columns: Default::default(),
                },
                DaxTableInput {
                    name: "d_Mes".into(),
                    columns: vec!["MesNum".into(), "Mes".into()],
                    rows: vec![
                        row(json!({"MesNum": 1, "Mes": "Jan"})),
                        row(json!({"MesNum": 2, "Mes": "Fev"})),
                        row(json!({"MesNum": 3, "Mes": "Mar"})),
                    ],
                    duckdb_table: None,
                    column_types: vec![],
                    sort_by_columns: Default::default(),
                },
                DaxTableInput {
                    name: "f_Vendas".into(),
                    columns: vec!["VendaID".into(), "ProdutoID".into(), "MesNum".into(), "Valor".into()],
                    rows: vec![
                        row(json!({"VendaID": 1, "ProdutoID": 1, "MesNum": 1, "Valor": 100.0})),
                        row(json!({"VendaID": 2, "ProdutoID": 1, "MesNum": 2, "Valor": 150.0})),
                        row(json!({"VendaID": 3, "ProdutoID": 1, "MesNum": 3, "Valor": 200.0})),
                        row(json!({"VendaID": 4, "ProdutoID": 3, "MesNum": 1, "Valor": 400.0})),
                        row(json!({"VendaID": 5, "ProdutoID": 3, "MesNum": 2, "Valor": 500.0})),
                        row(json!({"VendaID": 6, "ProdutoID": 5, "MesNum": 3, "Valor": 900.0})),
                    ],
                    duckdb_table: None,
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
                    from_column: "MesNum".into(),
                    to_table: "d_Mes".into(),
                    to_column: "MesNum".into(),
                    cross_filter_direction: "single".into(),
                    is_active: true,
                },
            ],
            measures: vec![DaxMeasureInput {
                name: "Total Vendas".into(),
                table_name: "f_Vendas".into(),
                formula: "SUM(f_Vendas[Valor])".into(),
            }],
        });
        model
    }

    #[test]
    fn offset_previous_and_next_and_out_of_bounds_returns_blank() {
        let model = window_test_model();
        let mut ctx_feb = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Mes".into(),
                column: "MesNum".into(),
                values: vec![json!(2)],
            }],
            &model,
        );
        // Simulate a visual row cell where no slicer restricts d_Mes (shadow = unfiltered)
        ctx_feb.shadow = Some(std::sync::Arc::new(FilterContext::default()));
        let prev = model
            .evaluate_dax(
                "CALCULATE([Total Vendas], OFFSET(-1, ALLSELECTED(d_Mes[MesNum]), ORDERBY(d_Mes[MesNum], ASC)))",
                &ctx_feb,
            )
            .unwrap();
        assert_eq!(prev.as_f64(), Some(500.0)); // Jan: 100 + 400

        let next = model
            .evaluate_dax(
                "CALCULATE([Total Vendas], OFFSET(1, ALLSELECTED(d_Mes[MesNum]), ORDERBY(d_Mes[MesNum], ASC)))",
                &ctx_feb,
            )
            .unwrap();
        assert_eq!(next.as_f64(), Some(1100.0)); // Mar: 200 + 900

        // Out of bounds at first month (-1 before Jan) -> BLANK
        let mut ctx_jan = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Mes".into(),
                column: "MesNum".into(),
                values: vec![json!(1)],
            }],
            &model,
        );
        ctx_jan.shadow = Some(std::sync::Arc::new(FilterContext::default()));
        let oob = model
            .evaluate_dax(
                "CALCULATE([Total Vendas], OFFSET(-1, ALLSELECTED(d_Mes[MesNum]), ORDERBY(d_Mes[MesNum], ASC)))",
                &ctx_jan,
            )
            .unwrap();
        assert_eq!(oob, DaxValue::Blank);
    }

    #[test]
    fn offset_with_multicol_all_and_partitionby() {
        let model = window_test_model();
        // Produto = Caderno (20, Papelaria): previous in Papelaria by Preco ASC is Caneta (10, Papelaria), Total = 450
        let ctx_caderno = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Produto".into(),
                values: vec![json!("Caderno")],
            }],
            &model,
        );
        let v = model
            .evaluate_dax(
                "CALCULATE(MAX(d_Produto[Preco]), OFFSET(-1, ALL(d_Produto[Produto], d_Produto[Categoria], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC), DEFAULT, PARTITIONBY(d_Produto[Categoria])))",
                &ctx_caderno,
            )
            .unwrap();
        assert_eq!(v.as_f64(), Some(10.0));
    }

    #[test]
    fn index_positive_negative_and_partitioned() {
        let model = window_test_model();
        let ctx = FilterContext::default();
        // Cheapest product overall -> 10
        assert_eq!(
            model
                .evaluate_dax(
                    "CALCULATE(MAX(d_Produto[Preco]), INDEX(1, ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC)))",
                    &ctx,
                )
                .unwrap()
                .as_f64(),
            Some(10.0)
        );
        // Most expensive product overall via -1 -> 300
        assert_eq!(
            model
                .evaluate_dax(
                    "CALCULATE(MAX(d_Produto[Preco]), INDEX(-1, ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC)))",
                    &ctx,
                )
                .unwrap()
                .as_f64(),
            Some(300.0)
        );
        // Most expensive within Papelaria partition -> 20
        let ctx_pap = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Categoria".into(),
                values: vec![json!("Papelaria")],
            }],
            &model,
        );
        assert_eq!(
            model
                .evaluate_dax(
                    "CALCULATE(MAX(d_Produto[Preco]), INDEX(-1, ALL(d_Produto[Produto], d_Produto[Categoria], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC), DEFAULT, PARTITIONBY(d_Produto[Categoria])))",
                    &ctx_pap,
                )
                .unwrap()
                .as_f64(),
            Some(20.0)
        );
    }

    #[test]
    fn window_running_total_and_moving_sum() {
        let model = window_test_model();
        let mut ctx_feb = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Mes".into(),
                column: "MesNum".into(),
                values: vec![json!(2)],
            }],
            &model,
        );
        ctx_feb.shadow = Some(std::sync::Arc::new(FilterContext::default()));
        // Running total up to Feb: Jan (500) + Feb (650) = 1150
        let running = model
            .evaluate_dax(
                "CALCULATE([Total Vendas], WINDOW(1, ABS, 0, REL, ALLSELECTED(d_Mes[MesNum]), ORDERBY(d_Mes[MesNum], ASC)))",
                &ctx_feb,
            )
            .unwrap();
        assert_eq!(running.as_f64(), Some(1150.0));

        // 2-month moving sum ending at Mar: Feb (650) + Mar (1100) = 1750
        let mut ctx_mar = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Mes".into(),
                column: "MesNum".into(),
                values: vec![json!(3)],
            }],
            &model,
        );
        ctx_mar.shadow = Some(std::sync::Arc::new(FilterContext::default()));
        let moving = model
            .evaluate_dax(
                "CALCULATE([Total Vendas], WINDOW(-1, REL, 0, REL, ALLSELECTED(d_Mes[MesNum]), ORDERBY(d_Mes[MesNum], ASC)))",
                &ctx_mar,
            )
            .unwrap();
        assert_eq!(moving.as_f64(), Some(1750.0));
    }

    #[test]
    fn rank_skip_vs_dense_and_rownumber_with_ties() {
        let model = window_test_model();
        // Preco in d_Produto: Caneta=10, Caderno=20, Mouse=80, Teclado=80, Monitor=300
        // Ordered ASC: Caneta(1), Caderno(2), Mouse(3), Teclado(3), Monitor -> SKIP=5, DENSE=4
        let ctx_mon = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Produto".into(),
                values: vec![json!("Monitor")],
            }],
            &model,
        );
        let skip = model
            .evaluate_dax(
                "RANK(SKIP, ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC))",
                &ctx_mon,
            )
            .unwrap();
        let dense = model
            .evaluate_dax(
                "RANK(DENSE, ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC))",
                &ctx_mon,
            )
            .unwrap();
        assert_eq!(skip, DaxValue::Integer(5));
        assert_eq!(dense, DaxValue::Integer(4));

        // ROWNUMBER gives distinct sequential numbers even across Mouse & Teclado tie (80 vs 80)
        let ctx_mouse = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Produto".into(),
                values: vec![json!("Mouse")],
            }],
            &model,
        );
        let ctx_teclado = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Produto".into(),
                values: vec![json!("Teclado")],
            }],
            &model,
        );
        let rn_m = model
            .evaluate_dax(
                "ROWNUMBER(ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC))",
                &ctx_mouse,
            )
            .unwrap();
        let rn_t = model
            .evaluate_dax(
                "ROWNUMBER(ALL(d_Produto[Produto], d_Produto[Preco]), ORDERBY(d_Produto[Preco], ASC))",
                &ctx_teclado,
            )
            .unwrap();
        assert_eq!(rn_m, DaxValue::Integer(3)); // "Mouse" < "Teclado"
        assert_eq!(rn_t, DaxValue::Integer(4));
    }
}
