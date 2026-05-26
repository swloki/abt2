//! Graph Linter — 模板发布时的图结构校验
//!
//! 纯函数模块，不依赖数据库。

use std::collections::HashSet;

use anyhow::{bail, Result};

use crate::workflow::model::{NodeType, WorkflowGraph};

/// 检查 action 是否已注册（由 ActionRegistry 提供）
pub trait ActionLookup: Send + Sync {
    fn is_registered(&self, action_name: &str) -> bool;
}

/// 校验流程图定义是否合法
pub fn lint_graph(graph: &WorkflowGraph, action_lookup: &dyn ActionLookup) -> Result<()> {
    // 规则 1: 有且仅有一个 start，至少一个 end
    let start_count = graph
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Start)
        .count();
    if start_count == 0 {
        bail!("graph must have exactly one start node");
    }
    if start_count > 1 {
        bail!("graph must have exactly one start node, found {start_count}");
    }

    let end_count = graph
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::End)
        .count();
    if end_count == 0 {
        bail!("graph must have at least one end node");
    }

    // 构建 node_id set 用于校验边引用
    let node_ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // 校验边引用的节点存在
    for edge in &graph.edges {
        if !node_ids.contains(edge.from.as_str()) {
            bail!("edge references unknown source node: {}", edge.from);
        }
        if !node_ids.contains(edge.to.as_str()) {
            bail!("edge references unknown target node: {}", edge.to);
        }
    }

    // 规则 2: DFS 环检测
    detect_cycles(graph)?;

    // 校验每个节点
    for node in &graph.nodes {
        match node.node_type {
            NodeType::Start | NodeType::End => {}
            NodeType::Approval => {
                // 规则 3: approval 节点必须有 fallback_assignee
                let config = &node.config;
                if config.get("fallback_assignee").is_none() {
                    bail!(
                        "approval node '{}' must have fallback_assignee",
                        node.id
                    );
                }
            }
            NodeType::AutoTask => {
                // 规则 4: auto_task 节点必须有 action，且 action 已注册
                let config = &node.config;
                if let Some(action) = config.get("action").and_then(|v| v.as_str()) {
                    if !action_lookup.is_registered(action) {
                        bail!(
                            "auto_task node '{}' references unregistered action: '{}'",
                            node.id,
                            action
                        );
                    }
                } else {
                    bail!("auto_task node '{}' must have action config", node.id);
                }
            }
            NodeType::Join => {
                // 规则 7: join 节点入边数量至少为 2
                let incoming = graph.find_incoming_edges(&node.id);
                if incoming.len() < 2 {
                    bail!(
                        "join node '{}' must have at least 2 incoming edges, found {}",
                        node.id,
                        incoming.len()
                    );
                }
            }
        }
    }

    // 规则 8: back_to_previous 只能配置在入边源全部为 approval 且唯一入边的节点
    for node in &graph.nodes {
        if node.node_type != NodeType::Approval {
            continue;
        }
        let reject_action = node
            .config
            .get("reject_action")
            .and_then(|v| v.as_str())
            .unwrap_or("terminate");
        if reject_action == "back_to_previous" {
            // 找哪些节点通过边指向这个节点
            let incoming = graph.find_incoming_edges(&node.id);
            if incoming.len() != 1 {
                bail!(
                    "node '{}' with back_to_previous must have exactly 1 incoming edge, found {}",
                    node.id,
                    incoming.len()
                );
            }
            // 入边源必须是 approval
            let source_id = &incoming[0].from;
            let source_node = graph.find_node(source_id);
            if let Some(src) = source_node
                && src.node_type != NodeType::Approval
            {
                bail!(
                    "node '{}' with back_to_previous: incoming source '{}' must be approval, found {:?}",
                    node.id,
                    source_id,
                    src.node_type
                );
            }
        }
    }

    Ok(())
}

