use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PaginatedResult, ServiceContext};

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn create_notification(&self, ctx: ServiceContext<'_>, req: CreateNotificationReq) -> Result<i64, DomainError>;
    async fn mark_read(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn mark_all_read(&self, ctx: ServiceContext<'_>, notification_type: Option<NotificationType>) -> Result<u64, DomainError>;
    async fn get_unread_count(&self, ctx: ServiceContext<'_>) -> Result<i64, DomainError>;
    async fn list_notifications(&self, ctx: ServiceContext<'_>, query: NotificationQuery) -> Result<PaginatedResult<Notification>, DomainError>;
}
