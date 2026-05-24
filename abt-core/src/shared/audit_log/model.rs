use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use super::super::enums::audit::AuditAction;

/// 审计日志实体 — Append-only, 不可修改或删除
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub action: AuditAction,
    pub changes: Option<JsonValue>,
    pub operator_id: i64,
    pub context: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

/// 审计日志查询条件
#[derive(Debug, Clone, Default)]
pub struct AuditLogQuery {
    pub entity_type: Option<String>,
    pub operator_id: Option<i64>,
    pub action: Option<AuditAction>,
    pub time_range_start: Option<DateTime<Utc>>,
    pub time_range_end: Option<DateTime<Utc>>,
}
