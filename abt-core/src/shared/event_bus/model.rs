use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use crate::shared::enums::event::{DomainEventType, EventStatus};

/// 领域事件实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DomainEvent {
    pub id: i64,
    pub event_type: DomainEventType,
    pub event_version: i32,
    pub aggregate_type: String,
    pub aggregate_id: i64,
    pub payload: JsonValue,
    pub operator_id: i64,
    pub idempotency_key: String,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
    pub status: EventStatus,
    pub retry_count: i32,
    pub failure_reason: Option<String>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 事件发布请求
#[derive(Debug, Clone)]
pub struct EventPublishRequest {
    pub event_type: DomainEventType,
    pub aggregate_type: String,
    pub aggregate_id: i64,
    pub payload: JsonValue,
    /// None 时自动生成 "{aggregate_type}:{aggregate_id}:{event_type}"
    pub idempotency_key: Option<String>,
}

impl EventPublishRequest {
    /// 获取幂等键，如果未设置则自动生成
    pub fn resolve_idempotency_key(&self) -> String {
        self.idempotency_key.clone().unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                self.aggregate_type,
                self.aggregate_id,
                self.event_type.as_i16()
            )
        })
    }
}

/// 事件查询条件
#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    pub aggregate_type: Option<String>,
    pub event_type: Option<DomainEventType>,
    pub status: Option<EventStatus>,
    pub since: Option<DateTime<Utc>>,
}
