use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{MaterialRequisition, MaterialReqItem, RequisitionFilter, IssueMaterialReq, CreateManualReq, ReturnMaterialReq};

#[async_trait]
pub trait MaterialRequisitionService: Send + Sync {
    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<i64>;

    /// 工序级领料（产出品驱动，Issue #122）：按工序产出品展开其子 BOM 的叶子组件，
    /// items 挂 operation_id=routing_id。产出品无 BOM 的工序（散料）不在本单，走完工倒冲。
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
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateManualReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<MaterialRequisition>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: RequisitionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<MaterialRequisition>>;

    /// 读取领料单明细行（只读查询，供前端带出待领料明细）
    async fn list_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        requisition_id: i64,
    ) -> Result<Vec<MaterialReqItem>>;

    /// 查询批次已领料的工序 routing_id 集合（判断工序是否已领料，驱动批次矩阵动作位推进）。
    async fn list_requisitioned_routing_ids(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<i64>>;

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    async fn issue(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: IssueMaterialReq,
    ) -> Result<()>;

    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;
    /// 退料：Issued/PartiallyIssued → 退料入库（对标 Odoo stock.move.reverse）
    async fn return_materials(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReturnMaterialReq,
    ) -> Result<()>;
}
