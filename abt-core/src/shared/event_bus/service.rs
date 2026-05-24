use async_trait::async_trait;

use super::model::{DomainEvent, EventPublishRequest, EventQuery};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

/// 领域事件总线 — 发布、确认、查询
#[async_trait]
pub trait DomainEventBus: Send + Sync {
    /// 发布事件（INSERT ON CONFLICT DO NOTHING + NOTIFY）。
    /// 返回事件 id（若重复则返回已有 id）。
    async fn publish(
        &self,
        ctx: ServiceContext<'_>,
        req: EventPublishRequest,
    ) -> Result<i64, DomainError>;

    /// 批量标记已处理
    async fn mark_processed(
        &self,
        ctx: ServiceContext<'_>,
        ids: Vec<i64>,
    ) -> Result<u64, DomainError>;

    /// 标记失败并记录原因
    async fn mark_failed(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        reason: &str,
    ) -> Result<(), DomainError>;

    /// 多维度可选过滤 + 分页查询
    async fn find_events(
        &self,
        ctx: ServiceContext<'_>,
        query: EventQuery,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DomainEvent>, DomainError>;
}
