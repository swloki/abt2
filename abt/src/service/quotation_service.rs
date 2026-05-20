use anyhow::Result;
use async_trait::async_trait;
use crate::models::{Quotation, QuotationQuery};
use crate::repositories::{Executor, PaginatedResult};

#[async_trait]
pub trait QuotationService: Send + Sync {
    async fn create(&self, operator_id: Option<i64>, quotation: Quotation, executor: Executor<'_>) -> Result<i64>;
    async fn update(&self, operator_id: Option<i64>, quotation: Quotation, executor: Executor<'_>) -> Result<()>;
    async fn delete(&self, quotation_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, quotation_id: i64) -> Result<Option<Quotation>>;
    async fn list(&self, query: QuotationQuery) -> Result<PaginatedResult<Quotation>>;
    async fn update_status(&self, quotation_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
