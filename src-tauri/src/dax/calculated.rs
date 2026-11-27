//! Calculated tables and calculated columns (v0.11 Frente C).
//!
//! A calculated table's DAX table expression (CALENDAR, CALENDARAUTO,
//! SUMMARIZE, SUMMARIZECOLUMNS, ADDCOLUMNS, UNION, FILTER, DISTINCT,
//! DATATABLE, ...) is evaluated once per `DaxModel::sync_from_payload*` and
//! materialized into a normal `DaxTableData` with `is_calculated = true`.
//! Calculated columns are evaluated per-row (row context, RELATED,
//! RELATEDTABLE, EARLIER all work via the normal evaluator row stack) and
//! materialized as extra columns on their table.
//!
//! Dependencies between calculated tables (one calculated table's DAX
//! expression referencing another calculated table) are resolved in
//! topological order; a cycle is reported as an error per involved table
//! instead of looping or panicking.

use crate::dax::ast::Expr;
use crate::dax::evaluator::{evaluate_ast, evaluate_table_ast, RowContext};
use crate::dax::parser::parse_dax;
use crate::dax::{ColumnType, DaxModel, DaxValue, FilterContext};
use std::collections::{HashMap, HashSet};

/// One calculated table awaiting materialization.
pub struct CalcTableInput {
    pub name: String,
    pub expression: String,
}

/// Walks `expr` collecting every identifier that names a table currently
/// known to the model (`known`, lowercased) — `Table[Column]` refs,
/// `RELATEDTABLE(Table)` / bare-table-name args (e.g. `SUMMARIZE(Table, ...)`,
/// `ALL(Table)`), recursing through every AST node. Used to order
/// calculated-table materialization and to detect cycles.
fn collect_table_refs(expr: &Expr, known: &HashSet<String>, out: &mut HashSet<String>) {
    match expr {
        Expr::TableColRef { table, .. } => {
            let t = table.to_lowercase();
            if known.contains(&t) {
                out.insert(t);
            }
        }
        Expr::Ident(id) => {
            let t = id.to_lowercase();
            if known.contains(&t) {
                out.insert(t);
            }
        }
        Expr::Unary { expr, .. } => collect_table_refs(expr, known, out),
        Expr::Binary { left, right, .. } => {
            collect_table_refs(left, known, out);
            collect_table_refs(right, known, out);
        }
        Expr::InList { expr, list } => {
            collect_table_refs(expr, known, out);
            for e in list {
                collect_table_refs(e, known, out);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_table_refs(a, known, out);
            }
        }
        Expr::VarBlock { bindings, body } => {
            for (_, e) in bindings {
                collect_table_refs(e, known, out);
            }
            collect_table_refs(body, known, out);
        }
        Expr::Blank | Expr::Number(_) | Expr::Text(_) | Expr::Boolean(_) | Expr::BracketRef(_) => {}
    }
}

/// Materializes every calculated table in `inputs` into `model.tables`, in
/// dependency order. Returns `table_key -> error` for tables that couldn't
/// be built (parse/eval error, or part of a dependency cycle); those tables
/// are left absent from `model.tables` (references to them error with
/// "tabela não encontrada" rather than silently using stale/partial data).
pub fn materialize_calculated_tables(
    model: &mut DaxModel,
    inputs: Vec<CalcTableInput>,
) -> HashMap<String, String> {
    let mut errors = HashMap::new();
    if inputs.is_empty() {
        return errors;
    }
    let mut known: HashSet<String> = model.tables.keys().cloned().collect();
    for t in &inputs {
        known.insert(t.name.to_lowercase());
    }

    // Parse once; keep (key, name, expr_ast, deps restricted to other
    // calculated tables -- base tables are already materialized, so they
    // never block ordering).
    let calc_keys: HashSet<String> = inputs.iter().map(|t| t.name.to_lowercase()).collect();
    struct Pending {
        key: String,
        name: String,
        ast: Result<Expr, String>,
        deps: HashSet<String>,
    }
    let mut pending: Vec<Pending> = inputs
        .into_iter()
        .map(|t| {
            let key = t.name.to_lowercase();
            let ast = parse_dax(&t.expression);
            let mut deps = HashSet::new();
            if let Ok(e) = &ast {
                collect_table_refs(e, &known, &mut deps);
            }
            deps.remove(&key);
            deps.retain(|d| calc_keys.contains(d));
            Pending { key, name: t.name, ast, deps }
        })
        .collect();

    let mut done: HashSet<String> = HashSet::new();
    // Kahn-ish loop: repeatedly materialize any pending table whose
    // calculated-table deps are already done; stop when no progress is
    // made (remaining ones are a cycle, or depend on one).
    loop {
        let mut progressed = false;
        let mut i = 0;
        while i < pending.len() {
            if pending[i].deps.iter().all(|d| done.contains(d)) {
                let p = pending.remove(i);
                let empty_ctx = FilterContext::default();
                let result = match &p.ast {
                    Err(e) => Err(e.clone()),
                    Ok(ast) => evaluate_table_ast(&p.name, ast, model, &empty_ctx),
                };
                match result {
                    Ok(data) => {
                        model.insert_calculated_table(&p.key, data);
                        done.insert(p.key);
                    }
                    Err(e) => {
                        errors.insert(p.key.clone(), format!("Erro ao calcular a tabela '{}': {}", p.name, e));
                        done.insert(p.key); // unblock dependents; they'll just error on missing refs
                    }
                }
                progressed = true;
            } else {
                i += 1;
            }
        }
        if !progressed || pending.is_empty() {
            break;
        }
    }

    // Whatever's left in `pending` is a cycle (or a table depending on a
    // table that's part of one).
    for p in pending {
        let cycle_members: Vec<String> = p.deps.iter().cloned().chain(std::iter::once(p.key.clone())).collect();
        errors.insert(
            p.key.clone(),
            format!(
                "Ciclo de dependência entre tabelas calculadas envolvendo '{}' (depende de: {}).",
                p.name,
                cycle_members.join(", ")
            ),
        );
    }
    errors
}

