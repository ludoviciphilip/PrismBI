//! Real-project repro, `#[ignore]`d (needs local data): loads the Rhino
//! project through the exact production path (`pbi::pbip::open_pbip_project`
//! -> `DuckDbStore` -> `dax::duckdb_load` -> `DaxModel::sync_from_payload_with_duckdb`,
//! same as `lib.rs`'s `open_pbip_project` Tauri command) from a **copy**
//! under `$TMPDIR`, never touching `~/Documents/Power BI/Rhino` itself
//! (read-only project per the task brief). Set `VETROBI_RHINO_COPY` to the
//! copy's `.pbip` path to run it:
//! `cargo test --lib -- --ignored rhino_real_project --nocapture`.
//!
//! Investigation note (left here because it changes what "correct" means
//! for this fixture): `dCalendario`'s M partition is
//! `Query1(#date(2026, 4, 1), 275, #duration(1, 0, 0, 0))` — 275 **days
//! starting 2026-04-01**, i.e. the calendar table in this copy of the
//! project only ever contains the single year 2026. So "one row, year
//! 2026" from `SUMMARIZECOLUMNS(dCalendario[Ano], ...)` is the CORRECT
//! answer for this data, not the bug — confirmed with `rhino_diag` below
//! before writing any assertion. The real, reproduced bugs are: (1) `T`
//! must not be BLANK (it wasn't, in this data, once the DEFINE MEASURE
//! resolution itself was exercised — see the synthetic
//! `tests_repro_rhino.rs` for the chain-of-measures shape); (2) the year
//! must render as "2026", not "2.026" (a UI-only formatting bug, fixed in
//! `DaxQueryView.tsx`/`formatter.ts`, verified by the `format_string`
//! ("0", no grouping) captured below reaching `DaxQueryColumn`).
use crate::dax::duckdb_load::load_table_from_duckdb;
use crate::dax::query_view::qexec::{execute_query, DaxQueryOptions};
use crate::dax::{DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use crate::engine::DuckDbStore;
use crate::pbi::pbip::open_pbip_project;
use std::collections::HashMap;
use std::path::PathBuf;

fn rhino_copy_path() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("VETROBI_RHINO_COPY").ok()?);
    p.exists().then_some(p)
}

fn load_rhino() -> (DaxModel, DuckDbStore) {
    let path = rhino_copy_path().expect("set VETROBI_RHINO_COPY to a copy of Rhino Bip.pbip");
    let db = DuckDbStore::new_in_memory().unwrap();
    let summary = open_pbip_project(&path, Some(&db)).expect("abrir cópia do Rhino");

    let mut tables = Vec::new();
    let mut measures = Vec::new();
    let mut freshly = HashMap::new();
    for t in &summary.tables {
        for meas in &t.measures {
            measures.push(DaxMeasureInput { name: meas.name.clone(), table_name: t.name.clone(), formula: meas.expression.clone() });
        }
    }
    for lt in &summary.loaded_tables {
        let col_types: Vec<Option<String>> = lt.columns.iter().map(|c| Some(c.data_type.clone())).collect();
        let duckdb_name = lt.table_name.clone();
        let conn = db.connection().lock().unwrap();
        let data = load_table_from_duckdb(&conn, &duckdb_name, &col_types)
            .unwrap_or_else(|e| panic!("load '{}': {}", duckdb_name, e));
        drop(conn);
        freshly.insert(duckdb_name.to_lowercase(), (1u64, data));
        let column_format_strings: HashMap<String, String> = summary
            .tables
            .iter()
            .find(|tm| tm.name.eq_ignore_ascii_case(&lt.table_name))
            .map(|tm| tm.columns.iter().filter_map(|c| c.format_string.as_ref().map(|fs| (c.name.clone(), fs.clone()))).collect())
            .unwrap_or_default();
        tables.push(DaxTableInput {
            name: lt.table_name.clone(),
            columns: vec![],
            rows: vec![],
            column_types: col_types,
            duckdb_table: Some(duckdb_name),
            column_format_strings,
            ..Default::default()
        });
    }
    let relationships: Vec<DaxRelationshipInput> = summary
        .relationships
        .iter()
        .map(|r| DaxRelationshipInput {
            from_table: r.from_table.clone(),
            from_column: r.from_column.clone(),
            to_table: r.to_table.clone(),
            to_column: r.to_column.clone(),
            cross_filter_direction: r.cross_filter_direction.clone(),
            is_active: r.is_active,
        })
        .collect();

    let mut model = DaxModel::new();
    model.sync_from_payload_with_duckdb(DaxModelSyncPayload { tables, relationships, measures }, freshly);
    (model, db)
}

