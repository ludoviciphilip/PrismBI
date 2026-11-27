//! v0.11 Frente A: differential tests — the same `NativeVisualQueryRequest`
//! run through `pushdown::try_pushdown` and through the plain DAX engine
//! (`query_planner::execute_visual_query`, no connection) must produce
//! identical `categories`/`series`/`summary_value` whenever pushdown
//! accepts the request; whenever it should reject (inactive/bidirectional
//! relationship, matrix/levels, legend), it must reject with `Err` and the
//! full-stack `execute_visual_query_with_conn` must still return the
//! correct DAX-engine result (`engine == "rust-native"`) — "não aproxime".

use super::*;
use crate::dax::query_planner::{
    execute_visual_query, execute_visual_query_with_conn, NativeGroupLevel, NativeVisualQueryRequest,
};
use crate::dax::{DaxMeasureInput, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
use std::collections::HashMap;
use std::sync::Mutex;

fn open_mem() -> Connection {
    Connection::open_in_memory().expect("open duckdb in-memory")
}

/// Builds a `DaxModel` whose tables are genuinely backed by DuckDB tables
/// (loaded through the same `duckdb_load::load_table_from_duckdb` production
/// uses), so the differential test compares pushdown and DAX against the
/// *same* physical data, not two hand-written fixtures that happen to agree.
fn build_model(
    conn: &Connection,
    tables: &[(&str, &str, &[Option<&str>])],
    relationships: Vec<DaxRelationshipInput>,
    measures: Vec<DaxMeasureInput>,
) -> DaxModel {
    let mut freshly = HashMap::new();
    let mut inputs = Vec::new();
    for (dax_name, duckdb_name, types) in tables {
        let decl: Vec<Option<String>> = types.iter().map(|o| o.map(str::to_string)).collect();
        let data = super::super::duckdb_load::load_table_from_duckdb(conn, duckdb_name, &decl)
            .unwrap_or_else(|e| panic!("load '{}': {}", duckdb_name, e));
        freshly.insert(duckdb_name.to_lowercase(), (1u64, data));
        inputs.push(DaxTableInput {
            name: dax_name.to_string(),
            columns: vec![],
            rows: vec![],
            column_types: decl,
            duckdb_table: Some(duckdb_name.to_string()),
            sort_by_columns: Default::default(),
            ..Default::default()
        });
    }
    let mut model = DaxModel::new();
    model.sync_from_payload_with_duckdb(DaxModelSyncPayload { tables: inputs, relationships, measures }, freshly);
    model
}

/// Base fixture: `dCategoria` (dimension) and `fVendas` (fact), one active
/// single-direction relationship, covering: a BLANK measure cell (NULL
/// `Valor`), a blank *dimension* member (`Nome` NULL and `Nome = ''`, which
/// must both bucket under "Sem dados"), a repeated value (DISTINCTCOUNT),
/// and an orphan fact row (`CategoriaId = 99`, no matching dimension row —
/// must disappear from every category, not surface as a synthetic one).
fn base_fixture(conn: &Connection) -> DaxModel {
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'B'), (3, NULL), (4, '');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES
            (1, 100.0), (1, 50.0), (1, NULL),
            (2, 200.0), (2, 200.0),
            (3, 10.0), (4, 20.0),
            (99, 500.0);",
    )
    .expect("seed base fixture");

    build_model(
        conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![],
    )
}

fn req(visual_type: &str, x_axis: Option<&str>, y: &[&str], agg: Option<&str>) -> NativeVisualQueryRequest {
    NativeVisualQueryRequest {
        visual_id: "v1".into(),
        visual_type: visual_type.into(),
        table_id: "fVendas".into(),
        x_axis_field: x_axis.map(str::to_string),
        legend_field: None,
        y_axis_fields: y.iter().map(|s| s.to_string()).collect(),
        aggregation: agg.map(str::to_string),
        filters: vec![],
        ..Default::default()
    }
}

