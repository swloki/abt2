//! AutoAction trait + ActionRegistry + 表达式映射引擎

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// AutoAction trait — 每个业务动作实现此接口
#[async_trait]
pub trait AutoAction: Send + Sync {
    async fn execute(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        inputs: HashMap<String, Value>,
    ) -> Result<ActionOutput>;
}

/// Action 执行输出
#[derive(Debug, Clone)]
pub struct ActionOutput {
    pub data: HashMap<String, Value>,
}

impl ActionOutput {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn with_data(data: HashMap<String, Value>) -> Self {
        Self { data }
    }
}

impl Default for ActionOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Action 参数字段定义
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub description: String,
}

/// Action 元数据定义
#[derive(Debug, Clone)]
pub struct ActionDef {
    pub name: String,
    pub label: String,
    pub description: String,
    pub inputs: Vec<FieldDef>,
    pub outputs: Vec<FieldDef>,
}

/// ActionRegistry — action 名称到实现的映射
pub struct ActionRegistry {
    actions: HashMap<String, Arc<dyn AutoAction>>,
    defs: Vec<ActionDef>,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            defs: Vec::new(),
        }
    }

    pub fn register(&mut self, def: ActionDef, action: Arc<dyn AutoAction>) {
        self.actions.insert(def.name.clone(), action);
        self.defs.push(def);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn AutoAction>> {
        self.actions.get(name)
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.actions.contains_key(name)
    }

    pub fn list_defs(&self) -> &[ActionDef] {
        &self.defs
    }

    /// 启动时校验：所有 active 模板引用的 action 已注册（Improvement 9）
    /// 任一未注册 → panic（fail-closed）
    pub fn validate_startup(&self, active_templates: &[crate::models::WorkflowTemplate]) {
        let mut unregistered = Vec::new();

        for template in active_templates {
            if let Some(graph_value) = &template.graph
                && let Ok(graph) =
                    serde_json::from_value::<crate::models::WorkflowGraph>(graph_value.clone())
            {
                for node in &graph.nodes {
                    if node.node_type == crate::models::NodeType::AutoTask
                        && let Some(action_name) =
                            node.config.get("action").and_then(|v| v.as_str())
                        && !self.is_registered(action_name)
                    {
                        unregistered.push(format!(
                            "template '{}' (id={}): node '{}' references unregistered action '{}'",
                            template.name, template.id, node.id, action_name
                        ));
                    }
                }
            }
        }

        if !unregistered.is_empty() {
            panic!(
                "FATAL: ActionRegistry startup validation failed — unregistered actions found:\n{}\
                \nRefusing to start. Register the missing actions or archive the templates.",
                unregistered.join("\n")
            );
        }
    }
}

impl crate::implt::graph_linter::ActionLookup for ActionRegistry {
    fn is_registered(&self, action_name: &str) -> bool {
        self.actions.contains_key(action_name)
    }
}

/// 表达式映射引擎：解析 `${variable.path}` 语法
pub fn resolve_mapping(
    mapping: &HashMap<String, String>,
    context: &serde_json::Value,
) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    for (target_key, expr) in mapping {
        if let Some(value) = resolve_expression(expr, context) {
            result.insert(target_key.clone(), value);
        }
    }
    result
}

/// 解析单个表达式 `${path.to.field}`
fn resolve_expression(expr: &str, context: &serde_json::Value) -> Option<Value> {
    let expr = expr.trim();
    if let Some(path) = expr.strip_prefix("${").and_then(|s| s.strip_suffix('}')) {
        resolve_path(path, context)
    } else {
        // 字面量
        Some(Value::String(expr.to_string()))
    }
}

/// 沿点号路径提取值
fn resolve_path(path: &str, context: &serde_json::Value) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = context;

    for part in &parts {
        match current {
            Value::Object(map) => {
                current = map.get(*part)?;
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// 应用 output_mapping：将 action 输出写入 context.variables
pub fn apply_output_mapping(
    output: &ActionOutput,
    output_mapping: &HashMap<String, String>,
    context: &mut serde_json::Value,
) {
    let variables = context
        .as_object_mut()
        .expect("context must be object")
        .entry("variables")
        .or_insert_with(|| Value::Object(Default::default()));

    for (action_field, context_key) in output_mapping {
        if let Some(value) = output.data.get(action_field) {
            variables
                .as_object_mut()
                .expect("variables must be object")
                .insert(context_key.clone(), value.clone());
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopAction;

    #[async_trait]
    impl AutoAction for NoopAction {
        async fn execute(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _inputs: HashMap<String, Value>,
        ) -> Result<ActionOutput> {
            Ok(ActionOutput::new())
        }
    }

    #[test]
    fn test_resolve_mapping() {
        let context = serde_json::json!({
            "entity_snapshot": {"product_id": 42, "quantity": 100},
            "variables": {"production_order_id": 99}
        });

        let mapping = HashMap::from([
            ("product_id".into(), "${entity_snapshot.product_id}".into()),
            ("order_id".into(), "${variables.production_order_id}".into()),
        ]);

        let result = resolve_mapping(&mapping, &context);
        assert_eq!(result.get("product_id").unwrap(), &Value::Number(42.into()));
        assert_eq!(result.get("order_id").unwrap(), &Value::Number(99.into()));
    }

    #[test]
    fn test_resolve_mapping_missing_field() {
        let context = serde_json::json!({"entity_snapshot": {}});
        let mapping = HashMap::from([(
            "product_id".into(),
            "${entity_snapshot.nonexistent}".into(),
        )]);
        let result = resolve_mapping(&mapping, &context);
        assert!(result.get("product_id").is_none());
    }

    #[test]
    fn test_apply_output_mapping() {
        let output = ActionOutput::with_data(HashMap::from([
            ("order_id".into(), Value::Number(123.into())),
            ("status".into(), Value::String("created".into())),
        ]));

        let mut context = serde_json::json!({
            "entity_snapshot": {},
            "variables": {},
            "join_progress": {}
        });

        let mapping = HashMap::from([
            ("order_id".into(), "production_order_id".into()),
            ("status".into(), "production_order_status".into()),
        ]);

        apply_output_mapping(&output, &mapping, &mut context);

        let vars = context.get("variables").unwrap();
        assert_eq!(vars.get("production_order_id").unwrap(), &Value::Number(123.into()));
        assert_eq!(
            vars.get("production_order_status").unwrap(),
            &Value::String("created".into())
        );
    }

    #[test]
    fn test_action_registry() {
        let mut registry = ActionRegistry::new();
        assert!(!registry.is_registered("test_action"));
        assert!(registry.get("test_action").is_none());
        assert!(registry.list_defs().is_empty());

        registry.register(
            ActionDef {
                name: "test_action".into(),
                label: "测试动作".into(),
                description: String::new(),
                inputs: vec![],
                outputs: vec![],
            },
            Arc::new(NoopAction),
        );
        assert!(registry.is_registered("test_action"));
        assert_eq!(registry.list_defs().len(), 1);
        assert_eq!(registry.list_defs()[0].name, "test_action");
    }

    #[test]
    fn test_resolve_literal() {
        let context = serde_json::json!({});
        let result = resolve_expression("hello", &context);
        assert_eq!(result, Some(Value::String("hello".into())));
    }

    #[test]
    fn test_resolve_deep_path() {
        let context = serde_json::json!({
            "entity_snapshot": {"product": {"id": 1, "name": "test"}}
        });
        let result = resolve_expression("${entity_snapshot.product.id}", &context);
        assert_eq!(result, Some(Value::Number(1.into())));
    }
}
