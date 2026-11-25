//! Rodada 2 (docs/ETL_V09.md §"Rodada 2"): new step ops, new expression
//! functions, `_` in transformColumns, and `etl_step_m`. Expected values are
//! derived by hand from Power Query semantics, like the rest of this suite.

use super::*;
use crate::etl::commands::etl_step_m_impl;
use crate::etl::m_from::query_from_m;
use crate::etl::m_to::query_to_m;
use serde_json::json;

const EVIL_COL: &str = "a\"; DROP TABLE x; --";

fn custom(new_column: &str, expr: &str) -> StepKind {
    StepKind::AddCustomColumn { new_column: new_column.into(), expression: expr.into(), ty: None }
}

// ------------------------------------------------------------ steps

#[test]
fn transform_columns_applies_expression_and_flags_errors() {
    let src = text_source(&["t"], &[&[Some("10")], &[Some("x")], &[Some("3.7")]]);
    let tc = StepKind::TransformColumns { columns: strs(&["t"]), expression: "Number.From(_)".into(), ty: None };
    let r = run(&query(src, vec![tc.clone()]));
    assert_eq!(r.types, vec!["decimal"]);
    assert_rows(&r, &[json!([10.0]), json!([null]), json!([3.7])]);

    let src2 = text_source(&["t"], &[&[Some("10")], &[Some("x")], &[Some("3.7")]]);
    let r2 = run(&query(src2, vec![tc, StepKind::RemoveRowsWithErrors { columns: None }]));
    assert_rows(&r2, &[json!([10.0]), json!([3.7])]);
}

#[test]
fn transform_columns_text_start_multiple_columns() {
    let src = text_source(&["a", "b"], &[&[Some("hello"), Some("world")]]);
    let q = query(
        src,
        vec![StepKind::TransformColumns {
            columns: strs(&["a", "b"]),
            expression: "Text.Start(_, 3)".into(),
            ty: Some(EtlType::Text),
        }],
    );
    let r = run(&q);
    assert_rows(&r, &[json!(["hel", "wor"])]);
}

#[test]
fn transform_columns_rejects_underscore_outside_its_scope() {
    // `_` is only bound inside transformColumns, not addCustomColumn.
    let src = text_source(&["a"], &[&[Some("1")]]);
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(src, vec![custom("b", "_")]);
    let p = preview(&db, &q, None);
    assert!(p.res.failed_step.is_some());
}

#[test]
fn reorder_columns_moves_and_validates() {
    let src = text_source(&["a", "b", "c"], &[&[Some("1"), Some("2"), Some("3")]]);
    let r = run(&query(src, vec![StepKind::ReorderColumns { columns: strs(&["c", "a", "b"]) }]));
    assert_cols(&r, &["c", "a", "b"]);
    assert_rows(&r, &[json!(["3", "1", "2"])]);

    let db = DuckDbStore::new_in_memory().unwrap();
    let bad = query(
        text_source(&["a", "b"], &[&[Some("1"), Some("2")]]),
        vec![StepKind::ReorderColumns { columns: strs(&["a"]) }],
    );
    assert!(preview(&db, &bad, None).res.failed_step.is_some());
}

#[test]
fn demote_headers_inserts_header_row_and_renames_columns() {
    let src = text_source(&["Nome", "Valor"], &[&[Some("Ana"), Some("10")], &[Some("Bia"), Some("20")]]);
    let r = run(&query(src, vec![StepKind::DemoteHeaders]));
    assert_cols(&r, &["Column1", "Column2"]);
    assert_rows(&r, &[json!(["Nome", "Valor"]), json!(["Ana", "10"]), json!(["Bia", "20"])]);
}

#[test]
fn transpose_swaps_rows_and_columns_as_text() {
    let src = text_source(&["a", "b", "c"], &[&[Some("1"), Some("2"), Some("3")], &[Some("4"), Some("5"), Some("6")]]);
    let r = run(&query(src, vec![StepKind::Transpose]));
    assert_cols(&r, &["Column1", "Column2"]);
    assert_rows(&r, &[json!(["1", "4"]), json!(["2", "5"]), json!(["3", "6"])]);
}