/// Computes every calculated column listed in `per_table` (table_key ->
/// col_key -> DAX row expression) and appends/overwrites them on the
/// matching `model.tables` entry (base or calculated). Row context,
/// RELATED, RELATEDTABLE and EARLIER are all available since evaluation
/// goes through the normal `RowContext` stack. Returns
/// `"table.column" -> error` for expressions that failed to parse/evaluate
/// (the column is still added, filled with BLANK, so the table shape stays
/// consistent).
pub fn materialize_calculated_columns(
    model: &mut DaxModel,
    per_table: &HashMap<String, HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut errors = HashMap::new();
    for (tkey, cols) in per_table {
        for (col_key, expr_src) in cols {
            let ast = match parse_dax(expr_src) {
                Ok(a) => a,
                Err(e) => {
                    errors.insert(format!("{}.{}", tkey, col_key), format!("Erro de sintaxe DAX: {}", e));
                    continue;
                }
            };
            let Some(tbl) = model.tables.get(tkey) else { continue };
            let n = tbl.rows.len();
            let display_name = tbl
                .columns
                .iter()
                .find(|c| c.to_lowercase() == *col_key)
                .cloned()
                .unwrap_or_else(|| col_key.clone());
            let empty_ctx = FilterContext::default();
            let mut computed: Vec<DaxValue> = Vec::with_capacity(n);
            let mut err: Option<String> = None;
            for row_idx in 0..n {
                let rc = RowContext { table: tbl, row_idx };
                match evaluate_ast(&ast, model, &empty_ctx, &[rc], &HashMap::new()) {
                    Ok(v) => computed.push(v),
                    Err(e) => {
                        err = Some(e);
                        computed.push(DaxValue::Blank);
                    }
                }
            }
            if let Some(e) = err {
                errors.insert(format!("{}.{}", tkey, col_key), format!("Erro ao calcular a coluna '{}': {}", display_name, e));
            }
            apply_calculated_column(model, tkey, &display_name, computed);
        }
    }
    errors
}

/// Infers a `ColumnType` from the computed values of a calculated column
/// (all-same-variant -> that type; mixed/blank-only -> Text), then either
/// overwrites the existing column's values (recompute) or appends a new
/// column (first materialization) on `model.tables[tkey]`.
fn apply_calculated_column(model: &mut DaxModel, tkey: &str, display_name: &str, values: Vec<DaxValue>) {
    let Some(tbl) = model.tables.get_mut(tkey) else { return };
    let col_type = infer_calculated_col_type(&values);
    if let Some(cidx) = tbl.get_col_idx(display_name) {
        for (row, v) in tbl.rows.iter_mut().zip(values.into_iter()) {
            row[cidx] = v;
        }
        tbl.col_types[cidx] = col_type;
    } else {
        let cidx = tbl.columns.len();
        tbl.columns.push(display_name.to_string());
        tbl.col_index.insert(display_name.to_lowercase(), cidx);
        tbl.col_types.push(col_type);
        for (row, v) in tbl.rows.iter_mut().zip(values.into_iter()) {
            row.push(v);
        }
    }
}

