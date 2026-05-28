pub mod batch;
pub mod context;
pub mod error;
pub mod pagination;
pub mod transaction;

pub use batch::{BatchFailure, BatchMode, BatchResult};
pub use context::ServiceContext;
pub use error::{DomainError, Result};
pub use pagination::{DataScope, PageParams, PaginatedResult};
pub use transaction::TransactionMode;

// Re-export sqlx types for downstream crates that need PgPool but shouldn't depend on sqlx directly
pub use sqlx::PgPool;
pub use sqlx::Postgres;
pub use sqlx::postgres::PgPoolOptions;
pub use sqlx::pool::PoolConnection;

/// PostgreSQL 执行器类型
pub type PgExecutor<'a> = &'a mut sqlx::postgres::PgConnection;

/// PostgreSQL 连接池连接类型
pub type PgPoolConn = PoolConnection<Postgres>;
