use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use super::super::enums::InspectionResultType;
use super::model::*;

#[async_trait]
pub trait ProductionInspectionService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionReq,
    ) -> Result<i64>;
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionInspection>;
    async fn record_result(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        result: InspectionResultType,
    ) -> Result<()>;
    async fn list_inspections(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InspectionListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InspectionListItem>>;
}
