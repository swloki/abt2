//! 工作流数据模型
//!
//! 包含工作流模板、实例、任务、历史的模型定义，
//! 以及 Graph JSONB 类型、Condition AST。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// 状态枚举
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStatus {
    Draft,
    Active,
    Archived,
}

impl TemplateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl std::fmt::Display for TemplateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TemplateStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown template status: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Running,
    Completed,
    Rejected,
    Suspended,
    Cancelled,
    Terminated,
}

impl InstanceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Suspended => "suspended",
            Self::Cancelled => "cancelled",
            Self::Terminated => "terminated",
        }
    }
}

impl std::fmt::Display for InstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for InstanceStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "rejected" => Ok(Self::Rejected),
            "suspended" => Ok(Self::Suspended),
            "cancelled" => Ok(Self::Cancelled),
            "terminated" => Ok(Self::Terminated),
            _ => Err(format!("unknown instance status: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowTaskStatus {
    Pending,
    Completed,
    Rejected,
    Delegated,
    TimedOut,
    Cancelled,
}

impl WorkflowTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Delegated => "delegated",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for WorkflowTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for WorkflowTaskStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "rejected" => Ok(Self::Rejected),
            "delegated" => Ok(Self::Delegated),
            "timed_out" => Ok(Self::TimedOut),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("unknown workflow task status: {s}")),
        }
    }
}

// ============================================================================
// Graph JSONB 类型
// ============================================================================

/// 当前引擎支持的 graph_version
pub const CURRENT_GRAPH_VERSION: u32 = 1;

