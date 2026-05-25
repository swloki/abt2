use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum TaskStatus {
    Pending = 1,
    Running = 2,
    Completed = 3,
    Failed = 4,
}

impl TaskStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Pending),
            2 => Some(Self::Running),
            3 => Some(Self::Completed),
            4 => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

/// 任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunResult {
    pub processed: usize,
    pub succeeded: usize,
    pub message: String,
}

/// 定时任务定义
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduledTaskDef {
    pub task_id: i64,
    pub name: String,
    pub interval_secs: i64,
    pub timeout_secs: i64,
    pub is_enabled: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_elapsed_ms: Option<i64>,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
    pub total_runs: i64,
}

/// 任务执行记录
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskRunLog {
    pub run_id: i64,
    pub task_id: i64,
    pub status: i16,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub elapsed_ms: Option<i64>,
    pub result: Option<String>,
    pub error: Option<String>,
}
