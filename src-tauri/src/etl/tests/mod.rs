//! ETL test suite (unit tests compiled into the lib test binary, so they
//! share the existing link step instead of adding new test executables).
//!
//! Expected values in `steps.rs` / `security.rs` were derived by hand from
//! Power Query semantics, never by running the engine and copying output.

mod contract;
mod fixtures;
mod rhino;
mod roundtrip;
mod security;
mod steps;

use crate::engine::DuckDbStore;
use crate::etl::commands::{etl_apply_impl, etl_load_raw_impl, etl_preview_impl, PreviewResult};
use crate::etl::model::*;
use serde_json::Value as JsonValue;

pub(super) fn s(v: &str) -> Scalar {
    Scalar::text(v)
}

pub(super) fn n(v: f64) -> Scalar {
    Scalar::Number(v)
}

pub(super) fn step(kind: StepKind) -> EtlStep {
    let name = kind.default_step_name().to_string();
    EtlStep::new("", name, kind)
}

pub(super) fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Inline source from rows of `Option<&str>` (None = null), all text.
pub(super) fn text_source(columns: &[&str], rows: &[&[Option<&str>]]) -> EtlSource {
    EtlSource::inline(
        strs(columns),
        rows.iter()
            .map(|r| {
                r.iter()
                    .map(|c| match c {
                        Some(t) => Scalar::text(*t),
                        None => Scalar::Null,
                    })
                    .collect()
            })
            .collect(),
    )
}

pub(super) struct Run {
    pub cols: Vec<String>,
    pub types: Vec<String>,
    pub rows: Vec<Vec<JsonValue>>,
    pub res: PreviewResult,
}

pub(super) fn preview(db: &DuckDbStore, q: &EtlQuery, upto: Option<i64>) -> Run {
    etl_load_raw_impl(db, &q.name, &q.source, None).expect("load raw");
    let res = etl_preview_impl(db, q, upto, Some(10_000)).expect("preview");
    let cols: Vec<String> = res.columns.iter().map(|c| c.name.clone()).collect();
    let types: Vec<String> = res.columns.iter().map(|c| c.ty.clone()).collect();
    let rows = res
        .rows
        .iter()
        .map(|r| cols.iter().map(|c| r.get(c).cloned().unwrap_or(JsonValue::Null)).collect())
        .collect();
    Run { cols, types, rows, res }
}

/// Runs the whole query; panics if any step failed.
pub(super) fn run(q: &EtlQuery) -> Run {
    let db = DuckDbStore::new_in_memory().unwrap();
    run_in(&db, q)
}

pub(super) fn run_in(db: &DuckDbStore, q: &EtlQuery) -> Run {
    let r = preview(db, q, None);
    assert!(r.res.failed_step.is_none(), "falhou: {:?}", r.res.failed_step);
    r
}

pub(super) fn query(source: EtlSource, steps: Vec<StepKind>) -> EtlQuery {
    EtlQuery {
        name: "t".into(),
        source,
        steps: steps.into_iter().map(step).collect(),
    }
}

fn val_eq(a: &JsonValue, b: &JsonValue) -> bool {
    match (a, b) {
        (JsonValue::Number(x), JsonValue::Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9,
        _ => a == b,
    }
}

#[track_caller]
pub(super) fn assert_rows(r: &Run, expected: &[JsonValue]) {
    let exp: Vec<Vec<JsonValue>> = expected
        .iter()
        .map(|e| e.as_array().expect("linha esperada deve ser array").clone())
        .collect();
    let ok = r.rows.len() == exp.len()
        && r.rows
            .iter()
            .zip(&exp)
            .all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| val_eq(x, y)));
    assert!(
        ok,
        "linhas diferentes\n colunas: {:?}\n obtido:   {:?}\n esperado: {:?}",
        r.cols, r.rows, exp
    );
}

#[track_caller]
pub(super) fn assert_cols(r: &Run, expected: &[&str]) {
    assert_eq!(r.cols, strs(expected));
}

pub(super) fn apply(db: &DuckDbStore, q: &EtlQuery, target: &str) -> crate::engine::ImportedTablePayload {
    etl_load_raw_impl(db, &q.name, &q.source, None).expect("load raw");
    etl_apply_impl(db, q, target).expect("apply")
}
