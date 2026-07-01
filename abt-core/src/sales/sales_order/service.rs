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

    /// 按多个订单 ID 批量取明细，避免逐单查询（N+1）。
    async fn list_items_by_order_ids(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_ids: &[i64],
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

    /// wms 出库后回写订单行已发数量 + 重算头状态（事务内调用，累加语义）。
    /// 替代跨模块直访 sales_order repo，供 wms::outbound 的 ship() 调用。
    async fn record_shipment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        lines: &[ShipmentLineQty],
    ) -> Result<SalesOrderStatus>;

    /// 只读发货状态（按订单行 Σshipped_qty vs Σquantity 推导）
    async fn delivery_status(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<DeliveryStatus>;
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

// ---------------------------------------------------------------------------
// DemandService — 需求池生命周期管理
// ---------------------------------------------------------------------------

/// 需求服务 — 管理需求池生命周期
#[async_trait]
pub trait DemandService: Send + Sync {
    /// 从订单创建需求（在 confirm 事务内调用）
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<i64>>;

    /// 按 ID 查询需求
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Demand>;

    /// 分页查询需求
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: DemandQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Demand>>;

    /// 下游确认需求（记录关联下游单据）
    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: ConfirmDemandReq,
    ) -> Result<()>;

    /// 下游驳回需求
    async fn reject(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 需求完成（下游单据执行完毕）
    async fn fulfill(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 取消需求
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 更新需求的关联下游单据（target_doc_type + target_doc_id）
    /// 供跨模块（采购/MES）在创建下游单据后关联需求时调用
    async fn update_target_doc(
        &self,
        db: PgExecutor<'_>,
        id: i64,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<()>;

    /// 释放下游单据关联的需求回池（status→Pending + 清 target_doc），
    /// 并发布 DemandReleased 事件对称回退履行计划行/订单行。
    /// 供跨模块（MES 工单/采购单）取消时回退需求调用 —— 与 update_target_doc 对称。
    async fn release_back_to_pool(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<()>;

    /// 对账：查询 fulfillment_plan_lines 与 demands 状态不一致的记录
    async fn find_mismatched(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>>;

    /// 按来源单据查询所有需求（如某销售订单关联的全部 demand）
    /// 用于销售订单详情页展示「需求状态」列的真实需求池状态
    async fn find_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
    ) -> Result<Vec<Demand>>;
}
