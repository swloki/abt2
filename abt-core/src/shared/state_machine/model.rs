use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::SideEffect;

// ── 数据库实体 ──────────────────────────────────────────────

/// 状态定义 — 描述实体类型下每个状态的元数据
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StateDefinition {
    pub id: i64,
    pub entity_type: String,
    pub state_name: String,
    pub label: String,
    pub is_initial: bool,
    pub is_final: bool,
}

/// 状态转换规则 — 定义合法的 from_state -> to_state 路径
#[derive(Debug, Clone)]
pub struct StateTransitionDef {
    pub id: i64,
    pub entity_type: String,
    pub from_state: String,
    pub to_state: String,
    pub trigger_event: Option<DomainEventType>,
    pub guard_condition: Option<JsonValue>,
    pub side_effects: Vec<SideEffect>,
    pub sort_order: i32,
}

impl<'r> TryFrom<&'r PgRow> for StateTransitionDef {
    type Error = sqlx::Error;
    fn try_from(row: &'r PgRow) -> Result<Self, Self::Error> {
        let side_effects_json: Option<JsonValue> = row.try_get("side_effects")?;
        let side_effects = side_effects_json
            .and_then(|v| serde_json::from_value::<Vec<SideEffect>>(v).ok())
            .unwrap_or_default();

        let trigger_event: Option<i16> = row.try_get("trigger_event")?;
        let trigger_event = trigger_event.and_then(DomainEventType::from_i16);

        Ok(StateTransitionDef {
            id: row.try_get("id")?,
            entity_type: row.try_get("entity_type")?,
            from_state: row.try_get("from_state")?,
            to_state: row.try_get("to_state")?,
            trigger_event,
            guard_condition: row.try_get("guard_condition")?,
            side_effects,
            sort_order: row.try_get("sort_order")?,
        })
    }
}

/// 实体状态变更日志 — 追加写，不可修改
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityStateLog {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub from_state: Option<String>,
    pub to_state: String,
    pub transition_id: i64,
    pub operator_id: i64,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── 输入结构体 ──────────────────────────────────────────────

/// 状态定义输入 — 用于 configure 批量写入
#[derive(Debug, Clone)]
pub struct StateDefinitionInput {
    pub state_name: String,
    pub label: String,
    pub is_initial: bool,
    pub is_final: bool,
}

/// 转换规则输入 — 用于 configure 批量写入
#[derive(Debug, Clone)]
pub struct TransitionDefInput {
    pub from_state: String,
    pub to_state: String,
    pub trigger_event: Option<DomainEventType>,
    pub guard_condition: Option<JsonValue>,
    pub side_effects: Vec<SideEffect>,
    pub sort_order: i32,
}

/// 状态变更日志插入参数
#[derive(Debug, Clone)]
pub struct StateLogEntry<'a> {
    pub entity_type: &'a str,
    pub entity_id: i64,
    pub from_state: Option<&'a str>,
    pub to_state: &'a str,
    pub transition_id: i64,
    pub operator_id: i64,
    pub remark: Option<&'a str>,
}
