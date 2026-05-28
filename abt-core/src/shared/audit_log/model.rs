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

/// 记录审计日志请求参数
#[derive(Debug, Clone)]
pub struct RecordAuditLogReq {
    pub entity_type: &'static str,
    pub entity_id: i64,
    pub action: AuditAction,
    pub changes: Option<JsonValue>,
    pub context: Option<JsonValue>,
}

impl RecordAuditLogReq {
    pub fn new(entity_type: &'static str, entity_id: i64, action: AuditAction) -> Self {
        Self {
            entity_type,
            entity_id,
            action,
            changes: None,
            context: None,
        }
    }

    pub fn with_changes(mut self, changes: JsonValue) -> Self {
        self.changes = Some(changes);
        self
    }
}