/// The exact query Luis reported: every returned row must have a
/// non-blank `T` equal to `2 * Faturamento` for that row's year, the year
/// must render without a BLANK `T`, and `dCalendario[Ano]`'s model
/// `formatString` ("0") must reach the result column (so the UI can skip
/// thousands-grouping for it; see `tests_ui_format` in TS for the other
/// half of this fix).
#[test]
#[ignore]
fn rhino_real_project_summarizecolumns_define_measure_over_chain() {
    let (model, _db) = load_rhino();
    let query = "DEFINE MEASURE fLançamentos[Teste] = [Faturamento] * 2\nEVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"T\", [Teste])\nORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query falhou: {:?}", res.error);
    let rs = &res.result_sets[0];
    println!("rows: {:?}", rs.display_rows);
    assert!(rs.row_count >= 1, "esperava ao menos 1 linha, veio {}", rs.row_count);
    assert_eq!(rs.columns[0].format_string.as_deref(), Some("0"), "dCalendario[Ano] deveria carregar o formatString do TMDL");
    for row in &rs.display_rows {
        assert!(!row[0].contains(['.', ',']), "ano não deveria ter separador: {:?}", row);
        assert!(!row[1].is_empty(), "T veio em branco para o ano {}", row[0]);
    }

    // query_visual_native parity (task item 5): SUM(Faturamento) by
    // dCalendario[Ano] through the visual engine must match 1/2 of the
    // query view's T for every year.
    use crate::dax::query_planner::{execute_visual_query, NativeVisualQueryRequest};
    let visual_req = NativeVisualQueryRequest {
        visual_id: "v1".into(),
        visual_type: "bar".into(),
        table_id: "fLançamentos".into(),
        x_axis_field: Some("dCalendario.Ano".into()),
        legend_field: None,
        y_axis_fields: vec!["Faturamento".into()],
        aggregation: None,
        filters: vec![],
        ..Default::default()
    };
    let visual = execute_visual_query(&model, &visual_req).expect("visual query falhou");
    let mut from_query_view: HashMap<String, f64> = HashMap::new();
    for row in &rs.display_rows {
        from_query_view.insert(row[0].clone(), row[1].parse().expect("T deveria ser numérico"));
    }
    assert_eq!(visual.categories.len(), rs.row_count, "visual e query view divergem no número de anos");
    for (cat, val) in visual.categories.iter().zip(visual.series[0].data.iter()) {
        let t = *from_query_view.get(cat).unwrap_or_else(|| panic!("ano {} não está na query view", cat));
        let faturamento = val.unwrap_or(0.0);
        assert!((faturamento * 2.0 - t).abs() < 0.01, "ano {}: visual Faturamento={} x2 != query view T={}", cat, faturamento, t);
    }
}

/// Baseline: `[Faturamento]` alone (no DEFINE, no `* 2`) matches the same
/// shape — isolates whether the bug is in DEFINE substitution or in plain
/// `SUMMARIZECOLUMNS` context propagation from `dCalendario[Ano]` (it's
/// neither; see the module doc).
#[test]
#[ignore]
fn rhino_real_project_faturamento_alone_by_year() {
    let (model, _db) = load_rhino();
    let query = "EVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"F\", [Faturamento]) ORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    assert!(res.error.is_none(), "query falhou: {:?}", res.error);
    let rs = &res.result_sets[0];
    println!("rows (Faturamento alone): {:?}", rs.display_rows);
    assert!(rs.row_count >= 1, "esperava ao menos 1 linha, veio {}", rs.row_count);
    for row in &rs.display_rows {
        assert!(!row[1].is_empty(), "Faturamento veio em branco para o ano {}", row[0]);
    }
}

/// `[AAAAAA]` doesn't exist in the model: the error must carry a real
/// line/column (task item 4), not `(0, 0)`/`(1, 1)` defaults.
#[test]
#[ignore]
fn rhino_real_project_unknown_reference_reports_position() {
    let (model, _db) = load_rhino();
    let query = "DEFINE MEASURE fLançamentos[Teste] = [AAAAAA] * 2\nEVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], \"T\", [Teste])\nORDER BY dCalendario[Ano] DESC";
    let res = execute_query(&model, query, &DaxQueryOptions::default());
    let err = res.error.expect("esperava erro de referência desconhecida");
    assert!(err.message.contains("AAAAAA"), "mensagem: {}", err.message);
    assert_eq!(err.line, 1, "linha errada: {:?}", err);
    assert!(err.column > 1, "coluna não apontou para [AAAAAA]: {:?}", err);
}

/// Diagnostic kept for future debugging of this fixture's calendar shape
/// (not a regression test — prints, doesn't assert much).
#[test]
#[ignore]
fn rhino_diag_calendar_shape() {
    use crate::dax::FilterContext;
    let (model, _db) = load_rhino();
    let ctx = FilterContext::default();
    let idxs = model.get_visible_row_indices("dCalendario", &ctx);
    let tbl = model.get_table("dCalendario").unwrap();
    let cidx = tbl.get_col_idx("Ano").unwrap();
    let mut years: Vec<_> = idxs.iter().map(|&i| tbl.rows[i][cidx].clone()).collect();
    years.sort_by(|a, b| a.as_f64().partial_cmp(&b.as_f64()).unwrap());
    years.dedup_by(|a, b| a.canonical_key() == b.canonical_key());
    println!("visible dCalendario rows: {}, distinct years: {:?}", idxs.len(), years);
}
