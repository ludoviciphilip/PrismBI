use super::*;

// Hand-derived grid: RUNNINGSUM([Total Vendas]) over monthly sales
// 100, 200, BLANK, 50 -> running sums 100, 300, 300 (blank skipped, not 0), 350
const GRID: [GridValue; 4] = [Some(100.0), Some(200.0), None, Some(50.0)];

#[test]
fn running_sum_matches_hand_derived_sequence() {
    assert_eq!(running_sum(&GRID, 0), Some(100.0));
    assert_eq!(running_sum(&GRID, 1), Some(300.0));
    assert_eq!(running_sum(&GRID, 2), Some(300.0)); // BLANK doesn't reset or zero the sum
    assert_eq!(running_sum(&GRID, 3), Some(350.0));
}

#[test]
fn running_sum_empty_grid_is_blank() {
    assert_eq!(running_sum(&[], 0), None);
}

#[test]
fn moving_average_window_3_hand_derived() {
    // window=3 at index 3 covers indices 1..=3 = [200, BLANK, 50] -> avg of
    // non-blank cells only = (200+50)/2 = 125 (BLANK doesn't count toward
    // the denominator, matching the TS `count++` only on non-blank).
    assert_eq!(moving_average(&GRID, 3, 3), Some(125.0));
    // window=1 at index 0 -> just cell 0 = 100
    assert_eq!(moving_average(&GRID, 0, 1), Some(100.0));
    // window=2 at index 1 covers [100, 200] -> avg 150
    assert_eq!(moving_average(&GRID, 1, 2), Some(150.0));
    // window clamps at the start of the grid: window=10 at index 1 covers [0,1] -> 150
    assert_eq!(moving_average(&GRID, 1, 10), Some(150.0));
}

#[test]
fn moving_average_window_all_blank_is_blank() {
    let g: [GridValue; 2] = [None, None];
    assert_eq!(moving_average(&g, 1, 2), None);
}

#[test]
fn previous_and_next_hand_derived() {
    // previous([Total], 1) at index 1 -> grid[0] = 100
    assert_eq!(previous(&GRID, 1, 1), Some(100.0));
    // previous at index 0 -> before the grid -> BLANK
    assert_eq!(previous(&GRID, 0, 1), None);
    // previous with offset 2 at index 3 -> grid[1] = 200
    assert_eq!(previous(&GRID, 3, 2), Some(200.0));
    // previous landing on a BLANK cell returns BLANK (not skipped)
    assert_eq!(previous(&GRID, 3, 1), None); // grid[2] is None

    // next([Total], 1) at index 0 -> grid[1] = 200
    assert_eq!(next(&GRID, 0, 1), Some(200.0));
    // next at the last index -> past the grid -> BLANK
    assert_eq!(next(&GRID, 3, 1), None);
    // next with offset 2 at index 1 -> grid[3] = 50
    assert_eq!(next(&GRID, 1, 2), Some(50.0));
}

#[test]
fn first_and_last_hand_derived() {
    assert_eq!(first(&GRID), Some(100.0));
    assert_eq!(last(&GRID), Some(50.0));
    assert_eq!(first(&[]), None);
}

#[test]
fn range_resolves_relative_window_clamped_to_grid() {
    // RANGE(-2, 0) at index 3 of a 4-cell grid -> indices 1..=3
    let r = VisualRange { from: -2, to: 0 };
    assert_eq!(resolve_range(r, 3, 4), Some((1, 3)));

    // RANGE(-2, 0) at index 0 -> clamps start to 0 -> indices 0..=0
    assert_eq!(resolve_range(r, 0, 4), Some((0, 0)));

    // RANGE(1, 5) at index 3 of a 4-cell grid (indices 0..3) -> end clamps to 3,
    // start = 4 > end(3) -> no valid window -> None
    let r2 = VisualRange { from: 1, to: 5 };
    assert_eq!(resolve_range(r2, 3, 4), None);

    // Empty grid never has a window
    assert_eq!(resolve_range(r, 0, 0), None);
}

#[test]
fn collapse_sum_hand_derived() {
    // COLLAPSE/COLLAPSEALL sum every visible cell: 100+200+50 = 350 (BLANK skipped)
    assert_eq!(collapse_sum(&GRID), 350.0);
    assert_eq!(collapse_sum(&[]), 0.0);
}

#[test]
fn expand_value_passes_through_current_cell() {
    assert_eq!(expand_value(&GRID, 1), Some(200.0));
    assert_eq!(expand_value(&GRID, 2), None); // BLANK cell stays BLANK
    assert_eq!(expand_value(&GRID, 99), None); // out of range -> BLANK
}
