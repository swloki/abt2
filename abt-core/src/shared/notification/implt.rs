use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::NotificationRepo;
use super::service::NotificationService;
use crate::shared::identity::repo::IdentityRepo;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

pub struct NotificationServiceImpl {
    repo: NotificationRepo,
    _pool: PgPool,
}

impl NotificationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: NotificationRepo,
            _pool: pool,
        }
    }
}

#[async_trait]
impl NotificationService for NotificationServiceImpl {
    async fn create_notification(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateNotificationReq,
    ) -> Result<i64> {
        self.repo
            .create(db, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn mark_read(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let found = self
            .repo
            .mark_read(db, id, ctx.operator_id)
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
        ctx: &ServiceContext, db: PgExecutor<'_>,
        notification_type: Option<NotificationType>,
    ) -> Result<u64> {
        self.repo
            .mark_all_read(db, ctx.operator_id, notification_type)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_unread_count(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<i64> {
        self.repo
            .get_unread_count(db, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_notifications(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: NotificationQuery,
    ) -> Result<PaginatedResult<Notification>> {
        self.repo
            .query(db, ctx.operator_id, &query)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn batch_create_notifications(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        user_ids: &[i64],
        req: BatchNotificationReq,
    ) -> Result<u64> {
        if user_ids.is_empty() {
            return Ok(0);
        }
        self.repo
            .batch_create(db, user_ids, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn notify_by_role(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        role_id: i64,
        req: BatchNotificationReq,
    ) -> Result<u64> {
        let user_ids = IdentityRepo::get_user_ids_by_role(&mut *db, role_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if user_ids.is_empty() {
            return Ok(0);
        }
        self.repo
            .batch_create(db, &user_ids, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn notify_by_department(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        department_id: i64,
        req: BatchNotificationReq,
    ) -> Result<u64> {
        let user_ids = IdentityRepo::get_user_ids_by_department(&mut *db, department_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if user_ids.is_empty() {
            return Ok(0);
        }
        self.repo
            .batch_create(db, &user_ids, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
