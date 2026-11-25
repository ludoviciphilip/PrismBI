//! Every `EtlStep` variant executed on small in-memory fixtures. Expected
//! rows are written by hand from Power Query semantics.

use super::*;
use serde_json::json;

fn people() -> EtlSource {
    // Already-promoted text table.
    text_source(
        &["Nome", "Cidade", "Valor"],
        &[
            &[Some("Ana"), Some("Rio"), Some("10")],
            &[Some("Bia"), None, Some("x")],
            &[Some("Caio"), Some("SP"), Some("")],
            &[Some("Ana"), Some("SP"), Some("7.5")],
        ],
    )
}

fn nums(vals: &[Option<f64>]) -> EtlSource {
    EtlSource::inline(
        strs(&["k", "v"]),
        vals.iter()
            .enumerate()
            .map(|(i, v)| vec![n(i as f64 + 1.0), v.map(Scalar::Number).unwrap_or(Scalar::Null)])
            .collect(),
    )
}

#[test]
fn promote_headers_uses_first_row_and_keeps_order() {
    let src = text_source(
        &["Column1", "Column2", "Column3", "Column4"],
        &[
            &[Some("Nome"), Some("Valor"), None, Some("Nome")],
            &[Some("Ana"), Some("1"), Some("a"), Some("x")],
            &[Some("Bia"), Some("2"), None, Some("y")],
        ],
    );
    let r = run(&query(src, vec![StepKind::PromoteHeaders]));
    // null header keeps the old name; duplicate header gets "_1".
    assert_cols(&r, &["Nome", "Valor", "Column3", "Nome_1"]);
    assert_rows(&r, &[json!(["Ana", "1", "a", "x"]), json!(["Bia", "2", null, "y"])]);
}

#[test]
fn change_type_conversions_and_error_flags() {
    let src = text_source(
        &["t"],
        &[
            &[Some("10")],
            &[Some("2.5")],
            &[Some("3.5")],
            &[Some("x")],
            &[Some("")],
            &[None],
            &[Some(" 42 ")],
        ],
    );
    let ct = |ty: EtlType| StepKind::ChangeType {
        columns: vec![ColumnTypeSpec { column: "t".into(), ty }],
        culture: None,
    };
    // Int64: banker's rounding (2.5 → 2, 3.5 → 4); "x" fails; "" and null are null.
    let r = run(&query(src.clone(), vec![ct(EtlType::Integer)]));
    assert_eq!(r.types, vec!["integer"]);
    assert_rows(&r, &[json!([10]), json!([2]), json!([4]), json!([null]), json!([null]), json!([null]), json!([42])]);
    // Removing errors drops only the "x" row: "" and null were genuine nulls.
    let r = run(&query(src.clone(), vec![ct(EtlType::Integer), StepKind::RemoveRowsWithErrors { columns: None }]));
    assert_rows(&r, &[json!([10]), json!([2]), json!([4]), json!([null]), json!([null]), json!([42])]);

    let r = run(&query(src, vec![ct(EtlType::Decimal)]));
    assert_eq!(r.types, vec!["decimal"]);
    assert_rows(&r, &[json!([10.0]), json!([2.5]), json!([3.5]), json!([null]), json!([null]), json!([null]), json!([42.0])]);

    let money = text_source(&["v"], &[&[Some("1234.56789")], &[Some("0.00004")]]);
    let r = run(&query(
        money,
        vec![StepKind::ChangeType {
            columns: vec![ColumnTypeSpec { column: "v".into(), ty: EtlType::Currency }],
            culture: None,
        }],
    ));
    assert_eq!(r.types, vec!["currency"]);
    // 4 decimal places: 0.00004 → 0.0000, 1234.56789 → 1234.5679
    assert_rows(&r, &[json!([1234.5679]), json!([0.0])]);
}

#[test]
fn change_type_datetime_preview_as_text() {
    let src = text_source(&["t"], &[&[Some("2026-01-05 10:30:00")], &[Some("05/01/2026 08:00:00")], &[Some("x")]]);
    let r = run(&query(
        src,
        vec![StepKind::ChangeType {
            columns: vec![ColumnTypeSpec { column: "t".into(), ty: EtlType::Datetime }],
            culture: Some("pt-BR".into()),
        }],
    ));
    assert_eq!(r.types, vec!["datetime"]);
    assert_rows(&r, &[json!(["2026-01-05 10:30:00"]), json!(["2026-01-05 08:00:00"]), json!([null])]);
}

#[test]
fn preview_reports_runtime_failures_at_the_failing_step() {
    // make_date(2026, 13, 1) compiles but fails when executed.
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(
        nums(&[Some(1.0), Some(13.0)]),
        vec![
            StepKind::RenameColumns { renames: vec![("v".into(), "m".into())] },
            StepKind::AddCustomColumn { new_column: "d".into(), expression: "#date(2026, [m], 1)".into(), ty: None },
            StepKind::RemoveColumns { columns: strs(&["k"]) },
        ],
    );
    let r = preview(&db, &q, None);
    let f = r.res.failed_step.clone().expect("failedStep");
    assert_eq!(f.index, 1, "{}", f.message);
    assert_cols(&r, &["k", "m"]);
    assert_eq!(r.rows.len(), 2);
}

