use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::instrument;

use super::model::{DomainEvent, EventPublishRequest, EventQuery};
use super::repo::{DomainEventRepo, InsertParams};
use super::service::DomainEventBus;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct DomainEventBusImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl DomainEventBusImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DomainEventBus for DomainEventBusImpl {
    #[instrument(skip(self, ctx, req), fields(aggregate_type = %req.aggregate_type, aggregate_id = req.aggregate_id))]
    async fn publish(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: EventPublishRequest,
    ) -> Result<i64> {
        let params = InsertParams::from_request(
            &req,
            ctx.operator_id,
            ctx.trace_id.clone(),
            ctx.request_id.clone(),
        );

        let id = DomainEventRepo::insert(&mut *db, &params)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // NOTIFY
        DomainEventRepo::notify(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(id)
    }

    #[instrument(skip(self, _ctx, ids))]
    async fn mark_processed(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        ids: Vec<i64>,
    ) -> Result<u64> {
        let affected = DomainEventRepo::mark_processed(&mut *db, &ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(affected)
    }

    #[instrument(skip(self, _ctx))]
    async fn mark_failed(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        reason: &str,
    ) -> Result<()> {
        DomainEventRepo::mark_failed(&mut *db, id, reason)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    #[instrument(skip(self, _ctx))]
    async fn find_events(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        query: EventQuery,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DomainEvent>> {
        let params = PageParams::new(page, page_size);

        let (items, total) = DomainEventRepo::query(
            &mut *db,
            &query,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }
}
