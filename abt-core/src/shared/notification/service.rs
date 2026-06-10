use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor, PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn create_notification(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateNotificationReq) -> Result<i64>;
    async fn mark_read(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn mark_all_read(&self, ctx: &ServiceContext, db: PgExecutor<'_>, notification_type: Option<NotificationType>) -> Result<u64>;
    async fn get_unread_count(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<i64>;
    async fn list_notifications(&self, ctx: &ServiceContext, db: PgExecutor<'_>, query: NotificationQuery) -> Result<PaginatedResult<Notification>>;

    /// Batch create notifications for a list of user IDs. Returns the count created.
    async fn batch_create_notifications(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        user_ids: &[i64],
        req: BatchNotificationReq,
    ) -> Result<u64>;

    /// Send notification to all active users with the given role.
    async fn notify_by_role(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        role_id: i64,
        req: BatchNotificationReq,
    ) -> Result<u64>;

    /// Send notification to all active users in the given department.
    async fn notify_by_department(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        department_id: i64,
        req: BatchNotificationReq,
    ) -> Result<u64>;
}
