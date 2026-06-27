use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use super::model::*;

#[async_trait]
pub trait WorkOrderService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateWorkOrderReq) -> Result<i64>;
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<WorkOrder>;
    async fn release(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    /// 标记工单为生产中：Released → InProduction
    /// 条件 UPDATE，幂等。用于批次首次报工时自动传播。
    async fn mark_in_production(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;
    /// 反下达工单：Released -> Draft
    /// 安全网操作：取消领料单、释放库存预留、删除批次和工序
    async fn unrelease(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn close(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()>;
    async fn get_product_name(&self, db: PgExecutor<'_>, product_id: i64) -> Result<Option<String>>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: WorkOrderFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WorkOrder>>;
    /// 按生产计划 ID 查询关联工单
    async fn list_by_plan(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>>;

    /// 工单工作台聚合视图：单次返回 detail-header + 摘要带 + 6 disclosure 全部数据。
    async fn get_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderHubSummary>;

    /// 列表批量物料可用性（降级 2 级：Available / Unavailable）。
    ///
    /// 性能优先：仅判 ATP（不查 PO ETA），4 级判定留 `get_hub_summary`。
    /// 对每个工单的 BOM 快照叶子：`required = node.quantity × planned_qty`，
    /// `atp = available_atp(product_id, None)`，任一叶子 `atp < required` →
    /// `Unavailable`（headline = 该物料名），否则 `Available`。
    /// 已关闭/取消工单：`Available` + None（不计算）。
    /// 返回 HashMap<work_order_id, (Level, headline)>。
    async fn compute_availability_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        orders: &[super::model::WorkOrder],
    ) -> Result<std::collections::HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>>;
}
