//! Row-context functions: RELATED (relationship lookups) and EARLIER/EARLIEST.

use super::*;

/// DAX `EARLIER`, `EARLIEST`.
pub(super) fn eval_earlier(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let depth = if fname == "EARLIEST" {
        None
    } else {
        let n = match args.get(1) {
            Some(e) => evaluate_ast(e, model, ctx, row_stack, vars)?.as_f64().unwrap_or(1.0),
            None => 1.0,
        };
        if n < 1.0 || n.fract() != 0.0 {
            return Err(format!("EARLIER: o número de níveis deve ser um inteiro >= 1 (recebido {})", n));
        }
        Some(n as usize)
    };
    earlier_value(args.first(), row_stack, depth)
}

/// DAX `RELATED`.
pub(super) fn eval_related(
    args: &[Expr],
    model: &DaxModel,
    row_stack: &[RowContext<'_>],
) -> Result<DaxValue, String> {
    let Some(Expr::TableColRef { table, column }) = args.first() else {
        return Err("RELATED requer Tabela[Coluna]".to_string());
    };
    if let Some(rc) = row_stack.last() {
        Ok(resolve_related(model, rc, table, column).unwrap_or(DaxValue::Blank))
    } else {
        Ok(DaxValue::Blank)
    }
}

/// EARLIER(col, n) / EARLIEST(col): value of `col` in an *outer* row context.
///
/// `row_stack` is ordered outermost -> innermost; every iterator (SUMX, FILTER, ADDCOLUMNS,
/// RANKX, ...) pushes its current row before evaluating its per-row expression. Only row
/// contexts that actually contain `col` count as levels: the innermost such context is
/// level 0 (what a plain `T[col]` reads), EARLIER(col, n) reads level `n` and EARLIEST
/// (`depth = None`) the outermost level. Both require at least one outer level, as in DAX.
pub(super) fn earlier_value(
    arg: Option<&Expr>,
    row_stack: &[RowContext<'_>],
    depth: Option<usize>,
) -> Result<DaxValue, String> {
    let levels: Vec<(&RowContext<'_>, usize)> = match arg {
        Some(Expr::TableColRef { table, column }) => row_stack
            .iter()
            .rev()
            .filter_map(|rc| row_col_idx(rc.table, table, column).map(|c| (rc, c)))
            .collect(),
        Some(Expr::BracketRef(column)) => row_stack
            .iter()
            .rev()
            .filter_map(|rc| bare_col_idx(rc.table, column).map(|c| (rc, c)))
            .collect(),
        _ => return Err("EARLIER/EARLIEST requer uma referência de coluna".to_string()),
    };
    let level = match depth {
        Some(n) => n,
        None => levels.len().saturating_sub(1).max(1),
    };
    match levels.get(level) {
        Some((rc, cidx)) => Ok(rc.table.rows[rc.row_idx][*cidx].clone()),
        None => Err("EARLIER/EARLIEST refere-se a um contexto de linha anterior que não existe".to_string()),
    }
}

pub(super) fn resolve_related(
    model: &DaxModel,
    rc: &RowContext<'_>,
    target_table: &str,
    target_col: &str,
) -> Option<DaxValue> {
    resolve_related_hops(model, rc, target_table, target_col, 4)
}

/// RELATED lookup following active many-to-one relationships from the row's table; a
/// direct hop to `target_table` wins, otherwise chains (fact -> dim -> sub-dim) are walked
/// depth-first up to `hops_left` relationships.
pub(super) fn resolve_related_hops(
    model: &DaxModel,
    rc: &RowContext<'_>,
    target_table: &str,
    target_col: &str,
    hops_left: usize,
) -> Option<DaxValue> {
    if hops_left == 0 {
        return None;
    }
    let outgoing = || {
        model
            .relationships
            .iter()
            .filter(|rel| rel.is_active && rel.from_table.eq_ignore_ascii_case(&rc.table.name))
    };
    // Row of the "one" side matched by the current row's foreign key.
    let follow = |rel: &crate::dax::DaxRelationshipInput| -> Option<(&DaxTableData, usize)> {
        let fk_idx = rc.table.get_col_idx(&rel.from_column)?;
        let fk_key = rc.table.rows[rc.row_idx][fk_idx].canonical_key();
        let to_tbl = model.get_table(&rel.to_table)?;
        let pk_idx = to_tbl.get_col_idx(&rel.to_column)?;
        let row = to_tbl.rows.iter().position(|row| row[pk_idx].canonical_key() == fk_key)?;
        Some((to_tbl, row))
    };
    for rel in outgoing().filter(|rel| rel.to_table.eq_ignore_ascii_case(target_table)) {
        if let Some((to_tbl, row)) = follow(rel) {
            let val_idx = to_tbl.get_col_idx(target_col)?;
            return Some(to_tbl.rows[row][val_idx].clone());
        }
    }
    for rel in outgoing().filter(|rel| !rel.to_table.eq_ignore_ascii_case(target_table)) {
        if let Some((to_tbl, row)) = follow(rel) {
            let next = RowContext { table: to_tbl, row_idx: row };
            if let Some(v) = resolve_related_hops(model, &next, target_table, target_col, hops_left - 1) {
                return Some(v);
            }
        }
    }
    None
}