/// DFS 环检测：沿着出边遍历，如果在当前路径上遇到已访问节点则有环
fn detect_cycles(graph: &WorkflowGraph) -> Result<()> {
    let start_node = graph
        .find_start_node()
        .ok_or_else(|| anyhow::anyhow!("no start node found"))?;

    let mut visited: HashSet<String> = HashSet::new();
    let mut path: HashSet<String> = HashSet::new();

    fn dfs(
        graph: &WorkflowGraph,
        node_id: &str,
        visited: &mut HashSet<String>,
        path: &mut HashSet<String>,
    ) -> Result<()> {
        if path.contains(node_id) {
            bail!("cycle detected at node: {node_id}");
        }
        if visited.contains(node_id) {
            return Ok(());
        }

        path.insert(node_id.to_string());
        visited.insert(node_id.to_string());

        for edge in graph.find_outgoing_edges(node_id) {
            dfs(graph, &edge.to, visited, path)?;
        }

        path.remove(node_id);
        Ok(())
    }

    dfs(graph, &start_node.id, &mut visited, &mut path)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::model::{WorkflowEdge, WorkflowNode};
    use serde_json::json;

    struct MockActionLookup {
        registered: HashSet<String>,
    }

    impl MockActionLookup {
        fn new(actions: &[&str]) -> Self {
            Self {
                registered: actions.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl ActionLookup for MockActionLookup {
        fn is_registered(&self, action_name: &str) -> bool {
            self.registered.contains(action_name)
        }
    }

    fn make_linear_graph() -> WorkflowGraph {
        WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode {
                    id: "start".into(),
                    node_type: NodeType::Start,
                    name: "开始".into(),
                    config: json!(null),
                },
                WorkflowNode {
                    id: "approval1".into(),
                    node_type: NodeType::Approval,
                    name: "审批".into(),
                    config: json!({
                        "assignee_type": "role",
                        "assignee_value": "manager",
                        "multi_approval": "any",
                        "reject_action": "terminate",
                        "fallback_assignee": 1
                    }),
                },
                WorkflowNode {
                    id: "end".into(),
                    node_type: NodeType::End,
                    name: "结束".into(),
                    config: json!(null),
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from: "start".into(),
                    to: "approval1".into(),
                    condition: None,
                },
                WorkflowEdge {
                    from: "approval1".into(),
                    to: "end".into(),
                    condition: None,
                },
            ],
        }
    }

    #[test]
    fn test_valid_linear_graph() {
        let graph = make_linear_graph();
        let lookup = MockActionLookup::new(&[]);
        assert!(lint_graph(&graph, &lookup).is_ok());
    }

    #[test]
    fn test_valid_parallel_graph() {
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
                    id: "approval1".into(),
                    node_type: NodeType::Approval,
                    name: "审批1".into(),
                    config: json!({"assignee_type": "role", "assignee_value": "a", "multi_approval": "any", "reject_action": "terminate", "fallback_assignee": 1}),
                },
                WorkflowNode {
                    id: "approval2".into(),
                    node_type: NodeType::Approval,
                    name: "审批2".into(),
                    config: json!({"assignee_type": "role", "assignee_value": "b", "multi_approval": "any", "reject_action": "terminate", "fallback_assignee": 1}),
                },
                WorkflowNode {
                    id: "join1".into(),
                    node_type: NodeType::Join,
                    name: "汇聚".into(),
                    config: json!({"join_strategy": "all"}),
                },
                WorkflowNode {
                    id: "end".into(),
                    node_type: NodeType::End,
                    name: "结束".into(),
                    config: json!(null),
                },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "approval1".into(), condition: None },
                WorkflowEdge { from: "start".into(), to: "approval2".into(), condition: None },
                WorkflowEdge { from: "approval1".into(), to: "join1".into(), condition: None },
                WorkflowEdge { from: "approval2".into(), to: "join1".into(), condition: None },
                WorkflowEdge { from: "join1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        assert!(lint_graph(&graph, &lookup).is_ok());
    }

    #[test]
    fn test_missing_start() {
        let mut graph = make_linear_graph();
        graph.nodes.retain(|n| n.node_type != NodeType::Start);
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("start"));
    }

    #[test]
    fn test_missing_end() {
        let mut graph = make_linear_graph();
        graph.nodes.retain(|n| n.node_type != NodeType::End);
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("end"));
    }

    #[test]
    fn test_cycle_detection() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "a".into(), node_type: NodeType::Approval, name: "A".into(), config: json!({"fallback_assignee": 1}) },
                WorkflowNode { id: "b".into(), node_type: NodeType::Approval, name: "B".into(), config: json!({"fallback_assignee": 1}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "a".into(), condition: None },
                WorkflowEdge { from: "a".into(), to: "b".into(), condition: None },
                WorkflowEdge { from: "b".into(), to: "a".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_approval_missing_fallback() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "approval1".into(), node_type: NodeType::Approval, name: "审批".into(), config: json!({"assignee_type": "role"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "approval1".into(), condition: None },
                WorkflowEdge { from: "approval1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fallback_assignee"));
    }

    #[test]
    fn test_auto_task_unregistered_action() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "auto1".into(), node_type: NodeType::AutoTask, name: "自动".into(), config: json!({"action": "nonexistent"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "auto1".into(), condition: None },
                WorkflowEdge { from: "auto1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&["create_order"]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unregistered"));
    }

    #[test]
    fn test_auto_task_registered_action() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "auto1".into(), node_type: NodeType::AutoTask, name: "自动".into(), config: json!({"action": "create_order"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "auto1".into(), condition: None },
                WorkflowEdge { from: "auto1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&["create_order"]);
        assert!(lint_graph(&graph, &lookup).is_ok());
    }

    #[test]
    fn test_back_to_previous_multi_incoming() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "approval1".into(), node_type: NodeType::Approval, name: "审批1".into(), config: json!({"fallback_assignee": 1}) },
                WorkflowNode { id: "approval2".into(), node_type: NodeType::Approval, name: "审批2".into(), config: json!({"fallback_assignee": 1}) },
                WorkflowNode { id: "approval3".into(), node_type: NodeType::Approval, name: "审批3".into(),
                    config: json!({"fallback_assignee": 1, "reject_action": "back_to_previous"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "approval1".into(), condition: None },
                WorkflowEdge { from: "start".into(), to: "approval2".into(), condition: None },
                WorkflowEdge { from: "approval1".into(), to: "approval3".into(), condition: None },
                WorkflowEdge { from: "approval2".into(), to: "approval3".into(), condition: None },
                WorkflowEdge { from: "approval3".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly 1 incoming"));
    }

    #[test]
    fn test_join_with_single_incoming() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "join1".into(), node_type: NodeType::Join, name: "汇聚".into(), config: json!({"join_strategy": "all"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "join1".into(), condition: None },
                WorkflowEdge { from: "join1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 2"));
    }

    #[test]
    fn test_duplicate_start_nodes() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start1".into(), node_type: NodeType::Start, name: "开始1".into(), config: json!(null) },
                WorkflowNode { id: "start2".into(), node_type: NodeType::Start, name: "开始2".into(), config: json!(null) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("found 2"));
    }

    #[test]
    fn test_edge_to_unknown_node() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "nonexistent".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown target node"));
    }

    #[test]
    fn test_auto_task_missing_action_key() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "auto1".into(), node_type: NodeType::AutoTask, name: "自动".into(), config: json!({}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "auto1".into(), condition: None },
                WorkflowEdge { from: "auto1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&[]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have action config"));
    }

    #[test]
    fn test_back_to_previous_non_approval_source() {
        let graph = WorkflowGraph {
            graph_version: 1,
            nodes: vec![
                WorkflowNode { id: "start".into(), node_type: NodeType::Start, name: "开始".into(), config: json!(null) },
                WorkflowNode { id: "auto1".into(), node_type: NodeType::AutoTask, name: "自动".into(), config: json!({"action": "test"}) },
                WorkflowNode { id: "approval1".into(), node_type: NodeType::Approval, name: "审批".into(),
                    config: json!({"fallback_assignee": 1, "reject_action": "back_to_previous"}) },
                WorkflowNode { id: "end".into(), node_type: NodeType::End, name: "结束".into(), config: json!(null) },
            ],
            edges: vec![
                WorkflowEdge { from: "start".into(), to: "auto1".into(), condition: None },
                WorkflowEdge { from: "auto1".into(), to: "approval1".into(), condition: None },
                WorkflowEdge { from: "approval1".into(), to: "end".into(), condition: None },
            ],
        };
        let lookup = MockActionLookup::new(&["test"]);
        let result = lint_graph(&graph, &lookup);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be approval"));
    }
}