#[test]
fn reverse_rows_flips_order() {
    let src = text_source(&["v"], &[&[Some("1")], &[Some("2")], &[Some("3")]]);
    let r = run(&query(src, vec![StepKind::ReverseRows]));
    assert_rows(&r, &[json!(["3"]), json!(["2"]), json!(["1"])]);
}

#[test]
fn keep_range_slices_by_zero_based_offset() {
    let src = text_source(&["v"], &[&[Some("10")], &[Some("20")], &[Some("30")], &[Some("40")]]);
    let r = run(&query(src, vec![StepKind::KeepRange { offset: 1, count: 2 }]));
    assert_rows(&r, &[json!(["20"]), json!(["30"])]);
}

#[test]
fn remove_alternate_rows_cycles_after_the_offset() {
    let src = text_source(
        &["v"],
        &[
            &[Some("1")],
            &[Some("2")],
            &[Some("3")],
            &[Some("4")],
            &[Some("5")],
            &[Some("6")],
            &[Some("7")],
            &[Some("8")],
        ],
    );
    let r = run(&query(src, vec![StepKind::RemoveAlternateRows { offset: 1, remove: 1, keep: 2 }]));
    // offset 1 kept; then cycles of (remove 1, keep 2): remove 2, keep 3&4, remove 5, keep 6&7, remove 8.
    assert_rows(&r, &[json!(["1"]), json!(["3"]), json!(["4"]), json!(["6"]), json!(["7"])]);
}

#[test]
fn keep_duplicates_keeps_rows_whose_key_repeats() {
    let src = text_source(&["k"], &[&[Some("a")], &[Some("b")], &[Some("a")], &[Some("c")], &[Some("b")]]);
    let r = run(&query(src, vec![StepKind::KeepDuplicates { columns: None }]));
    assert_rows(&r, &[json!(["a"]), json!(["b"]), json!(["a"]), json!(["b"])]);
}

#[test]
fn replace_errors_clears_the_flag() {
    let src = text_source(&["t"], &[&[Some("10")], &[Some("x")], &[Some("20")]]);
    let ct = StepKind::ChangeType { columns: vec![ColumnTypeSpec { column: "t".into(), ty: EtlType::Integer }], culture: None };
    let re = StepKind::ReplaceErrors { replacements: vec![ReplaceErrorSpec { column: "t".into(), value: n(-1.0) }] };
    let r = run(&query(src.clone(), vec![ct.clone(), re.clone()]));
    assert_rows(&r, &[json!([10]), json!([-1]), json!([20])]);
    // Flag cleared: removeRowsWithErrors afterwards keeps all 3 rows.
    let r2 = run(&query(src, vec![ct, re, StepKind::RemoveRowsWithErrors { columns: None }]));
    assert_rows(&r2, &[json!([10]), json!([-1]), json!([20])]);
}

#[test]
fn keep_rows_with_errors_keeps_only_flagged_rows() {
    let src = text_source(&["t"], &[&[Some("10")], &[Some("x")], &[Some("20")]]);
    let ct = StepKind::ChangeType { columns: vec![ColumnTypeSpec { column: "t".into(), ty: EtlType::Integer }], culture: None };
    let r = run(&query(src, vec![ct, StepKind::KeepRowsWithErrors { columns: None }]));
    assert_rows(&r, &[json!([null])]);
}

// ------------------------------------------------------------ M round-trip

fn round_trips(q: &EtlQuery) {
    let m = query_to_m(q).expect("to_m");
    let back = query_from_m(&q.name, &m).expect("from_m").query;
    assert_eq!(back.without_ids(), q.without_ids(), "M:\n{}", m);
}

