//! Repro of Luis's report: open Rhino, remove steps from a query, apply,
//! then "Atualizar Dados" (refresh_table_native per file table + refresh_pbip_tables).
//! Each backend call must return (no deadlock/hang).

use crate::engine::DuckDbStore;
use crate::etl::commands::{etl_apply_impl, etl_from_m_impl, etl_load_raw_impl, etl_preview_impl};
use crate::pbi::m_loader::{refresh_single_table, RefreshTableRequest};
use crate::pbi::pbip::open_pbip_project;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn rhino_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join("Documents/Power BI/Rhino");
    p.join("Rhino Bip.pbip").exists().then_some(p)
}

fn within<T: Send + 'static>(label: &str, secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .unwrap_or_else(|_| panic!("{label}: não retornou em {secs}s (travou)"))
}

#[test]
fn rhino_remove_steps_apply_then_refresh_all_returns() {
    let Some(dir) = rhino_dir() else { return };
    let db = Arc::new(DuckDbStore::new_in_memory().unwrap());
    let pbip = dir.join("Rhino Bip.pbip");

    let d = db.clone();
    let p = pbip.clone();
    let summary = within("abrir", 60, move || open_pbip_project(&p, Some(&d)).unwrap());

    let m = std::fs::read_to_string(dir.join("Rhino Bip.SemanticModel/definition/tables/dCalendario.tmdl")).unwrap();
    let start = m.find("let").unwrap();
    let end = m.find("annotation PBI_NavigationStepName").unwrap_or(m.len());
    let script: String = m[start..end].lines().map(|l| l.trim_start_matches('\t')).collect::<Vec<_>>().join("\n");
    let mut q = etl_from_m_impl("dCalendario", &script).unwrap().query;
    q.steps.truncate(3);

    let d = db.clone();
    let q2 = q.clone();
    within("preview", 30, move || etl_preview_impl(&d, &q2, None, Some(200)).unwrap());
    let d = db.clone();
    let q2 = q.clone();
    let applied = within("aplicar", 30, move || etl_apply_impl(&d, &q2, "dCalendario").unwrap());
    assert!(applied.columns.len() <= 3, "{:?}", applied.columns.iter().map(|c| &c.name).collect::<Vec<_>>());

    // refreshAllData: tabelas com arquivo via refresh_table_native…
    for t in summary.loaded_tables.iter().filter(|t| t.source_path.is_some()) {
        // Igual ao frontend: o M original da partição vai junto (sourceConfig.mScript).
        let tm = std::fs::read_to_string(dir.join(format!("Rhino Bip.SemanticModel/definition/tables/{}.tmdl", t.table_name))).unwrap_or_default();
        let m_script = tm.find("let").map(|a| {
            let b = tm.find("annotation PBI_NavigationStepName").unwrap_or(tm.len());
            tm[a..b].lines().map(|l| l.trim_start_matches('\t')).collect::<Vec<_>>().join("\n")
        });
        let req: RefreshTableRequest = serde_json::from_value(serde_json::json!({
            "tableName": t.table_name,
            "sourceType": "xlsx",
            "filePath": t.source_path,
            "sheetName": t.sheet_name,
            "mScript": m_script,
            "pbipRoot": dir.to_string_lossy(),
        }))
        .unwrap();
        let d = db.clone();
        let label = format!("refresh {}", t.table_name);
        within(&label.clone(), 60, move || refresh_single_table(&req, &d).map(|_| ()).unwrap_or_else(|e| eprintln!("{label}: {e}")));
    }
    // …e depois o .pbip inteiro.
    let d = db.clone();
    let p = pbip.clone();
    within("refresh_pbip_tables", 60, move || open_pbip_project(&p, Some(&d)).unwrap());

    // refreshAllData termina com `await pushModelToRustAsync` (sync_model_to_rust)
    // e os visuais avaliam as medidas: com a dCalendario cortada, nada pode travar.
    {
        use crate::dax::{DaxMeasureInput, DaxModel, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
        let mut tables: Vec<DaxTableInput> = summary
            .loaded_tables
            .iter()
            .filter(|t| t.table_name != "dCalendario")
            .map(|t| DaxTableInput { duckdb_table: None,
                name: t.table_name.clone(),
                columns: t.columns.iter().map(|c| c.name.clone()).collect(),
                rows: t.rows.clone(),
                column_types: t.columns.iter().map(|c| Some(c.data_type.clone())).collect(),
                sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
            })
            .collect();
        tables.push(DaxTableInput { duckdb_table: None,
            name: "dCalendario".into(),
            columns: applied.columns.iter().map(|c| c.name.clone()).collect(),
            rows: applied.rows.clone(),
            column_types: applied.columns.iter().map(|c| Some(c.data_type.clone())).collect(),
            sort_by_columns: Default::default(), is_calculated: false, expression: None, calculated_columns: ::std::collections::HashMap::new() 
        });
        let payload = DaxModelSyncPayload {
            tables,
            relationships: summary
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
                .collect(),
            measures: summary
                .tables
                .iter()
                .flat_map(|t| t.measures.iter())
                .map(|m| DaxMeasureInput { name: m.name.clone(), table_name: m.table_name.clone(), formula: m.expression.clone() })
                .collect(),
        };
        let names: Vec<String> = payload.measures.iter().map(|m| m.name.clone()).collect();
        eprintln!("medidas: {}", names.len());
        within("sync_model + medidas", 60, move || {
            let mut model = DaxModel::new();
            model.sync_from_payload(payload);
            for n in names {
                let t = std::time::Instant::now();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    model.evaluate_dax(&format!("[{n}]"), &Default::default())
                }));
                match r {
                    Err(_) => panic!("PANIC avaliando [{n}]"),
                    Ok(v) => eprintln!("[{n}] {:?} ({:?})", v.map(|x| x.as_display_string()), t.elapsed()),
                }
            }
        });
    }

    // E o raw da consulta editada continua utilizável.
    let d = db.clone();
    within("raw de novo", 30, move || etl_load_raw_impl(&d, &q.name, &q.source, None).unwrap());
}
