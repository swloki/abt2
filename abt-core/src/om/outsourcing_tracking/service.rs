use async_trait::async_trait;

use super::model::{OutsourcingTracking, OverdueTrackingQuery, RecordNodeReq};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait OutsourcingTrackingService: Send + Sync {
    async fn record_node(
        &self,
        ctx: ServiceContext<'_>,
        req: RecordNodeReq,
    ) -> Result<i64, DomainError>;

    async fn list_by_outsourcing(
        &self,
        ctx: ServiceContext<'_>,
        outsourcing_id: i64,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>, DomainError>;

    async fn list_overdue(
        &self,
        ctx: ServiceContext<'_>,
        filter: OverdueTrackingQuery,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>, DomainError>;
}
