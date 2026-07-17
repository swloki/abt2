use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use super::model::*;

#[async_trait]
pub trait ProductionBatchService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBatchReq) -> Result<i64>;
    async fn split_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
        splits: Vec<SplitReq>,
    ) -> Result<Vec<i64>>;
    async fn find_by_id(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ProductionBatch>;
    async fn find_by_card_sn(&self, ctx: &ServiceContext, db: PgExecutor<'_>, card_sn: String) -> Result<Option<ProductionBatch>>;
    async fn list_by_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ProductionBatch>>;
    async fn confirm_routing_step(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        step_no: i32,
        req: StepConfirmationReq,
    ) -> Result<StepConfirmationResult>;
    async fn advance_to_receipt(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<()>;
    /// 开工：Pending → InProgress，置 actual_start
    async fn start_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<()>;
    async fn suspend(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        reason: String,
    ) -> Result<()>;
    async fn resume(&self, ctx: &ServiceContext, db: PgExecutor<'_>, batch_id: i64) -> Result<()>;
    async fn scrap(&self, ctx: &ServiceContext, db: PgExecutor<'_>, batch_id: i64, reason: String) -> Result<()>;
    /// 记录部分报废：不改变批次状态，仅递增 scrap_qty 并记录原因。
    /// 与 scrap（整批 Cancel + 释放预留）不同，此方法保持批次在 InProgress/Suspended。
    async fn record_scrap(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
        scrap_qty: rust_decimal::Decimal,
        reason: String,
        notes: Option<String>,
    ) -> Result<()>;
    async fn get_product_name(
        &self,
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<String>>;
    async fn list_routings(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderRouting>>;

    /// 列出工单中「需要领料」的工序 routing_id 集合。
    /// 需领料 = 工序产出品的 BOM 直接子级含外购物料（非本工单任何工序产出品）。
    /// 纯消耗半成品 / 无产出 / 无子级的工序不需领料（半成品走报工倒冲、散料走完工倒冲）。
    /// 供工作台动作位识别「无需领料直接收料」的工序，避免纯半成品工序卡在领料按钮。
    async fn list_routings_needing_requisition(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<std::collections::HashSet<i64>>;

    /// 从 BOM 内联工序（bom_operations）加载到工单快照。返回插入行数。
    /// per-order lock（D8）：任一 step 报工即整单冻结，跳过 reload。
    /// 价从 bom_step_prices LEFT JOIN（未定价则 NULL，由 release drawer 填）。
    async fn load_operations_from_bom(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64, product_code: String,
    ) -> Result<usize>;

    async fn delete_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
    ) -> Result<()>;

    async fn list_batches(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: BatchListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<crate::shared::types::PaginatedResult<BatchListItem>>;

    /// 工单是否已有任意报工记录（删除工序的全局守卫 + UI 删除按钮可见性）
    async fn order_has_any_report(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<bool>;

    /// 查询某批次各工序的执行进度（写真相源：status/completed_qty/defect_qty）。
    async fn list_progress_by_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<BatchRoutingProgress>>;
}