/// 完整的流程图定义，存储为 JSONB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// 模式版本号，用于引擎演进时的反序列化分发
    pub graph_version: u32,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowGraph {
    pub fn validate_version(&self) -> Result<(), String> {
        if self.graph_version != CURRENT_GRAPH_VERSION {
            return Err(format!(
                "unsupported graph_version: {} (current engine supports: {})",
                self.graph_version, CURRENT_GRAPH_VERSION
            ));
        }
        Ok(())
    }

    pub fn find_node(&self, node_id: &str) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    pub fn find_outgoing_edges(&self, from_node: &str) -> Vec<&WorkflowEdge> {
        self.edges
            .iter()
            .filter(|e| e.from == from_node)
            .collect()
    }

    pub fn find_incoming_edges(&self, to_node: &str) -> Vec<&WorkflowEdge> {
        self.edges.iter().filter(|e| e.to == to_node).collect()
    }

    pub fn find_start_node(&self) -> Option<&WorkflowNode> {
        self.nodes
            .iter()
            .find(|n| n.node_type == NodeType::Start)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "node_type")]
    pub node_type: NodeType,
    pub name: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Start,
    End,
    Approval,
    AutoTask,
    Join,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start => write!(f, "start"),
            Self::End => write!(f, "end"),
            Self::Approval => write!(f, "approval"),
            Self::AutoTask => write!(f, "auto_task"),
            Self::Join => write!(f, "join"),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            "approval" => Ok(Self::Approval),
            "auto_task" => Ok(Self::AutoTask),
            "join" => Ok(Self::Join),
            _ => Err(format!("unknown node type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub condition: Option<Condition>,
}

// ============================================================================
// Condition AST
// ============================================================================

#[derive(Debug, Clone)]
pub enum Condition {
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
    FieldCompare {
        field: String,
        op: CompareOp,
        value: serde_json::Value,
    },
    Always,
    Never,
}

impl Serialize for Condition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Condition::And(children) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "and")?;
                map.serialize_entry("children", children)?;
                map.end()
            }
            Condition::Or(children) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "or")?;
                map.serialize_entry("children", children)?;
                map.end()
            }
            Condition::Not(inner) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "not")?;
                map.serialize_entry("child", inner)?;
                map.end()
            }
            Condition::FieldCompare { field, op, value } => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "field_compare")?;
                map.serialize_entry("field", field)?;
                map.serialize_entry("op", op)?;
                map.serialize_entry("value", value)?;
                map.end()
            }
            Condition::Always => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("type", "always")?;
                map.end()
            }
            Condition::Never => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("type", "never")?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ConditionVisitor;

        impl<'de> serde::de::Visitor<'de> for ConditionVisitor {
            type Value = Condition;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a condition object with a 'type' field")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut type_: Option<String> = None;
                let mut children: Option<Vec<Condition>> = None;
                let mut child: Option<Condition> = None;
                let mut field: Option<String> = None;
                let mut op: Option<CompareOp> = None;
                let mut value: Option<serde_json::Value> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "type" => type_ = Some(map.next_value()?),
                        "children" => children = Some(map.next_value()?),
                        "child" => child = Some(map.next_value()?),
                        "field" => field = Some(map.next_value()?),
                        "op" => op = Some(map.next_value()?),
                        "value" => value = Some(map.next_value()?),
                        _ => {
                            map.next_value::<serde_json::Value>()?;
                        }
                    }
                }

                let type_ = type_.ok_or_else(|| serde::de::Error::missing_field("type"))?;

                match type_.as_str() {
                    "and" => Ok(Condition::And(children.unwrap_or_default())),
                    "or" => Ok(Condition::Or(children.unwrap_or_default())),
                    "not" => {
                        let inner =
                            child.ok_or_else(|| serde::de::Error::missing_field("child"))?;
                        Ok(Condition::Not(Box::new(inner)))
                    }
                    "field_compare" => {
                        let field =
                            field.ok_or_else(|| serde::de::Error::missing_field("field"))?;
                        let value =
                            value.ok_or_else(|| serde::de::Error::missing_field("value"))?;
                        Ok(Condition::FieldCompare {
                            field,
                            op: op.unwrap_or_default(),
                            value,
                        })
                    }
                    "always" => Ok(Condition::Always),
                    "never" => Ok(Condition::Never),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["and", "or", "not", "field_compare", "always", "never"],
                    )),
                }
            }
        }

        deserializer.deserialize_map(ConditionVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompareOp {
    #[default]
    Eq,
    Neq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    In,
}

impl CompareOp {
    pub fn evaluate(&self, left: &serde_json::Value, right: &serde_json::Value) -> bool {
        match self {
            Self::Eq => left == right,
            Self::Neq => left != right,
            Self::Gt => compare_values(left, right).is_some_and(|o| o.is_gt()),
            Self::GtEq => compare_values(left, right).is_some_and(|o| o.is_ge()),
            Self::Lt => compare_values(left, right).is_some_and(|o| o.is_lt()),
            Self::LtEq => compare_values(left, right).is_some_and(|o| o.is_le()),
            Self::In => {
                if let serde_json::Value::Array(arr) = right {
                    arr.contains(left)
                } else {
                    false
                }
            }
        }
    }
}

use std::cmp::Ordering;

fn compare_values(
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> Option<Ordering> {
    match (left, right) {
        (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
            if let (Some(ln), Some(rn)) = (l.as_f64(), r.as_f64()) {
                ln.partial_cmp(&rn)
            } else {
                None
            }
        }
        (serde_json::Value::String(l), serde_json::Value::String(r)) => {
            Some(l.cmp(r))
        }
        _ => None,
    }
}

/// 条件求值上下文
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    pub entity_snapshot: serde_json::Value,
    pub variables: HashMap<String, serde_json::Value>,
}

impl EvaluationContext {
    pub fn get_value(&self, field: &str) -> Option<&serde_json::Value> {
        if let Some(key) = field.strip_prefix("entity_snapshot.") {
            self.entity_snapshot.get(key)
        } else if let Some(key) = field.strip_prefix("variables.") {
            self.variables.get(key)
        } else {
            self.variables.get(field)
        }
    }
}

/// Condition AST 求值
pub fn evaluate_condition(condition: &Condition, ctx: &EvaluationContext) -> bool {
    match condition {
        Condition::Always => true,
        Condition::Never => false,
        Condition::And(conditions) => conditions
            .iter()
            .all(|c| evaluate_condition(c, ctx)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|c| evaluate_condition(c, ctx)),
        Condition::Not(inner) => !evaluate_condition(inner, ctx),
        Condition::FieldCompare { field, op, value } => {
            match ctx.get_value(field) {
                Some(actual) => op.evaluate(actual, value),
                None => false,
            }
        }
    }
}

// ============================================================================
// 触发器事件定义
// ============================================================================

/// 触发事件定义（后端静态注册，暴露给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEventDef {
    pub name: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub bound_template_id: i64,
    pub bound_template_name: String,
}

