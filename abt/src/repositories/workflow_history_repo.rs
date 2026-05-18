//! 工作流审计历史数据访问层

use anyhow::Result;
use sqlx::PgPool;

use crate::models::WorkflowHistory;
use crate::repositories::Executor;

pub struct WorkflowHistoryRepo;

#[allow(dead_code)]
impl WorkflowHistoryRepo {
    pub async fn insert(
        executor: Executor<'_>,
        instance_id: i64,
        task_id: Option<i64>,
        node_id: Option<&str>,
        event_type: &str,
        actor_id: Option<i64>,
        payload: Option<serde_json::Value>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO workflow_history (instance_id, task_id, node_id, event_type, actor_id, payload) VALUES ($1, $2, $3, $4, $5, $6::jsonb) RETURNING id",
        )
        .bind(instance_id)
        .bind(task_id)
        .bind(node_id)
        .bind(event_type)
        .bind(actor_id)
        .bind(payload)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn list_by_instance(
        pool: &PgPool,
        instance_id: i64,
    ) -> Result<Vec<WorkflowHistory>> {
        let rows = sqlx::query_as::<_, WorkflowHistory>(
            "SELECT id, instance_id, task_id, node_id, event_type, actor_id, payload, created_at FROM workflow_history WHERE instance_id = $1 ORDER BY created_at ASC",
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_latest_failed_hook(
        pool: &PgPool,
        instance_id: i64,
    ) -> Result<Option<WorkflowHistory>> {
        let row = sqlx::query_as::<_, WorkflowHistory>(
            "SELECT id, instance_id, task_id, node_id, event_type, actor_id, payload, created_at FROM workflow_history WHERE instance_id = $1 AND event_type = 'hook_executed' AND payload->>'success' = 'false' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
