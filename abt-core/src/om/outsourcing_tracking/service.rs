use async_trait::async_trait;

use super::model::{OutsourcingTracking, OverdueTrackingQuery, RecordNodeReq};
use crate::om::enums::TrackingNodeType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait OutsourcingTrackingService: Send + Sync {
    async fn record_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: RecordNodeReq,
    ) -> Result<i64>;

    async fn list_by_outsourcing(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        outsourcing_id: i64,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>>;

    async fn list_overdue(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: OverdueTrackingQuery,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>>;

    async fn list_active_summary(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: Option<i64>,
        node_type: Option<TrackingNodeType>,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>>;
}
