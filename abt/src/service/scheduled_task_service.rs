//! 定时任务调度接口

use anyhow::Result;
use async_trait::async_trait;

/// 单次任务执行结果
#[derive(Debug, Clone)]
pub struct TaskRunResult {
    pub processed: usize,
    pub succeeded: usize,
    pub message: String,
}

/// 任务运行状态（内存维护）
#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub name: String,
    pub is_running: bool,
    pub last_run_at: Option<String>,
    pub last_elapsed_ms: Option<u64>,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
    pub total_runs: u64,
    pub interval_secs: u64,
}

/// 定时任务 trait — 所有后台定时任务实现此接口
#[async_trait]
pub trait ScheduledTask: Send + Sync {
    /// 任务唯一标识（如 "stock_alert"）
    fn name(&self) -> &str;

    /// 执行间隔（秒），默认 300（5 分钟）
    fn interval_secs(&self) -> u64 {
        300
    }

    /// 单次执行超时（秒），默认 60
    fn timeout_secs(&self) -> u64 {
        60
    }

    /// 执行一次任务
    async fn run_once(&self) -> Result<TaskRunResult>;
}
