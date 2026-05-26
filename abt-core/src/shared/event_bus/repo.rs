use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, Row};
use crate::shared::types::RepoResult;

use super::model::{DomainEvent, EventPublishRequest, EventQuery};

pub struct DomainEventRepo;

/// INSERT 参数包 — 避免 repo 函数参数过多
pub struct InsertParams<'a> {
    pub event_type: crate::shared::enums::event::DomainEventType,
    pub aggregate_type: &'a str,
    pub aggregate_id: i64,
    pub payload: &'a JsonValue,
    pub operator_id: i64,
    pub idempotency_key: String,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
}

impl<'a> InsertParams<'a> {
    pub fn from_request(
        req: &'a EventPublishRequest,
        operator_id: i64,
        trace_id: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            event_type: req.event_type,
            aggregate_type: &req.aggregate_type,
            aggregate_id: req.aggregate_id,
            payload: &req.payload,
            operator_id,
            idempotency_key: req.resolve_idempotency_key(),
            trace_id,
            request_id,
        }
    }
}

impl DomainEventRepo {
    /// INSERT ON CONFLICT (idempotency_key) DO NOTHING → 返回 event id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        params: &InsertParams<'_>,
    ) -> RepoResult<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO domain_events
                (event_type, event_version, aggregate_type, aggregate_id,
                 payload, operator_id, idempotency_key, trace_id, request_id, status)
            VALUES ($1, 1, $2, $3, $4, $5, $6, $7, $8, 1)
            ON CONFLICT (idempotency_key) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(params.event_type)
        .bind(params.aggregate_type)
        .bind(params.aggregate_id)
        .bind(params.payload)
        .bind(params.operator_id)
        .bind(&params.idempotency_key)
        .bind(&params.trace_id)
        .bind(&params.request_id)
        .fetch_optional(&mut *executor)
        .await?;

        match row {
            Some(r) => Ok(r.try_get("id")?),
            None => {
                // ON CONFLICT DO NOTHING — 返回已有 id
                let existing = sqlx::query(
                    "SELECT id FROM domain_events WHERE idempotency_key = $1",
                )
                .bind(&params.idempotency_key)
                .fetch_one(&mut *executor)
                .await?;
                Ok(existing.try_get("id")?)
            }
        }
    }

    /// NOTIFY domain_event channel
    pub async fn notify(
        executor: &mut sqlx::postgres::PgConnection,
        event_id: i64,
    ) -> RepoResult<()> {
        sqlx::query(&format!("NOTIFY domain_event, '{event_id}'"))
            .execute(executor)
            .await?;
        Ok(())
    }

    /// UPDATE status=Processed, processed_at=NOW() WHERE id = ANY($1) AND status != Processed
    pub async fn mark_processed(
        executor: &mut sqlx::postgres::PgConnection,
        ids: &[i64],
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE domain_events
            SET status = 3, processed_at = NOW()
            WHERE id = ANY($1) AND status != 3
            "#,
        )
        .bind(ids)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 标记失败: retry_count+1, status=Failed, failure_reason
    pub async fn mark_failed(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        reason: &str,
    ) -> RepoResult<()> {
        sqlx::query(
            r#"
            UPDATE domain_events
            SET retry_count = retry_count + 1,
                status = 4,
                failure_reason = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(reason)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 标记为死信
    pub async fn mark_dead_letter(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        reason: &str,
    ) -> RepoResult<()> {
        sqlx::query(
            r#"
            UPDATE domain_events
            SET status = 5, failure_reason = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(reason)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 将卡在 Processing 超过 min_minutes 分钟的事件重置为 Pending
    /// min_minutes=0 时重置所有 Processing 事件（用于优雅停机）
    pub async fn reset_stale_processing(
        executor: &mut sqlx::postgres::PgConnection,
        min_minutes: i32,
    ) -> RepoResult<u64> {
        let result = if min_minutes > 0 {
            sqlx::query(
                r#"
                UPDATE domain_events
                SET status = 1
                WHERE status = 2
                  AND created_at < NOW() - ($1 || ' minutes')::interval
                "#,
            )
            .bind(min_minutes)
            .execute(executor)
            .await?
        } else {
            sqlx::query(
                "UPDATE domain_events SET status = 1 WHERE status = 2",
            )
            .execute(executor)
            .await?
        };
        Ok(result.rows_affected())
    }

    /// 重置为 Pending: status=Pending, retry_count=0
    pub async fn reset_to_pending(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<()> {
        sqlx::query(
            r#"
            UPDATE domain_events
            SET status = 1, retry_count = 0, failure_reason = NULL, processed_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 动态条件分页查询 — IS NULL OR 模式
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &EventQuery,
        limit: i64,
        offset: i64,
    ) -> RepoResult<(Vec<DomainEvent>, u64)> {
        let sql_base = "
            WHERE ($1::text IS NULL OR aggregate_type = $1)
              AND ($2::smallint IS NULL OR event_type = $2)
              AND ($3::smallint IS NULL OR status = $3)
              AND ($4::timestamptz IS NULL OR created_at >= $4)
        ";

        let count_sql = format!("SELECT COUNT(*) AS cnt FROM domain_events {sql_base}");
        let count_row = sqlx::query(&count_sql)
            .bind(q.aggregate_type.as_deref())
            .bind(q.event_type.map(|t| t.as_i16()))
            .bind(q.status.map(|s| s.as_i16()))
            .bind(q.since)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        let data_sql = format!(
            "SELECT id, event_type, event_version, aggregate_type, aggregate_id, \
             payload, operator_id, idempotency_key, trace_id, request_id, \
             status, retry_count, failure_reason, processed_at, created_at \
             FROM domain_events {sql_base} \
             ORDER BY created_at DESC \
             LIMIT $5 OFFSET $6"
        );
        let rows = sqlx::query(&data_sql)
            .bind(q.aggregate_type.as_deref())
            .bind(q.event_type.map(|t| t.as_i16()))
            .bind(q.status.map(|s| s.as_i16()))
            .bind(q.since)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        let items: Vec<DomainEvent> = rows
            .iter()
            .map(DomainEvent::from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((items, total as u64))
    }

    /// 获取待处理事件（FETCH FOR UPDATE SKIP LOCKED）— 用于 EventProcessor
    pub async fn fetch_pending(
        executor: &mut sqlx::postgres::PgConnection,
        batch_size: i32,
    ) -> RepoResult<Vec<DomainEvent>> {
        let rows = sqlx::query(
            r#"
            UPDATE domain_events SET status = 2
            WHERE id IN (
                SELECT id FROM domain_events
                WHERE status = 1
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, event_type, event_version, aggregate_type, aggregate_id,
                      payload, operator_id, idempotency_key, trace_id, request_id,
                      status, retry_count, failure_reason, processed_at, created_at
            "#,
        )
        .bind(batch_size)
        .fetch_all(executor)
        .await?;

        let items: Vec<DomainEvent> = rows
            .iter()
            .map(DomainEvent::from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    /// 获取失败事件（可重试）— status=Failed 或 status=Pending 且 created_at 超过阈值
    pub async fn fetch_retryable(
        executor: &mut sqlx::postgres::PgConnection,
        max_retries: i32,
        batch_size: i32,
    ) -> RepoResult<Vec<DomainEvent>> {
        let rows = sqlx::query(
            r#"
            UPDATE domain_events SET status = 2
            WHERE id IN (
                SELECT id FROM domain_events
                WHERE status = 4 AND retry_count <= $1
                ORDER BY created_at ASC
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, event_type, event_version, aggregate_type, aggregate_id,
                      payload, operator_id, idempotency_key, trace_id, request_id,
                      status, retry_count, failure_reason, processed_at, created_at
            "#,
        )
        .bind(max_retries)
        .bind(batch_size)
        .fetch_all(executor)
        .await?;

        let items: Vec<DomainEvent> = rows
            .iter()
            .map(DomainEvent::from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    /// 归档（删除）死信事件 — status=DeadLetter AND created_at < before
    pub async fn archive_dead_letters(
        executor: &mut sqlx::postgres::PgConnection,
        before: DateTime<Utc>,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            "DELETE FROM domain_events WHERE status = 5 AND created_at < $1",
        )
        .bind(before)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 查询死信事件（分页）
    pub async fn query_dead_letters(
        executor: &mut sqlx::postgres::PgConnection,
        limit: i64,
        offset: i64,
    ) -> RepoResult<(Vec<DomainEvent>, u64)> {
        let count_row = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM domain_events WHERE status = 5",
        )
        .fetch_one(&mut *executor)
        .await?;
        let total: i64 = count_row.try_get("cnt")?;

        let rows = sqlx::query(
            r#"
            SELECT id, event_type, event_version, aggregate_type, aggregate_id,
                   payload, operator_id, idempotency_key, trace_id, request_id,
                   status, retry_count, failure_reason, processed_at, created_at
            FROM domain_events
            WHERE status = 5
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<DomainEvent> = rows
            .iter()
            .map(DomainEvent::from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((items, total as u64))
    }
}
