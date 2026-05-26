use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::Row;
use crate::shared::types::RepoResult;

pub struct IdempotencyRepo;

impl IdempotencyRepo {
    fn make_key(event_id: i64, handler_name: &str) -> String {
        format!("{event_id}:{handler_name}")
    }

    /// 尝试标记为 Processing。返回 true=首次, false=已完成或重复。
    /// 若记录已处于 Processed 状态，直接返回 false（跳过重复处理）。
    /// 若记录卡在 Processing（crash 残留），重置后允许重新处理。
    pub async fn check_and_mark(
        executor: &mut sqlx::postgres::PgConnection,
        event_id: i64,
        handler_name: &str,
    ) -> RepoResult<bool> {
        let key = Self::make_key(event_id, handler_name);

        // 1. 尝试 INSERT
        let result = sqlx::query(
            r#"
            INSERT INTO idempotency_records (idempotency_key, event_id, handler_name, status)
            VALUES ($1, $2, $3, 'Processing')
            ON CONFLICT (idempotency_key) DO NOTHING
            "#,
        )
        .bind(&key)
        .bind(event_id)
        .bind(handler_name)
        .execute(&mut *executor)
        .await?;

        if result.rows_affected() > 0 {
            return Ok(true);
        }

        // 2. INSERT 冲突 — 检查现有状态
        let row = sqlx::query(
            "SELECT status FROM idempotency_records WHERE idempotency_key = $1",
        )
        .bind(&key)
        .fetch_optional(&mut *executor)
        .await?;

        let Some(row) = row else { return Ok(false) };

        let status: String = row.try_get("status")?;
        match status.as_str() {
            "Processed" => Ok(false),
            "Processing" => {
                // Crash 残留 — 重置为 Processing 允许重试
                sqlx::query(
                    "UPDATE idempotency_records SET status = 'Processing' WHERE idempotency_key = $1",
                )
                .bind(&key)
                .execute(&mut *executor)
                .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// 标记为 Processed 并存储可选的 result
    pub async fn mark_processed(
        executor: &mut sqlx::postgres::PgConnection,
        event_id: i64,
        handler_name: &str,
        result: Option<&JsonValue>,
    ) -> RepoResult<()> {
        let key = Self::make_key(event_id, handler_name);
        sqlx::query(
            r#"
            UPDATE idempotency_records
            SET status = 'Processed', result = $1
            WHERE idempotency_key = $2
            "#,
        )
        .bind(result)
        .bind(&key)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 删除已过期的幂等记录，返回删除条数
    pub async fn cleanup_expired(
        executor: &mut sqlx::postgres::PgConnection,
        before: DateTime<Utc>,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            "DELETE FROM idempotency_records WHERE expires_at IS NOT NULL AND expires_at < $1",
        )
        .bind(before)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }
}