#[test]
fn round2_steps_round_trip_through_m() {
    let src = EtlSource::table_ref("Base");
    let cases: Vec<StepKind> = vec![
        StepKind::TransformColumns { columns: strs(&["a", "b"]), expression: "Text.Start(_, 3)".into(), ty: Some(EtlType::Text) },
        StepKind::TransformColumns { columns: strs(&["a"]), expression: "_ * 2".into(), ty: None },
        StepKind::ReorderColumns { columns: strs(&["b", "a"]) },
        StepKind::DemoteHeaders,
        StepKind::Transpose,
        StepKind::ReverseRows,
        StepKind::KeepRange { offset: 2, count: 5 },
        StepKind::RemoveAlternateRows { offset: 1, remove: 2, keep: 3 },
        StepKind::KeepDuplicates { columns: None },
        StepKind::KeepDuplicates { columns: Some(strs(&["a", "b"])) },
        StepKind::ReplaceErrors { replacements: vec![ReplaceErrorSpec { column: "a".into(), value: n(0.0) }] },
        StepKind::KeepRowsWithErrors { columns: None },
        StepKind::KeepRowsWithErrors { columns: Some(strs(&["a"])) },
    ];
    for kind in cases {
        round_trips(&query(src.clone(), vec![kind]));
    }
}

#[test]
fn etl_step_m_formats_formula_bar_expression() {
    let q = query(
        EtlSource::table_ref("Base"),
        vec![StepKind::RenameColumns { renames: vec![("a".into(), "b".into())] }, StepKind::ReverseRows],
    );
    let expr = etl_step_m_impl(&q, 1).unwrap();
    assert!(expr.starts_with("= Table.ReverseRows("));
    assert!(expr.contains("Renamed Columns"), "{}", expr);
}

// ------------------------------------------------------------ new functions

fn nums1(v: &str) -> EtlSource {
    text_source(&["x"], &[&[Some(v)]])
}

#[test]
fn text_functions_round2() {
    let cases: &[(&str, &str, &str)] = &[
        ("Text.BeforeDelimiter([x], \"-\")", "a-b-c", "a"),
        ("Text.AfterDelimiter([x], \"-\")", "a-b-c", "b-c"),
        ("Text.BetweenDelimiters([x], \"[\", \"]\")", "[abc]", "abc"),
        ("Text.PadStart([x], 3, \"0\")", "5", "005"),
        ("Text.PadEnd([x], 3, \"0\")", "5", "500"),
        ("Text.Replace([x], \"a\", \"X\")", "abcabc", "XbcXbc"),
        ("Text.Reverse([x])", "abc", "cba"),
        ("Text.Repeat([x], 3)", "ab", "ababab"),
    ];
    for (expr, input, expected) in cases {
        let r = run(&query(nums1(input), vec![custom("y", expr)]));
        assert_rows(&r, &[json!([input, expected])]);
    }
    let r = run(&query(nums1("a\u{1}b"), vec![custom("y", "Text.Clean([x])")]));
    assert_rows(&r, &[json!(["a\u{1}b", "ab"])]);
}

#[test]
fn number_functions_round2() {
    let cases: &[(&str, f64, f64)] = &[
        ("Number.Power([x], 10)", 2.0, 1024.0),
        ("Number.Sqrt([x])", 16.0, 4.0),
        ("Number.Mod([x], 3)", 7.0, 1.0),
        ("Number.Mod([x], 3)", -7.0, 2.0),
        ("Number.RoundUp([x])", 2.1, 3.0),
        ("Number.RoundUp([x])", -2.1, -2.0),
        ("Number.RoundDown([x])", 2.9, 2.0),
        ("Number.RoundDown([x])", -2.1, -3.0),
        ("Number.Sign([x])", -5.0, -1.0),
        ("Number.Sign([x])", 0.0, 0.0),
        ("Number.Ln([x])", 1.0, 0.0),
        ("Number.Log10([x])", 100.0, 2.0),
        ("Number.Exp([x])", 0.0, 1.0),
    ];
    for (expr, input, expected) in cases {
        let r = run(&query(EtlSource::inline(strs(&["x"]), vec![vec![n(*input)]]), vec![custom("y", expr)]));
        assert_rows(&r, &[json!([input, expected])]);
    }
    let int_cases: &[(&str, f64, i64)] = &[
        ("Number.IntegerDivide([x], 3)", 7.0, 2),
        ("Number.IntegerDivide([x], 3)", -7.0, -3),
    ];
    for (expr, input, expected) in int_cases {
        let r = run(&query(EtlSource::inline(strs(&["x"]), vec![vec![n(*input)]]), vec![custom("y", expr)]));
        assert_rows(&r, &[json!([input, expected])]);
    }
    let bool_cases: &[(&str, f64, bool)] =
        &[("Number.IsEven([x])", 4.0, true), ("Number.IsEven([x])", 3.0, false), ("Number.IsOdd([x])", 3.0, true)];
    for (expr, input, expected) in bool_cases {
        let r = run(&query(EtlSource::inline(strs(&["x"]), vec![vec![n(*input)]]), vec![custom("y", expr)]));
        assert_rows(&r, &[json!([input, expected])]);
    }
}

