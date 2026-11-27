//! Executes a parsed `QueryProgram` against the shared `DaxModel`.
//!
//! `DEFINE MEASURE/VAR/TABLE` are implemented as an **AST substitution**
//! pass (`substitute`) rather than by mutating/cloning `DaxModel` — a
//! `DEFINE MEASURE Sales[Total] = ...` that overrides a model measure only
//! for this query, a `DEFINE VAR`, or a reference to a `DEFINE TABLE`, all
//! show up in the parsed `Expr` as a plain `Expr::BracketRef`/`Expr::Ident`
//! that gets swapped for the define's own (recursively substituted) `Expr`
//! before the shared evaluator ever sees it. Measures *not* overridden are
//! left untouched and keep resolving through `model.get_measure` exactly as
//! today. This avoids deep-cloning the model's tables (which `DaxModel`'s
//! private fields make impossible to do cheaply from outside `dax::mod`
//! anyway — see the Rust test `define_measure_overrides_model_measure`) at
//! the cost of not sharing one cached evaluation across repeated
//! references to the same DEFINE within a query; fine for a query-authoring
//! tool, not a visual hot path (see the performance note in the final
//! report).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::dax::ast::Expr;
use crate::dax::evaluator::{evaluate_ast, evaluate_table_ast, RowContext};
use crate::dax::lexer::{tokenize, Token};
use crate::dax::{ColumnType, DaxModel, DaxTableData, DaxValue, FilterContext};

use super::qast::{Define, OrderDir, Pos, QError, QueryProgram, TableExprSrc};
use super::qparser::parse_query;
use super::qvalidate::locate_runtime_error;

/// Re-tokenizes the whole query text (cheap: queries are short) to turn a
/// shared-evaluator runtime error (plain `String`, e.g. "Referência
/// '[AAAAAA]' não encontrada.") into a `QError` with the real line/column
/// of the offending `[Name]`/`Table[Col]`/identifier — the same
/// token-scan `dax_validate` already uses for a single formula (see
/// `qvalidate::locate_runtime_error`), reused here so a multi-statement
/// `DEFINE`/`EVALUATE` query gets the same precision instead of always
/// reporting line 1 column 1 (task item 4: "a mensagem saiu sem linha e
/// coluna").
pub(crate) struct QueryPos {
    tokens: Vec<Token>,
    chars: Vec<char>,
}

impl QueryPos {
    fn new(query: &str) -> Self {
        let chars: Vec<char> = query.chars().collect();
        let tokens = tokenize(query).unwrap_or_default();
        QueryPos { tokens, chars }
    }

    fn locate(&self, message: impl Into<String>) -> QError {
        let message = message.into();
        let (pos, _len) = locate_runtime_error(&self.tokens, &self.chars, &message);
        QError::new(message, pos)
    }
}

#[derive(Debug, Default)]
pub struct Defines {
    /// measure name (lowercase) -> its DEFINE'd expression.
    pub measures: HashMap<String, Expr>,
    /// var name (lowercase) -> its DEFINE'd expression.
    pub vars: HashMap<String, Expr>,
    /// table name (lowercase) -> its DEFINE'd table expression.
    pub tables: HashMap<String, TableExprSrc>,
    /// table name (lowercase) -> [(column name, row expression), ...].
    pub columns: HashMap<String, Vec<(String, Expr)>>,
}

impl Defines {
    pub fn from_program(defines: &[Define]) -> Self {
        let mut d = Defines::default();
        for def in defines {
            match def {
                Define::Measure { name, expr, .. } => {
                    d.measures.insert(name.to_lowercase(), expr.clone());
                }
                Define::Var { name, expr } => {
                    d.vars.insert(name.to_lowercase(), expr.clone());
                }
                Define::Table { name, expr } => {
                    d.tables.insert(name.to_lowercase(), expr.clone());
                }
                Define::Column { table, name, expr } => {
                    d.columns.entry(table.to_lowercase()).or_default().push((name.clone(), expr.clone()));
                }
            }
        }
        d
    }
}

