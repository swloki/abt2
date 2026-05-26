use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::repo::DomainEventRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use super::model::DomainEvent;

/// 死信服务 trait — 管理无法处理的事件
#[async_trait]
pub trait DeadLetterService: Send + Sync {
    /// 查询死信事件（分页）
    async fn list_dead_letters(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DomainEvent>>;

    /// 重试单个死信事件（重置为 Pending）
    async fn retry_one(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
    ) -> Result<()>;

    /// 归档（删除）过期死信
    async fn archive(
        &self,
        ctx: ServiceContext<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64>;
}

/// 死信服务实现
pub struct DeadLetterServiceImpl;

impl DeadLetterServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeadLetterServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeadLetterService for DeadLetterServiceImpl {
    async fn list_dead_letters(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DomainEvent>> {
        let params = PageParams::new(page, page_size);

        let (items, total) = DomainEventRepo::query_dead_letters(
            &mut *ctx.executor,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }

    async fn retry_one(
        &self,
        ctx: ServiceContext<'_>,
        event_id: i64,
    ) -> Result<()> {
        DomainEventRepo::reset_to_pending(&mut *ctx.executor, event_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn archive(
        &self,
        ctx: ServiceContext<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64> {
        let deleted = DomainEventRepo::archive_dead_letters(&mut *ctx.executor, before)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(deleted)
    }
}
