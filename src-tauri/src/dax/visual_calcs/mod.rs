//! v0.11 Frente E (Rodada 2): Visual Calculations computed in Rust over the
//! visual's own result grid (ROWS/COLUMNS), instead of the TS port in
//! `src/engine/functions/visualCalculations.ts` (kept as the browser-only
//! fallback — see docs/V011.md Rodada 2, Frente E, item 5).
//!
//! These functions operate on a flat "grid" of already-evaluated numeric
//! cells (one per visual row/column, in display order) plus the index of
//! the "current" cell, exactly like the TS functions operate on
//! `context.visualRows` + `context.visualRowIndex`. Porting at this level
//! (not re-parsing the whole visual-calc formula into a DAX AST) keeps the
//! semantics identical to the already-shipped TS engine while giving a
//! typed, directly testable Rust surface that `query_planner.rs` can later
//! splice into a visual's evaluated column.
//!
//! Semantics follow Microsoft's "Visual calculations" docs
//! (https://learn.microsoft.com/power-bi/transform-model/desktop-visual-calculations-overview)
//! and mirror the TS implementation function-for-function.

use serde::{Deserialize, Serialize};

/// One cell of the grid a visual calculation runs over. `None` is BLANK.
pub type GridValue = Option<f64>;

/// RUNNINGSUM(column): cumulative sum of `grid[0..=index]`, BLANK-safe (BLANK
/// cells are skipped, not treated as 0 — matches TS `daxAdd`/`isBlank` logic).
pub fn running_sum(grid: &[GridValue], index: usize) -> GridValue {
    if grid.is_empty() || index >= grid.len() {
        return None;
    }
    let mut sum: Option<f64> = None;
    for v in &grid[0..=index] {
        if let Some(n) = v {
            sum = Some(sum.unwrap_or(0.0) + n);
        }
    }
    sum
}

/// MOVINGAVERAGE(column, windowSize): average of the last `window_size`
/// cells ending at `index` (clamped at the start of the grid), ignoring
/// BLANK cells in both the sum and the count. BLANK (no non-blank cell in
/// the window) if the window has no data.
pub fn moving_average(grid: &[GridValue], index: usize, window_size: usize) -> GridValue {
    if grid.is_empty() || index >= grid.len() {
        return None;
    }
    let window_size = window_size.max(1);
    let start = index.saturating_sub(window_size - 1);
    let mut sum = 0.0;
    let mut count = 0usize;
    for v in &grid[start..=index] {
        if let Some(n) = v {
            sum += n;
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some(sum / count as f64)
    }
}

/// PREVIOUS(column, [offset]): value `offset` rows before `index` (default
/// 1). BLANK before the start of the grid.
pub fn previous(grid: &[GridValue], index: usize, offset: usize) -> GridValue {
    let offset = offset.max(1);
    let target = index.checked_sub(offset)?;
    grid.get(target).copied().flatten()
}

/// NEXT(column, [offset]): value `offset` rows after `index` (default 1).
/// BLANK past the end of the grid.
pub fn next(grid: &[GridValue], index: usize, offset: usize) -> GridValue {
    let offset = offset.max(1);
    let target = index.checked_add(offset)?;
    grid.get(target).copied().flatten()
}

/// FIRST(column): value of the first row in the visual's scope.
pub fn first(grid: &[GridValue]) -> GridValue {
    grid.first().copied().flatten()
}

/// LAST(column): value of the last row in the visual's scope.
pub fn last(grid: &[GridValue]) -> GridValue {
    grid.last().copied().flatten()
}

/// RANGE(from, to): a *relative* window spec (e.g. `RANGE(-2, 0)`), resolved
/// by the caller against the current index — Power BI uses it as the 3rd
/// argument of windowed functions, not evaluated standalone. Returned as a
/// typed struct instead of a magic `__isVisualRange` object (as the TS side
/// does) so Rust callers get compile-time checked bounds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisualRange {
    pub from: i64,
    pub to: i64,
}

/// Resolves a `RANGE(from, to)` window at `index` into absolute [start, end]
/// grid indices (clamped to the grid), inclusive, or `None` if the window
/// falls entirely outside the grid.
pub fn resolve_range(range: VisualRange, index: usize, len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }
    let idx = index as i64;
    let start = (idx + range.from).max(0);
    let end = (idx + range.to).min(len as i64 - 1);
    if start > end {
        return None;
    }
    Some((start as usize, end as usize))
}

/// COLLAPSE(column) / COLLAPSEALL(column): sum of every (visible) cell in
/// the grid — the TS implementation treats both identically today (no
/// partial-hierarchy distinction yet), so this single function backs both.
pub fn collapse_sum(grid: &[GridValue]) -> f64 {
    grid.iter().filter_map(|v| *v).sum()
}

/// EXPAND(column) / EXPANDALL(column): the current row's own (unaggregated)
/// value — i.e. a pass-through of the cell at `index`. Same function backs
/// both today, mirroring the TS implementation.
pub fn expand_value(grid: &[GridValue], index: usize) -> GridValue {
    grid.get(index).copied().flatten()
}

#[cfg(test)]
mod tests;
