//! v0.11 Frente F: DAX Query View (`DEFINE`/`EVALUATE`/`ORDER BY`) and DAX
//! expression validation/diagnostics, executed in Rust via two Tauri
//! commands (`dax_query_execute`, `dax_validate`; wired in `lib.rs`).
//!
//! Split into:
//! - `qast`: the query-level AST (not the scalar/table one in `dax::ast`,
//!   which every sub-expression still parses into via `dax::parser`).
//! - `qparser`: `DEFINE`/`EVALUATE`/`ORDER BY`/`START AT` + `DATATABLE`
//!   parsing.
//! - `qexec`: `DEFINE` resolution (AST substitution) + table materialization
//!   (delegates SUMMARIZECOLUMNS/ADDCOLUMNS/FILTER/TOPN/VALUES/... to the
//!   shared `dax::evaluator`) + ORDER BY/START AT/row-limit.
//! - `qvalidate`: `dax_validate`'s diagnostics.
//!
//! Browser-mode-without-Tauri fallback: there is **none** in Rust (there
//! can't be — this *is* the Tauri backend). The frontend's
//! `src/components/daxquery/**` and the measure-formula modals must keep a
//! TS-engine fallback path for the no-Tauri-bridge case (see
//! `src/bridge/tauriBridge.ts`); that fallback is explicitly marked in the
//! frontend code as browser-only, per the task's rule 4.

pub mod qast;
pub mod qexec;
pub mod qparser;
pub mod qvalidate;
#[cfg(test)]
mod tests;

pub use qexec::{execute_query, DaxQueryExecuteResult, DaxQueryOptions};
pub use qvalidate::{validate_expression, ValidateContext, ValidationResult};
