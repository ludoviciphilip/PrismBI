//! v0.9 ETL core: Power Query-style queries executed on DuckDB, with an M
//! round-trip. Contract: `docs/ETL_V09.md`.
//!
//! - [`model`]: serde types of the contract (`EtlQuery`, `EtlStep`, …).
//! - [`m_parse`]: M tokenizer + recursive-descent parser.
//! - [`m_from`] / [`m_to`]: M ⇄ `EtlQuery`.
//! - [`sql`]: `EtlQuery` → chain of DuckDB CTEs (one per step).
//! - [`custom_expr`]: `addCustomColumn` formulas → SQL expressions.
//! - [`raw`]: loads sources into `raw__<query>`.
//! - [`commands`]: the Tauri commands.

pub mod commands;
pub mod custom_expr;
pub mod m_from;
pub mod m_parse;
pub mod m_to;
pub mod model;
pub mod raw;
pub mod sql;