#[test]
fn change_type_culture_aware() {
    let src = text_source(
        &["n", "d", "b"],
        &[
            &[Some("1.234,5"), Some("05/01/2026"), Some("true")],
            &[Some("-0,25"), Some("2026-02-03"), Some("FALSE")],
            &[Some("abc"), Some("31/02/2026"), Some("sim")],
        ],
    );
    let q = query(
        src,
        vec![StepKind::ChangeType {
            columns: vec![
                ColumnTypeSpec { column: "n".into(), ty: EtlType::Decimal },
                ColumnTypeSpec { column: "d".into(), ty: EtlType::Date },
                ColumnTypeSpec { column: "b".into(), ty: EtlType::Boolean },
            ],
            culture: Some("pt-BR".into()),
        }],
    );
    let r = run(&q);
    assert_eq!(r.types, vec!["decimal", "date", "boolean"]);
    // pt-BR: '.' thousands, ',' decimal, day-first dates. 31/02 is invalid.
    assert_rows(
        &r,
        &[
            json!([1234.5, "2026-01-05", true]),
            json!([-0.25, "2026-02-03", false]),
            json!([null, null, null]),
        ],
    );
    // en-US reads the same text month-first.
    let us = text_source(&["d"], &[&[Some("05/01/2026")]]);
    let r = run(&query(
        us,
        vec![StepKind::ChangeType {
            columns: vec![ColumnTypeSpec { column: "d".into(), ty: EtlType::Date }],
            culture: Some("en-US".into()),
        }],
    ));
    assert_rows(&r, &[json!(["2026-05-01"])]);
    // Number → text with a culture uses its decimal separator.
    let r = run(&query(
        nums(&[Some(2026.0), Some(1.5)]),
        vec![StepKind::ChangeType {
            columns: vec![ColumnTypeSpec { column: "v".into(), ty: EtlType::Text }],
            culture: Some("pt-BR".into()),
        }],
    ));
    assert_rows(&r, &[json!([1.0, "2026"]), json!([2.0, "1,5"])]);
}

#[test]
fn rename_remove_select_duplicate() {
    let r = run(&query(
        people(),
        vec![StepKind::RenameColumns {
            renames: vec![("Nome".into(), "Cidade".into()), ("Cidade".into(), "Nome".into())],
        }],
    ));
    // simultaneous swap, positions kept
    assert_cols(&r, &["Cidade", "Nome", "Valor"]);
    assert_eq!(r.rows[0], vec![json!("Ana"), json!("Rio"), json!("10")]);

    let r = run(&query(people(), vec![StepKind::RemoveColumns { columns: strs(&["Cidade"]) }]));
    assert_cols(&r, &["Nome", "Valor"]);
    assert_eq!(r.rows.len(), 4);

    let r = run(&query(people(), vec![StepKind::SelectColumns { columns: strs(&["Valor", "Nome"]) }]));
    assert_cols(&r, &["Valor", "Nome"]);
    assert_rows(&r, &[json!(["10", "Ana"]), json!(["x", "Bia"]), json!(["", "Caio"]), json!(["7.5", "Ana"])]);

    let r = run(&query(
        people(),
        vec![StepKind::DuplicateColumn { column: "Nome".into(), new_column: "Nome - Copy".into() }],
    ));
    assert_cols(&r, &["Nome", "Cidade", "Valor", "Nome - Copy"]);
    assert_eq!(r.rows[2][3], json!("Caio"));

    // Renaming to an existing column is an error (as in Power Query).
    let db = DuckDbStore::new_in_memory().unwrap();
    let bad = query(people(), vec![StepKind::RenameColumns { renames: vec![("Nome".into(), "Valor".into())] }]);
    let p = preview(&db, &bad, None);
    assert_eq!(p.res.failed_step.as_ref().map(|f| f.index), Some(0));
}

fn filter(src: &EtlSource, c: Cond) -> Run {
    run(&query(src.clone(), vec![StepKind::FilterRows { condition: c }]))
}

fn col0(r: &Run) -> Vec<JsonValue> {
    r.rows.iter().map(|row| row[1].clone()).collect()
}

#[test]
fn filter_rows_m_null_semantics() {
    let src = nums(&[Some(5.0), None, Some(10.0)]);
    let v = |op| Cond::cmp("v", op);
    // null > 6 is null → dropped
    assert_eq!(col0(&filter(&src, v(CondOp::Gt { value: n(6.0) }))), vec![json!(10.0)]);
    assert_eq!(col0(&filter(&src, v(CondOp::Lte { value: n(5.0) }))), vec![json!(5.0)]);
    // null <> 7 is true in M
    assert_eq!(
        col0(&filter(&src, v(CondOp::Neq { value: n(7.0) }))),
        vec![json!(5.0), json!(null), json!(10.0)]
    );
    // null = 5 is false
    assert_eq!(col0(&filter(&src, v(CondOp::Eq { value: n(5.0) }))), vec![json!(5.0)]);
    assert_eq!(col0(&filter(&src, v(CondOp::IsNull))), vec![json!(null)]);
    assert_eq!(col0(&filter(&src, v(CondOp::IsNotNull))), vec![json!(5.0), json!(10.0)]);
    // not (null > 6) is still null → dropped; only 5 survives
    assert_eq!(
        col0(&filter(&src, Cond::Not { not: Box::new(v(CondOp::Gt { value: n(6.0) })) })),
        vec![json!(5.0)]
    );
    // comparing to a null value: false for every relational operator
    assert!(filter(&src, v(CondOp::Gt { value: Scalar::Null })).rows.is_empty());
    assert!(filter(&src, v(CondOp::Lt { value: Scalar::Null })).rows.is_empty());
    // List.Contains treats null as a value
    assert_eq!(
        col0(&filter(&src, v(CondOp::In { values: vec![n(5.0), Scalar::Null] }))),
        vec![json!(5.0), json!(null)]
    );
    assert_eq!(
        col0(&filter(&src, v(CondOp::NotIn { values: vec![n(5.0)] }))),
        vec![json!(null), json!(10.0)]
    );
    // text vs number never equal (M): eq false, neq true for everything
    assert!(filter(&src, v(CondOp::Eq { value: s("5") })).rows.is_empty());
    assert_eq!(filter(&src, v(CondOp::Neq { value: s("5") })).rows.len(), 3);
    // and / or
    assert_eq!(
        col0(&filter(
            &src,
            Cond::Or { or: vec![v(CondOp::IsNull), v(CondOp::Gt { value: n(7.0) })] }
        )),
        vec![json!(null), json!(10.0)]
    );
    assert_eq!(
        col0(&filter(
            &src,
            Cond::And { and: vec![v(CondOp::Gte { value: n(5.0) }), v(CondOp::Lt { value: n(10.0) })] }
        )),
        vec![json!(5.0)]
    );
}