const MAX_SUBSTITUTION_DEPTH: u32 = 64;

/// Recursively replaces `Expr::Ident`/`Expr::BracketRef` nodes that name a
/// `DEFINE VAR`/`TABLE`/`MEASURE` with that define's own (also substituted)
/// expression. `depth` guards against a cycle (measure A -> B -> A).
pub fn substitute(expr: &Expr, defines: &Defines, depth: u32) -> Result<Expr, QError> {
    if depth > MAX_SUBSTITUTION_DEPTH {
        return Err(QError::at_start(
            "Referência circular entre MEASURE/VAR do DEFINE desta consulta.",
        ));
    }
    Ok(match expr {
        Expr::Ident(name) => {
            let key = name.to_lowercase();
            if let Some(v) = defines.vars.get(&key) {
                substitute(v, defines, depth + 1)?
            } else if let Some(TableExprSrc::Generic(t)) = defines.tables.get(&key) {
                substitute(t, defines, depth + 1)?
            } else {
                expr.clone()
            }
        }
        Expr::BracketRef(name) => {
            let key = name.to_lowercase();
            if let Some(m) = defines.measures.get(&key) {
                substitute(m, defines, depth + 1)?
            } else {
                expr.clone()
            }
        }
        Expr::Blank | Expr::Number(_) | Expr::Text(_) | Expr::Boolean(_) | Expr::TableColRef { .. } => expr.clone(),
        Expr::Unary { op, expr: e } => Expr::Unary { op: op.clone(), expr: Box::new(substitute(e, defines, depth)?) },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: op.clone(),
            left: Box::new(substitute(left, defines, depth)?),
            right: Box::new(substitute(right, defines, depth)?),
        },
        Expr::InList { expr: e, list } => Expr::InList {
            expr: Box::new(substitute(e, defines, depth)?),
            list: list.iter().map(|x| substitute(x, defines, depth)).collect::<Result<_, _>>()?,
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| substitute(a, defines, depth)).collect::<Result<_, _>>()?,
        },
        Expr::VarBlock { bindings, body } => Expr::VarBlock {
            bindings: bindings
                .iter()
                .map(|(n, e)| Ok::<_, QError>((n.clone(), substitute(e, defines, depth)?)))
                .collect::<Result<_, _>>()?,
            body: Box::new(substitute(body, defines, depth)?),
        },
    })
}

fn datatable_type(t: &str) -> ColumnType {
    match t.to_uppercase().as_str() {
        "STRING" => ColumnType::Text,
        "INTEGER" | "INT64" | "WHOLE NUMBER" => ColumnType::Integer,
        "DOUBLE" | "NUMBER" => ColumnType::Decimal,
        "CURRENCY" => ColumnType::Currency,
        "DATETIME" | "DATE" => ColumnType::Date,
        "BOOLEAN" => ColumnType::Boolean,
        _ => ColumnType::Text,
    }
}

fn coerce_to(v: DaxValue, ty: ColumnType) -> DaxValue {
    match ty {
        ColumnType::Text => match v {
            DaxValue::Text(_) => v,
            other => DaxValue::Text(other.as_display_string()),
        },
        ColumnType::Integer => v.as_f64().map(|f| DaxValue::Integer(f.round() as i64)).unwrap_or(DaxValue::Blank),
        ColumnType::Decimal => v.as_f64().map(DaxValue::Decimal).unwrap_or(DaxValue::Blank),
        ColumnType::Currency => v.as_f64().map(|f| DaxValue::Currency((f * 10_000.0).round() as i64)).unwrap_or(DaxValue::Blank),
        ColumnType::Date => v.as_date_serial().map(DaxValue::DateTime).unwrap_or(DaxValue::Blank),
        ColumnType::Boolean => DaxValue::Boolean(v.as_bool()),
    }
}

