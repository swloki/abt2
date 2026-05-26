use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use tracing::instrument;

use super::repo::IdempotencyRepo;
use super::service::IdempotencyService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct IdempotencyServiceImpl {
    #[allow(dead_code)] // 保留供未来独立事务模式使用
    pool: Arc<PgPool>,
}

impl IdempotencyServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyService for IdempotencyServiceImpl {
    #[instrument(skip(self, ctx), fields(event_id, handler_name))]
    async fn check_and_mark(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
    ) -> Result<bool> {
        let is_first = IdempotencyRepo::check_and_mark(
            &mut *ctx.executor,
            event_id,
            handler_name,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(is_first)
    }

    #[instrument(skip(self, ctx, result), fields(event_id, handler_name))]
    async fn mark_processed(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
        result: Option<serde_json::Value>,
    ) -> Result<()> {
        IdempotencyRepo::mark_processed(
            &mut *ctx.executor,
            event_id,
            handler_name,
            result.as_ref(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    #[instrument(skip(self, ctx), fields(before = %before.to_rfc3339()))]
    async fn cleanup_expired(
        &self,
        ctx: ServiceContext<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64> {
        let deleted = IdempotencyRepo::cleanup_expired(&mut *ctx.executor, before)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(deleted)
    }
}
