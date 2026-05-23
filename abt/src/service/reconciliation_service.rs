use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::{ReconciliationQuery, ReconciliationStatement};
use crate::repositories::{Executor, PaginatedResult};

pub struct CreateReconciliationParams<'a> {
    pub customer_name: &'a str,
    pub period_year: i16,
    pub period_month: i16,
    pub remark: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct AdjustmentItemParams {
    pub product_id: Option<i64>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub remark: Option<String>,
}

#[async_trait]
pub trait ReconciliationService: Send + Sync {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateReconciliationParams<'_>,
    ) -> Result<i64>;

    async fn add_adjustments(
        &self,
        executor: Executor<'_>,
        statement_id: i64,
        items: Vec<AdjustmentItemParams>,
    ) -> Result<()>;

    async fn update(&self, executor: Executor<'_>, statement_id: i64, remark: Option<&str>) -> Result<()>;

    async fn delete(&self, executor: Executor<'_>, statement_id: i64) -> Result<()>;

    async fn get_by_id(&self, statement_id: i64) -> Result<Option<ReconciliationStatement>>;

    async fn list(&self, query: &ReconciliationQuery) -> Result<PaginatedResult<ReconciliationStatement>>;

    async fn update_status(
        &self,
        executor: Executor<'_>,
        statement_id: i64,
        status: i16,
    ) -> Result<()>;
}
