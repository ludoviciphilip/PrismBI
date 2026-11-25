//! v0.9 ETL core: Power Query-style queries executed on DuckDB, with an M
//! round-trip. Contract: `docs/ETL_V09.md`.

pub mod custom_expr;
pub mod m_from;
pub mod m_parse;
pub mod m_to;
pub mod model;
pub mod raw;
pub mod sql;
