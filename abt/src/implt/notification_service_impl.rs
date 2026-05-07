//! 通知服务实现

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{Notification, NotificationQuery, UnreadCountByType};
use crate::repositories::NotificationRepo;
use crate::service::NotificationService;

pub struct NotificationServiceImpl {
    pool: Arc<PgPool>,
}

impl NotificationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationService for NotificationServiceImpl {
    async fn list_notifications(
        &self,
        user_id: i64,
        query: &NotificationQuery,
    ) -> Result<(Vec<Notification>, i64)> {
        NotificationRepo::find_by_user(&self.pool, user_id, query).await
    }

    async fn mark_as_read(&self, notification_id: i64, user_id: i64) -> Result<bool> {
        NotificationRepo::mark_as_read(&self.pool, notification_id, user_id).await
    }

    async fn mark_all_as_read(&self, user_id: i64, notification_type: Option<&str>) -> Result<u64> {
        NotificationRepo::mark_all_as_read(&self.pool, user_id, notification_type).await
    }

    async fn get_unread_count(&self, user_id: i64) -> Result<(i64, Vec<UnreadCountByType>)> {
        NotificationRepo::count_unread(&self.pool, user_id).await
    }
}
