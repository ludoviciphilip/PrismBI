//! SQL-injection safety: hostile column/table names and values must be
//! carried as data, never executed.

use super::*;
use crate::etl::sql::{lit_str, quote_ident};
use serde_json::json;

const EVIL_COL: &str = "a\"; DROP TABLE x; --";
const EVIL_VAL: &str = "x'); DROP TABLE x; --";

fn table_exists(db: &DuckDbStore, name: &str) -> bool {
    db.query_sql(&format!(
        "SELECT count(*) AS n FROM information_schema.tables WHERE table_name = {}",
        lit_str(name)
    ))
    .unwrap()
    .rows[0]["n"]
        .as_i64()
        .unwrap()
        > 0
}

#[test]
fn quoting_helpers() {
    assert_eq!(quote_ident(EVIL_COL), "\"a\"\"; DROP TABLE x; --\"");
    assert_eq!(lit_str("it's"), "'it''s'");
    assert_eq!(lit_str("a\0b"), "('a' || chr(0) || 'b')");
}

#[test]
fn hostile_column_names_and_values_round_trip_as_data() {
    let db = DuckDbStore::new_in_memory().unwrap();
    db.query_sql("CREATE TABLE x AS SELECT 1 AS keep").unwrap();
    let src = text_source(
        &["Column1", "Column2"],
        &[
            &[Some(EVIL_COL), Some("v; --")],
            &[Some(EVIL_VAL), Some("o'brien")],
            &[Some("\"quoted\""), Some("semi;colon")],
        ],
    );
    let q = EtlQuery {
        name: "q'; DROP TABLE x; --".into(),
        source: src,
        steps: vec![
            step(StepKind::PromoteHeaders),
            step(StepKind::FilterRows {
                condition: Cond::Or {
                    or: vec![
                        Cond::cmp(EVIL_COL, CondOp::Eq { value: s(EVIL_VAL) }),
                        Cond::cmp(EVIL_COL, CondOp::Contains { value: s("\"quo") }),
                        Cond::cmp("v; --", CondOp::In { values: vec![s("'; DELETE FROM x; --")] }),
                    ],
                },
            }),
            step(StepKind::ReplaceValues {
                columns: vec![EVIL_COL.into()],
                find: s("'"),
                replace: s("'';--"),
                match_entire_cell: false,
            }),
            step(StepKind::AddCustomColumn {
                new_column: "x\"y".into(),
                expression: "[#\"a\"\"; DROP TABLE x; --\"] & \"'; DROP TABLE x; --\"".to_string(),
                ty: None,
            }),
            step(StepKind::RenameColumns { renames: vec![("v; --".into(), "\"); DROP TABLE x; --".into())] }),
            step(StepKind::GroupBy {
                keys: vec![EVIL_COL.into(), "\"); DROP TABLE x; --".into(), "x\"y".into()],
                aggregations: vec![Agg { name: "n'; --".into(), op: AggOp::CountRows, column: None }],
            }),
        ],
    };
    let r = run_in(&db, &q);
    assert_cols(&r, &[EVIL_COL, "\"); DROP TABLE x; --", "x\"y", "n'; --"]);
    assert_rows(
        &r,
        &[
            json!(["x'';--); DROP TABLE x; --", "o'brien", "x'';--); DROP TABLE x; --'; DROP TABLE x; --", 1]),
            json!(["\"quoted\"", "semi;colon", "\"quoted\"'; DROP TABLE x; --", 1]),
        ],
    );
    assert!(table_exists(&db, "x"), "a tabela x não pode ter sido apagada");

    // The model table keeps the hostile names/values intact.
    let payload = crate::etl::commands::etl_apply_impl(&db, &q, "alvo\"; DROP TABLE x; --").unwrap();
    assert_eq!(payload.columns[0].name, EVIL_COL);
    assert_eq!(payload.rows[1][EVIL_COL], json!("\"quoted\""));
    assert!(table_exists(&db, "x"));
    assert!(table_exists(&db, "alvo\"; DROP TABLE x; --"));
}

