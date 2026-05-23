use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::{SalesOrder, SalesOrderQuery};
use crate::repositories::{Executor, PaginatedResult};

pub struct CreateSalesOrderParams<'a> {
    pub quotation_id: Option<i64>,
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub remark: Option<&'a str>,
    pub delivery_date: Option<chrono::DateTime<chrono::Utc>>,
    pub operator_id: Option<i64>,
}

pub struct UpdateSalesOrderHeaderParams<'a> {
    pub customer_name: &'a str,
    pub contact_person: Option<&'a str>,
    pub contact_phone: Option<&'a str>,
    pub remark: Option<&'a str>,
    pub delivery_date: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct CreateSalesOrderItemParams {
    pub product_id: i64,
    pub unit_price: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub remark: Option<String>,
}

#[async_trait]
pub trait SalesOrderService: Send + Sync {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateSalesOrderParams<'_>,
        items: Vec<CreateSalesOrderItemParams>,
    ) -> Result<i64>;

    async fn update_header(
        &self,
        executor: Executor<'_>,
        order_id: i64,
        params: &UpdateSalesOrderHeaderParams<'_>,
    ) -> Result<()>;

    async fn delete(&self, executor: Executor<'_>, order_id: i64) -> Result<()>;

    async fn get_by_id(&self, order_id: i64) -> Result<Option<SalesOrder>>;

    async fn list(&self, query: &SalesOrderQuery) -> Result<PaginatedResult<SalesOrder>>;

    async fn update_status(
        &self,
        executor: Executor<'_>,
        order_id: i64,
        status: i16,
    ) -> Result<()>;
}
