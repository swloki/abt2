use anyhow::Result;
use async_trait::async_trait;

use crate::models::{SalesReturn, SalesReturnQuery};
use crate::repositories::{Executor, PaginatedResult};

#[async_trait]
pub trait SalesReturnService: Send + Sync {
    async fn create(&self, operator_id: Option<i64>, ret: SalesReturn, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, ret: SalesReturn, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, return_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, return_id: i64) -> Result<Option<SalesReturn>>;
    async fn list(&self, query: SalesReturnQuery) -> Result<PaginatedResult<SalesReturn>>;
    async fn update_status(&self, return_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
