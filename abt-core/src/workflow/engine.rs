//! 工作流引擎核心实现

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;

use super::actions::ActionRegistry;
use super::hooks::{fire_hook, HookRegistry};
use crate::workflow::model::{
    evaluate_condition, Condition, EvaluationContext,
    InstanceStatus, NodeType, TemplateStatus, WorkflowGraph, WorkflowHistory,
    WorkflowInstance, WorkflowTask, WorkflowTaskStatus, WorkflowTemplate,
    event_type, SYSTEM_USER_ID,
};
use crate::workflow::repo::{
    InstanceInsertParams, TaskInsertParams,
    WorkflowHistoryRepo, WorkflowInstanceRepo, WorkflowTaskRepo, WorkflowTemplateRepo,
};
use super::service::WorkflowService;

#[derive(Clone)]
pub struct WorkflowEngine {
    pool: PgPool,
    action_registry: Arc<ActionRegistry>,
    hook_registry: Arc<HookRegistry>,
}

impl WorkflowEngine {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            action_registry: Arc::new(ActionRegistry::new()),
            hook_registry: Arc::new(HookRegistry::new()),
        }
    }

    pub fn hook_registry(&self) -> &HookRegistry {
        &self.hook_registry
    }

    pub fn action_registry(&self) -> &ActionRegistry {
        &self.action_registry
    }

    async fn start_instance_from_template(
        &self,
        template: &WorkflowTemplate,
        entity_id: i64,
        initiator_id: i64,
        history_payload: &serde_json::Value,
    ) -> Result<i64> {
        let graph_value = template
            .graph
            .clone()
            .ok_or_else(|| anyhow::anyhow!("template {} has no graph", template.id))?;
        let graph: WorkflowGraph = serde_json::from_value(graph_value)?;
        let frozen_graph = serde_json::to_value(&graph)?;
        let context = json!({
            "entity_snapshot": {},
            "variables": {},
            "join_progress": {}
        });

        let mut tx = self.pool.begin().await?;

        let instance_id = WorkflowInstanceRepo::insert(
            &mut tx,
            &InstanceInsertParams {
                template_id: template.id,
                template_version: Some(template.version),
                entity_type: &template.entity_type,
                entity_id,
                frozen_graph,
                context,
                initiator_id,
            },
        )
        .await?;

        let start_node = graph
            .find_start_node()
            .ok_or_else(|| anyhow::anyhow!("no start node in template {}", template.id))?;
        let outgoing = graph.find_outgoing_edges(&start_node.id);

        for edge in &outgoing {
            process_next_node(
                &mut tx,
                instance_id,
                None,
                &graph,
                &edge.to,
                &edge.condition,
            )
            .await?;
        }

        WorkflowHistoryRepo::insert(
            &mut tx,
            instance_id,
            None,
            Some(&start_node.id),
            event_type::INSTANCE_STARTED,
            Some(initiator_id),
            Some(history_payload.clone()),
        )
        .await?;

        tx.commit().await?;
        Ok(instance_id)
    }
}

#[async_trait]
impl WorkflowService for WorkflowEngine {
    async fn create_template(
        &self,
        entity_type: &str,
        name: &str,
        graph_json: &str,
        trigger_event: Option<&str>,
    ) -> Result<i64> {
        let graph: serde_json::Value = serde_json::from_str(graph_json)?;
        let id = WorkflowTemplateRepo::insert(
            self.pool.acquire().await?.as_mut(),
            entity_type,
            name,
            Some(graph),
            trigger_event,
        )
        .await?;
        Ok(id)
    }

    async fn update_template(
        &self,
        id: i64,
        name: Option<&str>,
        graph_json: Option<&str>,
        trigger_event: Option<&str>,
    ) -> Result<()> {
        let graph = match graph_json {
            Some(g) => Some(serde_json::from_str(g)?),
            None => None,
        };
        WorkflowTemplateRepo::update(
            self.pool.acquire().await?.as_mut(),
            id,
            name,
            graph,
            Some(trigger_event),
        )
            .await?;
        Ok(())
    }

