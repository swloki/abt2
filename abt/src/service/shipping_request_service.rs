use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::{ShippingRequest, ShippingRequestQuery};
use crate::repositories::{Executor, PaginatedResult};

pub struct CreateShippingRequestParams<'a> {
    pub order_id: i64,
    pub remark: Option<&'a str>,
    pub operator_id: Option<i64>,
}

pub struct UpdateShippingRequestParams<'a> {
    pub remark: Option<&'a str>,
}

pub struct CreateShippingRequestItemParams {
    pub order_item_id: i64,
    pub quantity: Decimal,
    pub remark: Option<String>,
}

#[async_trait]
pub trait ShippingRequestService: Send + Sync {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateShippingRequestParams<'_>,
        items: Vec<CreateShippingRequestItemParams>,
    ) -> Result<i64>;

    async fn update(
        &self,
        executor: Executor<'_>,
        request_id: i64,
        params: &UpdateShippingRequestParams<'_>,
        items: Vec<CreateShippingRequestItemParams>,
    ) -> Result<()>;

    async fn delete(&self, executor: Executor<'_>, request_id: i64) -> Result<()>;

    async fn get_by_id(&self, request_id: i64) -> Result<Option<ShippingRequest>>;

    async fn list(&self, query: &ShippingRequestQuery) -> Result<PaginatedResult<ShippingRequest>>;

    async fn update_status(
        &self,
        executor: Executor<'_>,
        request_id: i64,
        status: i16,
    ) -> Result<()>;
}
