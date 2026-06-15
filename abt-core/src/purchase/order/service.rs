use async_trait::async_trait;

use super::model::{CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderItem, PurchaseOrderQuery, UpdatePurchaseOrderRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

#[async_trait]
pub trait PurchaseOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseOrderRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn create_from_quotation(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseOrder>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: PurchaseOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseOrder>>;

    async fn list_items(&self, ctx: &ServiceContext, db: PgExecutor<'_>, order_id: i64) -> Result<Vec<PurchaseOrderItem>>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdatePurchaseOrderRequest,
        items: Vec<CreateOrderItemRequest>,
    ) -> Result<()>;
}