use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};

use super::model::{CreatePickingReq, DoneItemReq, PickingFilter, StockPicking, StockPickingItem};

/// 统一库存作业单据 Service（Issue #146）
///
/// 把收货/发货/领料/调拨 4 类作业单据收口为单一 service，按 `picking_type` 区分业务，
/// 统一 4 态状态机。底层库存流水仍由 `InventoryTransactionService` 承载（done 时写入）。
#[async_trait]
pub trait PickingService: Send + Sync {
    /// 创建作业单据（状态: Draft）
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePickingReq,
    ) -> Result<i64>;

    /// 查询作业单据（头）
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<StockPicking>;

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
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockPicking>>;

    /// 确认（Draft → Confirmed）
    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 取消（Draft / Confirmed → Cancelled）
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 执行完成（Confirmed → Done）
    ///
    /// done 时事务内：① 更新行级 qty_done ② 写 inventory_transactions 流水
    /// ③ 回写来源单据（PO/WO/SO）④ 发 PickingDone 事件。
    ///
    /// **阶段 1（本 PR）**：仅状态转换 + done_at + 行级 qty_done，**不写流水、不回写来源**
    /// （库存流水/回写/事件按 picking_type 分发的逻辑在阶段 2-5 迁移各业务时补全）。
    async fn done(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        items: Vec<DoneItemReq>,
    ) -> Result<()>;
}
