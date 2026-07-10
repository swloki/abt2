use async_trait::async_trait;

use super::model::{CreateOrderItemRequest, CreatePurchaseOrderRequest, PoItemChange, PurchaseOrder, PurchaseOrderItem, PurchaseOrderQuery, UpdatePurchaseOrderRequest};
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

    /// 批量取多个 PO 的明细（扁平 Vec，调用方按 order_id 分组）；避免逐个 list_items 的 N+1。
    async fn list_items_by_order_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_ids: &[i64],
    ) -> Result<Vec<PurchaseOrderItem>>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdatePurchaseOrderRequest,
        items: Vec<CreateOrderItemRequest>,
    ) -> Result<()>;

    /// 确认后修改明细（追加/修改行，不允许删除已收货行）
    /// 仅 Confirmed / PartiallyReceived 状态可调用
    async fn update_items_after_confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        item_changes: Vec<PoItemChange>,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    /// 提交 PO（自动判断是否需要审批）
    async fn submit(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    /// 审批通过（PendingApproval → Confirmed）
    async fn approve_po(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    /// 退回修改（PendingApproval → Draft）
    async fn reject(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        reason: String,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    /// 合并多个 Draft PO（必须是同一供应商）
    async fn merge_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_ids: Vec<i64>,
        idempotency_key: Option<String>,
    ) -> Result<i64>;
}