fn date_src(d: &str, ty: EtlType) -> EtlQuery {
    query(
        text_source(&["d"], &[&[Some(d)]]),
        vec![StepKind::ChangeType { columns: vec![ColumnTypeSpec { column: "d".into(), ty }], culture: None }],
    )
}

fn with_custom(mut q: EtlQuery, new_column: &str, expr: &str, ty: Option<EtlType>) -> EtlQuery {
    q.steps.push(step(StepKind::AddCustomColumn { new_column: new_column.into(), expression: expr.into(), ty }));
    q
}

#[test]
fn date_functions_round2() {
    let r = run(&with_custom(date_src("2024-01-08", EtlType::Date), "w", "Date.WeekOfYear([d])", None));
    assert_rows(&r, &[json!(["2024-01-08", 2])]);

    let r = run(&with_custom(date_src("2024-03-01", EtlType::Date), "y", "Date.DayOfYear([d])", None));
    assert_rows(&r, &[json!(["2024-03-01", 61])]);

    let r = run(&with_custom(date_src("2024-02-15", EtlType::Date), "n", "Date.DaysInMonth([d])", None));
    assert_rows(&r, &[json!(["2024-02-15", 29])]);

    let r = run(&with_custom(date_src("2024-03-15", EtlType::Date), "s", "Date.StartOfMonth([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-03-15", "2024-03-01"])]);
    let r = run(&with_custom(date_src("2024-03-15", EtlType::Date), "e", "Date.EndOfMonth([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-03-15", "2024-03-31"])]);

    let r = run(&with_custom(date_src("2024-05-10", EtlType::Date), "s", "Date.StartOfQuarter([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-05-10", "2024-04-01"])]);
    let r = run(&with_custom(date_src("2024-05-10", EtlType::Date), "e", "Date.EndOfQuarter([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-05-10", "2024-06-30"])]);

    let r = run(&with_custom(date_src("2024-05-10", EtlType::Date), "s", "Date.StartOfYear([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-05-10", "2024-01-01"])]);
    let r = run(&with_custom(date_src("2024-05-10", EtlType::Date), "e", "Date.EndOfYear([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-05-10", "2024-12-31"])]);

    // 2024-01-01 is a Monday; default week start is Sunday.
    let r = run(&with_custom(date_src("2024-01-01", EtlType::Date), "s", "Date.StartOfWeek([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-01-01", "2023-12-31"])]);
    let r = run(&with_custom(date_src("2024-01-01", EtlType::Date), "e", "Date.EndOfWeek([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-01-01", "2024-01-06"])]);

    let r = run(&with_custom(date_src("2024-03-15", EtlType::Date), "a", "Date.AddYears([d], 1)", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-03-15", "2025-03-15"])]);
    let r = run(&with_custom(date_src("2024-01-15", EtlType::Date), "a", "Date.AddQuarters([d], 1)", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-01-15", "2024-04-15"])]);
    let r = run(&with_custom(date_src("2024-01-01", EtlType::Date), "a", "Date.AddWeeks([d], 1)", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-01-01", "2024-01-08"])]);

    let r = run(&with_custom(date_src("2000-01-01", EtlType::Date), "c", "Date.IsInCurrentYear([d])", None));
    assert_rows(&r, &[json!(["2000-01-01", false])]);
}

#[test]
fn datetime_functions_round2() {
    let q = query(
        text_source(&["d"], &[&[Some("2024-03-15 13:45:30")]]),
        vec![StepKind::ChangeType { columns: vec![ColumnTypeSpec { column: "d".into(), ty: EtlType::Datetime }], culture: None }],
    );
    let r = run(&with_custom(q.clone(), "dd", "DateTime.Date([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-03-15 13:45:30", "2024-03-15"])]);
    let r = run(&with_custom(q, "tt", "DateTime.Time([d])", Some(EtlType::Text)));
    assert_rows(&r, &[json!(["2024-03-15 13:45:30", "13:45:30"])]);
}

#[test]
fn datetime_localnow_executes_without_failing() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(text_source(&["x"], &[&[Some("1")]]), vec![custom("now", "DateTime.LocalNow()")]);
    let p = preview(&db, &q, None);
    assert!(p.res.failed_step.is_none(), "{:?}", p.res.failed_step);
}

#[test]
fn date_to_text_supports_documented_tokens() {
    let r = run(&with_custom(date_src("2024-03-05", EtlType::Date), "t", "Date.ToText([d], \"yyyy-MM-dd\")", None));
    assert_rows(&r, &[json!(["2024-03-05", "2024-03-05"])]);
    let r = run(&with_custom(date_src("2024-03-05", EtlType::Date), "t", "Date.ToText([d], \"d/M/yyyy\")", None));
    assert_rows(&r, &[json!(["2024-03-05", "5/3/2024"])]);
    let r = run(&with_custom(date_src("2024-03-05", EtlType::Date), "t", "Date.ToText([d], \"MMM\")", None));
    assert_rows(&r, &[json!(["2024-03-05", "Mar"])]);
    let r = run(&with_custom(date_src("2024-03-05", EtlType::Date), "t", "Date.ToText([d], \"MMMM yyyy\", \"pt-BR\")", None));
    assert_rows(&r, &[json!(["2024-03-05", "março 2024"])]);
}

#[test]
fn duration_days_between_dates() {
    let q = query(
        text_source(&["a", "b"], &[&[Some("2024-03-10"), Some("2024-03-01")]]),
        vec![StepKind::ChangeType {
            columns: vec![
                ColumnTypeSpec { column: "a".into(), ty: EtlType::Date },
                ColumnTypeSpec { column: "b".into(), ty: EtlType::Date },
            ],
            culture: None,
        }],
    );
    let r = run(&with_custom(q, "d", "Duration.Days([a] - [b])", None));
    assert_rows(&r, &[json!(["2024-03-10", "2024-03-01", 9])]);
}

#[test]
fn duration_days_requires_date_subtraction() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(nums1("5"), vec![custom("d", "Duration.Days([x])")]);
    let p = preview(&db, &q, None);
    assert!(p.res.failed_step.is_some());
}

// ------------------------------------------------------------ security

fn table_exists(db: &DuckDbStore, name: &str) -> bool {
    db.query_sql(&format!(
        "SELECT count(*) AS n FROM information_schema.tables WHERE table_name = {}",
        crate::etl::sql::lit_str(name)
    ))
    .unwrap()
    .rows[0]["n"]
        .as_i64()
        .unwrap()
        > 0
}

#[test]
fn round2_steps_quote_hostile_names_and_values() {
    let db = DuckDbStore::new_in_memory().unwrap();
    db.query_sql("CREATE TABLE x AS SELECT 1 AS keep").unwrap();
    let src = text_source(&[EVIL_COL, "b"], &[&[Some("v; --"), Some("2")]]);
    let q = query(
        src,
        vec![
            StepKind::TransformColumns { columns: strs(&[EVIL_COL]), expression: "_ & \"'; DROP TABLE x; --\"".into(), ty: None },
            StepKind::ReorderColumns { columns: strs(&["b", EVIL_COL]) },
        ],
    );
    let r = run(&q);
    assert_cols(&r, &["b", EVIL_COL]);
    assert_rows(&r, &[json!(["2", "v; --'; DROP TABLE x; --"])]);
    assert!(table_exists(&db, "x"));
}