#[test]
fn filter_rows_text_predicates() {
    let src = text_source(&["k", "t"], &[&[Some("1"), Some("Abacate")], &[Some("2"), Some("")], &[Some("3"), None], &[Some("4"), Some("banana")]]);
    let t = |op| Cond::cmp("t", op);
    let keys = |r: &Run| r.rows.iter().map(|row| row[0].clone()).collect::<Vec<_>>();
    assert_eq!(keys(&filter(&src, t(CondOp::Contains { value: s("ana") }))), vec![json!("4")]);
    // case-sensitive like M
    assert_eq!(keys(&filter(&src, t(CondOp::Contains { value: s("aba") }))), Vec::<JsonValue>::new());
    // not Text.Contains(null, ..) is null → dropped
    assert_eq!(keys(&filter(&src, t(CondOp::NotContains { value: s("ana") }))), vec![json!("1"), json!("2")]);
    assert_eq!(keys(&filter(&src, t(CondOp::StartsWith { value: s("Ab") }))), vec![json!("1")]);
    assert_eq!(keys(&filter(&src, t(CondOp::EndsWith { value: s("na") }))), vec![json!("4")]);
    assert_eq!(keys(&filter(&src, t(CondOp::IsBlank))), vec![json!("2"), json!("3")]);
    assert_eq!(keys(&filter(&src, t(CondOp::IsNotBlank))), vec![json!("1"), json!("4")]);
    assert_eq!(keys(&filter(&src, t(CondOp::Gt { value: s("B") }))), vec![json!("4")]);
}

#[test]
fn remove_blank_rows_and_errors() {
    let src = text_source(
        &["a", "b"],
        &[&[Some("1"), None], &[None, None], &[Some(""), None], &[Some(""), Some("")], &[None, Some("x")]],
    );
    let r = run(&query(src, vec![StepKind::RemoveBlankRows]));
    assert_rows(&r, &[json!(["1", null]), json!([null, "x"])]);

    // errors only in the listed column are removed
    let src = text_source(&["a", "b"], &[&[Some("1"), Some("x")], &[Some("y"), Some("2")], &[Some("3"), Some("4")]]);
    let ct = StepKind::ChangeType {
        columns: vec![
            ColumnTypeSpec { column: "a".into(), ty: EtlType::Integer },
            ColumnTypeSpec { column: "b".into(), ty: EtlType::Integer },
        ],
        culture: None,
    };
    let r = run(&query(src.clone(), vec![ct.clone(), StepKind::RemoveRowsWithErrors { columns: Some(strs(&["a"])) }]));
    assert_rows(&r, &[json!([1, null]), json!([3, 4])]);
    let r = run(&query(src, vec![ct, StepKind::RemoveRowsWithErrors { columns: None }]));
    assert_rows(&r, &[json!([3, 4])]);
}

#[test]
fn errors_follow_renames_and_survive_later_steps() {
    let src = text_source(&["a"], &[&[Some("1")], &[Some("bad")], &[Some("2")]]);
    let r = run(&query(
        src,
        vec![
            StepKind::ChangeType {
                columns: vec![ColumnTypeSpec { column: "a".into(), ty: EtlType::Integer }],
                culture: None,
            },
            StepKind::RenameColumns { renames: vec![("a".into(), "b".into())] },
            StepKind::DuplicateColumn { column: "b".into(), new_column: "c".into() },
            StepKind::RemoveColumns { columns: strs(&["b"]) },
            StepKind::RemoveRowsWithErrors { columns: Some(strs(&["c"])) },
        ],
    ));
    assert_rows(&r, &[json!([1]), json!([2])]);
}

#[test]
fn keep_and_remove_top_bottom_rows() {
    let src = nums(&[Some(10.0), Some(20.0), Some(30.0), Some(40.0), Some(50.0)]);
    let v = |k: StepKind| col0(&run(&query(src.clone(), vec![k])));
    assert_eq!(v(StepKind::KeepTopRows { count: 2 }), vec![json!(10.0), json!(20.0)]);
    assert_eq!(v(StepKind::RemoveTopRows { count: 2 }), vec![json!(30.0), json!(40.0), json!(50.0)]);
    assert_eq!(v(StepKind::KeepBottomRows { count: 2 }), vec![json!(40.0), json!(50.0)]);
    assert_eq!(v(StepKind::RemoveBottomRows { count: 2 }), vec![json!(10.0), json!(20.0), json!(30.0)]);
    assert_eq!(v(StepKind::KeepTopRows { count: 0 }), Vec::<JsonValue>::new());
}

#[test]
fn remove_duplicates_keeps_first_occurrence() {
    let r = run(&query(people(), vec![StepKind::RemoveDuplicates { columns: Some(strs(&["Nome"])) }]));
    assert_rows(&r, &[json!(["Ana", "Rio", "10"]), json!(["Bia", null, "x"]), json!(["Caio", "SP", ""])]);
    let src = text_source(&["a", "b"], &[&[Some("1"), None], &[Some("1"), None], &[Some("1"), Some("2")]]);
    let r = run(&query(src, vec![StepKind::RemoveDuplicates { columns: None }]));
    assert_rows(&r, &[json!(["1", null]), json!(["1", "2"])]);
}

