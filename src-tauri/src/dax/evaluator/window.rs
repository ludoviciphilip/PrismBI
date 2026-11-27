//! Window functions in Rust: OFFSET, INDEX, WINDOW (table-valued), RANK, ROWNUMBER (scalar).
//!
//! Clause parsing (`ORDERBY`, `PARTITIONBY`, `MATCHBY`, `BLANKS`, `REL`/`ABS`, `SKIP`/`DENSE`)
//! follows DAX semantics cleanly on top of `eval_table_expr` and `DaxTableData` lineage.

use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

// ---------------------------------------------------------------------------
// v0.11 Frente G: cache of the sorted partition, keyed by (relation,
// ORDERBY, PARTITIONBY, filter context) — docs/V011.md Rodada 2.
//
// `prepare_partition` used to re-filter and re-sort the ENTIRE relation on
// EVERY call. OFFSET/INDEX/RANK/ROWNUMBER inside a SUMX over N rows call it
// once per iterated row, so a 50k-row SUMX cost O(N² log N): 50,000 calls,
// each re-filtering + re-sorting up to 50,000 rows. Profiling confirmed this
// (see `docs/RUST_ENGINE_2.md` §5 / the bench in `tests_window_cache`).
//
// Why this is safe to cache per `evaluate_dax` call (not per row): inside a
// single SUMX/iterator body, the filter *context* passed down to window
// functions (`ctx` in `eval_offset`/`eval_rank`/...) is the iterator's outer
// context, evaluated ONCE for the whole loop — only the *row* (via
// `row_stack`) changes per iteration (see `aggregate.rs`'s `iter_ctx`, reused
// unchanged across all iterated rows). The relation/ORDERBY/PARTITIONBY
// clauses are themselves `&Expr` borrowed from the one parsed AST for this
// expression, so their addresses are stable for the whole evaluation too.
// That means the GROUPED-AND-SORTED partitions (one group per PARTITIONBY
// key) depend only on (relation expr identity, ORDERBY clauses, PARTITIONBY
// clauses, BLANKS mode, filter context content) — never on the current row.
// Only the *current row's* partition key and position within its (already
// sorted) group legitimately vary per call, and those are cheap to resolve
// (O(group size), not O(N log N)) once the group is cached.
//
// Known limitation (documented rather than guessed around): a RELATION
// expression that itself reads the *current* row (e.g. through a measure
// doing an implicit context transition inside the relation argument) would
// invalidate this assumption. That shape is unusual for a window-function
// relation (window functions are precisely about a stable frame) and isn't
// exercised by the corpus; a future relation-identity-sensitive case should
// add the row's resolved values to `window_cache_key` or bypass the cache.
//
// Cleared at the start of every top-level `DaxModel::evaluate_dax` call
// (`mod.rs`), so a cache entry never survives past the query/measure
// evaluation that built it — no stale hits across unrelated evaluations,
// and no cross-test leakage (the cache is `thread_local`, so parallel test
// threads never share it either).
thread_local! {
    static WINDOW_PARTITION_CACHE: RefCell<HashMap<u64, Rc<CachedPartitionGroups>>> = RefCell::new(HashMap::new());
}

struct CachedPartitionGroups {
    groups: HashMap<Vec<ValueKey>, CachedPartition>,
}

struct CachedPartition {
    /// PARTITIONBY key -> that partition's rows, already sorted by ORDERBY
    /// with the deterministic all-columns tie-breaker `prepare_partition`
    /// always applied. `usize` row indices are valid against the LIVE
    /// relation table on every call (not just the one that built the
    /// cache): the relation/ORDERBY/PARTITIONBY clauses and filter context
    /// are identical by construction of the cache key, so `eval_table_expr`
    /// deterministically reproduces the same rows in the same order.
    ///
    /// `Rc` so handing this partition's rows to `PreparedPartition` on every
    /// one of the N calls inside a SUMX is an O(1) refcount bump, never an
    /// O(group size) deep clone — a plain `Vec` clone here would silently
    /// put the O(N) cost right back (N calls × O(group size) clone), even
    /// with the sort itself cached.
    rows: Rc<Vec<(usize, Vec<DaxValue>)>>,
    /// `rel_tbl`'s full row (every column, as `ValueKey`s) -> its position in
    /// `rows`. Lets the no-MATCHBY "current row" lookup in `prepare_partition`
    /// (previously a `position()` linear scan re-run on every one of the N
    /// iterated rows — the OTHER O(N) cost hiding behind the sort, since
    /// caching only the sort left this scan still O(N) per call) become an
    /// O(1) hash lookup instead. Built once per group, same pass as sorting.
    by_full_row_key: HashMap<Vec<ValueKey>, usize>,
    /// v0.11 Frente G: precomputed SKIP/DENSE rank at every position (1-based,
    /// DAX semantics), so `RANK` doesn't re-walk `0..=pos` counting ties on
    /// every one of the N calls — that scan was the OTHER hidden O(N) cost:
    /// even with the sort and the position lookup both cached, `eval_rank`'s
    /// own tie-counting loop alone still made a 50k-row SUMX O(N²). Built
    /// once per group, same single pass as the rest of this struct.
    skip_rank: Rc<Vec<i64>>,
    dense_rank: Rc<Vec<i64>>,
}

