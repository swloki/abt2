use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait SalesOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64>;

    async fn create_from_quotation(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<i64>;

    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<SalesOrder>;

    async fn update_header(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<()>;

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
        items: Vec<CreateSalesOrderItemReq>,
    ) -> Result<()>;

    async fn list_items(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<SalesOrderItem>>;

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>>;

    // -- P1 新增 --

    /// 取消订单行（部分或全部）。增加 cancelled_qty。
    async fn cancel_line(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
        line_id: i64,
        req: CancelLineReq,
    ) -> Result<()>;

    /// 查询履行计划行
    async fn list_fulfillment_plan(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: FulfillmentPlanQuery,
    ) -> Result<Vec<FulfillmentPlanLine>>;

    /// 幂等重算订单头状态（根据行状态聚合推导）
    async fn recalc_header_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<SalesOrderStatus>;

    /// 手动对账：检测 fulfillment_plan_lines 状态不一致并修复
    async fn reconcile_fulfillment_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<u32>;
}

/// 分配策略接口 — P1 定义接口，后续实现 FIFO
pub struct AllocationResult {
    pub fulfillment_line_id: i64,
    pub allocated_qty: Decimal,
}

#[async_trait]
pub trait ReplenishmentAllocationStrategy: Send + Sync {
    /// 给定可用量和候选履行计划行，按策略分配
    fn allocate(
        &self,
        product_id: i64,
        available_qty: Decimal,
        candidates: &[FulfillmentPlanLine],
    ) -> Vec<AllocationResult>;
}