#[test]
fn sort_is_stable_with_nulls_first_ascending() {
    let r = run(&query(
        people(),
        vec![StepKind::Sort { by: vec![SortKey { column: "Cidade".into(), descending: false }] }],
    ));
    // null first, then Rio, then SP in original order (Caio before Ana)
    let names: Vec<_> = r.rows.iter().map(|x| x[0].clone()).collect();
    assert_eq!(names, vec![json!("Bia"), json!("Ana"), json!("Caio"), json!("Ana")]);
    assert_eq!(r.rows[1][1], json!("Rio"));

    let r = run(&query(
        people(),
        vec![StepKind::Sort {
            by: vec![
                SortKey { column: "Nome".into(), descending: true },
                SortKey { column: "Valor".into(), descending: false },
            ],
        }],
    ));
    let pairs: Vec<_> = r.rows.iter().map(|x| (x[0].clone(), x[2].clone())).collect();
    assert_eq!(
        pairs,
        vec![
            (json!("Caio"), json!("")),
            (json!("Bia"), json!("x")),
            (json!("Ana"), json!("10")),
            (json!("Ana"), json!("7.5")),
        ]
    );
}

#[test]
fn replace_values_text_and_whole_cell() {
    let r = run(&query(
        people(),
        vec![StepKind::ReplaceValues {
            columns: strs(&["Nome"]),
            find: s("a"),
            replace: s("4"),
            match_entire_cell: false,
        }],
    ));
    let names: Vec<_> = r.rows.iter().map(|x| x[0].clone()).collect();
    // case-sensitive substring replace
    assert_eq!(names, vec![json!("An4"), json!("Bi4"), json!("C4io"), json!("An4")]);

    let r = run(&query(
        people(),
        vec![StepKind::ReplaceValues {
            columns: strs(&["Cidade"]),
            find: Scalar::Null,
            replace: s("?"),
            match_entire_cell: true,
        }],
    ));
    assert_eq!(r.rows[1][1], json!("?"));
    assert_eq!(r.rows[0][1], json!("Rio"));

    let r = run(&query(
        nums(&[Some(1.0), Some(11.0), None]),
        vec![StepKind::ReplaceValues {
            columns: strs(&["v"]),
            find: n(1.0),
            replace: n(100.0),
            match_entire_cell: true,
        }],
    ));
    assert_eq!(col0(&r), vec![json!(100.0), json!(11.0), json!(null)]);
}

#[test]
fn fill_down_and_up_follow_row_order() {
    let src = text_source(&["g", "v"], &[&[Some("a"), Some("1")], &[None, Some("2")], &[None, None], &[Some("b"), Some("4")], &[None, None]]);
    let r = run(&query(src.clone(), vec![StepKind::FillDown { columns: strs(&["g"]) }]));
    assert_rows(&r, &[json!(["a", "1"]), json!(["a", "2"]), json!(["a", null]), json!(["b", "4"]), json!(["b", null])]);
    let r = run(&query(src.clone(), vec![StepKind::FillUp { columns: strs(&["g", "v"]) }]));
    assert_rows(&r, &[json!(["a", "1"]), json!(["b", "2"]), json!(["b", "4"]), json!(["b", "4"]), json!([null, null])]);
    // fill after a sort uses the sorted order
    let r = run(&query(
        src,
        vec![
            StepKind::Sort { by: vec![SortKey { column: "v".into(), descending: true }] },
            StepKind::FillDown { columns: strs(&["g"]) },
        ],
    ));
    // sorted v desc (nulls last): 4(b), 2(null), 1(a), null(null), null(null)
    assert_rows(&r, &[json!(["b", "4"]), json!(["b", "2"]), json!(["a", "1"]), json!(["a", null]), json!(["a", null])]);
}

#[test]
fn split_column_modes() {
    let src = text_source(&["x", "z"], &[&[Some("a,b,c"), Some("1")], &[Some("solo"), Some("2")], &[None, Some("3")]]);
    let split = |mode, delimiter: Option<&str>, positions: Option<Vec<u64>>, length: Option<u64>, into: &[&str]| {
        run(&query(
            src.clone(),
            vec![StepKind::SplitColumn {
                column: "x".into(),
                mode,
                delimiter: delimiter.map(str::to_string),
                positions,
                length,
                into: strs(into),
            }],
        ))
    };
    let r = split(SplitMode::DelimiterEach, Some(","), None, None, &["x.1", "x.2"]);
    // split columns replace the original in place; extra parts are dropped
    assert_cols(&r, &["x.1", "x.2", "z"]);
    assert_rows(&r, &[json!(["a", "b", "1"]), json!(["solo", null, "2"]), json!([null, null, "3"])]);
    let r = split(SplitMode::DelimiterLeftmost, Some(","), None, None, &["l", "r"]);
    assert_rows(&r, &[json!(["a", "b,c", "1"]), json!(["solo", null, "2"]), json!([null, null, "3"])]);
    let r = split(SplitMode::DelimiterRightmost, Some(","), None, None, &["l", "r"]);
    assert_rows(&r, &[json!(["a,b", "c", "1"]), json!(["solo", null, "2"]), json!([null, null, "3"])]);
    let r = split(SplitMode::Positions, None, Some(vec![0, 2]), None, &["p1", "p2"]);
    assert_rows(&r, &[json!(["a,", "b,c", "1"]), json!(["so", "lo", "2"]), json!([null, null, "3"])]);
    let r = split(SplitMode::FixedLength, None, None, Some(2), &["f1", "f2", "f3"]);
    assert_rows(&r, &[json!(["a,", "b,", "c", "1"]), json!(["so", "lo", null, "2"]), json!([null, null, null, "3"])]);
}

