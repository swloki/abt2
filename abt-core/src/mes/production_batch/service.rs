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
    async fn suspend(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        reason: String,
    ) -> Result<()>;
    async fn resume(&self, ctx: &ServiceContext, db: PgExecutor<'_>, batch_id: i64) -> Result<()>;
    async fn scrap(&self, ctx: &ServiceContext, db: PgExecutor<'_>, batch_id: i64, reason: String) -> Result<()>;
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

    /// 修改工序产出品、计件单价、工作中心、标准工时、委外（单事务，首次报工前可改）
    async fn update_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        product_id: Option<i64>,
        unit_price: rust_decimal::Decimal,
        work_center_id: Option<i64>,
        standard_time: Option<rust_decimal::Decimal>,
        is_outsourced: bool,
    ) -> Result<WorkOrderRouting>;

    /// 从工艺路径模板加载工序步骤到工单（删除未报工旧行 → 插入模板步骤）。返回插入行数
    async fn load_routings_from_template(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64, routing_id: i64,
    ) -> Result<usize>;

    /// 从最近同 routing_id 且有产出品的工单按 step_no 复制（仅未报工 + 原空行）。返回填充行数
    async fn load_routings_from_recent(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64,
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
}