    async fn get_template(&self, id: i64) -> Result<Option<WorkflowTemplate>> {
        WorkflowTemplateRepo::find_by_id(&self.pool, id).await.map_err(Into::into)
    }

    async fn list_templates(&self, entity_type: &str) -> Result<Vec<WorkflowTemplate>> {
        WorkflowTemplateRepo::list_by_entity_type(&self.pool, entity_type).await.map_err(Into::into)
    }

    async fn publish_template(&self, id: i64) -> Result<()> {
        let template = WorkflowTemplateRepo::find_by_id(&self.pool, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("template not found: {id}"))?;

        if template.status != TemplateStatus::Draft.as_str() {
            bail!("only draft templates can be published");
        }

        let graph_value = template
            .graph
            .ok_or_else(|| anyhow::anyhow!("template has no graph"))?;
        let graph: WorkflowGraph = serde_json::from_value(graph_value)?;

        // 使用 ActionRegistry 校验
        super::graph_linter::lint_graph(&graph, self.action_registry.as_ref())?;

        // 计算 checksum
        let checksum = compute_checksum(&serde_json::to_string(&graph)?);

        let published = WorkflowTemplateRepo::publish(
            self.pool.acquire().await?.as_mut(),
            id,
            &checksum,
        )
        .await?;

        if !published {
            bail!("failed to publish template {id}");
        }
        Ok(())
    }

    async fn archive_template(&self, id: i64) -> Result<()> {
        let archived = WorkflowTemplateRepo::archive(self.pool.acquire().await?.as_mut(), id)
            .await?;
        if !archived {
            bail!("failed to archive template {id}");
        }
        Ok(())
    }

    async fn start_instance(
        &self,
        entity_type: &str,
        entity_id: i64,
        initiator_id: i64,
    ) -> Result<i64> {
        let template = WorkflowTemplateRepo::find_active_by_entity_type(&self.pool, entity_type)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no active template for entity type: {entity_type}"))?;

        let history_payload = json!({"template_id": template.id, "template_version": template.version});
        self.start_instance_from_template(&template, entity_id, initiator_id, &history_payload)
            .await
    }