#[test]
fn merge_columns_null_is_empty_and_position_kept() {
    let r = run(&query(
        people(),
        vec![StepKind::MergeColumns {
            columns: strs(&["Cidade", "Valor"]),
            separator: "/".into(),
            new_column: "CV".into(),
        }],
    ));
    assert_cols(&r, &["Nome", "CV"]);
    assert_rows(&r, &[json!(["Ana", "Rio/10"]), json!(["Bia", "/x"]), json!(["Caio", "SP/"]), json!(["Ana", "SP/7.5"])]);
    // numbers are converted to text (2026, not 2026.0)
    let r = run(&query(
        nums(&[Some(2026.0), Some(0.5)]),
        vec![StepKind::MergeColumns { columns: strs(&["v", "k"]), separator: "-".into(), new_column: "m".into() }],
    ));
    assert_rows(&r, &[json!(["2026-1"]), json!(["0.5-2"])]);
}

#[test]
fn text_transforms() {
    let src = text_source(&["t"], &[&[Some("  olá MUNDO\t")], &[Some("nov/2026")], &[Some("a\u{7}b")], &[None]]);
    let t = |f| run(&query(src.clone(), vec![StepKind::TextTransform { columns: strs(&["t"]), func: f }]));
    assert_eq!(t(TextFn::Upper).rows[0][0], json!("  OLÁ MUNDO\t"));
    assert_eq!(t(TextFn::Lower).rows[0][0], json!("  olá mundo\t"));
    let p = t(TextFn::Proper);
    assert_eq!(p.rows[0][0], json!("  Olá Mundo\t"));
    assert_eq!(p.rows[1][0], json!("Nov/2026"));
    assert_eq!(p.rows[3][0], json!(null));
    assert_eq!(t(TextFn::Trim).rows[0][0], json!("olá MUNDO"));
    assert_eq!(t(TextFn::Clean).rows[2][0], json!("ab"));
}

#[test]
fn unpivot_skips_nulls_and_keeps_row_major_order() {
    let src = EtlSource::inline(
        strs(&["k", "A", "B"]),
        vec![vec![n(1.0), n(10.0), Scalar::Null], vec![n(2.0), n(20.0), n(30.0)]],
    );
    let r = run(&query(
        src.clone(),
        vec![StepKind::UnpivotOtherColumns {
            keep: strs(&["k"]),
            attribute_column: "Atributo".into(),
            value_column: "Valor".into(),
        }],
    ));
    assert_cols(&r, &["k", "Atributo", "Valor"]);
    assert_rows(&r, &[json!([1.0, "A", 10.0]), json!([2.0, "A", 20.0]), json!([2.0, "B", 30.0])]);
    let r = run(&query(
        src,
        vec![StepKind::UnpivotColumns {
            columns: strs(&["B"]),
            attribute_column: "Atributo".into(),
            value_column: "Valor".into(),
        }],
    ));
    assert_cols(&r, &["k", "A", "Atributo", "Valor"]);
    assert_rows(&r, &[json!([2.0, 20.0, "B", 30.0])]);
}

#[test]
fn pivot_with_aggregations() {
    let src = EtlSource::inline(
        strs(&["k", "attr", "v"]),
        vec![
            vec![n(1.0), s("B"), n(5.0)],
            vec![n(1.0), s("A"), n(10.0)],
            vec![n(2.0), s("A"), n(7.0)],
            vec![n(1.0), s("A"), n(1.0)],
        ],
    );
    let p = |agg| {
        run(&query(src.clone(), vec![StepKind::Pivot { column: "attr".into(), values_column: "v".into(), aggregation: agg }]))
    };
    let r = p(AggOp::Sum);
    // columns in order of first appearance: B, then A
    assert_cols(&r, &["k", "B", "A"]);
    assert_rows(&r, &[json!([1.0, 5.0, 11.0]), json!([2.0, null, 7.0])]);
    let r = p(AggOp::CountRows);
    assert_rows(&r, &[json!([1.0, 1, 2]), json!([2.0, 0, 1])]);
    let r = p(AggOp::Max);
    assert_rows(&r, &[json!([1.0, 5.0, 10.0]), json!([2.0, null, 7.0])]);
}

#[test]
fn group_by_all_aggregations() {
    let src = EtlSource::inline(
        strs(&["g", "v"]),
        vec![
            vec![s("b"), n(4.0)],
            vec![s("a"), n(1.0)],
            vec![s("b"), Scalar::Null],
            vec![s("a"), n(3.0)],
            vec![s("b"), n(4.0)],
            vec![s("b"), n(10.0)],
        ],
    );
    let a = |name: &str, op, col: Option<&str>| Agg { name: name.into(), op, column: col.map(str::to_string) };
    let r = run(&query(
        src,
        vec![StepKind::GroupBy {
            keys: strs(&["g"]),
            aggregations: vec![
                a("sum", AggOp::Sum, Some("v")),
                a("avg", AggOp::Avg, Some("v")),
                a("min", AggOp::Min, Some("v")),
                a("max", AggOp::Max, Some("v")),
                a("cnt", AggOp::Count, Some("v")),
                a("dist", AggOp::CountDistinct, Some("v")),
                a("rows", AggOp::CountRows, None),
                a("med", AggOp::Median, Some("v")),
            ],
        }],
    ));
    assert_cols(&r, &["g", "sum", "avg", "min", "max", "cnt", "dist", "rows", "med"]);
    // groups in order of first appearance (b first). b: {4, null, 4, 10}:
    // sum 18, avg 6, min 4, max 10, non-null count 3, distinct {4, null, 10} = 3,
    // rows 4, median of {4,4,10} = 4. a: {1, 3}: median 2.
    assert_rows(
        &r,
        &[
            json!(["b", 18.0, 6.0, 4.0, 10.0, 3, 3, 4, 4.0]),
            json!(["a", 4.0, 2.0, 1.0, 3.0, 2, 2, 2, 2.0]),
        ],
    );
}