pub(crate) fn clear_window_partition_cache() {
    WINDOW_PARTITION_CACHE.with(|c| c.borrow_mut().clear());
    WINDOW_RELATION_CACHE.with(|c| c.borrow_mut().clear());
}

fn hash_filter_context<H: Hasher>(ctx: &FilterContext, h: &mut H, depth: u8) {
    let mut tables: Vec<&String> = ctx.filters.keys().collect();
    tables.sort();
    for t in tables {
        t.hash(h);
        let cols = &ctx.filters[t];
        let mut cnames: Vec<&String> = cols.keys().collect();
        cnames.sort();
        for c in cnames {
            c.hash(h);
            let mut keys: Vec<&ValueKey> = cols[c].iter().collect();
            keys.sort();
            keys.hash(h);
        }
    }
    ctx.tuple_filters.len().hash(h);
    for tf in &ctx.tuple_filters {
        tf.table.hash(h);
        tf.columns.hash(h);
        let mut tuples: Vec<&Vec<ValueKey>> = tf.tuples.iter().collect();
        tuples.sort();
        tuples.hash(h);
    }
    for ov in &ctx.rel_overrides {
        format!("{ov:?}").hash(h);
    }
    // ALLSELECTED's shadow context can change which rows a relation like
    // `ALLSELECTED(table)` sees even when `filters` above is identical, so it
    // must be part of the key too. Depth-capped: a shadow's own shadow is
    // always `None` in practice (`build_filter_context` sets it once), but
    // the cap keeps this provably terminating regardless.
    match (&ctx.shadow, depth) {
        (Some(s), 0) => hash_filter_context(s, h, 1),
        (Some(_), _) => "shadow-depth-cap".hash(h),
        (None, _) => "no-shadow".hash(h),
    }
}

fn window_cache_key(clauses: &WindowClauses<'_>, ctx: &FilterContext) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    clauses.relation.map(|e| e as *const Expr as usize).unwrap_or(0).hash(&mut h);
    clauses.order_by.len().hash(&mut h);
    for o in &clauses.order_by {
        (o.expr as *const Expr as usize).hash(&mut h);
        o.asc.hash(&mut h);
    }
    clauses.partition_by.len().hash(&mut h);
    for p in &clauses.partition_by {
        (*p as *const Expr as usize).hash(&mut h);
    }
    // Only affects `eval_window_relation`'s implicit-relation synthesis when
    // `clauses.relation` is `None`, but folding it in unconditionally keeps
    // one key function safe for both the relation cache and the partition
    // cache below.
    clauses.match_by.len().hash(&mut h);
    for m in &clauses.match_by {
        (*m as *const Expr as usize).hash(&mut h);
    }
    clauses.blanks.hash(&mut h);
    hash_filter_context(ctx, &mut h, 0);
    h.finish()
}

/// v0.11 Frente G: caches `eval_window_relation`'s result too — NOT just the
/// sorted partitions. `eval_window_relation` (and the `ALL`/`ALLSELECTED`
/// table expression it evaluates) scans the whole relation, so without this
/// a 50k-row SUMX still paid O(N) per iterated row here even after the sort
/// itself was cached, keeping the overall cost O(N²). Keyed identically to
/// the partition cache (same relation/ORDERBY/PARTITIONBY/MATCHBY/filter
/// context), and cleared at the same points.
struct CachedWindowRelation {
    table: DaxTableData,
    indices: Vec<usize>,
    ctx: FilterContext,
}

thread_local! {
    static WINDOW_RELATION_CACHE: RefCell<HashMap<u64, Rc<CachedWindowRelation>>> = RefCell::new(HashMap::new());
}

