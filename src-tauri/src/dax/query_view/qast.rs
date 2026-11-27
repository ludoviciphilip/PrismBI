//! AST for the DAX **query** surface (DEFINE/EVALUATE/ORDER BY), separate
//! from the scalar/table expression AST in `dax::ast` (which this module
//! reuses for every sub-expression). See `query_view/mod.rs` for the
//! overview.

use crate::dax::ast::Expr;

/// 1-based line/column inside the original query text, for error underlines
/// in the UI's formula editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct QError {
    pub message: String,
    pub pos: Pos,
}

impl QError {
    pub fn new(message: impl Into<String>, pos: Pos) -> Self {
        Self { message: message.into(), pos }
    }

    /// Used when no better position is known (e.g. a runtime/semantic error
    /// surfaced by the shared evaluator, which carries no span).
    pub fn at_start(message: impl Into<String>) -> Self {
        Self { message: message.into(), pos: Pos { line: 1, col: 1 } }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct OrderItem {
    pub expr: Expr,
    pub dir: OrderDir,
}

/// A table-valued expression as written in a `TABLE` define or in an
/// `EVALUATE`. `DataTable` is split out because its `{{...}, {...}}` row
/// literal syntax isn't part of the shared scalar/table grammar in
/// `dax::parser` (which has no use for brace-delimited row lists outside an
/// `IN {...}` list) — it's parsed by hand in `qparser::parse_datatable`.
#[derive(Debug, Clone)]
pub enum TableExprSrc {
    Generic(Expr),
    DataTable { columns: Vec<(String, String)>, rows: Vec<Vec<Expr>> },
}

#[derive(Debug, Clone)]
pub struct EvaluateStmt {
    pub table_expr: TableExprSrc,
    pub order_by: Vec<OrderItem>,
    pub start_at: Vec<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Define {
    /// `MEASURE Table[Name] = <expr>` — only `Name` matters for lookup
    /// (DAX measures are referenced as bare `[Name]`, never table-qualified),
    /// `table` is kept for diagnostics/round-tripping only.
    Measure { table: String, name: String, expr: Expr },
    /// `VAR Name = <expr>`, visible to every `EVALUATE` that follows it.
    Var { name: String, expr: Expr },
    /// `TABLE Name = <table expr>`.
    Table { name: String, expr: TableExprSrc },
    /// `COLUMN Table[Name] = <row expr>` — only meaningful when `table`
    /// names a `DEFINE TABLE` in the same query; applied as a calculated
    /// column on that table after it's materialized.
    Column { table: String, name: String, expr: Expr },
}

#[derive(Debug, Default)]
pub struct QueryProgram {
    pub defines: Vec<Define>,
    pub evaluates: Vec<EvaluateStmt>,
}