#[test]
fn conditional_column() {
    let r = run(&query(
        nums(&[Some(3.0), Some(8.0), None]),
        vec![StepKind::AddConditionalColumn {
            new_column: "faixa".into(),
            rules: vec![
                ConditionalRule { when: Cond::cmp("v", CondOp::Lte { value: n(6.0) }), then: ValueOrColumn::Value(s("baixo")) },
                ConditionalRule { when: Cond::cmp("v", CondOp::IsNull), then: ValueOrColumn::Value(Scalar::Null) },
            ],
            otherwise: ValueOrColumn::Column { column: "k".into() },
            ty: None,
        }],
    ));
    // mixed text/number branches → text; otherwise uses column k (2 → "2")
    assert_rows(&r, &[json!([1.0, 3.0, "baixo"]), json!([2.0, 8.0, "2"]), json!([3.0, null, null])]);
}

#[test]
fn custom_columns_subset() {
    let src = EtlSource::inline(
        strs(&["d", "t", "x"]),
        vec![
            vec![s("2026-09-06"), s("setembro"), n(2.5)],
            vec![s("2026-12-31"), s("ab"), n(-3.0)],
        ],
    );
    let base = StepKind::ChangeType {
        columns: vec![ColumnTypeSpec { column: "d".into(), ty: EtlType::Date }],
        culture: None,
    };
    let add = |expr: &str, ty: Option<EtlType>| -> Run {
        run(&query(
            src.clone(),
            vec![base.clone(), StepKind::AddCustomColumn { new_column: "r".into(), expression: expr.into(), ty }],
        ))
    };
    let last = |r: &Run| r.rows.iter().map(|row| row[3].clone()).collect::<Vec<_>>();
    assert_eq!(last(&add("Date.MonthName([d], \"pt-BR\")", None)), vec![json!("setembro"), json!("dezembro")]);
    assert_eq!(last(&add("Date.DayOfWeekName([d], \"pt-BR\")", None)), vec![json!("domingo"), json!("quinta-feira")]);
    assert_eq!(last(&add("Date.MonthName([d])", None)), vec![json!("September"), json!("December")]);
    assert_eq!(last(&add("Date.Year([d]) * 100 + Date.Month([d])", Some(EtlType::Integer))), vec![json!(202609), json!(202612)]);
    // 2026-09-06 Sunday → 0; 2026-12-31 Thursday → 4
    assert_eq!(last(&add("Date.DayOfWeek([d])", None)), vec![json!(0), json!(4)]);
    assert_eq!(last(&add("Date.DayOfWeek([d], Day.Monday)", None)), vec![json!(6), json!(3)]);
    assert_eq!(last(&add("Date.Quarter([d])", None)), vec![json!(3), json!(4)]);
    // Dec 2026 starts on Tuesday: (31-1+2) div 7 + 1 = 5
    assert_eq!(last(&add("Date.WeekOfMonth([d])", None)), vec![json!(2), json!(5)]);
    assert_eq!(last(&add("Date.AddDays([d], 1)", None)), vec![json!("2026-09-07"), json!("2027-01-01")]);
    assert_eq!(last(&add("Date.AddMonths([d], 2)", None)), vec![json!("2026-11-06"), json!("2027-02-28")]);
    assert_eq!(last(&add("Text.Upper(Text.Start([t], 3)) & \"!\"", None)), vec![json!("SET!"), json!("AB!")]);
    assert_eq!(last(&add("Text.Proper([t])", None)), vec![json!("Setembro"), json!("Ab")]);
    assert_eq!(last(&add("Text.End([t], 2)", None)), vec![json!("ro"), json!("ab")]);
    assert_eq!(last(&add("Text.Middle([t], 1, 3)", None)), vec![json!("ete"), json!("b")]);
    assert_eq!(last(&add("Text.Length([t])", None)), vec![json!(8), json!(2)]);
    assert_eq!(last(&add("Text.Contains([t], \"tem\")", None)), vec![json!(true), json!(false)]);
    assert_eq!(last(&add("Number.Round([x])", None)), vec![json!(2.0), json!(-3.0)]);
    assert_eq!(last(&add("Number.Abs([x]) / 2", None)), vec![json!(1.25), json!(1.5)]);
    assert_eq!(last(&add("if [x] > 0 and not ([t] = \"ab\") then \"pos\" else null", None)), vec![json!("pos"), json!(null)]);
    assert_eq!(last(&add("Text.From([x]) & \"|\" & Text.From([d])", None)), vec![json!("2.5|2026-09-06"), json!("-3|2026-12-31")]);
    assert_eq!(last(&add("Number.From(\"1,5\", \"pt-BR\") + [x]", None)), vec![json!(4.0), json!(-1.5)]);
    assert_eq!(last(&add("Date.From(\"31/12/2026\", \"pt-BR\") = [d]", None)), vec![json!(false), json!(true)]);
    assert_eq!(last(&add("[x] = null", None)), vec![json!(false), json!(false)]);

    // Outside the subset → clear error and the step is reported as failed.
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(
        src,
        vec![StepKind::AddCustomColumn { new_column: "r".into(), expression: "List.Sum({1,2})".into(), ty: None }],
    );
    let p = preview(&db, &q, None);
    let f = p.res.failed_step.expect("falha esperada");
    assert_eq!(f.index, 0);
    assert!(f.message.contains("List.Sum"), "{}", f.message);
}

#[test]
fn index_column() {
    let r = run(&query(
        people(),
        vec![
            StepKind::FilterRows { condition: Cond::cmp("Nome", CondOp::Neq { value: s("Bia") }) },
            StepKind::AddIndexColumn { new_column: "Índice".into(), start: 1, step: 2 },
        ],
    ));
    let idx: Vec<_> = r.rows.iter().map(|x| x[3].clone()).collect();
    assert_eq!(idx, vec![json!(1), json!(3), json!(5)]);
}

