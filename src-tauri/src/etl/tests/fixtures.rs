//! Real (sanitized) M from the Rhino report: every fixture must convert with
//! zero `opaqueM` steps. dCalendario's `Query1(#date, n, #duration)` source
//! is handled by the `listDates` generator (Query1 is `List.Dates` in the
//! report's expressions.tmdl), so even that source is not opaque.

use super::*;
use crate::etl::m_from::query_from_m;
use crate::etl::m_to::query_to_m;
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/m-fixtures")
}

fn load(name: &str) -> (EtlQuery, Vec<String>) {
    let text = std::fs::read_to_string(fixture_dir().join(name)).expect("fixture");
    let r = query_from_m(name.trim_end_matches(".m"), &text).expect("parse fixture");
    (r.query, r.warnings)
}

fn ops(q: &EtlQuery) -> Vec<&'static str> {
    q.steps.iter().map(|s| s.kind.op_name()).collect()
}

#[test]
fn every_fixture_converts_without_opaque_steps() {
    let mut count = 0;
    for entry in std::fs::read_dir(fixture_dir()).expect("dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("m") {
            continue;
        }
        count += 1;
        let file = path.file_name().unwrap().to_str().unwrap().to_string();
        let (q, warnings) = load(&file);
        let opaque: Vec<_> = q
            .steps
            .iter()
            .filter(|s| matches!(s.kind, StepKind::OpaqueM { .. }))
            .map(|s| s.name.clone())
            .collect();
        assert!(opaque.is_empty(), "{}: passos opacos {:?} ({:?})", file, opaque, warnings);
        assert_ne!(q.source.kind, SourceKind::OpaqueM, "{}: fonte opaca", file);

        // etl_to_m(etl_from_m(m)) preserves semantics: compare by re-parsing.
        let m2 = query_to_m(&q).expect("to_m");
        let q2 = query_from_m(&q.name, &m2).expect("re-parse").query;
        assert_eq!(q2.without_ids(), q.without_ids(), "{}: re-parse divergiu\n{}", file, m2);
    }
    assert_eq!(count, 5, "esperados 5 fixtures");
}

#[test]
fn dcategorias_structure() {
    let (q, _) = load("rhino_dCategorias.m");
    assert_eq!(q.source, EtlSource::excel(r"C:\Dados\Fluxo de Caixa Tab. Fato - Rhino.xlsx", "Categorias"));
    assert_eq!(
        ops(&q),
        vec!["promoteHeaders", "changeType", "renameColumns", "removeRowsWithErrors", "removeRowsWithErrors", "removeBlankRows"]
    );
    assert_eq!(q.steps[0].name, "Promoted Headers");
    assert_eq!(q.steps[5].name, "Linhas em Branco Removidas");
    let StepKind::ChangeType { columns, culture } = &q.steps[1].kind else { panic!() };
    assert_eq!(culture, &None);
    assert_eq!(columns.len(), 7);
    assert_eq!(columns[2], ColumnTypeSpec { column: "Ordem DRE".into(), ty: EtlType::Integer });
    assert_eq!(
        q.steps[2].kind,
        StepKind::RenameColumns {
            renames: vec![
                ("Entra na DRE (competência)".into(), "DRE".into()),
                ("Entra no Fluxo de Caixa (caixa)".into(), "DFC".into())
            ]
        }
    );
}

#[test]
fn faberturas_and_flancamentos_structure() {
    let (q, _) = load("rhino_fAberturas.m");
    assert_eq!(q.source.sheet.as_deref(), Some("Aberturas do Mês"));
    assert_eq!(ops(&q), vec!["promoteHeaders", "changeType", "removeRowsWithErrors", "removeBlankRows"]);
    let StepKind::ChangeType { columns, .. } = &q.steps[1].kind else { panic!() };
    assert_eq!(columns[1].ty, EtlType::Currency);

    let (q, _) = load("rhino_fLançamentos.m");
    assert_eq!(q.source.sheet.as_deref(), Some("Fato"));
    assert_eq!(ops(&q), vec!["promoteHeaders", "changeType"]);
}

#[test]
fn medidas_is_an_inline_table() {
    let (q, _) = load("rhino_Medidas.m");
    assert_eq!(q.source, EtlSource::inline(vec!["Column1".into()], vec![vec![Scalar::text("Medidas=")]]));
    assert_eq!(
        q.steps.iter().map(|s| s.kind.clone()).collect::<Vec<_>>(),
        vec![
            StepKind::RemoveBottomRows { count: 1 },
            StepKind::RemoveColumns { columns: vec!["Column1".into()] }
        ]
    );
}

