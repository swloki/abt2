use serde::{Deserialize, Serialize};

/// SideEffect 枚举 — 存储为 JSONB，类型安全
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "params")]
pub enum SideEffect {
    #[serde(rename = "publish_event")]
    PublishEvent {
        event_type: String,
        payload_template: serde_json::Value,
    },
    #[serde(rename = "notify")]
    Notify {
        role_ids: Vec<i64>,
        template: String,
    },
    #[serde(rename = "trigger_workflow")]
    TriggerWorkflow {
        definition_id: i64,
    },
    #[serde(rename = "update_field")]
    UpdateField {
        field: String,
        value_template: serde_json::Value,
    },
}