#[test]
fn merge_queries_use_materialized_model_tables() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let right = EtlQuery {
        name: "dCat".into(),
        source: text_source(&["Cat", "Grupo"], &[&[Some("A"), Some("G1")], &[Some("B"), Some("G2")], &[Some("A"), Some("G3")], &[None, Some("Gnull")]]),
        steps: vec![],
    };
    apply(&db, &right, "dCat");
    let left_src = text_source(&["id", "Cat"], &[&[Some("1"), Some("A")], &[Some("2"), Some("C")], &[Some("3"), None], &[Some("4"), Some("B")]]);
    let mq = |jk| StepKind::MergeQueries {
        right_query: "dCat".into(),
        left_keys: strs(&["Cat"]),
        right_keys: strs(&["Cat"]),
        join_kind: jk,
        expand: vec![ExpandColumn { column: "Grupo".into(), as_name: "dCat.Grupo".into() }],
    };
    let left = |jk| EtlQuery { name: "fato".into(), source: left_src.clone(), steps: vec![step(mq(jk))] };
    let r = run_in(&db, &left(JoinKind::LeftOuter));
    assert_cols(&r, &["id", "Cat", "dCat.Grupo"]);
    // left order kept, A matches two rows (right order), null matches null (M semantics)
    assert_rows(
        &r,
        &[
            json!(["1", "A", "G1"]),
            json!(["1", "A", "G3"]),
            json!(["2", "C", null]),
            json!(["3", null, "Gnull"]),
            json!(["4", "B", "G2"]),
        ],
    );
    let r = run_in(&db, &left(JoinKind::Inner));
    assert_eq!(r.rows.len(), 4);
    let r = run_in(&db, &left(JoinKind::LeftAnti));
    assert_rows(&r, &[json!(["2", "C", null])]);
    let right_extra = EtlQuery {
        name: "dCat".into(),
        source: text_source(&["Cat", "Grupo"], &[&[Some("A"), Some("G1")], &[Some("Z"), Some("GZ")]]),
        steps: vec![],
    };
    apply(&db, &right_extra, "dCat");
    let r = run_in(&db, &left(JoinKind::RightAnti));
    assert_rows(&r, &[json!([null, null, "GZ"])]);
    let r = run_in(&db, &left(JoinKind::FullOuter));
    assert_eq!(r.rows.len(), 5);
    let r = run_in(&db, &left(JoinKind::RightOuter));
    assert_rows(&r, &[json!(["1", "A", "G1"]), json!([null, null, "GZ"])]);

    // Missing right query → failed step with a clear message.
    let mut missing = left(JoinKind::Inner);
    if let StepKind::MergeQueries { right_query, .. } = &mut missing.steps[0].kind {
        *right_query = "naoExiste".into();
    }
    let p = preview(&db, &missing, None);
    assert!(p.res.failed_step.unwrap().message.contains("naoExiste"));
}

#[test]
fn append_queries_union_by_name() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let other = EtlQuery {
        name: "q2".into(),
        source: text_source(&["b", "c"], &[&[Some("b2"), Some("c2")], &[Some("b3"), Some("c3")]]),
        steps: vec![],
    };
    apply(&db, &other, "q2");
    let q = EtlQuery {
        name: "q1".into(),
        source: text_source(&["a", "b"], &[&[Some("a1"), Some("b1")]]),
        steps: vec![step(StepKind::AppendQueries { queries: strs(&["q2"]) })],
    };
    let r = run_in(&db, &q);
    assert_cols(&r, &["a", "b", "c"]);
    assert_rows(&r, &[json!(["a1", "b1", null]), json!([null, "b2", "c2"]), json!([null, "b3", "c3"])]);
}

#[test]
fn opaque_step_fails_and_preview_stops_before_it() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = EtlQuery {
        name: "t".into(),
        source: people(),
        steps: vec![
            step(StepKind::RemoveColumns { columns: strs(&["Valor"]) }),
            step(StepKind::OpaqueM { m: "Table.Buffer(x)".into() }),
            step(StepKind::RemoveColumns { columns: strs(&["Cidade"]) }),
        ],
    };
    let r = preview(&db, &q, None);
    let f = r.res.failed_step.clone().expect("failedStep");
    assert_eq!(f.index, 1);
    // data shown = result of the step before the failing one
    assert_cols(&r, &["Nome", "Cidade"]);
    assert_eq!(r.res.row_count, 4);
    assert!(crate::etl::commands::etl_apply_impl(&db, &q, "t_out").is_err());
}

#[test]
fn preview_upto_step_is_inclusive() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = EtlQuery {
        name: "t".into(),
        source: people(),
        steps: vec![
            step(StepKind::RemoveColumns { columns: strs(&["Valor"]) }),
            step(StepKind::KeepTopRows { count: 1 }),
            step(StepKind::RemoveColumns { columns: strs(&["Cidade"]) }),
        ],
    };
    let r = preview(&db, &q, Some(0));
    assert_cols(&r, &["Nome", "Cidade"]);
    assert_eq!(r.res.row_count, 4);
    let r = preview(&db, &q, Some(1));
    assert_cols(&r, &["Nome", "Cidade"]);
    assert_eq!(r.res.row_count, 1);
    let r = preview(&db, &q, None);
    assert_cols(&r, &["Nome"]);
    // negative = source only
    let r = preview(&db, &q, Some(-1));
    assert_cols(&r, &["Nome", "Cidade", "Valor"]);
    // failing step after uptoStep is not evaluated
    let mut q2 = q.clone();
    q2.steps.push(step(StepKind::OpaqueM { m: "x".into() }));
    let r = preview(&db, &q2, Some(1));
    assert!(r.res.failed_step.is_none());
    // limit applies to rows, rowCount is the full count
    etl_load_raw_impl(&db, &q.name, &q.source, None).unwrap();
    let p = etl_preview_impl(&db, &q, Some(0), Some(2)).unwrap();
    assert_eq!(p.rows.len(), 2);
    assert_eq!(p.row_count, 4);
}