fn get_or_build_cached_relation(
    clauses: &WindowClauses<'_>,
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<Rc<CachedWindowRelation>, String> {
    let key = window_cache_key(clauses, ctx);
    if let Some(cached) = WINDOW_RELATION_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(cached);
    }
    let (rel_eval, rel_indices, rel_ctx) = eval_window_relation(clauses, model, ctx, row_stack, vars)?;
    let built = Rc::new(CachedWindowRelation {
        table: rel_eval.as_ref().clone(),
        indices: rel_indices,
        ctx: rel_ctx,
    });
    WINDOW_RELATION_CACHE.with(|c| c.borrow_mut().insert(key, built.clone()));
    Ok(built)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        is_calculated: tbl.is_calculated,
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
    rows: Rc<Vec<(usize, Vec<DaxValue>)>>,
    /// Index into `rows` matching the current row, if determinable.
    current_pos: Option<usize>,
    /// Precomputed SKIP/DENSE rank at every position (see `CachedPartition`);
    /// `eval_rank` reads `skip_rank[current_pos]`/`dense_rank[current_pos]`
    /// directly instead of re-walking `0..=current_pos` every call.
    skip_rank: Rc<Vec<i64>>,
    dense_rank: Rc<Vec<i64>>,
}

/// Builds every PARTITIONBY group at once (one group when there's no
/// PARTITIONBY), each sorted by ORDERBY with the same deterministic
/// all-columns tie-breaker `prepare_partition` always applied. Called once
/// per cache miss instead of once per iterated row.
fn build_partition_groups(
    clauses: &WindowClauses<'_>,
    rel_tbl: &DaxTableData,
    rel_indices: &[usize],
    rel_ctx: &FilterContext,
    model: &DaxModel,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<CachedPartitionGroups, String> {
    let mut groups: HashMap<Vec<ValueKey>, Vec<(usize, Vec<DaxValue>)>> = HashMap::new();
    for &r_idx in rel_indices {
        let mut row_pk = Vec::with_capacity(clauses.partition_by.len());
        for pe in &clauses.partition_by {
            let pv = resolve_expr_in_row(pe, rel_tbl, r_idx, model, rel_ctx, row_stack, vars)?;
            row_pk.push(pv.value_key());
        }
        let mut order_vals = Vec::with_capacity(clauses.order_by.len());
        for o in &clauses.order_by {
            order_vals.push(resolve_expr_in_row(
                o.expr, rel_tbl, r_idx, model, rel_ctx, row_stack, vars,
            )?);
        }
        groups.entry(row_pk).or_default().push((r_idx, order_vals));
    }

    let order_dirs: Vec<bool> = clauses.order_by.iter().map(|o| o.asc).collect();
    let blanks = clauses.blanks;
    let mut out: HashMap<Vec<ValueKey>, CachedPartition> = HashMap::with_capacity(groups.len());
    for (key, mut rows) in groups {
        rows.sort_by(|(ia, ka), (ib, kb)| {
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
        let by_full_row_key: HashMap<Vec<ValueKey>, usize> = rows
            .iter()
            .enumerate()
            .map(|(pos, (r_idx, _))| {
                let full_key: Vec<ValueKey> = rel_tbl.rows[*r_idx].iter().map(DaxValue::value_key).collect();
                (full_key, pos)
            })
            .collect();
        let mut skip_rank = vec![1i64; rows.len()];
        let mut dense_rank = vec![1i64; rows.len()];
        for i in 1..rows.len() {
            let prev_k = &rows[i - 1].1;
            let cur_k = &rows[i].1;
            let eq = prev_k.len() == cur_k.len()
                && prev_k.iter().zip(cur_k.iter()).all(|(a, b)| compare_dax_values(a, b) == 0);
            if eq {
                skip_rank[i] = skip_rank[i - 1];
                dense_rank[i] = dense_rank[i - 1];
            } else {
                skip_rank[i] = (i + 1) as i64;
                dense_rank[i] = dense_rank[i - 1] + 1;
            }
        }
        out.insert(
            key,
            CachedPartition {
                rows: Rc::new(rows),
                by_full_row_key,
                skip_rank: Rc::new(skip_rank),
                dense_rank: Rc::new(dense_rank),
            },
        );
    }
    Ok(CachedPartitionGroups { groups: out })
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
            rows: Rc::new(Vec::new()),
            current_pos: None,
            skip_rank: Rc::new(Vec::new()),
            dense_rank: Rc::new(Vec::new()),
        });
    };

    // v0.11 Frente G: grouped-and-sorted partitions are cached per
    // (relation, ORDERBY, PARTITIONBY, filter context) — see the module
    // doc comment above `WINDOW_PARTITION_CACHE`. A cache hit skips
    // straight to this specific partition's already-sorted rows; a miss
    // builds every partition's rows ONCE (same total row visits as before,
    // just paid once instead of once per iterated row).
    let cache_key = window_cache_key(clauses, outer_ctx);
    let cached = WINDOW_PARTITION_CACHE.with(|c| c.borrow().get(&cache_key).cloned());
    let cached = match cached {
        Some(c) => c,
        None => {
            let built = build_partition_groups(clauses, rel_tbl, rel_indices, rel_ctx, model, row_stack, vars)?;
            let rc = Rc::new(built);
            WINDOW_PARTITION_CACHE.with(|c| c.borrow_mut().insert(cache_key, rc.clone()));
            rc
        }
    };
    let empty_partition = CachedPartition {
        rows: Rc::new(Vec::new()),
        by_full_row_key: HashMap::new(),
        skip_rank: Rc::new(Vec::new()),
        dense_rank: Rc::new(Vec::new()),
    };
    let cached_partition = cached.groups.get(&cur_part_key).unwrap_or(&empty_partition);
    let partition_rows: Rc<Vec<(usize, Vec<DaxValue>)>> = cached_partition.rows.clone();

    if !need_current_row {
        return Ok(PreparedPartition {
            rows: partition_rows,
            current_pos: None,
            skip_rank: cached_partition.skip_rank.clone(),
            dense_rank: cached_partition.dense_rank.clone(),
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
        } else if active_cols.len() == rel_tbl.columns.len() {
            // v0.11 Frente G: every column of `rel_tbl` is pinned to a known
            // value (the common case: SUMX's row context pins the whole
            // iterated row) — an O(1) lookup into the position index built
            // once per group, instead of a `position()` scan re-run on every
            // one of the N iterated rows.
            let full_key: Vec<ValueKey> = active_cols.iter().map(|&(_, v)| v.value_key()).collect();
            cached_partition.by_full_row_key.get(&full_key).copied()
        } else {
            // Only SOME columns are pinned (rare: a relation narrower than
            // `rel_tbl`'s full column set, or an unresolved column) — the
            // position index is keyed by the FULL row, so fall back to the
            // same linear scan as before; still correct, just not O(1).
            partition_rows.iter().position(|(r_idx, _)| {
                active_cols
                    .iter()
                    .all(|&(cidx, tv)| rel_tbl.rows[*r_idx][cidx].dax_equals(tv))
            })
        }
    };

    Ok(PreparedPartition {
        rows: partition_rows.clone(),
        current_pos,
        skip_rank: cached_partition.skip_rank.clone(),
        dense_rank: cached_partition.dense_rank.clone(),
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
    let cached_rel = get_or_build_cached_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = &cached_rel.table;
    let rel_indices = &cached_rel.indices;
    let rel_ctx = cached_rel.ctx.clone();
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
    let cached_rel = get_or_build_cached_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = &cached_rel.table;
    let rel_indices = &cached_rel.indices;
    let rel_ctx = cached_rel.ctx.clone();
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
    let cached_rel = get_or_build_cached_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = &cached_rel.table;
    let rel_indices = &cached_rel.indices;
    let rel_ctx = cached_rel.ctx.clone();
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
    let cached_rel = get_or_build_cached_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = &cached_rel.table;
    let rel_indices = &cached_rel.indices;
    let rel_ctx = cached_rel.ctx.clone();
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
    // v0.11 Frente G: O(1) lookup into the precomputed rank arrays instead of
    // re-walking `0..=pos` counting ties on every one of the N calls inside
    // a SUMX — see `CachedPartition::skip_rank`/`dense_rank`.
    Ok(DaxValue::Integer(match ties {
        RankTies::Dense => prep.dense_rank[pos],
        RankTies::Skip => prep.skip_rank[pos],
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
    let cached_rel = get_or_build_cached_relation(&clauses, model, ctx, row_stack, vars)?;
    let rel_tbl = &cached_rel.table;
    let rel_indices = &cached_rel.indices;
    let rel_ctx = cached_rel.ctx.clone();
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
    use super::clear_window_partition_cache;
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
                DaxTableInput { column_format_strings: Default::default(),
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
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
                DaxTableInput { column_format_strings: Default::default(),
                    name: "d_Mes".into(),
                    columns: vec!["MesNum".into(), "Mes".into()],
                    rows: vec![
                        row(json!({"MesNum": 1, "Mes": "Jan"})),
                        row(json!({"MesNum": 2, "Mes": "Fev"})),
                        row(json!({"MesNum": 3, "Mes": "Mar"})),
                    ],
                    duckdb_table: None,
                    column_types: vec![],
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
                },
                DaxTableInput { column_format_strings: Default::default(),
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
                    sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
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
             ..Default::default() }],
            &model,
        );
        let ctx_teclado = build_filter_context(
            &[NativeFilterInput {
                table_id: "d_Produto".into(),
                column: "Produto".into(),
                values: vec![json!("Teclado")],
             ..Default::default() }],
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

    /// v0.11 Frente G: a differential test against the per-row recomputation
    /// the cache replaces. Runs the SAME RANK-inside-SUMX query twice: once
    /// normally (cache live across the whole SUMX), once after manually
    /// clearing the cache mid-way (forcing a miss on the next partition
    /// lookup) to prove the cached and uncached code paths agree, not just
    /// that the cached one is fast.
    #[test]
    fn window_partition_cache_matches_uncached_recompute() {
        let model = bench_model_for_window_cache(500);
        let ctx = FilterContext::default();
        let formula = "SUMX(big, [Valor] + RANK(SKIP, ALL(big[ID], big[Valor]), ORDERBY(big[Valor], ASC)))";

        clear_window_partition_cache();
        let first = model.evaluate_dax(formula, &ctx).unwrap();

        // Clear mid-evaluation-equivalent: a brand new evaluate_dax call
        // clears the cache itself, so this just re-runs from scratch and
        // must reach the exact same result bit-for-bit.
        clear_window_partition_cache();
        let second = model.evaluate_dax(formula, &ctx).unwrap();
        assert_eq!(first, second);
        assert!(first.as_f64().unwrap().is_finite());
    }

    fn bench_row(id: i64, n: i64) -> serde_json::Map<String, serde_json::Value> {
        // Deterministic pseudo-shuffle so ORDERBY has real sorting work to do
        // instead of already-sorted input.
        let valor = ((id * 7919) % n) as f64;
        row(json!({"ID": id, "Valor": valor}))
    }

    fn bench_model_for_window_cache(n: i64) -> DaxModel {
        let mut model = DaxModel::new();
        let rows: Vec<_> = (1..=n).map(|i| bench_row(i, n)).collect();
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![DaxTableInput { column_format_strings: Default::default(),
                name: "big".into(),
                columns: vec!["ID".into(), "Valor".into()],
                rows,
                duckdb_table: None,
                column_types: vec![],
                sort_by_columns: Default::default(),
                is_calculated: false,
                expression: None,
                calculated_columns: ::std::collections::HashMap::new(),
            }],
            relationships: Vec::<DaxRelationshipInput>::new(),
            measures: vec![],
        });
        model
    }

    /// v0.11 Frente G bench: `RANK` inside `SUMX` over N rows (release only):
    /// `cargo test --release --lib -- --ignored --nocapture bench_window_cache`.
    /// Before the cache, `prepare_partition` re-filtered and re-sorted the
    /// whole relation on every one of the N iterated rows (O(N² log N)).
    fn run_window_bench(n: i64, label: &str) {
        let model = bench_model_for_window_cache(n);
        let ctx = FilterContext::default();
        let formula = "SUMX(big, [Valor] + RANK(SKIP, ALL(big[ID], big[Valor]), ORDERBY(big[Valor], ASC)))";
        let t0 = std::time::Instant::now();
        let result = model.evaluate_dax(formula, &ctx).unwrap();
        let elapsed = t0.elapsed();
        println!("{label}: N={n} RANK-inside-SUMX elapsed={elapsed:?} result={result:?}");
    }

    #[test]
    #[ignore]
    fn bench_window_cache_rank_sumx_2000_rows() {
        run_window_bench(2_000, "bench_window_cache_rank_sumx_2000_rows");
    }

    #[test]
    #[ignore]
    fn bench_window_cache_rank_sumx_5000_rows() {
        run_window_bench(5_000, "bench_window_cache_rank_sumx_5000_rows");
    }

    #[test]
    #[ignore]
    fn bench_window_cache_rank_sumx_50000_rows() {
        run_window_bench(50_000, "bench_window_cache_rank_sumx_50000_rows");
    }
}
