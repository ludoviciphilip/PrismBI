//! End-to-end on the user's real report (skipped when the file is absent):
//! every M table must load through the new ETL path, not the legacy one.

use crate::engine::DuckDbStore;
use crate::pbi::pbip::open_pbip_project;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

fn rhino_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join("Documents/Power BI/Rhino/Rhino Bip.pbip");
    p.exists().then_some(p)
}

#[test]
fn rhino_pbip_tables_load_through_etl_path() {
    let Some(path) = rhino_path() else { return };
    let db = DuckDbStore::new_in_memory().unwrap();
    let summary = open_pbip_project(&path, Some(&db)).expect("abrir Rhino");
    let legacy: Vec<_> = summary.warnings.iter().filter(|w| w.contains("legado")).collect();
    assert!(legacy.is_empty(), "tabelas caíram no caminho legado: {:?}", legacy);

    // Every M table has a raw__ table (= went through etl_load_raw).
    for t in ["dCategorias", "fAberturas", "fLançamentos", "dCalendario", "Medidas"] {
        let n = db
            .query_sql(&format!(
                "SELECT count(*) AS n FROM information_schema.tables WHERE table_name = 'raw__{}'",
                t
            ))
            .unwrap()
            .rows[0]["n"]
            .as_i64()
            .unwrap();
        assert_eq!(n, 1, "raw__{} ausente", t);
    }
    // Measures host table has no columns → not loaded (as before).
    assert!(!summary.loaded_tables.iter().any(|t| t.table_name == "Medidas"));

    let cat = summary
        .loaded_tables
        .iter()
        .find(|t| t.table_name == "dCategorias")
        .expect("dCategorias carregada");
    let cols: Vec<&str> = cat.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(cols.contains(&"DRE") && cols.contains(&"DFC"), "colunas: {:?}", cols);
    assert!(!cols.contains(&"Entra na DRE (competência)"));
    assert!(!cols.iter().any(|c| c.starts_with("__")), "colunas ocultas vazaram: {:?}", cols);
    assert!(cat.total_rows > 0);
    for row in &cat.rows {
        let blank = row
            .values()
            .all(|v| v.is_null() || matches!(v, JsonValue::String(s) if s.is_empty()));
        assert!(!blank, "linha em branco não removida: {:?}", row);
    }
    let ordem = cat.columns.iter().find(|c| c.name == "Ordem DRE").unwrap();
    assert_eq!(ordem.duckdb_type, "BIGINT");

    let cal = summary.loaded_tables.iter().find(|t| t.table_name == "dCalendario").unwrap();
    assert_eq!(cal.total_rows, 275);
    let fl = summary.loaded_tables.iter().find(|t| t.table_name == "fLançamentos").unwrap();
    let valor = fl.columns.iter().find(|c| c.name == "Valor").unwrap();
    assert_eq!(valor.duckdb_type, "DECIMAL(19,4)");
    let data = fl.columns.iter().find(|c| c.name == "Data").unwrap();
    assert_eq!(data.duckdb_type, "DATE");
}

/// Parity with the legacy interpreter on the real data: same totals.
#[test]
fn rhino_etl_matches_legacy_totals() {
    let Some(path) = rhino_path() else { return };
    let new_db = DuckDbStore::new_in_memory().unwrap();
    let summary = open_pbip_project(&path, Some(&new_db)).expect("abrir Rhino");
    let root = path.parent().unwrap();
    let old_db = DuckDbStore::new_in_memory().unwrap();
    let mut warnings = Vec::new();
    let mut legacy = Vec::new();
    for t in &summary.tables {
        if let Some(p) = t.partitions.first() {
            if !p.expression.to_uppercase().contains("DATATABLE") && !t.name.contains("DateTable") {
                crate::pbi::m_loader::materialize_m_partition_legacy(t, p.expression.trim(), root, &old_db, &mut warnings, &mut legacy);
            }
        }
    }
    let sum = |db: &DuckDbStore, sql: &str| -> f64 {
        db.query_sql(sql).unwrap().rows[0]["v"].as_f64().unwrap_or(0.0)
    };
    for (table, col) in [("fLançamentos", "Valor"), ("fAberturas", "Valor")] {
        let q = format!("SELECT CAST(sum(\"{}\") AS DOUBLE) AS v FROM \"{}\"", col, table);
        let a = sum(&new_db, &q);
        let b = sum(&old_db, &q);
        println!("{}: novo={} legado={}", table, a, b);
        assert!((a - b).abs() < 0.01, "{}.{}: novo {} vs legado {}", table, col, a, b);
    }
    for table in ["fLançamentos", "fAberturas", "dCategorias", "dCalendario"] {
        let q = format!("SELECT CAST(count(*) AS DOUBLE) AS v FROM \"{}\"", table);
        let (a, b) = (sum(&new_db, &q), sum(&old_db, &q));
        assert_eq!(a, b, "{}: linhas novo {} vs legado {}", table, a, b);
    }
}