#[test]
fn apply_materializes_ordered_table_without_hidden_columns() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = EtlQuery {
        name: "t".into(),
        source: people(),
        steps: vec![
            step(StepKind::ChangeType {
                columns: vec![ColumnTypeSpec { column: "Valor".into(), ty: EtlType::Decimal }],
                culture: None,
            }),
            step(StepKind::Sort { by: vec![SortKey { column: "Valor".into(), descending: true }] }),
        ],
    };
    let payload = apply(&db, &q, "Saida Final");
    assert_eq!(payload.table_name, "Saida Final");
    assert_eq!(payload.total_rows, 4);
    let names: Vec<_> = payload.columns.iter().map(|c| c.name.clone()).collect();
    assert_eq!(names, strs(&["Nome", "Cidade", "Valor"]));
    let order: Vec<_> = payload.rows.iter().map(|r| r["Nome"].clone()).collect();
    // 10, 7.5, then the two nulls ("x" error, "" null) in original order
    assert_eq!(order, vec![json!("Ana"), json!("Ana"), json!("Bia"), json!("Caio")]);
}

// ---------------------------------------------------------------- raw sources

fn write_xlsx(path: &std::path::Path, sheet_xml: &str) {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let o = SimpleFileOptions::default();
    let parts: [(&str, String); 5] = [
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.into()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
        ("xl/workbook.xml", r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Dados" sheetId="1" r:id="rId1"/></sheets></workbook>"#.into()),
        ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.into()),
        ("xl/worksheets/sheet1.xml", format!(r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{}</sheetData></worksheet>"#, sheet_xml)),
    ];
    for (name, body) in parts {
        zip.start_file(name, o).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn is(r: &str, t: &str) -> String {
    format!(r#"<c r="{}" t="inlineStr"><is><t>{}</t></is></c>"#, r, t)
}

#[test]
fn excel_raw_sheet_promote_types_and_error_cells() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.xlsx");
    let sheet = format!(
        "<row r=\"1\">{}{}{}</row><row r=\"2\">{}<c r=\"B2\"><v>10.5</v></c><c r=\"C2\"><v>46027</v></c></row><row r=\"3\">{}<c r=\"B3\" t=\"e\"><v>#N/A</v></c><c r=\"C3\"><v>46028</v></c></row><row r=\"4\"></row><row r=\"5\">{}<c r=\"B5\"><v>2026</v></c><c r=\"C5\" t=\"b\"><v>1</v></c></row>",
        is("A1", "Nome"),
        is("B1", "Valor"),
        is("C1", "Data"),
        is("A2", "Ana"),
        is("A3", "Bia"),
        is("A5", "Caio"),
    );
    write_xlsx(&path, &sheet);
    let db = DuckDbStore::new_in_memory().unwrap();
    let src = EtlSource::excel(path.to_string_lossy(), "Dados");
    let raw = etl_load_raw_impl(&db, "f", &src, None).unwrap();
    assert_eq!(raw.raw_table, "raw__f");
    // header row is data, columns are Column1..N, the error cell's column is flagged (hidden)
    assert_eq!(raw.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Column1", "Column2", "Column3"]);
    assert_eq!(raw.row_count, 5);

    let q = EtlQuery {
        name: "f".into(),
        source: src,
        steps: vec![
            step(StepKind::PromoteHeaders),
            step(StepKind::ChangeType {
                columns: vec![
                    ColumnTypeSpec { column: "Valor".into(), ty: EtlType::Decimal },
                    ColumnTypeSpec { column: "Data".into(), ty: EtlType::Date },
                ],
                // canonical number cells are not re-read with pt-BR separators
                culture: Some("pt-BR".into()),
            }),
        ],
    };
    let r = preview(&db, &q, None);
    assert!(r.res.failed_step.is_none(), "{:?}", r.res.failed_step);
    assert_cols(&r, &["Nome", "Valor", "Data"]);
    // 46027 = 2026-01-05 (Excel serial); "true" is not a date → error (null)
    assert_rows(
        &r,
        &[
            json!(["Ana", 10.5, "2026-01-05"]),
            json!(["Bia", null, "2026-01-06"]),
            json!([null, null, null]),
            json!(["Caio", 2026.0, null]),
        ],
    );
    let mut q2 = q.clone();
    q2.steps.push(step(StepKind::RemoveRowsWithErrors { columns: None }));
    q2.steps.push(step(StepKind::RemoveBlankRows));
    let r = preview(&db, &q2, None);
    // Bia (#N/A in Valor) and Caio (bool in Data) are errors; the empty row is blank.
    assert_rows(&r, &[json!(["Ana", 10.5, "2026-01-05"])]);
}

#[test]
fn csv_raw_is_all_text_with_empty_strings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.csv");
    std::fs::write(&path, "Produto;Preço\nCafé;\"1.234,50\"\nChá;\n").unwrap();
    let q = EtlQuery {
        name: "v".into(),
        source: EtlSource::csv(path.to_string_lossy()),
        steps: vec![
            step(StepKind::PromoteHeaders),
            step(StepKind::ChangeType {
                columns: vec![ColumnTypeSpec { column: "Preço".into(), ty: EtlType::Decimal }],
                culture: Some("pt-BR".into()),
            }),
        ],
    };
    let r = run(&q);
    assert_cols(&r, &["Produto", "Preço"]);
    assert_rows(&r, &[json!(["Café", 1234.5]), json!(["Chá", null])]);
}
