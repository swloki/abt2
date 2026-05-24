use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 幂等记录 — 保证事件处理的 exactly-once 语义
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub id: i64,
    pub idempotency_key: String,
    pub event_id: i64,
    pub handler_name: String,
    pub status: String,
    pub result: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, PgRow> for IdempotencyRecord {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(IdempotencyRecord {
            id: row.try_get("id")?,
            idempotency_key: row.try_get("idempotency_key")?,
            event_id: row.try_get("event_id")?,
            handler_name: row.try_get("handler_name")?,
            status: row.try_get("status")?,
            result: row.try_get("result")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
        })
    }
}
