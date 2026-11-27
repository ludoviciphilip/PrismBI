//! Corpus de conformidade DAX compartilhado com o motor TS
//! (`src/engine/__tests__/dax_corpus.test.ts`). Formato em `tests/dax-corpus/README.md`.
//!
//! Catraca: casos listados em `tests/dax-corpus/baseline.json` (chave "rust")
//! não podem voltar a falhar. Para registrar casos novos que passaram:
//! `UPDATE_DAX_BASELINE=1 cargo test --test dax_corpus`.

use vetrobi_lib::dax::functions::{civil_to_days, days_to_civil};
use vetrobi_lib::dax::{
    build_filter_context, DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput,
    DaxTableInput, DaxValue, NativeFilterInput,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

const ENGINE: &str = "rust";

#[derive(Deserialize)]
struct CorpusColumn {
    name: String,
    #[serde(default, rename = "type")]
    col_type: Option<String>,
}

#[derive(Deserialize)]
struct CalendarSpec {
    start: String,
    end: String,
}

#[derive(Deserialize)]
struct CorpusTable {
    name: String,
    #[serde(default)]
    columns: Vec<CorpusColumn>,
    #[serde(default)]
    rows: Vec<Vec<Value>>,
    calendar: Option<CalendarSpec>,
    /// Marks this model table as a calculated table (TMDL `calculated`
    /// partition equivalent) -- CALENDARAUTO must skip it when scanning for
    /// date columns (see `window.calendarauto.fiscal_july_days`).
    #[serde(default)]
    calculated: bool,
    /// When present, this table is built by the real `dax::calculated`
    /// pipeline from a DAX table expression (CALENDAR, ADDCOLUMNS,
    /// SUMMARIZE, UNION, FILTER, ...) instead of from `rows` -- exercises
    /// the production calculated-table engine, not a hand-copied fixture.
    #[serde(default)]
    expression: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusRelationship {
    from: String,
    to: String,
    cross_filter: String,
    active: bool,
}

#[derive(Deserialize)]
struct CorpusMeasure {
    table: String,
    name: String,
    formula: String,
}

#[derive(Deserialize)]
struct CorpusModel {
    tables: Vec<CorpusTable>,
    relationships: Vec<CorpusRelationship>,
    measures: Vec<CorpusMeasure>,
    /// table name -> (column name -> DAX row expression). See
    /// `models/calculated.json` for RELATED/EARLIER examples.
    #[serde(default, rename = "calculatedColumns")]
    calculated_columns: HashMap<String, HashMap<String, String>>,
}

#[derive(Deserialize)]
struct CorpusFilter {
    table: String,
    column: String,
    values: Vec<Value>,
}

#[derive(Deserialize)]
struct CorpusCase {
    id: String,
    expr: String,
    #[serde(default)]
    filters: Vec<CorpusFilter>,
    expected: Value,
    tolerance: Option<f64>,
}

#[derive(Deserialize)]
struct CorpusFile {
    model: String,
    category: String,
    cases: Vec<CorpusCase>,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/dax-corpus")
}

fn split_ref(r: &str) -> (String, String) {
    let r = r.trim();
    let open = r.find('[').expect("referência sem '['");
    (r[..open].to_string(), r[open + 1..r.len() - 1].to_string())
}

fn parse_iso(s: &str) -> (i32, u32, u32) {
    let p: Vec<&str> = s.split('-').collect();
    (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].parse().unwrap())
}

/// Mesmo gerador do lado TS: Data (ISO), Ano, MesNum, Trimestre, AnoMes.
fn calendar_rows(spec: &CalendarSpec) -> Vec<Map<String, Value>> {
    let (sy, sm, sd) = parse_iso(&spec.start);
    let (ey, em, ed) = parse_iso(&spec.end);
    (civil_to_days(sy, sm, sd)..=civil_to_days(ey, em, ed))
        .map(|day| {
            let (y, m, d) = days_to_civil(day);
            let mut row = Map::new();
            row.insert("Data".into(), json!(format!("{:04}-{:02}-{:02}", y, m, d)));
            row.insert("Ano".into(), json!(y));
            row.insert("MesNum".into(), json!(m));
            row.insert("Trimestre".into(), json!((m - 1) / 3 + 1));
            row.insert("AnoMes".into(), json!(format!("{:04}-{:02}", y, m)));
            row
        })
        .collect()
}

fn build_model(m: &CorpusModel) -> DaxModel {
    let tables = m
        .tables
        .iter()
        .map(|t| {
            let columns: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
            let mut column_types: Vec<Option<String>> = t.columns.iter().map(|c| c.col_type.clone()).collect();
            let rows = match &t.calendar {
                Some(spec) => {
                    // Generated calendar columns (Data, Ano, MesNum, Trimestre, AnoMes) carry
                    // their own implicit types regardless of what the model json declares.
                    column_types = columns
                        .iter()
                        .map(|c| match c.as_str() {
                            "Data" => Some("date".to_string()),
                            "Ano" | "MesNum" | "Trimestre" => Some("number".to_string()),
                            _ => Some("text".to_string()),
                        })
                        .collect();
                    calendar_rows(spec)
                }
                None => t
                    .rows
                    .iter()
                    .map(|r| {
                        columns
                            .iter()
                            .enumerate()
                            .map(|(i, c)| (c.clone(), r.get(i).cloned().unwrap_or(Value::Null)))
                            .collect()
                    })
                    .collect(),
            };
            DaxTableInput {
                duckdb_table: None,
                name: t.name.clone(),
                columns,
                rows,
                column_types,
                sort_by_columns: Default::default(),
                is_calculated: t.expression.is_some(),
                expression: t.expression.clone(),
                calculated_columns: m
                    .calculated_columns
                    .get(&t.name)
                    .cloned()
                    .unwrap_or_default(),
            }
        })
        .collect();
    let relationships = m
        .relationships
        .iter()
        .map(|r| {
            let (ft, fc) = split_ref(&r.from);
            let (tt, tc) = split_ref(&r.to);
            DaxRelationshipInput {
                from_table: ft,
                from_column: fc,
                to_table: tt,
                to_column: tc,
                cross_filter_direction: r.cross_filter.clone(),
                is_active: r.active,
            }
        })
        .collect();
    let measures = m
        .measures
        .iter()
        .map(|x| DaxMeasureInput {
            name: x.name.clone(),
            table_name: x.table.clone(),
            formula: x.formula.clone(),
        })
        .collect();
    let mut model = DaxModel::new();
    model.sync_from_payload(DaxModelSyncPayload { tables, relationships, measures });
    // Corpus tables marked `"calculated": true` carry literal hand-derived
    // `rows` (never run through the real DAX calculated-table pipeline, per
    // the corpus rule against deriving expected values from any engine) but
    // still need `is_calculated` set so CALENDARAUTO and future
    // calculated-aware logic treat them like a real calculated table.
    for t in &m.tables {
        if t.calculated {
            if let Some(tbl) = model.tables.get_mut(&t.name.to_lowercase()) {
                tbl.is_calculated = true;
            }
        }
    }
    model
}

fn value_matches(expected: &Value, actual: &DaxValue, tol: f64) -> bool {
    match expected {
        Value::Null => actual.is_blank(),
        Value::Bool(b) => matches!(actual, DaxValue::Boolean(a) if a == b),
        Value::Number(n) => {
            let want = n.as_f64().unwrap();
            matches!(actual, DaxValue::Integer(_) | DaxValue::Decimal(_) | DaxValue::Currency(_))
                && actual
                    .as_f64()
                    .is_some_and(|a| a.is_finite() && (a - want).abs() <= tol * want.abs().max(1.0))
        }
        Value::String(s) => matches!(actual, DaxValue::Text(a) if a == s),
        Value::Object(o) => {
            if let Some(Value::String(d)) = o.get("date") {
                matches!(actual, DaxValue::Text(_) | DaxValue::DateTime(_))
                    && actual.as_display_string().get(..10) == Some(d.as_str())
            } else {
                false
            }
        }
        _ => false,
    }
}

#[test]
fn dax_corpus_rust_engine() {
    let dir = corpus_dir();
    let mut models: HashMap<String, DaxModel> = HashMap::new();
    for entry in fs::read_dir(dir.join("models")).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|e| e.to_str()) == Some("json") {
            let m: CorpusModel = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
            models.insert(p.file_stem().unwrap().to_string_lossy().into(), build_model(&m));
        }
    }

    let mut case_files: Vec<PathBuf> = fs::read_dir(dir.join("cases"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    case_files.sort();

    // DAX_CORPUS_ONLY=<categoria>: roda só essa categoria, grava relatório próprio
    // e não mexe na baseline (útil para trabalho em paralelo).
    let only = std::env::var("DAX_CORPUS_ONLY").ok().filter(|s| !s.is_empty());
    let mut results = Vec::new();
    let mut passing = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for path in case_files {
        let file: CorpusFile = serde_json::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
        if only.as_deref().is_some_and(|o| o != file.category) {
            continue;
        }
        let model = models
            .get(&file.model)
            .unwrap_or_else(|| panic!("modelo {} inexistente", file.model));
        for c in file.cases {
            assert!(seen.insert(c.id.clone()), "id duplicado: {}", c.id);
            let filters: Vec<NativeFilterInput> = c
                .filters
                .iter()
                .map(|f| NativeFilterInput {
                    table_id: f.table.clone(),
                    column: f.column.clone(),
                    values: f.values.clone(),
                })
                .collect();
            let ctx = build_filter_context(&filters, model);
            let expect_error = c.expected.get("error").is_some();
            // panics no motor contam como falha do caso, não do harness
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                model.evaluate_dax(&c.expr, &ctx)
            }));
            let (pass, actual, error) = match outcome {
                Ok(Ok(v)) => (
                    !expect_error && value_matches(&c.expected, &v, c.tolerance.unwrap_or(1e-9)),
                    v.to_json(),
                    None,
                ),
                Ok(Err(e)) => (expect_error, Value::Null, Some(e)),
                Err(_) => (false, Value::Null, Some("panic".to_string())),
            };
            if pass {
                passing.insert(c.id.clone());
            }
            results.push(json!({
                "id": c.id, "category": file.category, "pass": pass,
                "actual": actual, "error": error,
            }));
        }
    }

    let report = json!({
        "engine": ENGINE, "total": results.len(), "passing": passing.len(), "results": results,
    });
    fs::write(
        dir.join(match &only {
            Some(o) => format!(".report-{}.{}.json", ENGINE, o),
            None => format!(".report-{}.json", ENGINE),
        }),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();

    let baseline_path = dir.join("baseline.json");
    let mut baseline: BTreeMap<String, Vec<String>> = fs::read_to_string(&baseline_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let known: BTreeSet<String> = baseline.get(ENGINE).cloned().unwrap_or_default().into_iter().collect();

    if std::env::var("UPDATE_DAX_BASELINE").as_deref() == Ok("1") && only.is_none() {
        baseline.insert(ENGINE.into(), passing.iter().cloned().collect());
        fs::write(&baseline_path, serde_json::to_string_pretty(&baseline).unwrap() + "\n").unwrap();
    }

    let regressions: Vec<String> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| known.contains(r["id"].as_str().unwrap()) && !r["pass"].as_bool().unwrap())
        .map(|r| format!("{}: obtido {} {}", r["id"], r["actual"], r["error"]))
        .collect();
    let newly = passing.iter().filter(|id| !known.contains(*id)).count();
    eprintln!(
        "[dax-corpus:rust] {}/{} passando; {} novo(s) fora da baseline",
        passing.len(),
        report["total"],
        newly
    );
    assert!(regressions.is_empty(), "regressões no motor Rust:\n{}", regressions.join("\n"));
}
