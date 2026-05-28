//! 工作流超时扫描 Worker

use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::workflow::model::event_type;
use crate::workflow::model::SYSTEM_USER_ID;
use crate::workflow::repo::{WorkflowHistoryRepo, WorkflowTaskRepo};

pub struct WorkflowWorker {
    pool: PgPool,
    cancel_token: CancellationToken,
    scan_interval_secs: u64,
}

impl WorkflowWorker {
    pub fn new(pool: PgPool, cancel_token: CancellationToken) -> Self {
        let scan_interval_secs = std::env::var("WORKER_SCAN_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        Self {
            pool,
            cancel_token,
            scan_interval_secs,
        }
    }

    pub async fn run(&self) {
        loop {
            if self.cancel_token.is_cancelled() {
                break;
            }

            if let Err(e) = self.scan_overdue_tasks().await {
                tracing::error!("workflow worker scan error: {e:#}");
            }

            if let Err(e) = self.scan_remindable_tasks().await {
                tracing::error!("workflow worker remind scan error: {e:#}");
            }

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(self.scan_interval_secs)) => {}
                _ = self.cancel_token.cancelled() => break,
            }
        }
        tracing::info!("workflow worker shut down");
    }

    async fn scan_overdue_tasks(&self) -> Result<()> {
        // 用事务包裹 FOR UPDATE SKIP LOCKED
        let mut tx = self.pool.begin().await?;
        let tasks = WorkflowTaskRepo::find_overdue_pending_tx(&mut tx, 100).await?;

        for task in tasks {
            let timeout_action = task.timeout_action.as_deref().unwrap_or("notify");
            let instance_id = task.instance_id;

            match timeout_action {
                "auto_approve" | "auto_reject" => {
                    WorkflowTaskRepo::update_status_and_action(
                        &mut tx,
                        task.id,
                        if timeout_action == "auto_approve" { "completed" } else { "rejected" },
                        Some(timeout_action),
                        Some(json!({"reason": "timeout"})),
                    )
                    .await?;

                    if timeout_action == "auto_reject" {
                        WorkflowTaskRepo::cancel_pending_by_node(
                            &mut tx, instance_id, &task.node_id, Some(task.id),
                        ).await?;
                    }

                    WorkflowHistoryRepo::insert(
                        &mut tx,
                        instance_id,
                        Some(task.id),
                        Some(&task.node_id),
                        event_type::TIMEOUT_ACTION,
                        Some(SYSTEM_USER_ID),
                        Some(json!({"action": timeout_action})),
                    )
                    .await?;

                    // 推进工作流
                    if let Err(e) = super::engine::advance_after_timeout(
                        &mut tx, instance_id, task.id, &task.node_id, timeout_action,
                    ).await {
                        tracing::error!(
                            "failed to advance after timeout: instance_id={}, task_id={}, error={e:#}",
                            instance_id, task.id
                        );
                    }
                }
                _ => {
                    WorkflowHistoryRepo::insert(
                        &mut tx,
                        instance_id,
                        Some(task.id),
                        Some(&task.node_id),
                        event_type::TIMEOUT_ACTION,
                        Some(SYSTEM_USER_ID),
                        Some(json!({"action": timeout_action})),
                    )
                    .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn scan_remindable_tasks(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let tasks = WorkflowTaskRepo::find_remindable_pending_tx(&mut tx, 100).await?;

        for task in tasks {
            WorkflowHistoryRepo::insert(
                &mut tx,
                task.instance_id,
                Some(task.id),
                Some(&task.node_id),
                event_type::REMINDER,
                Some(SYSTEM_USER_ID),
                Some(json!({"assignee_id": task.assignee_id})),
            )
            .await?;

            // 标记已提醒，防止重复
            WorkflowTaskRepo::clear_remind_at(&mut tx, task.id).await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
