use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{MaterialRequisition, RequisitionFilter, IssueMaterialReq};

#[async_trait]
pub trait MaterialRequisitionService: Send + Sync {
    async fn create_for_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<MaterialRequisition>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: RequisitionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<MaterialRequisition>>;

    async fn confirm(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;

    async fn issue(
        &self,
        ctx: ServiceContext<'_>,
        req: IssueMaterialReq,
    ) -> Result<()>;

    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;
}