fn build_datatable(columns: &[(String, String)], rows: &[Vec<Expr>], defines: &Defines, model: &DaxModel, qp: &QueryPos) -> Result<DaxTableData, QError> {
    let col_names: Vec<String> = columns.iter().map(|(n, _)| n.clone()).collect();
    let col_types: Vec<ColumnType> = columns.iter().map(|(_, t)| datatable_type(t)).collect();
    let mut col_index = HashMap::new();
    for (idx, n) in col_names.iter().enumerate() {
        col_index.insert(n.to_lowercase(), idx);
    }
    let ctx = FilterContext::default();
    let mut out_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let mut vals = Vec::with_capacity(row.len());
        for (cell_idx, cell_expr) in row.iter().enumerate() {
            let substituted = substitute(cell_expr, defines, 0)?;
            let v = evaluate_ast(&substituted, model, &ctx, &[], &HashMap::new()).map_err(|e| qp.locate(e))?;
            vals.push(coerce_to(v, col_types[cell_idx]));
        }
        out_rows.push(vals);
    }
    Ok(DaxTableData {
        name: "DATATABLE".to_string(),
        columns: col_names,
        col_index,
        col_types,
        rows: out_rows,
        sort_by_columns: HashMap::new(),
        is_calculated: true,
    })
}

