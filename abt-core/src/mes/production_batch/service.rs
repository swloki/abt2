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

    async fn update_routing_unit_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        unit_price: rust_decimal::Decimal,
    ) -> Result<WorkOrderRouting>;

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
}
