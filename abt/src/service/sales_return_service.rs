use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::{SalesReturn, SalesReturnQuery};
use crate::repositories::{Executor, PaginatedResult};

pub struct CreateSalesReturnParams<'a> {
    pub request_id: i64,
    pub remark: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct UpdateSalesReturnParams<'a> {
    pub remark: Option<&'a str>,
    pub reason: Option<&'a str>,
}

pub struct CreateSalesReturnItemParams {
    pub request_item_id: i64,
    pub quantity: Decimal,
    pub remark: Option<String>,
}

#[async_trait]
pub trait SalesReturnService: Send + Sync {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateSalesReturnParams<'_>,
        items: Vec<CreateSalesReturnItemParams>,
    ) -> Result<i64>;

    async fn update(
        &self,
        executor: Executor<'_>,
        return_id: i64,
        params: &UpdateSalesReturnParams<'_>,
        items: Vec<CreateSalesReturnItemParams>,
    ) -> Result<()>;

    async fn delete(&self, executor: Executor<'_>, return_id: i64) -> Result<()>;

    async fn get_by_id(&self, return_id: i64) -> Result<Option<SalesReturn>>;

    async fn list(&self, query: &SalesReturnQuery) -> Result<PaginatedResult<SalesReturn>>;

    async fn update_status(
        &self,
        executor: Executor<'_>,
        return_id: i64,
        status: i16,
    ) -> Result<()>;
}
