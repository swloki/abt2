use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// 审计日志写入参数
pub struct AuditEntry {
    pub operator_id: Option<i64>,
    pub target_type: &'static str,
    pub target_id: i64,
    pub action: &'static str,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub log_id: i64,
    pub operator_id: Option<i64>,
    pub operator_name: Option<String>,
    pub target_type: String,
    pub target_id: i64,
    pub action: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for AuditLog {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(AuditLog {
            log_id: row.try_get("log_id")?,
            operator_id: row.try_get("operator_id")?,
            operator_name: row.try_get("operator_name")?,
            target_type: row.try_get("target_type")?,
            target_id: row.try_get("target_id")?,
            action: row.try_get("action")?,
            old_value: row.try_get("old_value")?,
            new_value: row.try_get("new_value")?,
            created_at: row.try_get("created_at")?,
        })
    }
}