/// Asserts pushdown is accepted and matches the DAX engine bit-for-bit
/// (categories, series names+data, summary) for one request/aggregation.
fn assert_parity(model: &DaxModel, conn: &Connection, r: &NativeVisualQueryRequest, agg: &str) {
    let dax = execute_visual_query(model, r).expect("DAX path failed");
    let pd = try_pushdown(model, conn, r, agg, Instant::now())
        .unwrap_or_else(|e| panic!("pushdown rejected a request it should accept: {}", e));
    assert_eq!(pd.categories, dax.categories, "categories differ for agg={}", agg);
    assert_eq!(pd.series.len(), dax.series.len(), "series count differs for agg={}", agg);
    for (ps, ds) in pd.series.iter().zip(dax.series.iter()) {
        assert_eq!(ps.name, ds.name, "series name differs for agg={}", agg);
        assert_eq!(ps.data, ds.data, "series data differs for agg={} series={}", agg, ps.name);
    }
    assert_eq!(pd.summary_value, dax.summary_value, "summary differs for agg={}", agg);
    assert_eq!(pd.engine, "duckdb");
}

#[test]
fn grouped_sum_matches_dax_with_blanks_blank_member_and_orphan_fk() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    // A=150 (100+50, NULL skipped), B=400 (200+200), "Sem dados"=30 (10+20,
    // from the two blank-Nome dim rows); the orphan Valor=500 never appears.
    assert_eq!(dax.categories, vec!["Sem dados", "A", "B"]);
    assert_eq!(dax.series[0].data, vec![Some(30.0), Some(150.0), Some(400.0)]);
}

#[test]
fn grouped_count_distinctcount_average_match_dax() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    for (agg, expected_b) in [("COUNT", 2.0), ("DISTINCTCOUNT", 1.0), ("AVERAGE", 200.0)] {
        let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some(agg));
        assert_parity(&model, &conn, &r, agg);
        let dax = execute_visual_query(&model, &r).unwrap();
        let b_idx = dax.categories.iter().position(|c| c == "B").unwrap();
        assert_eq!(dax.series[0].data[b_idx], Some(expected_b), "agg={}", agg);
    }
}

#[test]
fn card_and_multi_card_match_dax() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    let card = req("card", None, &["Valor"], Some("SUM"));
    assert_parity(&model, &conn, &card, "SUM");
    let multi = req("multi-card", None, &["Valor"], Some("DISTINCTCOUNT"));
    assert_parity(&model, &conn, &multi, "DISTINCTCOUNT");
}

#[test]
fn filter_pushdown_matches_dax() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    let mut r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    r.filters.push(crate::dax::NativeFilterInput {
        table_id: "dCategoria".into(),
        column: "Nome".into(),
        values: vec![serde_json::json!("A"), serde_json::json!("B")],
     ..Default::default() });
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(dax.categories, vec!["A", "B"]);
}

#[test]
fn calculate_simple_column_filter_measure_matches_dax() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'B');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE, Regiao VARCHAR);
         INSERT INTO fVendas VALUES
            (1, 100.0, 'Norte'), (1, 50.0, 'Sul'),
            (2, 300.0, 'Norte'), (2, 10.0, 'Sul');",
    )
    .expect("seed calculate fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number"), Some("text")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![DaxMeasureInput { name: "ValorNorte".into(),
            table_name: "fVendas".into(),
            formula: "CALCULATE(SUM(fVendas[Valor]), fVendas[Regiao] = \"Norte\")".into(), format_string: None }],
    );
    let r = req("column", Some("dCategoria.Nome"), &["ValorNorte"], Some("SUM"));
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(dax.categories, vec!["A", "B"]);
    assert_eq!(dax.series[0].data, vec![Some(100.0), Some(300.0)]);
}

#[test]
fn inactive_relationship_rejects_pushdown_and_dax_fallback_stays_correct() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'B');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (2, 200.0);",
    )
    .expect("seed inactive-rel fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: false, // <- the case under test
        }],
        vec![],
    );
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    let rejected = try_pushdown(&model, &conn, &r, "SUM", Instant::now());
    assert!(rejected.is_err(), "pushdown must reject an inactive relationship, not approximate it");

    let via_conn = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    let direct_dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(via_conn.engine, "rust-native");
    assert_eq!(via_conn.categories, direct_dax.categories);
    assert_eq!(via_conn.series[0].data, direct_dax.series[0].data);
}

#[test]
fn bidirectional_relationship_rejects_pushdown_and_dax_fallback_stays_correct() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'B');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (2, 200.0);",
    )
    .expect("seed bidirectional fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "both".into(), // <- the case under test
            is_active: true,
        }],
        vec![],
    );
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    assert!(try_pushdown(&model, &conn, &r, "SUM", Instant::now()).is_err());

    let via_conn = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    let direct_dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(via_conn.engine, "rust-native");
    assert_eq!(via_conn.categories, direct_dax.categories);
    assert_eq!(via_conn.series[0].data, direct_dax.series[0].data);
}

