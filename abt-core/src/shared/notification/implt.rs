use async_trait::async_trait;

use super::model::*;
use super::repo::NotificationRepo;
use super::service::NotificationService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;

pub struct NotificationServiceImpl {
    repo: NotificationRepo,
}

impl NotificationServiceImpl {
    pub fn new(repo: NotificationRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl NotificationService for NotificationServiceImpl {
    async fn create_notification(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateNotificationReq,
    ) -> Result<i64, DomainError> {
        self.repo
            .create(ctx.executor, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn mark_read(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let found = self
            .repo
            .mark_read(ctx.executor, id, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if !found {
            return Err(DomainError::NotFound(format!(
                "Notification {id} not found or already read"
            )));
        }
        Ok(())
    }

    async fn mark_all_read(
        &self,
        ctx: ServiceContext<'_>,
        notification_type: Option<NotificationType>,
    ) -> Result<u64, DomainError> {
        self.repo
            .mark_all_read(ctx.executor, ctx.operator_id, notification_type)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_unread_count(&self, ctx: ServiceContext<'_>) -> Result<i64, DomainError> {
        self.repo
            .get_unread_count(ctx.executor, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_notifications(
        &self,
        ctx: ServiceContext<'_>,
        query: NotificationQuery,
    ) -> Result<PaginatedResult<Notification>, DomainError> {
        self.repo
            .query(ctx.executor, ctx.operator_id, &query)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