    async fn cancel_instance(&self, id: i64, operator_id: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let instance = WorkflowInstanceRepo::find_for_update(&mut tx, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found: {id}"))?;

        if instance.status != InstanceStatus::Running.as_str() {
            bail!("only running instances can be cancelled");
        }

        WorkflowInstanceRepo::update_status(&mut tx, id, InstanceStatus::Cancelled.as_str()).await?;
        WorkflowTaskRepo::cancel_all_pending(&mut tx, id).await?;
        WorkflowHistoryRepo::insert(
            &mut tx,
            id,
            None,
            None,
            event_type::INSTANCE_CANCELLED,
            Some(operator_id),
            None,
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn get_instance(&self, id: i64) -> Result<Option<WorkflowInstance>> {
        WorkflowInstanceRepo::find_by_id(&self.pool, id).await.map_err(Into::into)
    }

    async fn list_instances(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Vec<WorkflowInstance>> {
        WorkflowInstanceRepo::find_by_entity(&self.pool, entity_type, entity_id).await.map_err(Into::into)
    }

    async fn approve_task(
        &self,
        task_id: i64,
        operator_id: i64,
        comment: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // 锁定 task
        let task = WorkflowTaskRepo::find_for_update(&mut tx, task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;

        if task.status != WorkflowTaskStatus::Pending.as_str() {
            bail!("task is not pending");
        }
        if task.assignee_id != Some(operator_id) {
            bail!("not authorized to approve this task");
        }

        let instance_id = task.instance_id;

        // 锁定 instance
        let instance = WorkflowInstanceRepo::find_for_update(&mut tx, instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

        if instance.status != InstanceStatus::Running.as_str() {
            bail!("instance is not running");
        }

        // 更新 task
        let result_json = comment.map(|c| json!({"comment": c}));
        WorkflowTaskRepo::update_status_and_action(
            &mut tx,
            task_id,
            WorkflowTaskStatus::Completed.as_str(),
            Some("approve"),
            result_json,
        )
        .await?;

        // 获取 frozen_graph
        let frozen_graph_value = instance
            .frozen_graph
            .ok_or_else(|| anyhow::anyhow!("instance has no frozen_graph"))?;
        let graph: WorkflowGraph = serde_json::from_value(frozen_graph_value)?;

        // 找出边
        let outgoing = graph.find_outgoing_edges(&task.node_id);

        // 记录 history
        WorkflowHistoryRepo::insert(
            &mut tx,
            instance_id,
            Some(task_id),
            Some(&task.node_id),
            event_type::TASK_COMPLETED,
            Some(operator_id),
            Some(json!({"action": "approve"})),
        )
        .await?;

        // multi_approval 检查
        let node = graph.find_node(&task.node_id);
        let multi_approval = node
            .and_then(|n| n.config.get("multi_approval"))
            .and_then(|v| v.as_str())
            .unwrap_or("any");

        let mut instance_completed = false;

        if multi_approval == "all" {
            let remaining = WorkflowTaskRepo::count_pending_by_node(
                &mut tx,
                instance_id,
                &task.node_id,
            )
            .await?;
            if remaining > 0 {
                // multi_approval=all 且还有 pending task，等待其他人审批
                WorkflowHistoryRepo::insert(
                    &mut tx,
                    instance_id,
                    Some(task_id),
                    Some(&task.node_id),
                    event_type::MULTI_APPROVAL_WAITING,
                    Some(operator_id),
                    Some(json!({"remaining": remaining})),
                )
                .await?;
                tx.commit().await?;
                return Ok(());
            }
            // 全部通过，取消剩余 pending（理论上不应该有）
        }

        // 推进到下一节点
        for edge in &outgoing {
            process_next_node(
                &mut tx,
                instance_id,
                Some(task_id),
                &graph,
                &edge.to,
                &edge.condition,
            )
            .await?;
        }

        // 检查是否到达 End 节点（实例变为 completed）
        let refreshed = WorkflowInstanceRepo::find_for_update(&mut tx, instance_id).await?;
        if let Some(inst) = refreshed
            && inst.status == InstanceStatus::Completed.as_str()
        {
            instance_completed = true;
        }

        tx.commit().await?;

        // 事务提交后触发 hook
        if instance_completed {
            let instance_for_hook = WorkflowInstanceRepo::find_by_id(&self.pool, instance_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;
            fire_hook(
                self.pool.clone(),
                self.hook_registry.clone(),
                instance_for_hook,
                "approved",
            )
            .await;
        }

        Ok(())
    }

    async fn reject_task(
        &self,
        task_id: i64,
        operator_id: i64,
        comment: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let task = WorkflowTaskRepo::find_for_update(&mut tx, task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;

        if task.status != WorkflowTaskStatus::Pending.as_str() {
            bail!("task is not pending");
        }
        if task.assignee_id != Some(operator_id) {
            bail!("not authorized to reject this task");
        }

        let instance_id = task.instance_id;

        let instance = WorkflowInstanceRepo::find_for_update(&mut tx, instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

        if instance.status != InstanceStatus::Running.as_str() {
            bail!("instance is not running");
        }

        // 更新 task
        let result_json = comment.map(|c| json!({"comment": c}));
        WorkflowTaskRepo::update_status_and_action(
            &mut tx,
            task_id,
            WorkflowTaskStatus::Rejected.as_str(),
            Some("reject"),
            result_json,
        )
        .await?;

        // 取消同 node_id 其余 pending task
        WorkflowTaskRepo::cancel_pending_by_node(
            &mut tx,
            instance_id,
            &task.node_id,
            Some(task_id),
        )
        .await?;

        // 获取节点配置中的 reject_action
        let frozen_graph_value = instance
            .frozen_graph
            .ok_or_else(|| anyhow::anyhow!("instance has no frozen_graph"))?;
        let graph: WorkflowGraph = serde_json::from_value(frozen_graph_value)?;
        let node = graph.find_node(&task.node_id);
        let reject_action = node
            .and_then(|n| n.config.get("reject_action"))
            .and_then(|v| v.as_str())
            .unwrap_or("terminate");

        match reject_action {
            "terminate" => {
                WorkflowInstanceRepo::update_status(&mut tx, instance_id, InstanceStatus::Rejected.as_str()).await?;
            }
            "back_to_previous" => {
                // 找入边的源节点
                let incoming = graph.find_incoming_edges(&task.node_id);
                if let Some(prev_edge) = incoming.first() {
                    let prev_node = graph.find_node(&prev_edge.from);
                    if let Some(prev) = prev_node {
                        let assignee = resolve_assignee(&prev.config);
                        WorkflowTaskRepo::insert(
                            &mut tx,
                            &TaskInsertParams {
                                instance_id,
                                node_id: &prev.id,
                                prev_task_id: Some(task_id),
                                assignee_id: assignee,
                                timeout_action: prev.config.get("timeout_action").and_then(|v| v.as_str()),
                                due_at: None,
                                remind_at: None,
                            },
                        )
                        .await?;
                    }
                }
            }
            _ => {
                WorkflowInstanceRepo::update_status(&mut tx, instance_id, InstanceStatus::Rejected.as_str()).await?;
            }
        }

        WorkflowHistoryRepo::insert(
            &mut tx,
            instance_id,
            Some(task_id),
            Some(&task.node_id),
            event_type::TASK_REJECTED,
            Some(operator_id),
            Some(json!({"action": "reject", "reject_action": reject_action})),
        )
        .await?;

        let instance_rejected = reject_action == "terminate";

        tx.commit().await?;

        // 事务提交后触发 hook
        if instance_rejected {
            let instance_for_hook = WorkflowInstanceRepo::find_by_id(&self.pool, instance_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;
            fire_hook(
                self.pool.clone(),
                self.hook_registry.clone(),
                instance_for_hook,
                "rejected",
            )
            .await;
        }

        Ok(())
    }

    async fn delegate_task(
        &self,
        task_id: i64,
        from_user_id: i64,
        to_user_id: i64,
    ) -> Result<i64> {
        let mut tx = self.pool.begin().await?;

        let task = WorkflowTaskRepo::find_for_update(&mut tx, task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;

        if task.status != WorkflowTaskStatus::Pending.as_str() {
            bail!("task is not pending");
        }
        if task.assignee_id != Some(from_user_id) {
            bail!("only the current assignee can delegate this task");
        }

        // 标记当前 task 为 delegated
        WorkflowTaskRepo::update_status_and_action(
            &mut tx,
            task_id,
            WorkflowTaskStatus::Delegated.as_str(),
            Some("delegate"),
            Some(json!({"to_user_id": to_user_id})),
        )
        .await?;

        // 创建新的 pending task
        let new_task_id = WorkflowTaskRepo::insert(
            &mut tx,
            &TaskInsertParams {
                instance_id: task.instance_id,
                node_id: &task.node_id,
                prev_task_id: Some(task_id),
                assignee_id: Some(to_user_id),
                timeout_action: task.timeout_action.as_deref(),
                due_at: None,
                remind_at: None,
            },
        )
        .await?;

        WorkflowHistoryRepo::insert(
            &mut tx,
            task.instance_id,
            Some(task_id),
            Some(&task.node_id),
            event_type::TASK_DELEGATED,
            Some(to_user_id),
            Some(json!({"new_task_id": new_task_id})),
        )
        .await?;

        tx.commit().await?;
        Ok(new_task_id)
    }

    async fn get_my_tasks(
        &self,
        user_id: i64,
        status: Option<&str>,
    ) -> Result<Vec<WorkflowTask>> {
        WorkflowTaskRepo::find_by_assignee(&self.pool, user_id, status).await.map_err(Into::into)
    }

    async fn retry_auto_task(&self, instance_id: i64, operator_id: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let instance = WorkflowInstanceRepo::find_for_update(&mut tx, instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

        if instance.status != InstanceStatus::Suspended.as_str() {
            bail!("only suspended instances can be retried");
        }

        // 检查 suspended_reason 中的 node_id
        let suspended_node = instance
            .suspended_reason
            .as_ref()
            .and_then(|r| r.get("node_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("no suspended node found in suspended_reason"))?;

        // 恢复为 running
        WorkflowInstanceRepo::update_status(&mut tx, instance_id, InstanceStatus::Running.as_str()).await?;
        WorkflowInstanceRepo::update_suspended_reason(&mut tx, instance_id, None).await?;

        // 获取 frozen_graph 并继续执行
        let frozen_graph_value = instance
            .frozen_graph
            .ok_or_else(|| anyhow::anyhow!("instance has no frozen_graph"))?;
        let graph: WorkflowGraph = serde_json::from_value(frozen_graph_value)?;

        // 继续从 suspended 节点推进
        let outgoing = graph.find_outgoing_edges(suspended_node);
        for edge in &outgoing {
            process_next_node(&mut tx, instance_id, None, &graph, &edge.to, &edge.condition)
                .await?;
        }

        WorkflowHistoryRepo::insert(
            &mut tx,
            instance_id,
            None,
            Some(suspended_node),
            event_type::AUTO_TASK_RETRIED,
            Some(operator_id),
            None,
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn retry_failed_hook(&self, instance_id: i64, operator_id: i64) -> Result<()> {
        super::hooks::retry_failed_hook(
            &self.pool,
            &self.hook_registry,
            instance_id,
            operator_id,
        )
        .await
    }

    async fn record_entity_change(
        &self,
        instance_id: i64,
        _entity_id: i64,
        change_type: &str,
        change_detail: &str,
    ) -> Result<Vec<i64>> {
        let instance = WorkflowInstanceRepo::find_by_id(&self.pool, instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

        if instance.status != InstanceStatus::Running.as_str() {
            bail!("instance is not running");
        }

        // 记录 history
        WorkflowHistoryRepo::insert(
            self.pool.acquire().await?.as_mut(),
            instance_id,
            None,
            None,
            event_type::ENTITY_CHANGED,
            None,
            Some(json!({"change_type": change_type, "change_detail": change_detail})),
        )
        .await?;

        // 返回当前 pending task 的 assignee 列表
        let pending_tasks = WorkflowTaskRepo::find_pending_by_instance(&self.pool, instance_id)
            .await?;
        let assignee_ids: Vec<i64> = pending_tasks
            .iter()
            .filter_map(|t| t.assignee_id)
            .collect();
        Ok(assignee_ids)
    }

    async fn list_history(&self, instance_id: i64) -> Result<Vec<WorkflowHistory>> {
        WorkflowHistoryRepo::list_by_instance(&self.pool, instance_id).await.map_err(Into::into)
    }

    async fn trigger(
        &self,
        event: &str,
        entity_id: i64,
        initiator_id: i64,
    ) -> Result<Option<i64>> {
        let template = WorkflowTemplateRepo::find_active_by_trigger(&self.pool, event).await?;
        match template {
            Some(t) => {
                let history_payload = json!({"template_id": t.id, "trigger_event": event});
                let id = self.start_instance_from_template(&t, entity_id, initiator_id, &history_payload).await?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }

    async fn list_trigger_events(&self) -> Result<Vec<crate::workflow::model::TriggerEventDef>> {
        let triggers = crate::workflow::model::all_trigger_events();
        let active_templates = WorkflowTemplateRepo::list_active(&self.pool).await?;

        let template_by_trigger: std::collections::HashMap<&str, &WorkflowTemplate> = active_templates
            .iter()
            .filter_map(|t| t.trigger_event.as_deref().map(|te| (te, t)))
            .collect();

        Ok(triggers
            .iter()
            .map(|(name, label, description)| {
                let (id, template_name) = match template_by_trigger.get(name) {
                    Some(t) => (t.id, t.name.clone()),
                    None => (0, String::new()),
                };
                crate::workflow::model::TriggerEventDef {
                    name,
                    label,
                    description,
                    bound_template_id: id,
                    bound_template_name: template_name,
                }
            })
            .collect())
    }
}

/// 处理推进到下一个节点（boxed 递归）
fn process_next_node<'a>(
    tx: &'a mut sqlx::Transaction<'_, sqlx::Postgres>,
    instance_id: i64,
    prev_task_id: Option<i64>,
    graph: &'a WorkflowGraph,
    node_id: &'a str,
    condition: &'a Option<Condition>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
    // 条件判断
    if let Some(cond) = condition {
        let instance = WorkflowInstanceRepo::find_for_update(&mut *tx, instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("instance not found"))?;
        let ctx = build_evaluation_context(&instance);
        if !evaluate_condition(cond, &ctx) {
            return Ok(());
        }
    }

    let node = graph
        .find_node(node_id)
        .ok_or_else(|| anyhow::anyhow!("node not found: {node_id}"))?;

    match node.node_type {
        NodeType::End => {
            // 到达 end 节点，标记实例 completed
            WorkflowInstanceRepo::update_status(tx, instance_id, InstanceStatus::Completed.as_str()).await?;
            WorkflowHistoryRepo::insert(
                tx,
                instance_id,
                prev_task_id,
                Some(node_id),
                event_type::INSTANCE_COMPLETED,
                None,
                None,
            )
            .await?;
        }
        NodeType::Approval => {
            // 解析 assignee
            let assignee_id = resolve_assignee(&node.config)
                .ok_or_else(|| anyhow::anyhow!(
                    "approval node '{}' has no valid assignee (fallback_assignee required)",
                    node_id
                ))?;

            // 计算 due_at 和 remind_at
            let timeout_hours = node.config.get("timeout_hours").and_then(|v| v.as_i64());
            let remind_hours_before = node
                .config
                .get("remind_hours_before")
                .and_then(|v| v.as_i64());
            let due_at = timeout_hours.map(|h| chrono::Utc::now() + chrono::Duration::hours(h));
            let remind_at = timeout_hours.zip(remind_hours_before).map(|(h, r)| {
                chrono::Utc::now() + chrono::Duration::hours(h - r)
            });
            let timeout_action = node
                .config
                .get("timeout_action")
                .and_then(|v| v.as_str());

            WorkflowTaskRepo::insert(
                tx,
                &TaskInsertParams {
                    instance_id,
                    node_id,
                    prev_task_id,
                    assignee_id: Some(assignee_id),
                    timeout_action,
                    due_at,
                    remind_at,
                },
            )
            .await?;

            WorkflowHistoryRepo::insert(
                tx,
                instance_id,
                prev_task_id,
                Some(node_id),
                event_type::NODE_ENTERED,
                None,
                Some(json!({"node_type": "approval", "assignee": assignee_id})),
            )
            .await?;
        }
        NodeType::AutoTask => {
            let action_name = node
                .config
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            // 记录进入节点
            WorkflowHistoryRepo::insert(
                tx,
                instance_id,
                prev_task_id,
                Some(node_id),
                event_type::NODE_ENTERED,
                Some(SYSTEM_USER_ID),
                Some(json!({"node_type": "auto_task", "action": action_name})),
            )
            .await?;

            // V1: auto_task 直接跳过，记录 history
            WorkflowHistoryRepo::insert(
                tx,
                instance_id,
                prev_task_id,
                Some(node_id),
                event_type::AUTO_TASK_COMPLETED,
                Some(SYSTEM_USER_ID),
                Some(json!({"action": action_name, "status": "completed"})),
            )
            .await?;

            // 继续推进到下一个节点
            let outgoing = graph.find_outgoing_edges(node_id);
            for edge in &outgoing {
                process_next_node(tx, instance_id, prev_task_id, graph, &edge.to, &edge.condition)
                    .await?;
            }
        }
        NodeType::Join => {
            // Join 判断：所有入边源节点必须有至少一个 completed 任务
            let incoming = graph.find_incoming_edges(node_id);
            let mut all_completed = true;

            for edge in &incoming {
                let has_completed =
                    WorkflowTaskRepo::has_completed_task_on_node(tx, instance_id, &edge.from).await?;
                if !has_completed {
                    all_completed = false;
                    break;
                }
            }

            if !all_completed {
                // 还有分支未完成，等待
                WorkflowHistoryRepo::insert(
                    tx,
                    instance_id,
                    prev_task_id,
                    Some(node_id),
                    event_type::JOIN_WAITING,
                    None,
                    Some(json!({"reason": "pending_branches"})),
                )
                .await?;
                return Ok(());
            }

            // 所有分支完成，记录并继续推进
            WorkflowHistoryRepo::insert(
                tx,
                instance_id,
                prev_task_id,
                Some(node_id),
                event_type::JOIN_COMPLETED,
                None,
                None,
            )
            .await?;

            let outgoing = graph.find_outgoing_edges(node_id);
            for edge in &outgoing {
                process_next_node(tx, instance_id, prev_task_id, graph, &edge.to, &edge.condition)
                    .await?;
            }
        }
        NodeType::Start => {
            bail!("unexpected start node in process_next_node");
        }
    }

    Ok(())
    })
}

/// 从节点配置中解析 assignee
fn resolve_assignee(config: &serde_json::Value) -> Option<i64> {
    config.get("fallback_assignee").and_then(|v| v.as_i64())
}

/// 处理超时任务后的流程推进（Worker 调用）
pub async fn advance_after_timeout(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    instance_id: i64,
    task_id: i64,
    node_id: &str,
    auto_action: &str,
) -> Result<()> {
    let instance = WorkflowInstanceRepo::find_for_update(&mut *tx, instance_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("instance not found: {instance_id}"))?;

    if instance.status != InstanceStatus::Running.as_str() {
        return Ok(());
    }

    let frozen_graph_value = instance
        .frozen_graph
        .ok_or_else(|| anyhow::anyhow!("instance has no frozen_graph"))?;
    let graph: WorkflowGraph = serde_json::from_value(frozen_graph_value)?;

    match auto_action {
        "auto_approve" => {
            let outgoing = graph.find_outgoing_edges(node_id);
            for edge in &outgoing {
                process_next_node(tx, instance_id, Some(task_id), &graph, &edge.to, &edge.condition)
                    .await?;
            }
        }
        "auto_reject" => {
            let node = graph.find_node(node_id);
            let reject_action = node
                .and_then(|n| n.config.get("reject_action"))
                .and_then(|v| v.as_str())
                .unwrap_or("terminate");

            match reject_action {
                "terminate" => {
                    WorkflowInstanceRepo::update_status(tx, instance_id, InstanceStatus::Rejected.as_str()).await?;
                }
                "back_to_previous" => {
                    let incoming = graph.find_incoming_edges(node_id);
                    if let Some(prev_edge) = incoming.first() {
                        let prev_node = graph.find_node(&prev_edge.from);
                        if let Some(prev) = prev_node {
                            let assignee = resolve_assignee(&prev.config);
                            WorkflowTaskRepo::insert(
                                tx,
                                &TaskInsertParams {
                                    instance_id,
                                    node_id: &prev.id,
                                    prev_task_id: Some(task_id),
                                    assignee_id: assignee,
                                    timeout_action: prev.config.get("timeout_action").and_then(|v| v.as_str()),
                                    due_at: None,
                                    remind_at: None,
                                },
                            ).await?;
                        }
                    }
                }
                _ => {
                    WorkflowInstanceRepo::update_status(tx, instance_id, InstanceStatus::Rejected.as_str()).await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// 从实例上下文构建条件求值上下文
fn build_evaluation_context(instance: &WorkflowInstance) -> EvaluationContext {
    let ctx_value = instance.context.clone().unwrap_or_default();
    let entity_snapshot = ctx_value.get("entity_snapshot").cloned().unwrap_or_default();
    let variables_raw = ctx_value.get("variables").cloned().unwrap_or_default();
    let mut variables = HashMap::new();
    if let serde_json::Value::Object(map) = variables_raw {
        for (k, v) in map {
            variables.insert(k, v);
        }
    }
    EvaluationContext {
        entity_snapshot,
        variables,
    }
}

/// V1 checksum（注意：DefaultHasher 输出不跨版本稳定，仅用于变更检测）
fn compute_checksum(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
