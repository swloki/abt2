//! 工作流服务接口

use anyhow::Result;
use async_trait::async_trait;

use super::model::{
    TriggerEventDef, WorkflowHistory, WorkflowInstance, WorkflowTask, WorkflowTemplate,
};

#[async_trait]
pub trait WorkflowService: Send + Sync {
    // 模板管理
    async fn create_template(
        &self,
        entity_type: &str,
        name: &str,
        graph_json: &str,
        trigger_event: Option<&str>,
    ) -> Result<i64>;

    async fn update_template(
        &self,
        id: i64,
        name: Option<&str>,
        graph_json: Option<&str>,
        trigger_event: Option<&str>,
    ) -> Result<()>;

    async fn get_template(&self, id: i64) -> Result<Option<WorkflowTemplate>>;

    async fn list_templates(&self, entity_type: &str) -> Result<Vec<WorkflowTemplate>>;

    async fn publish_template(&self, id: i64) -> Result<()>;

    async fn archive_template(&self, id: i64) -> Result<()>;

    // 实例管理
    async fn start_instance(
        &self,
        entity_type: &str,
        entity_id: i64,
        initiator_id: i64,
    ) -> Result<i64>;

    async fn cancel_instance(&self, id: i64, operator_id: i64) -> Result<()>;

    async fn get_instance(&self, id: i64) -> Result<Option<WorkflowInstance>>;

    async fn list_instances(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Vec<WorkflowInstance>>;

    // 任务操作
    async fn approve_task(&self, task_id: i64, operator_id: i64, comment: Option<&str>) -> Result<()>;

    async fn reject_task(&self, task_id: i64, operator_id: i64, comment: Option<&str>) -> Result<()>;

    async fn delegate_task(&self, task_id: i64, from_user_id: i64, to_user_id: i64) -> Result<i64>;

    async fn get_my_tasks(&self, user_id: i64, status: Option<&str>) -> Result<Vec<WorkflowTask>>;

    // Admin
    async fn retry_auto_task(&self, instance_id: i64, operator_id: i64) -> Result<()>;

    async fn retry_failed_hook(&self, instance_id: i64, operator_id: i64) -> Result<()>;

    async fn record_entity_change(
        &self,
        instance_id: i64,
        entity_id: i64,
        change_type: &str,
        change_detail: &str,
    ) -> Result<Vec<i64>>;

    // 历史
    async fn list_history(&self, instance_id: i64) -> Result<Vec<WorkflowHistory>>;

    // 触发器
    async fn trigger(
        &self,
        event: &str,
        entity_id: i64,
        initiator_id: i64,
    ) -> Result<Option<i64>>;

    async fn list_trigger_events(&self) -> Result<Vec<TriggerEventDef>>;
}
