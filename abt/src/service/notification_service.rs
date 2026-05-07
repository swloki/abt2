//! 通知服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Notification, NotificationQuery, UnreadCountByType};

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn list_notifications(
        &self,
        user_id: i64,
        query: &NotificationQuery,
    ) -> Result<(Vec<Notification>, i64)>;

    async fn mark_as_read(&self, notification_id: i64, user_id: i64) -> Result<bool>;

    async fn mark_all_as_read(&self, user_id: i64, notification_type: Option<&str>) -> Result<u64>;

    async fn get_unread_count(&self, user_id: i64) -> Result<(i64, Vec<UnreadCountByType>)>;
}