#[test]
fn dcalendario_structure() {
    let (q, warnings) = load("rhino_dCalendario.m");
    assert_eq!(q.source.kind, SourceKind::ListDates);
    assert_eq!(q.source.start.as_deref(), Some("2026-04-01"));
    assert_eq!(q.source.count, Some(275));
    assert_eq!(q.source.step_days, Some(1));
    assert_eq!(q.source.function.as_deref(), Some("Query1"));
    assert!(warnings.iter().any(|w| w.contains("Query1")), "{:?}", warnings);
    assert_eq!(
        ops(&q),
        vec![
            "renameColumns",
            "changeType",
            "addCustomColumn",
            "addCustomColumn",
            "addCustomColumn",
            "addCustomColumn",
            "addCustomColumn",
            "addCustomColumn",
            "renameColumns",
            "addConditionalColumn",
            "changeType",
            "addCustomColumn",
            "duplicateColumn",
            "changeType",
            "mergeColumns",
            "textTransform",
        ]
    );
    // Table.CombineColumns(Table.TransformColumnTypes(.., "it-IT"), ..) → two steps.
    assert_eq!(q.steps[13].name, "Merged Columns - Changed Type");
    assert_eq!(
        q.steps[13].kind,
        StepKind::ChangeType {
            columns: vec![ColumnTypeSpec { column: "Ano - Copy".into(), ty: EtlType::Text }],
            culture: Some("it-IT".into())
        }
    );
    assert_eq!(
        q.steps[14].kind,
        StepKind::MergeColumns {
            columns: vec!["First Characters".into(), "Ano - Copy".into()],
            separator: "/".into(),
            new_column: "Mês/Ano".into()
        }
    );
    assert_eq!(
        q.steps[4].kind,
        StepKind::AddCustomColumn {
            new_column: "Month Name".into(),
            expression: "Date.MonthName([Data], \"pt-BR\")".into(),
            ty: Some(EtlType::Text)
        }
    );
    assert_eq!(
        q.steps[9].kind,
        StepKind::AddConditionalColumn {
            new_column: "Semestre".into(),
            rules: vec![ConditionalRule {
                when: Cond::cmp("Mês", CondOp::Lte { value: n(6.0) }),
                then: ValueOrColumn::Value(s("S1"))
            }],
            otherwise: ValueOrColumn::Value(s("S2")),
            ty: None
        }
    );
}

/// The calendar fixture actually runs: hand-checked rows for 2026-04-01 and
/// 2026-09-06 (a Sunday, so the week of month increments).
#[test]
fn dcalendario_executes_with_pt_br_names() {
    let (q, _) = load("rhino_dCalendario.m");
    let r = run(&q);
    assert_cols(
        &r,
        &["Data", "Ano", "Mês", "Nome do Mês", "Dia", "Nome do Dia", "Semana do Mês", "Semestre", "Mês/Ano"],
    );
    assert_eq!(r.rows.len(), 275);
    // 2026-04-01 is a Wednesday; week 1 of April (April starts on Wednesday).
    assert_rows(
        &Run { rows: vec![r.rows[0].clone()], cols: r.cols.clone(), types: vec![], res: r.res.clone() },
        &[serde_json::json!(["2026-04-01", 2026, 4, "abril", 1, "quarta-feira", 1, "S1", "Abr/2026"])],
    );
    // 2026-09-06: offset 158 days from 2026-04-01 (30+31+30+31+31 = 153 → Sep 1, +5).
    // September 2026 starts on a Tuesday (dow 2): week = (6-1+2) div 7 + 1 = 2.
    let sep6 = r.rows.iter().find(|row| row[0] == "2026-09-06").expect("2026-09-06");
    let one = Run { rows: vec![sep6.clone()], cols: r.cols.clone(), types: vec![], res: r.res.clone() };
    assert_rows(&one, &[serde_json::json!(["2026-09-06", 2026, 9, "setembro", 6, "domingo", 2, "S2", "Set/2026"])]);
    // Last date: 2026-04-01 + 274 days = 2026-12-31.
    assert_eq!(r.rows[274][0], "2026-12-31");
    assert_eq!(r.types[1], "integer");
    assert_eq!(r.types[0], "date");
}
