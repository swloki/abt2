//! ABT 项目通用模块

pub use sqlx;

pub mod error;

/// PostgreSQL 执行器类型（用于 abt 等模块）
pub type PgExecutor<'a> = &'a mut sqlx::postgres::PgConnection;
