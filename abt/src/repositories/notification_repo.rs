//! 通知数据访问层

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{CreateNotificationRequest, Notification, NotificationQuery, UnreadCountByType};

pub struct NotificationRepo;

impl NotificationRepo {
    pub async fn insert(pool: &PgPool, req: &CreateNotificationRequest) -> Result<Notification> {
        let row = sqlx::query_as::<_, Notification>(
            r#"
            INSERT INTO notifications (user_id, type, title, content, related_type, related_id, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING notification_id, user_id, type AS "type", title, content, related_type, related_id,
                      is_read, read_at, created_at, metadata
            "#,
        )
        .bind(req.user_id)
        .bind(&req.notification_type)
        .bind(&req.title)
        .bind(&req.content)
        .bind(&req.related_type)
        .bind(req.related_id)
        .bind(&req.metadata)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_user(
        pool: &PgPool,
        user_id: i64,
        query: &NotificationQuery,
    ) -> Result<(Vec<Notification>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        // Count query
        let mut count_qb = sqlx::QueryBuilder::new(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ",
        );
        count_qb.push_bind(user_id);

        // Data query
        let mut data_qb = sqlx::QueryBuilder::new(
            r#"SELECT notification_id, user_id, type AS "type", title, content, related_type, related_id,
                      is_read, read_at, created_at, metadata
               FROM notifications WHERE user_id = "#,
        );
        data_qb.push_bind(user_id);

        if let Some(ref t) = query.notification_type {
            count_qb.push(" AND type = ");
            count_qb.push_bind(t);
            data_qb.push(" AND type = ");
            data_qb.push_bind(t);
        }
        if let Some(is_read) = query.is_read {
            count_qb.push(" AND is_read = ");
            count_qb.push_bind(is_read);
            data_qb.push(" AND is_read = ");
            data_qb.push_bind(is_read);
        }
        if let Some(ref start) = query.start_time {
            count_qb.push(" AND created_at >= ");
            count_qb.push_bind(start);
            data_qb.push(" AND created_at >= ");
            data_qb.push_bind(start);
        }
        if let Some(ref end) = query.end_time {
            count_qb.push(" AND created_at < ");
            count_qb.push_bind(end);
            data_qb.push(" AND created_at < ");
            data_qb.push_bind(end);
        }

        data_qb.push(" ORDER BY created_at DESC LIMIT ");
        data_qb.push_bind(page_size as i64);
        data_qb.push(" OFFSET ");
        data_qb.push_bind(offset as i64);

        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(pool)
            .await?;
        let items = data_qb
            .build_query_as::<Notification>()
            .fetch_all(pool)
            .await?;

        Ok((items, total))
    }

    pub async fn mark_as_read(pool: &PgPool, notification_id: i64, user_id: i64) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE notifications SET is_read = true, read_at = now() WHERE notification_id = $1 AND user_id = $2 AND is_read = false",
        )
        .bind(notification_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_all_as_read(
        pool: &PgPool,
        user_id: i64,
        notification_type: Option<&str>,
    ) -> Result<u64> {
        let mut qb = sqlx::QueryBuilder::new(
            "UPDATE notifications SET is_read = true, read_at = now() WHERE user_id = ",
        );
        qb.push_bind(user_id);
        qb.push(" AND is_read = false");
        if let Some(t) = notification_type {
            qb.push(" AND type = ");
            qb.push_bind(t);
        }
        let result = qb.build().execute(pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn count_unread(pool: &PgPool, user_id: i64) -> Result<(i64, Vec<UnreadCountByType>)> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = false",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let by_type = sqlx::query_as::<_, UnreadCountByType>(
            r#"SELECT type AS notification_type, COUNT(*) AS count
               FROM notifications WHERE user_id = $1 AND is_read = false
               GROUP BY type"#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok((total, by_type))
    }

    /// 检查是否已有未读的同类型同关联实体通知（Worker 去重用）
    pub async fn has_unread_alert(
        pool: &PgPool,
        user_id: i64,
        notification_type: &str,
        related_type: &str,
        related_id: i64,
    ) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM notifications
                WHERE user_id = $1 AND type = $2 AND related_type = $3 AND related_id = $4 AND is_read = false
                LIMIT 1
            )"#,
        )
        .bind(user_id)
        .bind(notification_type)
        .bind(related_type)
        .bind(related_id)
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    /// 批量检查哪些用户已有未读的同类型同关联实体通知（Worker 去重用）
    /// 返回拥有未读告警的 user_id 列表
    pub async fn batch_has_unread_alert(
        pool: &PgPool,
        user_ids: &[i64],
        notification_type: &str,
        related_type: &str,
        related_id: i64,
    ) -> Result<Vec<i64>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"SELECT DISTINCT user_id FROM notifications
            WHERE user_id = ANY($1) AND type = $2 AND related_type = $3 AND related_id = $4 AND is_read = false"#,
        )
        .bind(user_ids)
        .bind(notification_type)
        .bind(related_type)
        .bind(related_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