#[test]
fn matrix_rows_and_category_levels_reject_pushdown_and_dax_fallback_stays_correct() {
    let conn = open_mem();
    let model = base_fixture(&conn);

    let mut matrix_req = req("matrix", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    matrix_req.matrix_rows = vec![NativeGroupLevel { field: "dCategoria.Nome".into(), date_part: None }];
    assert!(try_pushdown(&model, &conn, &matrix_req, "SUM", Instant::now()).is_err());
    let via_conn = execute_visual_query_with_conn(&model, &matrix_req, Some(&conn)).unwrap();
    let direct_dax = execute_visual_query(&model, &matrix_req).unwrap();
    assert_eq!(via_conn.engine, "rust-native");
    assert_eq!(via_conn.matrix.map(|m| m.rows.len()), direct_dax.matrix.map(|m| m.rows.len()));

    let mut levels_req = req("column", None, &["Valor"], Some("SUM"));
    levels_req.category_levels = vec![NativeGroupLevel { field: "dCategoria.Nome".into(), date_part: None }];
    assert!(try_pushdown(&model, &conn, &levels_req, "SUM", Instant::now()).is_err());
    let via_conn2 = execute_visual_query_with_conn(&model, &levels_req, Some(&conn)).unwrap();
    let direct_dax2 = execute_visual_query(&model, &levels_req).unwrap();
    assert_eq!(via_conn2.engine, "rust-native");
    assert_eq!(via_conn2.categories, direct_dax2.categories);
}

/// `VETROBI_PUSHDOWN=0` disables the module; any other value (or unset)
/// leaves it enabled. Serialized with a process-wide mutex since env vars
/// are process-global and this is the only test touching this one.
static ENV_GUARD: Mutex<()> = Mutex::new(());

#[test]
fn env_flag_disables_pushdown_both_paths_covered() {
    let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
    let conn = open_mem();
    let model = base_fixture(&conn);
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));

    std::env::remove_var("VETROBI_PUSHDOWN");
    assert!(pushdown_enabled());
    let on = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    assert_eq!(on.engine, "duckdb");

    std::env::set_var("VETROBI_PUSHDOWN", "0");
    assert!(!pushdown_enabled());
    let off = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    assert_eq!(off.engine, "rust-native");
    assert_eq!(off.categories, on.categories);
    assert_eq!(off.series[0].data, on.series[0].data);

    std::env::remove_var("VETROBI_PUSHDOWN");
}

// ---------------------------------------------------------------------------
// v0.11 Frente H (Rodada 2): confirms/fixes the Rodada 1 pushdown-vs-DAX
// divergences raised in the review. Each test below targets exactly one
// item from the brief.
// ---------------------------------------------------------------------------

/// Item (a): text comparisons are case/trim-insensitive in DAX
/// (`DaxValue::value_key`'s `Text` branch) — a visual filter value typed in
/// a different case than the stored data must still match, in both engines.
#[test]
fn text_filter_is_case_insensitive_like_dax() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    let mut r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    // Stored value is "A"; the filter is typed lowercase, like a user
    // retyping a slicer selection or a bookmark captured with different casing.
    r.filters.push(crate::dax::NativeFilterInput {
        table_id: "dCategoria".into(),
        column: "Nome".into(),
        values: vec![serde_json::json!("a")],
        operator: None,
    });
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(dax.categories, vec!["A"], "lowercase filter value must still match the stored 'A'");
    assert_eq!(dax.series[0].data, vec![Some(150.0)]);
}

/// Item (b): a genuinely BLANK (NULL) key on both sides of a relationship
/// matches in DAX's relationship propagation (`ValueKey::Blank` is a normal
/// matchable key, not specially excluded) — a plain SQL `=` join would drop
/// that pairing since `NULL = NULL` is unknown in SQL.
#[test]
fn blank_matches_blank_across_the_relationship() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (NULL, 'SemChave');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (NULL, 999.0);",
    )
    .expect("seed blank-key fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![],
    );
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    // The blank FK (999.0) must land under the dimension's own blank-keyed
    // row ("SemChave"), not get dropped or bucketed as a generic orphan.
    let idx = dax.categories.iter().position(|c| c == "SemChave").expect("SemChave category present");
    assert_eq!(dax.series[0].data[idx], Some(999.0));
}

