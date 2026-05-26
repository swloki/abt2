pub mod batch;
pub mod context;
pub mod error;
pub mod pagination;
pub mod transaction;

pub use batch::{BatchFailure, BatchMode, BatchResult};
pub use context::ServiceContext;
pub use error::{DomainError, RepoResult};
pub use pagination::{DataScope, PageParams, PaginatedResult};
pub use transaction::TransactionMode;

/// PostgreSQL 执行器类型
pub type PgExecutor<'a> = &'a mut sqlx::postgres::PgConnection;
