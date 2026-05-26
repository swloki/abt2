use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;

pub struct ScheduledTaskRepo;

impl ScheduledTaskRepo {
    pub async fn find_by_name(&self, executor: PgExecutor<'_>, name: &str) -> Result<Option<ScheduledTaskDef>> {
        let task = sqlx::query_as::<sqlx::Postgres, ScheduledTaskDef>(
            "SELECT task_id, name, interval_secs, timeout_secs, is_enabled, last_run_at, last_elapsed_ms, last_result, last_error, total_runs FROM scheduled_tasks WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(executor)
        .await?;
        Ok(task)
    }

    pub async fn try_acquire_lock(&self, executor: PgExecutor<'_>, task_id: i64) -> Result<bool> {
        let locked: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock($1)",
        )
        .bind(task_id)
        .fetch_one(executor)
        .await?;
        Ok(locked)
    }

    pub async fn release_lock(&self, _executor: PgExecutor<'_>, _task_id: i64) -> Result<()> {
        // 事务级锁在事务结束时自动释放，无需手动 unlock
        Ok(())
    }

    pub async fn update_last_run(&self, executor: PgExecutor<'_>, task_id: i64, elapsed_ms: i64, result: Option<&str>, error: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"UPDATE scheduled_tasks SET
                last_run_at = NOW(), last_elapsed_ms = $2, last_result = $3,
                last_error = $4, total_runs = total_runs + 1
               WHERE task_id = $1"#,
        )
        .bind(task_id)
        .bind(elapsed_ms)
        .bind(result)
        .bind(error)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn insert_run_log(&self, executor: PgExecutor<'_>, task_id: i64, status: i16, elapsed_ms: Option<i64>, result: Option<&str>, error: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO task_run_logs (task_id, status, started_at, finished_at, elapsed_ms, result, error)
               VALUES ($1, $2, NOW(), NOW(), $3, $4, $5)"#,
        )
        .bind(task_id)
        .bind(status)
        .bind(elapsed_ms)
        .bind(result)
        .bind(error)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn list_tasks(&self, executor: PgExecutor<'_>) -> Result<Vec<ScheduledTaskDef>> {
        let tasks = sqlx::query_as::<sqlx::Postgres, ScheduledTaskDef>(
            "SELECT task_id, name, interval_secs, timeout_secs, is_enabled, last_run_at, last_elapsed_ms, last_result, last_error, total_runs FROM scheduled_tasks ORDER BY task_id",
        )
        .fetch_all(executor)
        .await?;
        Ok(tasks)
    }

    pub async fn list_run_logs(&self, executor: PgExecutor<'_>, task_id: i64, limit: i64) -> Result<Vec<TaskRunLog>> {
        let logs = sqlx::query_as::<sqlx::Postgres, TaskRunLog>(
            "SELECT run_id, task_id, status, started_at, finished_at, elapsed_ms, result, error FROM task_run_logs WHERE task_id = $1 ORDER BY started_at DESC LIMIT $2",
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(executor)
        .await?;
        Ok(logs)
    }
}
