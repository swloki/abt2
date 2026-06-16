use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{MaterialRequisition, RequisitionFilter, IssueMaterialReq, CreateManualReq, ReturnMaterialReq};

#[async_trait]
pub trait MaterialRequisitionService: Send + Sync {
    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
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