/// 所有可用的触发事件 (name, label, description)
pub fn all_trigger_events() -> &'static [(&'static str, &'static str, &'static str)] {
    &[("inventory_updated", "库存变更", "当库存数量发生增减时触发")]
}

// ============================================================================
// 数据库模型
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowTemplate {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub version: i32,
    pub status: String,
    pub graph: Option<serde_json::Value>,
    pub graph_checksum: Option<String>,
    pub trigger_event: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowInstance {
    pub id: i64,
    pub template_id: i64,
    pub template_version: Option<i32>,
    pub entity_type: String,
    pub entity_id: i64,
    pub status: String,
    pub frozen_graph: Option<serde_json::Value>,
    pub context: Option<serde_json::Value>,
    pub suspended_reason: Option<serde_json::Value>,
    pub initiator_id: i64,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_advanced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowTask {
    pub id: i64,
    pub instance_id: i64,
    pub node_id: String,
    pub prev_task_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub status: String,
    pub action: Option<String>,
    pub timeout_action: Option<String>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub remind_at: Option<chrono::DateTime<chrono::Utc>>,
    pub result: Option<serde_json::Value>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowHistory {
    pub id: i64,
    pub instance_id: i64,
    pub task_id: Option<i64>,
    pub node_id: Option<String>,
    pub event_type: String,
    pub actor_id: Option<i64>,
    pub payload: Option<serde_json::Value>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ============================================================================
// Event Type 常量
// ============================================================================

pub mod event_type {
    pub const INSTANCE_STARTED: &str = "instance_started";
    pub const INSTANCE_COMPLETED: &str = "instance_completed";
    pub const INSTANCE_CANCELLED: &str = "instance_cancelled";
    pub const TASK_COMPLETED: &str = "task_completed";
    pub const TASK_REJECTED: &str = "task_rejected";
    pub const TASK_DELEGATED: &str = "task_delegated";
    pub const MULTI_APPROVAL_WAITING: &str = "multi_approval_waiting";
    pub const NODE_ENTERED: &str = "node_entered";
    pub const AUTO_TASK_COMPLETED: &str = "auto_task_completed";
    pub const AUTO_TASK_RETRIED: &str = "auto_task_retried";
    pub const JOIN_WAITING: &str = "join_waiting";
    pub const JOIN_COMPLETED: &str = "join_completed";
    pub const ENTITY_CHANGED: &str = "entity_changed";
    pub const TIMEOUT_ACTION: &str = "timeout_action";
    pub const REMINDER: &str = "reminder";
    pub const HOOK_EXECUTED: &str = "hook_executed";
}

/// 系统用户 ID（Worker 使用）
pub const SYSTEM_USER_ID: i64 = 0;

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_graph_roundtrip() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode {
                    id: "start".into(),
                    node_type: NodeType::Start,
                    name: "开始".into(),
                    config: json!(null),
                },
                WorkflowNode {
                    id: "end".into(),
                    node_type: NodeType::End,
                    name: "结束".into(),
                    config: json!(null),
                },
            ],
            edges: vec![WorkflowEdge {
                from: "start".into(),
                to: "end".into(),
                condition: None,
            }],
        };

        let json_str = serde_json::to_string(&graph).unwrap();
        let deserialized: WorkflowGraph = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.graph_version, 1);
        assert_eq!(deserialized.nodes.len(), 2);
        assert_eq!(deserialized.edges.len(), 1);
        assert_eq!(deserialized.nodes[0].node_type, NodeType::Start);
    }

    #[test]
    fn test_graph_version_validation() {
        let graph = WorkflowGraph {
            graph_version: 2,
            nodes: vec![],
            edges: vec![],
        };
        assert!(graph.validate_version().is_err());
        assert!(graph.validate_version().unwrap_err().contains("unsupported"));
    }

    #[test]
    fn test_graph_version_missing() {
        let json = json!({
            "nodes": [],
            "edges": []
        });
        let result: Result<WorkflowGraph, _> = serde_json::from_value(json);
        // graph_version is required, should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_condition_always_never() {
        let ctx = EvaluationContext::default();
        assert!(evaluate_condition(&Condition::Always, &ctx));
        assert!(!evaluate_condition(&Condition::Never, &ctx));
    }

    #[test]
    fn test_condition_field_compare() {
        let ctx = EvaluationContext {
            entity_snapshot: json!({ "amount": 50000 }),
            variables: HashMap::new(),
        };

        let cond = Condition::FieldCompare {
            field: "entity_snapshot.amount".into(),
            op: CompareOp::Gt,
            value: json!(10000),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_condition_and_or_not() {
        let ctx = EvaluationContext {
            entity_snapshot: json!({ "amount": 5000 }),
            variables: HashMap::new(),
        };

        let cond = Condition::And(vec![
            Condition::FieldCompare {
                field: "entity_snapshot.amount".into(),
                op: CompareOp::Gt,
                value: json!(1000),
            },
            Condition::Not(Box::new(Condition::FieldCompare {
                field: "entity_snapshot.amount".into(),
                op: CompareOp::Gt,
                value: json!(100000),
            })),
        ]);
        assert!(evaluate_condition(&cond, &ctx));

        let cond_or = Condition::Or(vec![
            Condition::Never,
            Condition::FieldCompare {
                field: "entity_snapshot.amount".into(),
                op: CompareOp::Eq,
                value: json!(5000),
            },
        ]);
        assert!(evaluate_condition(&cond_or, &ctx));
    }

    #[test]
    fn test_condition_missing_field() {
        let ctx = EvaluationContext {
            entity_snapshot: json!({}),
            variables: HashMap::new(),
        };
        let cond = Condition::FieldCompare {
            field: "entity_snapshot.nonexistent".into(),
            op: CompareOp::Eq,
            value: json!(1),
        };
        assert!(!evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_condition_deserialization() {
        let json = json!({
            "type": "and",
            "children": [
                {"type": "always"},
                {"type": "field_compare", "field": "amount", "op": "gt", "value": 100}
            ]
        });
        let cond: Condition = serde_json::from_value(json).unwrap();
        match cond {
            Condition::And(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected And"),
        }
    }

    #[test]
    fn test_full_workflow_graph() {
        let graph_json = json!({
            "graph_version": 1,
            "nodes": [
                {"id": "start", "node_type": "start", "name": "开始", "config": null},
                {"id": "approval1", "node_type": "approval", "name": "主管审批",
                 "config": {"assignee_type": "role", "assignee_value": "manager", "multi_approval": "any", "reject_action": "terminate", "fallback_assignee": 1}},
                {"id": "auto1", "node_type": "auto_task", "name": "生成定单",
                 "config": {"action": "create_order", "retryable": true, "input_mapping": {}, "output_mapping": {}}},
                {"id": "end", "node_type": "end", "name": "结束", "config": null}
            ],
            "edges": [
                {"from": "start", "to": "approval1"},
                {"from": "approval1", "to": "auto1"},
                {"from": "auto1", "to": "end"}
            ]
        });

        let graph: WorkflowGraph = serde_json::from_value(graph_json).unwrap();
        assert!(graph.validate_version().is_ok());
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.edges.len(), 3);

        let start = graph.find_start_node().unwrap();
        assert_eq!(start.id, "start");

        let outgoing = graph.find_outgoing_edges("start");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to, "approval1");
    }

    #[test]
    fn test_parallel_graph() {
        let graph_json = json!({
            "graph_version": 1,
            "nodes": [
                {"id": "start", "node_type": "start", "name": "开始", "config": null},
                {"id": "approval1", "node_type": "approval", "name": "审批1",
                 "config": {"assignee_type": "role", "assignee_value": "a", "multi_approval": "any", "reject_action": "terminate"}},
                {"id": "approval2", "node_type": "approval", "name": "审批2",
                 "config": {"assignee_type": "role", "assignee_value": "b", "multi_approval": "any", "reject_action": "terminate"}},
                {"id": "join1", "node_type": "join", "name": "汇聚", "config": {"join_strategy": "all"}},
                {"id": "end", "node_type": "end", "name": "结束", "config": null}
            ],
            "edges": [
                {"from": "start", "to": "approval1"},
                {"from": "start", "to": "approval2"},
                {"from": "approval1", "to": "join1"},
                {"from": "approval2", "to": "join1"},
                {"from": "join1", "to": "end"}
            ]
        });

        let graph: WorkflowGraph = serde_json::from_value(graph_json).unwrap();
        let incoming = graph.find_incoming_edges("join1");
        assert_eq!(incoming.len(), 2);
    }

    #[test]
    fn test_node_type_display_roundtrip() {
        assert_eq!(NodeType::Start.to_string(), "start");
        assert_eq!(NodeType::Approval.to_string(), "approval");
        assert_eq!("auto_task".parse::<NodeType>().unwrap(), NodeType::AutoTask);
    }

    #[test]
    fn test_status_roundtrip() {
        assert_eq!(TemplateStatus::Draft.as_str(), "draft");
        assert_eq!("active".parse::<TemplateStatus>().unwrap(), TemplateStatus::Active);
        assert_eq!(InstanceStatus::Running.as_str(), "running");
        assert_eq!("suspended".parse::<InstanceStatus>().unwrap(), InstanceStatus::Suspended);
        assert_eq!(WorkflowTaskStatus::TimedOut.as_str(), "timed_out");
        assert_eq!("pending".parse::<WorkflowTaskStatus>().unwrap(), WorkflowTaskStatus::Pending);
    }

    #[test]
    fn test_compare_op() {
        assert!(CompareOp::Gt.evaluate(&json!(100), &json!(50)));
        assert!(!CompareOp::Gt.evaluate(&json!(50), &json!(100)));
        assert!(CompareOp::Eq.evaluate(&json!("hello"), &json!("hello")));
        assert!(CompareOp::In.evaluate(&json!(1), &json!([1, 2, 3])));
        assert!(!CompareOp::In.evaluate(&json!(4), &json!([1, 2, 3])));
    }

    #[test]
    fn test_compare_op_all_variants() {
        assert!(CompareOp::Neq.evaluate(&json!(1), &json!(2)));
        assert!(!CompareOp::Neq.evaluate(&json!(1), &json!(1)));
        assert!(CompareOp::GtEq.evaluate(&json!(5), &json!(5)));
        assert!(CompareOp::GtEq.evaluate(&json!(6), &json!(5)));
        assert!(!CompareOp::GtEq.evaluate(&json!(4), &json!(5)));
        assert!(CompareOp::Lt.evaluate(&json!(4), &json!(5)));
        assert!(!CompareOp::Lt.evaluate(&json!(5), &json!(5)));
        assert!(CompareOp::LtEq.evaluate(&json!(5), &json!(5)));
        assert!(!CompareOp::LtEq.evaluate(&json!(6), &json!(5)));
        assert!(!CompareOp::In.evaluate(&json!(1), &json!("not_array")));
        assert!(!CompareOp::In.evaluate(&json!(1), &json!([])));
    }

    #[test]
    fn test_evaluation_context_bare_field() {
        let ctx = EvaluationContext {
            entity_snapshot: json!({"a": 1}),
            variables: HashMap::from([("b".into(), json!(2))]),
        };
        assert_eq!(ctx.get_value("b"), Some(&json!(2)));
        assert_eq!(ctx.get_value("entity_snapshot.a"), Some(&json!(1)));
        assert_eq!(ctx.get_value("variables.b"), Some(&json!(2)));
        assert_eq!(ctx.get_value("nonexistent"), None);
    }

    #[test]
    fn test_nested_condition_depth_3() {
        let ctx = EvaluationContext {
            entity_snapshot: json!({ "level": 3 }),
            variables: HashMap::new(),
        };
        let cond = Condition::Not(Box::new(Condition::Or(vec![
            Condition::Never,
            Condition::And(vec![
                Condition::FieldCompare {
                    field: "entity_snapshot.level".into(),
                    op: CompareOp::Eq,
                    value: json!(3),
                },
                Condition::Always,
            ]),
        ])));
        // Not(Or(Never, And(Eq 3==3, Always))) = Not(Or(false, true)) = Not(true) = false
        assert!(!evaluate_condition(&cond, &ctx));
    }
}
