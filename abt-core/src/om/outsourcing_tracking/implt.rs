use chrono::Utc;

use super::model::{OutsourcingTracking, OverdueTrackingQuery, RecordNodeReq};
use super::repo::OutsourcingTrackingRepo;
use super::service::OutsourcingTrackingService;
use crate::om::enums::TrackingNodeType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

#[derive(Default)]
pub struct OutsourcingTrackingServiceImpl;

impl OutsourcingTrackingServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

fn validate_node_sequence(max_ordinal: Option<i16>, target: i16) -> Result<()> {
    if let Some(max) = max_ordinal
        && target <= max
    {
        return Err(DomainError::validation(format!(
            "追踪节点必须按顺序录入：当前最大序号 {max}，目标序号 {target}"
        )));
    }
    Ok(())
}

async fn validate_prerequisites(
    db: PgExecutor<'_>,
    outsourcing_id: i64,
    node_type: TrackingNodeType,
) -> Result<()> {
    let required = match node_type {
        TrackingNodeType::CarrierPickup => Some(TrackingNodeType::SendMaterial),
        TrackingNodeType::Warehoused => Some(TrackingNodeType::IqcInspected),
        _ => None,
    };
    if let Some(req_nt) = required {
        let exists = OutsourcingTrackingRepo::has_node_type(
            db,
            outsourcing_id,
            req_nt,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !exists {
            return Err(DomainError::validation(format!(
                "{node_type:?} 需要 {req_nt:?} 已完成",
            )));
        }
    }
    Ok(())
}

#[async_trait::async_trait]
impl OutsourcingTrackingService for OutsourcingTrackingServiceImpl {
    async fn record_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: RecordNodeReq,
    ) -> Result<i64> {
        let target_ordinal = req.node_type.ordinal();

        let max_ordinal = OutsourcingTrackingRepo::get_max_node_ordinal(
            &mut *db,
            req.outsourcing_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        validate_node_sequence(max_ordinal, target_ordinal)?;

        validate_prerequisites(db, req.outsourcing_id, req.node_type).await?;

        let tracked_at = req.tracked_at.or(Some(Utc::now()));

        let id = OutsourcingTrackingRepo::insert(
            &mut *db,
            req.outsourcing_id,
            req.node_type,
            tracked_at,
            req.remark.as_deref(),
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(id)
    }

    async fn list_by_outsourcing(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        outsourcing_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>> {
        let (items, total) = OutsourcingTrackingRepo::list_by_outsourcing_id(
            &mut *db,
            outsourcing_id,
            &page,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_overdue(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: OverdueTrackingQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>> {
        let (items, total) =
            OutsourcingTrackingRepo::query_overdue(&mut *db, &filter, &page)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_active_summary(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        supplier_id: Option<i64>,
        node_type: Option<crate::om::enums::TrackingNodeType>,
        page: PageParams,
    ) -> Result<PaginatedResult<OutsourcingTracking>> {
        let (items, total) =
            OutsourcingTrackingRepo::list_active_summary(&mut *db, supplier_id, node_type, &page)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}