/// Items (c)/(d): COUNT/COUNTROWS/DISTINCTCOUNT return BLANK, not 0, for a
/// dimension member with no matching fact rows at all — Power BI drops that
/// member from the visual (`dax.guide`: "If the table has no rows, COUNTROWS
/// returns BLANK, not 0"), exactly the rule `query_planner::execute_grouped`
/// already uses to drop any-value-blank rows after fetch.
#[test]
fn count_family_is_blank_not_zero_for_member_without_facts() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'SemVendas');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (1, 50.0);",
    )
    .expect("seed no-facts fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![],
    );
    for agg in ["COUNT", "DISTINCTCOUNT"] {
        let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some(agg));
        assert_parity(&model, &conn, &r, agg);
        let dax = execute_visual_query(&model, &r).unwrap();
        // "SemVendas" must have disappeared from the visual entirely (BLANK
        // dropped, like every other blank-value row), not show up as 0.
        assert!(!dax.categories.contains(&"SemVendas".to_string()), "agg={} should drop the member without facts", agg);
    }
}

/// Item (e): `DISTINCTCOUNT` counts a stored BLANK as one of its distinct
/// values (`dax.guide/DISTINCTCOUNT`), unlike plain SQL `COUNT(DISTINCT
/// col)` which drops `NULL`s.
#[test]
fn distinctcount_counts_blank_as_a_value() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (1, 100.0), (1, NULL);",
    )
    .expect("seed distinctcount-blank fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![],
    );
    let r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("DISTINCTCOUNT"));
    assert_parity(&model, &conn, &r, "DISTINCTCOUNT");
    let dax = execute_visual_query(&model, &r).unwrap();
    // Distinct values are {100, BLANK} = 2, not 1 (which plain `COUNT(DISTINCT)` would give).
    assert_eq!(dax.series[0].data, vec![Some(2.0)]);
}

/// Item (f): `CALCULATE`'s filter argument *replaces* the context filter on
/// the same column; it never intersects with an outer (visual-level) filter
/// on that column. Pushdown's shared `WHERE` can't express a per-series
/// override, so it must reject and fall back to the (correct) DAX engine.
#[test]
fn calculate_filter_overriding_outer_filter_falls_back_to_dax() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR);
         INSERT INTO dCategoria VALUES (1, 'A'), (2, 'B');
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE, Regiao VARCHAR);
         INSERT INTO fVendas VALUES
            (1, 100.0, 'Norte'), (1, 50.0, 'Sul'),
            (2, 300.0, 'Norte'), (2, 10.0, 'Sul');",
    )
    .expect("seed override fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number"), Some("text")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![DaxMeasureInput { name: "ValorSul".into(),
            table_name: "fVendas".into(),
            formula: "CALCULATE(SUM(fVendas[Valor]), fVendas[Regiao] = \"Sul\")".into(), format_string: None }],
    );
    // The visual itself is filtered to Regiao=Norte, but the measure's own
    // CALCULATE(..., Regiao="Sul") must override that, not AND with it.
    let mut r = req("column", Some("dCategoria.Nome"), &["ValorSul"], Some("SUM"));
    r.filters.push(crate::dax::NativeFilterInput {
        table_id: "fVendas".into(),
        column: "Regiao".into(),
        values: vec![serde_json::json!("Norte")],
        operator: None,
    });
    assert!(
        try_pushdown(&model, &conn, &r, "SUM", Instant::now()).is_err(),
        "pushdown must refuse to intersect a CALCULATE filter with an outer filter on the same column"
    );
    let via_conn = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    let direct_dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(via_conn.engine, "rust-native");
    assert_eq!(via_conn.series[0].data, direct_dax.series[0].data);
    // Sul total per category, unaffected by the outer Norte filter.
    assert_eq!(direct_dax.series[0].data, vec![Some(50.0), Some(10.0)]);
}

