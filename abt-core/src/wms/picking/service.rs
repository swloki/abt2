use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};

use super::model::{
    CreateFromOrderReq, CreateManualReq, CreatePickingReq, DoneItemReq, IssueMaterialReq,
    PickingFilter, RequestShippingItemReq, ReturnMaterialReq, ShippingHubSummary, StockPicking,
    StockPickingItem,
};

/// 统一库存作业单据 Service（Issue #146）
///
/// 把收货/发货/领料/调拨 4 类作业单据收口为单一 service，按 `picking_type` 区分业务，
/// 统一 4 态状态机。底层库存流水仍由 `InventoryTransactionService` 承载（done/issue 时写入）。
///
/// 阶段 2：领料（InternalIssue）从 `material_requisitions` 直接迁入——领料专用方法
/// （`create_for_work_order` / `create_for_routing_step` / `create_manual` / `issue` /
/// `return_materials`）承担原 `MaterialRequisitionService` 的全部业务逻辑。
#[async_trait]
pub trait PickingService: Send + Sync {
    // ── 通用作业单据 ──

    /// 创建作业单据（状态: Draft）
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePickingReq,
    ) -> Result<i64>;

    /// 查询作业单据（头）
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<StockPicking>;

    /// 按 id 查询（find_by_id 别名，发货调用方兼容）
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<StockPicking>;

    /// 查询作业单据明细列表
    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
    ) -> Result<Vec<StockPickingItem>>;

    /// 分页查询作业单据列表
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PickingFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<StockPicking>>;

    /// 确认（Draft → Confirmed）
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 取消（Draft / Confirmed → Cancelled）
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 删除（仅 Draft 软删除）
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 执行完成（Confirmed → Done）—— 通用入口，按 picking_type 分发业务
    async fn done(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        items: Vec<DoneItemReq>,
    ) -> Result<()>;

    // ── 领料专用（InternalIssue，从 MaterialRequisitionService 迁入）──

    /// 工单级领料：按工单 BOM 快照展开叶子组件建 InternalIssue picking
    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<i64>;

    /// 工序级领料（产出品驱动）：按产出品在成品 BOM 中的子级展开建 picking，
    /// items 挂 operation_id=routing_id + batch_id
    async fn create_for_routing_step(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        batch_id: Option<i64>,
    ) -> Result<i64>;

    /// 手动创建领料单（非工单驱动）
    async fn create_manual(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateManualReq,
    ) -> Result<i64>;

    /// 发料（Confirmed/PartiallyIssued → Issued/PartiallyIssued）：
    /// 写 MaterialIssue 流水（负数）+ 消耗 HARD 预留 + 记工单材料成本分录 + 审计
    async fn issue(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: IssueMaterialReq)
        -> Result<()>;

    /// 退料：Issued/PartiallyIssued → 退料入库（正数流水）+ 恢复预留
    async fn return_materials(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReturnMaterialReq,
    ) -> Result<()>;

    /// 批量查多个领料 picking 的明细（避免 N+1，参数名保留 req_ids 兼容调用方）
    async fn list_items_by_req_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        requisition_ids: &[i64],
    ) -> Result<Vec<StockPickingItem>>;

    /// 查询批次已领料的工序 routing_id 集合（驱动批次矩阵动作位推进）
    async fn list_requisitioned_routing_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<i64>>;

    // ── 调拨专用（InternalTransfer，从 TransferService 迁入）──

    /// 调拨发货（Draft → Confirmed）：扣减源仓库库存（Transfer 流水负数）
    async fn dispatch(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 调拨完成（Confirmed → Done）：增加目标仓库库存（Transfer 流水正数）
    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    // ── 发货专用（OutgoingSales，从 ShippingRequestService 迁入，#146 阶段 4b）──

    /// 从订单正式创建发货 picking（Draft，需 confirm）
    async fn create_from_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateFromOrderReq,
    ) -> Result<i64>;

    /// 一键申请发货（订单详情页弹窗）：跳 Draft → 直接 Confirmed，回写 SO ShippingRequested
    async fn request_from_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        items: Vec<RequestShippingItemReq>,
    ) -> Result<i64>;

    /// 直接发货（Confirmed → Done）：选仓 + SalesShipment 流水 + 释放预留 + 回写 SO Shipped + 事件。
    /// 拣货已移除，所有发货走此入口（仓库由选仓 drawer 传入）。
    async fn direct_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        bin_id: Option<i64>,
    ) -> Result<()>;

    /// 发货 Hub 摘要（缺货 ATP 判定）
    async fn hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingHubSummary>;

    /// 发货相关库存流水（懒加载 disclosure）
    async fn list_transactions(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<crate::wms::inventory_transaction::model::InventoryTransaction>>;
}