#[test]
fn hostile_names_in_merge_append_pivot_split_and_sources() {
    let db = DuckDbStore::new_in_memory().unwrap();
    db.query_sql("CREATE TABLE x AS SELECT 1 AS keep").unwrap();
    let right = EtlQuery {
        name: "r\"; DROP TABLE x; --".into(),
        source: text_source(&["k'", "g\""], &[&[Some("1"), Some("G'1")]]),
        steps: vec![],
    };
    apply(&db, &right, "r\"; DROP TABLE x; --");
    let q = EtlQuery {
        name: "l".into(),
        source: text_source(&["k'", "p"], &[&[Some("1"), Some("A';B")], &[Some("2"), Some("C")]]),
        steps: vec![
            step(StepKind::MergeQueries {
                right_query: "r\"; DROP TABLE x; --".into(),
                left_keys: vec!["k'".into()],
                right_keys: vec!["k'".into()],
                join_kind: JoinKind::LeftOuter,
                expand: vec![ExpandColumn { column: "g\"".into(), as_name: "g'); --".into() }],
            }),
            step(StepKind::AppendQueries { queries: vec!["r\"; DROP TABLE x; --".into()] }),
            step(StepKind::SplitColumn {
                column: "p".into(),
                mode: SplitMode::DelimiterEach,
                delimiter: Some("';".into()),
                positions: None,
                length: None,
                into: vec!["p'1".into(), "p\"2".into()],
            }),
            step(StepKind::Pivot { column: "p'1".into(), values_column: "k'".into(), aggregation: AggOp::Count }),
        ],
    };
    let r = run_in(&db, &q);
    // after split: k', p'1, p"2, g'); --, g"  → pivot keys (table order): p"2, g'); --, g"
    // pivot columns from the values of p'1 in order of appearance: "A", "C"
    assert_cols(&r, &["p\"2", "g'); --", "g\"", "A", "C"]);
    assert_rows(
        &r,
        &[
            json!(["B", "G'1", null, 1, 0]),
            json!([null, null, null, 0, 1]),
            json!([null, null, "G'1", 0, 0]),
        ],
    );
    assert!(table_exists(&db, "x"));
}

#[test]
fn hostile_paths_and_m_text_are_data() {
    let db = DuckDbStore::new_in_memory().unwrap();
    db.query_sql("CREATE TABLE x AS SELECT 1 AS keep").unwrap();
    let src = EtlSource::csv("'); DROP TABLE x; --.csv");
    assert!(etl_load_raw_impl(&db, "c", &src, None).is_err());
    assert!(table_exists(&db, "x"));

    // M with hostile identifiers parses into data, and to_m re-escapes it.
    let m = "let\n    Source = #table({\"a\"\"; DROP TABLE x; --\"}, {{\"'; DROP TABLE x; --\"}}),\n    #\"Renamed\"\"; --\" = Table.RenameColumns(Source,{{\"a\"\"; DROP TABLE x; --\", \"b'\"}})\nin\n    #\"Renamed\"\"; --\"";
    let parsed = crate::etl::m_from::query_from_m("m", m).unwrap().query;
    assert_eq!(parsed.steps[0].name, "Renamed\"; --");
    let r = run_in(&db, &parsed);
    assert_cols(&r, &["b'"]);
    assert_rows(&r, &[json!(["'; DROP TABLE x; --"])]);
    let back = crate::etl::m_to::query_to_m(&parsed).unwrap();
    assert_eq!(crate::etl::m_from::query_from_m("m", &back).unwrap().query.without_ids(), parsed.without_ids());
    assert!(table_exists(&db, "x"));
}

#[test]
fn reserved_names_in_headers_are_not_promoted() {
    let src = text_source(&["Column1", "Column2"], &[&[Some("__err__x"), Some("__row_idx")], &[Some("1"), Some("2")]]);
    let r = run(&query(src, vec![StepKind::PromoteHeaders]));
    assert_cols(&r, &["Column1", "Column2"]);
    assert_rows(&r, &[json!(["1", "2"])]);
}

#[test]
fn reserved_hidden_names_are_rejected() {
    let db = DuckDbStore::new_in_memory().unwrap();
    let q = query(
        text_source(&["a"], &[&[Some("1")]]),
        vec![StepKind::RenameColumns { renames: vec![("a".into(), "__row_idx".into())] }],
    );
    let p = preview(&db, &q, None);
    assert!(p.res.failed_step.unwrap().message.contains("reservado"));
}
