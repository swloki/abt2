use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use crate::models::{SalesOrder, SalesOrderQuery};
use crate::repositories::{Executor, PaginatedResult};

#[async_trait]
pub trait SalesOrderService: Send + Sync {
    async fn create(&self, operator_id: Option<i64>, order: SalesOrder, executor: Executor<'_>) -> Result<i64>;
    async fn update_header(&self, order_id: i64, customer_name: String, contact_person: Option<String>, contact_phone: Option<String>, remark: Option<String>, delivery_date: Option<NaiveDateTime>) -> Result<()>;
    async fn delete(&self, order_id: i64, executor: Executor<'_>) -> Result<()>;
    async fn get_by_id(&self, order_id: i64) -> Result<Option<SalesOrder>>;
    async fn list(&self, query: SalesOrderQuery) -> Result<PaginatedResult<SalesOrder>>;
    async fn update_status(&self, order_id: i64, status: i16, executor: Executor<'_>) -> Result<()>;
}
