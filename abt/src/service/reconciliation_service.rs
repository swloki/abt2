use anyhow::Result;
use async_trait::async_trait;

use crate::models::{ReconciliationItem, ReconciliationQuery, ReconciliationStatement};
use crate::repositories::{Executor, PaginatedResult};

#[async_trait]
pub trait ReconciliationService: Send + Sync {
    async fn create(&self, operator_id: Option<i64>, stmt: ReconciliationStatement, executor: Executor<'_>) -> Result<i64>;
    async fn add_adjustments(&self, statement_id: i64, items: Vec<ReconciliationItem>, executor: Executor<'_>) -> Result<()>;
    async fn update(&self, operator_id: Option<i64>, stmt: ReconciliationStatement, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, statement_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, statement_id: i64) -> Result<Option<ReconciliationStatement>>;
    async fn list(&self, query: ReconciliationQuery) -> Result<PaginatedResult<ReconciliationStatement>>;
    async fn update_status(&self, statement_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
