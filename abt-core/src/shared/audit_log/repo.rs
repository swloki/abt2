use serde_json::Value as JsonValue;
use sqlx::{FromRow, Row};
use crate::shared::types::Result;

use super::model::{AuditLog, AuditLogQuery};
use crate::shared::enums::audit::AuditAction;

pub struct AuditLogRepo;

impl AuditLogRepo {
    /// INSERT 一条审计日志，返回生成的主键 id
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        entity_type: &str,
        entity_id: i64,
        action: AuditAction,
        changes: Option<&JsonValue>,
        operator_id: i64,
        context: Option<&JsonValue>,
    ) -> Result<i64> {
        let row = sqlx::query!(
            r#"
            INSERT INTO audit_logs (entity_type, entity_id, action, changes, operator_id, context)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            entity_type,
            entity_id,
            action.as_i16(),
            changes,
            operator_id,
            context,
        )
        .fetch_one(executor)
        .await?;

        Ok(row.id)
    }

    /// 动态条件分页查询 — 始终绑定 6 个过滤参数，用 SQL IS NULL OR 模式处理可选条件
    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &AuditLogQuery,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<AuditLog>, u64)> {
        let sql_base = "
            WHERE ($1::text IS NULL OR entity_type = $1)
              AND ($2::bigint IS NULL OR operator_id = $2)
              AND ($3::smallint IS NULL OR action = $3)
              AND ($4::timestamptz IS NULL OR created_at >= $4)
              AND ($5::timestamptz IS NULL OR created_at <= $5)
              AND ($6::bigint IS NULL OR entity_id = $6)
        ";

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM audit_logs {sql_base}");
        let count_row = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.entity_type.as_deref())
            .bind(q.operator_id)
            .bind(q.action.map(|a| a.as_i16()))
            .bind(q.time_range_start)
            .bind(q.time_range_end)
            .bind(q.entity_id)
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let data_sql = format!(
            "SELECT id, entity_type, entity_id, action, changes, operator_id, context, created_at \
             FROM audit_logs {sql_base} \
             ORDER BY created_at DESC \
             LIMIT $7 OFFSET $8"
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(data_sql))
            .bind(q.entity_type.as_deref())
            .bind(q.operator_id)
            .bind(q.action.map(|a| a.as_i16()))
            .bind(q.time_range_start)
            .bind(q.time_range_end)
            .bind(q.entity_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        let items: Vec<AuditLog> = rows
            .iter()
            .map(AuditLog::from_row)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok((items, total as u64))
    }
}
