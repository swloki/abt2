//! 工作流任务数据访问层

use sqlx::PgPool;
use crate::shared::types::RepoResult;

use crate::workflow::model::WorkflowTask;

pub struct TaskInsertParams<'a> {
    pub instance_id: i64,
    pub node_id: &'a str,
    pub prev_task_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub timeout_action: Option<&'a str>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub remind_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct WorkflowTaskRepo;

impl WorkflowTaskRepo {
    pub async fn insert(executor: &mut sqlx::postgres::PgConnection, p: &TaskInsertParams<'_>) -> RepoResult<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO workflow_tasks (instance_id, node_id, prev_task_id, assignee_id, timeout_action, due_at, remind_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(p.instance_id)
        .bind(p.node_id)
        .bind(p.prev_task_id)
        .bind(p.assignee_id)
        .bind(p.timeout_action)
        .bind(p.due_at)
        .bind(p.remind_at)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    pub async fn find_for_update(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> RepoResult<Option<WorkflowTask>> {
        let row = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    pub async fn update_status_and_action(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        status: &str,
        action: Option<&str>,
        result: Option<serde_json::Value>,
    ) -> RepoResult<()> {
        let completed_at = if matches!(status, "completed" | "rejected" | "cancelled" | "delegated" | "timed_out") {
            Some(chrono::Utc::now())
        } else {
            None
        };
        sqlx::query(
            "UPDATE workflow_tasks SET status = $1, action = $2, result = $3::jsonb, completed_at = COALESCE($4, completed_at) WHERE id = $5",
        )
        .bind(status)
        .bind(action)
        .bind(result)
        .bind(completed_at)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// 检查某节点是否有至少一个 completed 状态的任务（Join 判断用）
    pub async fn has_completed_task_on_node(
        executor: &mut sqlx::postgres::PgConnection,
        instance_id: i64,
        node_id: &str,
    ) -> RepoResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_tasks WHERE instance_id = $1 AND node_id = $2 AND status = 'completed'",
        )
        .bind(instance_id)
        .bind(node_id)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }

    pub async fn count_pending_by_node(
        executor: &mut sqlx::postgres::PgConnection,
        instance_id: i64,
        node_id: &str,
    ) -> RepoResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_tasks WHERE instance_id = $1 AND node_id = $2 AND status = 'pending'",
        )
        .bind(instance_id)
        .bind(node_id)
        .fetch_one(executor)
        .await?;
        Ok(count)
    }

    pub async fn find_pending_by_instance(
        pool: &PgPool,
        instance_id: i64,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE instance_id = $1 AND status = 'pending'",
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_overdue_pending(
        pool: &PgPool,
        limit: i64,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE status = 'pending' AND due_at < NOW() ORDER BY due_at ASC LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 事务内版本（Worker 使用）
    pub async fn find_overdue_pending_tx(
        executor: &mut sqlx::postgres::PgConnection,
        limit: i64,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE status = 'pending' AND due_at < NOW() ORDER BY due_at ASC LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(limit)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    pub async fn find_by_assignee(
        pool: &PgPool,
        assignee_id: i64,
        status: Option<&str>,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, WorkflowTask>(
                "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE assignee_id = $1 AND status = $2 ORDER BY created_at DESC",
            )
            .bind(assignee_id)
            .bind(s)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, WorkflowTask>(
                "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE assignee_id = $1 ORDER BY created_at DESC",
            )
            .bind(assignee_id)
            .fetch_all(pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn cancel_pending_by_node(
        executor: &mut sqlx::postgres::PgConnection,
        instance_id: i64,
        node_id: &str,
        exclude_task_id: Option<i64>,
    ) -> RepoResult<u64> {
        let result = if let Some(exclude) = exclude_task_id {
            sqlx::query(
                "UPDATE workflow_tasks SET status = 'cancelled', completed_at = NOW() WHERE instance_id = $1 AND node_id = $2 AND status = 'pending' AND id != $3",
            )
            .bind(instance_id)
            .bind(node_id)
            .bind(exclude)
            .execute(executor)
            .await?
        } else {
            sqlx::query(
                "UPDATE workflow_tasks SET status = 'cancelled', completed_at = NOW() WHERE instance_id = $1 AND node_id = $2 AND status = 'pending'",
            )
            .bind(instance_id)
            .bind(node_id)
            .execute(executor)
            .await?
        };
        Ok(result.rows_affected())
    }

    pub async fn cancel_all_pending(
        executor: &mut sqlx::postgres::PgConnection,
        instance_id: i64,
    ) -> RepoResult<u64> {
        let result = sqlx::query(
            "UPDATE workflow_tasks SET status = 'cancelled', completed_at = NOW() WHERE instance_id = $1 AND status = 'pending'",
        )
        .bind(instance_id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn find_remindable_pending(
        pool: &PgPool,
        limit: i64,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE status = 'pending' AND remind_at < NOW() ORDER BY remind_at ASC LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 事务内版本（Worker 使用）
    pub async fn find_remindable_pending_tx(
        executor: &mut sqlx::postgres::PgConnection,
        limit: i64,
    ) -> RepoResult<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTask>(
            "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks WHERE status = 'pending' AND remind_at < NOW() ORDER BY remind_at ASC LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(limit)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 清除 remind_at，防止重复提醒
    pub async fn clear_remind_at(executor: &mut sqlx::postgres::PgConnection, task_id: i64) -> RepoResult<()> {
        sqlx::query("UPDATE workflow_tasks SET remind_at = NULL WHERE id = $1")
            .bind(task_id)
            .execute(executor)
            .await?;
        Ok(())
    }
}
