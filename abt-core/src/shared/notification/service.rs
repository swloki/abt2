use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn create_notification(&self, ctx: ServiceContext<'_>, req: CreateNotificationReq) -> Result<i64>;
    async fn mark_read(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;
    async fn mark_all_read(&self, ctx: ServiceContext<'_>, notification_type: Option<NotificationType>) -> Result<u64>;
    async fn get_unread_count(&self, ctx: ServiceContext<'_>) -> Result<i64>;
    async fn list_notifications(&self, ctx: ServiceContext<'_>, query: NotificationQuery) -> Result<PaginatedResult<Notification>>;
}