fn infer_col_type(values: &[DaxValue]) -> ColumnType {
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

/// Applies every `DEFINE COLUMN <table_key>[...]` pending for `table_key`
/// onto the already-materialized `tbl` (row context, RELATED etc. all work
/// since this reuses the normal `evaluate_ast`/`RowContext` path).
fn apply_define_columns(tbl: &mut DaxTableData, table_key: &str, defines: &Defines, model: &DaxModel, qp: &QueryPos) -> Result<(), QError> {
    let Some(cols) = defines.columns.get(table_key) else { return Ok(()) };
    let ctx = FilterContext::default();
    for (name, expr) in cols {
        let substituted = substitute(expr, defines, 0)?;
        let n = tbl.rows.len();
        let mut vals = Vec::with_capacity(n);
        for row_idx in 0..n {
            let rc = RowContext { table: &*tbl, row_idx };
            let v = evaluate_ast(&substituted, model, &ctx, &[rc], &HashMap::new()).map_err(|e| qp.locate(e))?;
            vals.push(v);
        }
        let col_type = infer_col_type(&vals);
        let cidx = tbl.columns.len();
        tbl.columns.push(name.clone());
        tbl.col_index.insert(name.to_lowercase(), cidx);
        tbl.col_types.push(col_type);
        for (row, v) in tbl.rows.iter_mut().zip(vals.into_iter()) {
            row.push(v);
        }
    }
    Ok(())
}

fn build_row(args: &[Expr], model: &DaxModel, qp: &QueryPos) -> Result<DaxTableData, QError> {
    if args.is_empty() || args.len() % 2 != 0 {
        return Err(QError::at_start("ROW requer pares (\"nome da coluna\", expressão)."));
    }
    let ctx = FilterContext::default();
    let mut columns = Vec::with_capacity(args.len() / 2);
    let mut col_index = HashMap::new();
    let mut col_types = Vec::with_capacity(args.len() / 2);
    let mut row = Vec::with_capacity(args.len() / 2);
    for pair in args.chunks(2) {
        let name = match &pair[0] {
            Expr::Text(s) => s.clone(),
            other => return Err(QError::at_start(format!("ROW: nome de coluna deve ser texto entre aspas, encontrado {:?}.", other))),
        };
        let v = evaluate_ast(&pair[1], model, &ctx, &[], &HashMap::new()).map_err(|e| qp.locate(e))?;
        col_index.insert(name.to_lowercase(), columns.len());
        col_types.push(infer_col_type(std::slice::from_ref(&v)));
        columns.push(name);
        row.push(v);
    }
    Ok(DaxTableData { name: "ROW".to_string(), columns, col_index, col_types, rows: vec![row], sort_by_columns: HashMap::new(), is_calculated: true })
}

/// Materializes the table expression an `EVALUATE`/`DEFINE TABLE` points
/// at, substituting every DEFINE first (see module doc). Also returns a
/// lowercased-alias -> (substituted) expr map for any trailing `"name",
/// expr` extension pairs of a top-level (or `FILTER`/`TOPN`-wrapped)
/// `SUMMARIZECOLUMNS`/`ADDCOLUMNS`/`SELECTCOLUMNS` call, so the caller can
/// try to trace an output column like `"T"` back to the real measure(s) it
/// references (see `infer_measure_format`, used for `DaxQueryColumn::format_string`).
pub(crate) fn resolve_table(
    src: &TableExprSrc,
    defines: &Defines,
    model: &DaxModel,
    qp: &QueryPos,
) -> Result<(DaxTableData, HashMap<String, Expr>), QError> {
    match src {
        TableExprSrc::Generic(expr) => {
            if let Expr::Ident(name) = expr {
                let key = name.to_lowercase();
                if let Some(TableExprSrc::DataTable { columns, rows }) = defines.tables.get(&key) {
                    let mut tbl = build_datatable(columns, rows, defines, model, qp)?;
                    apply_define_columns(&mut tbl, &key, defines, model, qp)?;
                    return Ok((tbl, HashMap::new()));
                }
            }
            let substituted = substitute(expr, defines, 0)?;
            if let Expr::Call { name, args } = &substituted {
                if name.eq_ignore_ascii_case("ROW") {
                    let mut tbl = build_row(args, model, qp)?;
                    if let Expr::Ident(name) = expr {
                        apply_define_columns(&mut tbl, &name.to_lowercase(), defines, model, qp)?;
                    }
                    return Ok((tbl, HashMap::new()));
                }
            }
            let extensions = find_extension_exprs(&substituted);
            let ctx = FilterContext::default();
            let mut tbl = evaluate_table_ast("Resultado", &substituted, model, &ctx).map_err(|e| qp.locate(e))?;
            if let Expr::Ident(name) = expr {
                apply_define_columns(&mut tbl, &name.to_lowercase(), defines, model, qp)?;
            }
            Ok((tbl, extensions))
        }
        TableExprSrc::DataTable { columns, rows } => Ok((build_datatable(columns, rows, defines, model, qp)?, HashMap::new())),
    }
}

/// Extracts the trailing `"name", expr` pairs of a
/// `SUMMARIZECOLUMNS`/`ADDCOLUMNS`/`SELECTCOLUMNS` call's `args` (same scan
/// `eval_summarizecolumns`/`eval_add_select_columns` use: the longest
/// all-`Expr::Text`-at-even-offsets suffix).
fn extension_pairs(args: &[Expr]) -> Vec<(String, &Expr)> {
    let ext_start = (0..=args.len())
        .find(|&i| (args.len() - i) % 2 == 0 && (i..args.len()).step_by(2).all(|k| matches!(args[k], Expr::Text(_))))
        .unwrap_or(args.len());
    args[ext_start..]
        .chunks(2)
        .filter_map(|c| match c {
            [Expr::Text(n), e] => Some((n.clone(), e)),
            _ => None,
        })
        .collect()
}

/// Finds the alias -> expr map of extension (`"name", expr`) columns for a
/// top-level `SUMMARIZECOLUMNS`/`ADDCOLUMNS`/`SELECTCOLUMNS` call, looking
/// through a `FILTER`/`TOPN` wrapper (the common `ORDER BY`-less "quick
/// query" shape). Anything else (nested CALCULATETABLE, UNION...) yields no
/// extension map, so those columns simply get no measure-based format.
fn find_extension_exprs(expr: &Expr) -> HashMap<String, Expr> {
    if let Expr::Call { name, args } = expr {
        let upper = name.to_uppercase();
        match upper.as_str() {
            "SUMMARIZECOLUMNS" | "ADDCOLUMNS" | "SELECTCOLUMNS" => {
                return extension_pairs(args).into_iter().map(|(n, e)| (n.to_lowercase(), e.clone())).collect();
            }
            "FILTER" => {
                if let Some(first) = args.first() {
                    return find_extension_exprs(first);
                }
            }
            "TOPN" => {
                if let Some(second) = args.get(1) {
                    return find_extension_exprs(second);
                }
            }
            _ => {}
        }
    }
    HashMap::new()
}

/// Collects the (lowercased) names of every `[Measure]` reference in `expr`
/// that resolves to a real model measure (`model.get_measure`).
fn collect_measure_names(expr: &Expr, model: &DaxModel, out: &mut HashSet<String>) {
    match expr {
        Expr::BracketRef(name) => {
            if model.get_measure(name).is_some() {
                out.insert(name.to_lowercase());
            }
        }
        Expr::Unary { expr, .. } => collect_measure_names(expr, model, out),
        Expr::Binary { left, right, .. } => {
            collect_measure_names(left, model, out);
            collect_measure_names(right, model, out);
        }
        Expr::InList { expr, list } => {
            collect_measure_names(expr, model, out);
            for l in list {
                collect_measure_names(l, model, out);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_measure_names(a, model, out);
            }
        }
        Expr::VarBlock { bindings, body } => {
            for (_, e) in bindings {
                collect_measure_names(e, model, out);
            }
            collect_measure_names(body, model, out);
        }
        Expr::Blank | Expr::Number(_) | Expr::Text(_) | Expr::Boolean(_) | Expr::TableColRef { .. } | Expr::Ident(_) => {}
    }
}

/// v0.11 Rodada 2 Frente F (bugfix): a `DEFINE MEASURE`'s new measure (e.g.
/// `Teste = [Faturamento] * 2`) is, after `substitute`, flattened down to
/// real model-measure references. When the extension expr references
/// EXACTLY ONE distinct real measure, this new measure "inherits" that
/// measure's own formatString (same rule Power BI quick measures follow:
/// copy the base measure's format). Two or more distinct measures (or none)
/// is ambiguous, so no format is inferred and the caller's fractional-value
/// fallback (`#,##0.00`) takes over instead.
fn infer_measure_format(expr: &Expr, model: &DaxModel) -> Option<String> {
    let mut names = HashSet::new();
    collect_measure_names(expr, model, &mut names);
    if names.len() == 1 {
        let name = names.into_iter().next().unwrap();
        model.measure_format_string(&name).map(str::to_string)
    } else {
        None
    }
}

/// True when `v` is a non-integer number (so a value-derived fallback
/// format can add 2 decimals instead of silently truncating cents — see
/// the "T aparece como '1.986.738', perde os centavos" bug report).
fn is_fractional(v: &DaxValue) -> bool {
    v.as_f64().map(|f| (f - f.round()).abs() > 1e-9).unwrap_or(false)
}


fn compare_dax(l: &DaxValue, r: &DaxValue) -> Ordering {
    if let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) {
        if !matches!(l, DaxValue::Text(_)) && !matches!(r, DaxValue::Text(_)) {
            return a.partial_cmp(&b).unwrap_or(Ordering::Equal);
        }
    }
    l.as_display_string().cmp(&r.as_display_string())
}