/// Item (g): aggregating a column that lives on a *dimension* table (not
/// the fact) fans out through the `LEFT JOIN` — each dimension row gets
/// physically repeated once per matching fact row, so `SUM` over it would
/// multiply by the fact row count instead of reading the dimension's own
/// single value. Pushdown must reject this shape outright.
#[test]
fn aggregating_a_dimension_column_falls_back_to_dax() {
    let conn = open_mem();
    conn.execute_batch(
        "CREATE TABLE dCategoria (Id INTEGER, Nome VARCHAR, Meta DOUBLE);
         INSERT INTO dCategoria VALUES (1, 'A', 10.0), (2, 'B', 20.0);
         CREATE TABLE fVendas (CategoriaId INTEGER, Valor DOUBLE);
         INSERT INTO fVendas VALUES (1, 100.0), (1, 50.0), (1, 25.0), (2, 200.0);",
    )
    .expect("seed dim-column fixture");
    let model = build_model(
        &conn,
        &[
            ("dCategoria", "dCategoria", &[Some("number"), Some("text"), Some("number")]),
            ("fVendas", "fVendas", &[Some("number"), Some("number")]),
        ],
        vec![DaxRelationshipInput {
            from_table: "fVendas".into(),
            from_column: "CategoriaId".into(),
            to_table: "dCategoria".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        }],
        vec![],
    );
    // Grouped by category, but the y-field ("Meta") lives on the dimension,
    // not the fact — category "A" has 3 matching fact rows, so a naive
    // `SUM(b0.Meta)` over `dim LEFT JOIN fact` would read 10*3=30 instead of 10.
    let r = req("column", Some("dCategoria.Nome"), &["dCategoria.Meta"], Some("SUM"));
    assert!(
        try_pushdown(&model, &conn, &r, "SUM", Instant::now()).is_err(),
        "pushdown must refuse to aggregate a dimension column under a fact join"
    );
    let via_conn = execute_visual_query_with_conn(&model, &r, Some(&conn)).unwrap();
    let direct_dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(via_conn.engine, "rust-native");
    assert_eq!(via_conn.series[0].data, direct_dax.series[0].data);
    assert_eq!(direct_dax.series[0].data, vec![Some(10.0), Some(20.0)], "no fan-out by the fact join");
}

/// Item (h): a `between` range filter (segmentador "Entre") is an inclusive
/// range, not a 2-value `IN` list — `VisualCard.tsx`'s request-building code
/// used to flatten `{operator: "between", value: [lo, hi]}` into an IN list
/// of exactly the two endpoints, losing everything strictly between them.
#[test]
fn between_filter_is_an_inclusive_range_not_a_two_value_in_list() {
    let conn = open_mem();
    let model = base_fixture(&conn);
    let mut r = req("column", Some("dCategoria.Nome"), &["Valor"], Some("SUM"));
    // fVendas.Valor has 100, 50, NULL, 200, 200, 10, 20, 500 — between
    // {60, 250} should keep 100 and 200/200 (strictly between the two
    // endpoints), which a naive IN(60, 250) list would miss entirely.
    r.filters.push(crate::dax::NativeFilterInput {
        table_id: "fVendas".into(),
        column: "Valor".into(),
        values: vec![serde_json::json!(60), serde_json::json!(250)],
        operator: Some("between".into()),
    });
    assert_parity(&model, &conn, &r, "SUM");
    let dax = execute_visual_query(&model, &r).unwrap();
    assert_eq!(dax.categories, vec!["A", "B"]);
    assert_eq!(dax.series[0].data, vec![Some(100.0), Some(400.0)]);
}

/// Item (i): a measure that references itself (directly or transitively)
/// must raise a DAX error, not blow the native call stack.
#[test]
fn circular_measure_reference_errors_instead_of_overflowing_stack() {
    let conn = open_mem();
    conn.execute_batch("CREATE TABLE fVendas (Valor DOUBLE); INSERT INTO fVendas VALUES (1.0);")
        .expect("seed circular-measure fixture");
    let model = build_model(
        &conn,
        &[("fVendas", "fVendas", &[Some("number")])],
        vec![],
        vec![
            DaxMeasureInput { name: "A".into(), table_name: "fVendas".into(), formula: "[B] + 1".into(), format_string: None },
            DaxMeasureInput { name: "B".into(), table_name: "fVendas".into(), formula: "[A] + 1".into(), format_string: None },
        ],
    );
    let ctx = crate::dax::build_filter_context(&[], &model);
    let err = model.evaluate_dax("[A]", &ctx).expect_err("mutually circular measures must error, not overflow");
    assert!(err.to_lowercase().contains("circular"), "error should name the cycle, got: {}", err);
}
