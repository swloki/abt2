use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,ServiceContext, Result};

/// 定时任务 — 每个后台任务独立实现此 trait
#[async_trait]
pub trait ScheduledTask: Send + Sync {
    fn name(&self) -> &str;
    fn interval_secs(&self) -> u64 { 300 }
    fn timeout_secs(&self) -> u64 { 60 }

    async fn run_once(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<TaskRunResult>;
}

/// 任务调度服务
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    async fn register_task(&self, ctx: &ServiceContext, db: PgExecutor<'_>, name: &str, interval_secs: i64, timeout_secs: i64) -> Result<i64>;
    async fn execute_task(&self, ctx: &ServiceContext, db: PgExecutor<'_>, name: &str) -> Result<TaskRunResult>;
    async fn list_tasks(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<Vec<ScheduledTaskDef>>;
    async fn get_task_history(&self, ctx: &ServiceContext, db: PgExecutor<'_>, name: &str, limit: i64) -> Result<Vec<TaskRunLog>>;
}