fn type_id(t: ColumnType) -> &'static str {
    match t {
        ColumnType::Text => "text",
        ColumnType::Integer => "integer",
        ColumnType::Decimal => "decimal",
        ColumnType::Currency => "currency",
        ColumnType::Date => "date",
        ColumnType::Boolean => "boolean",
    }
}

/// `"dCalendario[Ano]"` -> `Some(("dCalendario", "Ano"))`. Output columns
/// for group-by (SUMMARIZECOLUMNS/ADDCOLUMNS over a model table) keep this
/// `Table[Column]` shape; extension (`"name", expr`) columns don't, so they
/// correctly fall through to `None` here.
fn split_qualified(name: &str) -> Option<(&str, &str)> {
    let inner = name.strip_suffix(']')?;
    let open = inner.find('[')?;
    Some((&inner[..open], &inner[open + 1..]))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxQueryColumn {
    pub name: String,
    pub dax_type: &'static str,
    /// v0.11 Rodada 2 Frente F: the model's TMDL `formatString` for this
    /// column, when the output column traces back to a plain model
    /// `Table[Column]` (SUMMARIZECOLUMNS/ADDCOLUMNS/VALUES group-by
    /// column), so the UI can format an integer year ("0", no grouping)
    /// differently from a currency measure, instead of guessing from the
    /// JS `typeof` alone. `None` for computed/extension columns (measures,
    /// `"name", expr` pairs) — those have no single model column to copy
    /// a format from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_string: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxQueryResultSet {
    pub columns: Vec<DaxQueryColumn>,
    pub rows: Vec<Vec<JsonValue>>,
    pub display_rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxQueryErrorOut {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl From<QError> for DaxQueryErrorOut {
    fn from(e: QError) -> Self {
        DaxQueryErrorOut { message: e.message, line: e.pos.line.max(1), column: e.pos.col.max(1) }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxQueryExecuteResult {
    pub result_sets: Vec<DaxQueryResultSet>,
    pub elapsed_ms: f64,
    pub error: Option<DaxQueryErrorOut>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DaxQueryOptions {
    /// Max rows returned per result set (UI default: 1000). `None` = 1000.
    pub row_limit: Option<usize>,
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn fill_pos_if_unknown(mut e: QError, fallback: Pos) -> QError {
    if e.pos.line == 0 {
        e.pos = fallback;
    }
    e
}

fn run_evaluate(
    model: &DaxModel,
    defines: &Defines,
    stmt: &super::qast::EvaluateStmt,
    limit: usize,
    qp: &QueryPos,
) -> Result<DaxQueryResultSet, QError> {
    let (mut tbl, extensions) = resolve_table(&stmt.table_expr, defines, model, qp).map_err(|e| fill_pos_if_unknown(e, stmt.pos))?;

    if !stmt.order_by.is_empty() {
        let sub_exprs: Vec<Expr> = stmt
            .order_by
            .iter()
            .map(|it| substitute(&it.expr, defines, 0))
            .collect::<Result<_, _>>()
            .map_err(|e| fill_pos_if_unknown(e, stmt.pos))?;
        let ctx = FilterContext::default();
        let mut keyed: Vec<(usize, Vec<DaxValue>)> = Vec::with_capacity(tbl.rows.len());
        for row_idx in 0..tbl.rows.len() {
            let rc = RowContext { table: &tbl, row_idx };
            let keys: Vec<DaxValue> = sub_exprs
                .iter()
                .map(|e| evaluate_ast(e, model, &ctx, &[rc.clone()], &HashMap::new()).unwrap_or(DaxValue::Blank))
                .collect();
            keyed.push((row_idx, keys));
        }
        keyed.sort_by(|a, b| {
            for (i, item) in stmt.order_by.iter().enumerate() {
                let ord = compare_dax(&a.1[i], &b.1[i]);
                let ord = if item.dir == OrderDir::Desc { ord.reverse() } else { ord };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });

        if !stmt.start_at.is_empty() {
            let start_vals: Vec<DaxValue> = stmt
                .start_at
                .iter()
                .map(|e| {
                    let se = substitute(e, defines, 0)?;
                    evaluate_ast(&se, model, &ctx, &[], &HashMap::new()).map_err(|msg| qp.locate(msg))
                })
                .collect::<Result<_, _>>()
                .map_err(|e| fill_pos_if_unknown(e, stmt.pos))?;
            let mut from = keyed.len();
            for (i, (_, keys)) in keyed.iter().enumerate() {
                let mut meets = true;
                for (c, sv) in start_vals.iter().enumerate() {
                    let Some(k) = keys.get(c) else { break };
                    let dir = stmt.order_by.get(c).map(|o| o.dir).unwrap_or(OrderDir::Asc);
                    let mut cmp = compare_dax(k, sv);
                    if dir == OrderDir::Desc {
                        cmp = cmp.reverse();
                    }
                    if cmp == Ordering::Less {
                        meets = false;
                        break;
                    }
                }
                if meets {
                    from = i;
                    break;
                }
            }
            keyed = keyed.into_iter().skip(from).collect();
        }

        tbl.rows = keyed.into_iter().map(|(idx, _)| tbl.rows[idx].clone()).collect();
    }

    let total = tbl.rows.len();
    let truncated = total > limit;
    let take = total.min(limit);
    let columns: Vec<DaxQueryColumn> = tbl
        .columns
        .iter()
        .zip(tbl.col_types.iter())
        .enumerate()
        .map(|(idx, (n, t))| {
            let mut format_string = split_qualified(n).and_then(|(table, col)| model.column_format_string(table, col)).map(str::to_string);
            if format_string.is_none() {
                if let Some(ext_expr) = extensions.get(&n.to_lowercase()) {
                    // Extension (measure/computed) column: try to inherit the
                    // format of the single real measure it traces back to
                    // (e.g. a `DEFINE MEASURE Teste = [Faturamento] * 2`
                    // extension copies `[Faturamento]`'s format), else fall
                    // back to 2-decimal grouping when some value is
                    // fractional so cents aren't silently dropped by the
                    // UI's plain `formatNumber` (see v0.11 round 2 bug
                    // report: "T aparece como '1.986.738'").
                    format_string = infer_measure_format(ext_expr, model).or_else(|| {
                        if tbl.rows.iter().any(|r| r.get(idx).map(is_fractional).unwrap_or(false)) {
                            Some("#,##0.00".to_string())
                        } else {
                            None
                        }
                    });
                }
            }
            DaxQueryColumn { name: n.clone(), dax_type: type_id(*t), format_string }
        })
        .collect();
    let rows: Vec<Vec<JsonValue>> = tbl.rows[..take].iter().map(|r| r.iter().map(|v| v.to_json()).collect()).collect();
    let display_rows: Vec<Vec<String>> = tbl.rows[..take].iter().map(|r| r.iter().map(|v| v.as_display_string()).collect()).collect();
    Ok(DaxQueryResultSet { columns, rows, display_rows, row_count: total, truncated })
}

/// Entry point used by the `dax_query_execute` Tauri command and by the
/// browser-mode TS fallback's Rust-parity vitest fixtures.
pub fn execute_query(model: &DaxModel, query: &str, opts: &DaxQueryOptions) -> DaxQueryExecuteResult {
    let started = Instant::now();
    let limit = opts.row_limit.unwrap_or(1000).max(1);
    let program: QueryProgram = match parse_query(query) {
        Ok(p) => p,
        Err(e) => return DaxQueryExecuteResult { result_sets: vec![], elapsed_ms: elapsed_ms(started), error: Some(e.into()) },
    };
    let defines = Defines::from_program(&program.defines);
    let qp = QueryPos::new(query);
    let mut result_sets = Vec::new();
    for stmt in &program.evaluates {
        match run_evaluate(model, &defines, stmt, limit, &qp) {
            Ok(rs) => result_sets.push(rs),
            Err(e) => return DaxQueryExecuteResult { result_sets, elapsed_ms: elapsed_ms(started), error: Some(e.into()) },
        }
    }
    DaxQueryExecuteResult { result_sets, elapsed_ms: elapsed_ms(started), error: None }
}
