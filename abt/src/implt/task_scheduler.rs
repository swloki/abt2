//! 定时任务调度器

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;

use crate::service::{ScheduledTask, TaskRunResult, TaskStatus};

struct RunningGuard {
    name: String,
    statuses: Arc<Mutex<HashMap<String, TaskStatus>>>,
}

impl RunningGuard {
    async fn acquire(name: String, statuses: Arc<Mutex<HashMap<String, TaskStatus>>>) -> Option<Self> {
        {
            let mut map = statuses.lock().await;
            if let Some(s) = map.get_mut(&name) {
                if s.is_running {
                    return None;
                }
                s.is_running = true;
            }
        }
        Some(Self { name, statuses })
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        let name = self.name.clone();
        let statuses = self.statuses.clone();
        tokio::spawn(async move {
            let mut map = statuses.lock().await;
            if let Some(s) = map.get_mut(&name) {
                s.is_running = false;
            }
        });
    }
}

pub struct TaskScheduler {
    tasks: Vec<Arc<dyn ScheduledTask>>,
    statuses: Arc<Mutex<HashMap<String, TaskStatus>>>,
    shutdown: Arc<AtomicBool>,
}

impl TaskScheduler {
    pub fn new(shutdown: Arc<AtomicBool>) -> Self {
        Self {
            tasks: Vec::new(),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            shutdown,
        }
    }

    pub fn register(&mut self, task: impl ScheduledTask + 'static) {
        self.tasks.push(Arc::new(task));
    }

    pub async fn start(&self) {
        for task in &self.tasks {
            self.statuses.lock().await.insert(
                task.name().to_string(),
                TaskStatus {
                    name: task.name().to_string(),
                    is_running: false,
                    last_run_at: None,
                    last_elapsed_ms: None,
                    last_result: None,
                    last_error: None,
                    total_runs: 0,
                    interval_secs: task.interval_secs(),
                },
            );
        }

        for task in &self.tasks {
            let task = Arc::clone(task);
            let statuses = self.statuses.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                tracing::info!(
                    task = task.name(),
                    interval_secs = task.interval_secs(),
                    "TaskScheduler: task started"
                );
                run_task_loop(&*task, statuses, shutdown).await;
            });
        }
    }

    pub async fn trigger(&self, name: &str) -> anyhow::Result<TaskRunResult> {
        let _guard = RunningGuard::acquire(name.to_string(), self.statuses.clone())
            .await
            .ok_or_else(|| anyhow::anyhow!("task '{}' is already running", name))?;

        let task = self
            .tasks
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| anyhow::anyhow!("task not found: {}", name))?;

        let start = std::time::Instant::now();
        let result = task.run_once().await;
        let elapsed = start.elapsed().as_millis() as u64;

        update_status(&self.statuses, name, elapsed, &result).await;
        result
    }

    pub async fn list_statuses(&self) -> Vec<TaskStatus> {
        let statuses = self.statuses.lock().await;
        let mut list: Vec<TaskStatus> = statuses.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}

async fn update_status(
    statuses: &Arc<Mutex<HashMap<String, TaskStatus>>>,
    name: &str,
    elapsed: u64,
    result: &anyhow::Result<TaskRunResult>,
) {
    let mut map = statuses.lock().await;
    if let Some(s) = map.get_mut(name) {
        s.last_run_at = Some(Utc::now().to_rfc3339());
        s.last_elapsed_ms = Some(elapsed);
        s.total_runs += 1;
        match result {
            Ok(r) => {
                s.last_result = Some(format!(
                    "processed={}, succeeded={}",
                    r.processed, r.succeeded
                ));
                s.last_error = None;
            }
            Err(e) => {
                s.last_error = Some(e.to_string());
            }
        }
    }
}

async fn sleep_with_shutdown(secs: u64, shutdown: &AtomicBool) {
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(secs)) => {}
        _ = tokio::signal::ctrl_c() => {
            shutdown.store(true, Ordering::Release);
        }
    }
}

async fn run_task_loop(
    task: &dyn ScheduledTask,
    statuses: Arc<Mutex<HashMap<String, TaskStatus>>>,
    shutdown: Arc<AtomicBool>,
) {
    let name = task.name().to_string();
    let interval = task.interval_secs();
    let timeout_secs = task.timeout_secs();

    loop {
        if shutdown.load(Ordering::Acquire) {
            tracing::info!(task = name.as_str(), "task shutting down");
            return;
        }

        let _guard = RunningGuard::acquire(name.clone(), statuses.clone()).await;
        if _guard.is_none() {
            tracing::warn!(task = name.as_str(), "task already running, skipping");
            sleep_with_shutdown(interval.max(1), &shutdown).await;
            continue;
        }

        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            task.run_once(),
        )
        .await;
        let elapsed = start.elapsed().as_millis() as u64;

        {
            let mut map = statuses.lock().await;
            if let Some(s) = map.get_mut(&name) {
                s.last_run_at = Some(Utc::now().to_rfc3339());
                s.last_elapsed_ms = Some(elapsed);
                s.total_runs += 1;
                match result {
                    Ok(Ok(r)) => {
                        s.last_result = Some(format!(
                            "processed={}, succeeded={}",
                            r.processed, r.succeeded
                        ));
                        s.last_error = None;
                        tracing::info!(
                            task = name.as_str(),
                            elapsed_ms = elapsed,
                            processed = r.processed,
                            succeeded = r.succeeded,
                            "task completed"
                        );
                    }
                    Ok(Err(e)) => {
                        s.last_error = Some(e.to_string());
                        tracing::error!(
                            task = name.as_str(),
                            elapsed_ms = elapsed,
                            error = %e,
                            "task failed"
                        );
                    }
                    Err(_) => {
                        s.last_error = Some(format!("timed out after {}s", timeout_secs));
                        tracing::error!(
                            task = name.as_str(),
                            elapsed_ms = elapsed,
                            "task timed out"
                        );
                    }
                }
            }
        }

        drop(_guard);

        sleep_with_shutdown(interval.max(1), &shutdown).await;

        if shutdown.load(Ordering::Acquire) {
            tracing::info!(task = name.as_str(), "task shutting down during sleep");
            return;
        }
    }
}
