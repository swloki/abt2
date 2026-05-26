use crate::shared::types::PgExecutor;
use crate::shared::types::RepoResult;

use super::model::*;
use crate::shared::types::PaginatedResult;

pub struct NotificationRepo;

impl NotificationRepo {
    pub async fn create(&self, executor: PgExecutor<'_>, req: &CreateNotificationReq) -> RepoResult<i64> {
        let id = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO notifications (user_id, notification_type, title, content, related_type, related_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING notification_id"#,
        )
        .bind(req.user_id)
        .bind(req.notification_type.as_i16())
        .bind(&req.title)
        .bind(&req.content)
        .bind(&req.related_type)
        .bind(req.related_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn mark_read(&self, executor: PgExecutor<'_>, id: i64, user_id: i64) -> RepoResult<bool> {
        let rows = sqlx::query(
            "UPDATE notifications SET is_read = true, read_at = NOW() WHERE notification_id = $1 AND user_id = $2 AND is_read = false",
        )
        .bind(id)
        .bind(user_id)
        .execute(executor)
        .await?;
        Ok(rows.rows_affected() > 0)
    }

    pub async fn mark_all_read(&self, executor: PgExecutor<'_>, user_id: i64, notification_type: Option<NotificationType>) -> RepoResult<u64> {
        let rows = if let Some(nt) = notification_type {
            sqlx::query(
                "UPDATE notifications SET is_read = true, read_at = NOW() WHERE user_id = $1 AND is_read = false AND notification_type = $2",
            )
            .bind(user_id)
            .bind(nt.as_i16())
            .execute(executor)
            .await?
        } else {
            sqlx::query(
                "UPDATE notifications SET is_read = true, read_at = NOW() WHERE user_id = $1 AND is_read = false",
            )
            .bind(user_id)
            .execute(executor)
            .await?
        };
        Ok(rows.rows_affected())
    }

    pub async fn get_unread_count(&self, executor: PgExecutor<'_>, user_id: i64) -> RepoResult<i64> {
        let count = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = false",
        )
        .bind(user_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        user_id: i64,
        query: &NotificationQuery,
    ) -> RepoResult<PaginatedResult<Notification>> {
        let mut conditions = vec!["user_id = $1".to_string()];
        let mut param_idx = 2u32;

        let type_param = if let Some(nt) = query.notification_type {
            conditions.push(format!("notification_type = ${param_idx}"));
            param_idx += 1;
            Some(nt.as_i16())
        } else {
            None
        };

        let read_param = if let Some(is_read) = query.is_read {
            conditions.push(format!("is_read = ${param_idx}"));
            param_idx += 1;
            Some(is_read)
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM notifications WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql).bind(user_id);
        if let Some(v) = type_param { count_q = count_q.bind(v); }
        if let Some(v) = read_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let page = crate::shared::types::PageParams::new(query.page, query.page_size);

        let data_sql = format!(
            "SELECT notification_id, user_id, notification_type, title, content, related_type, related_id, is_read, read_at, created_at FROM notifications WHERE {where_clause} ORDER BY notification_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Notification>(&data_sql).bind(user_id);
        if let Some(v) = type_param { data_q = data_q.bind(v); }
        if let Some(v) = read_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}