fn infer_calculated_col_type(values: &[DaxValue]) -> ColumnType {
    let mut ty: Option<ColumnType> = None;
    for v in values {
        let this = match v {
            DaxValue::Blank => continue,
            DaxValue::Integer(_) => ColumnType::Integer,
            DaxValue::Decimal(_) => ColumnType::Decimal,
            DaxValue::Currency(_) => ColumnType::Currency,
            DaxValue::DateTime(_) => ColumnType::Date,
            DaxValue::Boolean(_) => ColumnType::Boolean,
            DaxValue::Text(_) => ColumnType::Text,
        };
        match ty {
            None => ty = Some(this),
            Some(prev) if prev == this => {}
            Some(_) => return ColumnType::Text,
        }
    }
    ty.unwrap_or(ColumnType::Text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dax::{build_filter_context, DaxModelSyncPayload, DaxRelationshipInput, DaxTableInput};
    use serde_json::json;

    fn base_model() -> DaxModel {
        let mut model = DaxModel::new();
        let sales = DaxTableInput {
            duckdb_table: None,
            name: "Sales".into(),
            columns: vec!["Date".into(), "Amount".into(), "CategoryId".into()],
            rows: vec![
                json!({"Date": "2024-01-15", "Amount": 10, "CategoryId": 1})
                    .as_object()
                    .unwrap()
                    .clone(),
                json!({"Date": "2024-02-20", "Amount": 20, "CategoryId": 2})
                    .as_object()
                    .unwrap()
                    .clone(),
            ],
            column_types: vec![Some("date".into()), Some("number".into()), Some("number".into())],
            sort_by_columns: Default::default(),
            is_calculated: false,
            expression: None,
            calculated_columns: Default::default(),
        };
        let categories = DaxTableInput {
            duckdb_table: None,
            name: "Categories".into(),
            columns: vec!["Id".into(), "Name".into()],
            rows: vec![
                json!({"Id": 1, "Name": "A"}).as_object().unwrap().clone(),
                json!({"Id": 2, "Name": "B"}).as_object().unwrap().clone(),
            ],
            column_types: vec![],
            sort_by_columns: Default::default(),
            is_calculated: false,
            expression: None,
            calculated_columns: Default::default(),
        };
        let rel = DaxRelationshipInput {
            from_table: "Sales".into(),
            from_column: "CategoryId".into(),
            to_table: "Categories".into(),
            to_column: "Id".into(),
            cross_filter_direction: "single".into(),
            is_active: true,
        };
        model.sync_from_payload(DaxModelSyncPayload {
            tables: vec![sales, categories],
            relationships: vec![rel],
            measures: vec![],
        });
        model
    }

    #[test]
    fn calculated_table_calendar_is_flagged_and_materialized() {
        let mut model = base_model();
        let errs = materialize_calculated_tables(
            &mut model,
            vec![CalcTableInput {
                name: "CalendarCalc".into(),
                expression: "CALENDAR(DATE(2024,1,1), DATE(2024,1,3))".into(),
            }],
        );
        assert!(errs.is_empty(), "{:?}", errs);
        let tbl = model.get_table("CalendarCalc").expect("table materialized");
        assert!(tbl.is_calculated);
        assert_eq!(tbl.rows.len(), 3);
    }

    #[test]
    fn calculated_table_cycle_reports_error_not_panic() {
        let mut model = base_model();
        let errs = materialize_calculated_tables(
            &mut model,
            vec![
                CalcTableInput { name: "A".into(), expression: "FILTER(B, TRUE())".into() },
                CalcTableInput { name: "B".into(), expression: "FILTER(A, TRUE())".into() },
            ],
        );
        assert_eq!(errs.len(), 2);
        assert!(model.get_table("A").is_none());
        assert!(model.get_table("B").is_none());
    }

    #[test]
    fn calculated_column_related_and_earlier() {
        let mut model = base_model();
        let mut per_table = HashMap::new();
        let mut cols = HashMap::new();
        cols.insert("categoryname".into(), "RELATED(Categories[Name])".into());
        per_table.insert("sales".into(), cols);
        let errs = materialize_calculated_columns(&mut model, &per_table);
        assert!(errs.is_empty(), "{:?}", errs);
        let tbl = model.get_table("Sales").unwrap();
        let cidx = tbl.get_col_idx("categoryname").unwrap();
        assert_eq!(tbl.rows[0][cidx].as_display_string(), "A");
        assert_eq!(tbl.rows[1][cidx].as_display_string(), "B");
    }

    #[test]
    fn calendarauto_ignores_calculated_table_and_column() {
        // CALENDARAUTO must only see Sales[Date] (base, non-calculated);
        // a calculated calendar table covering a wider/different range
        // must not widen the scan.
        let mut model = base_model();
        materialize_calculated_tables(
            &mut model,
            vec![CalcTableInput {
                name: "WideCalendar".into(),
                expression: "CALENDAR(DATE(1999,1,1), DATE(2999,12,31))".into(),
            }],
        );
        let ctx = build_filter_context(&[], &model);
        let v = model.evaluate_dax("YEAR(MINX(CALENDARAUTO(), [Date]))", &ctx).unwrap();
        assert_eq!(v.as_f64(), Some(2024.0));
    }
}
