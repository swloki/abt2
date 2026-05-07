# Task Scheduler Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a generic scheduled task framework with `ScheduledTask` trait, `TaskScheduler` dispatcher, and gRPC management API, then refactor `StockAlertWorker` into the first task implementation.

**Architecture:** Define a `ScheduledTask` trait in the service layer. `TaskScheduler` holds registered tasks, spawns each in its own tokio task, and tracks status in-memory. A gRPC service exposes ListTasks/TriggerTask. `StockAlertWorker` is refactored to implement `ScheduledTask`.

**Tech Stack:** Rust, async-trait, tokio, sqlx, tonic (gRPC), protobuf

---

### Task 1: ScheduledTask Trait + TaskRunResult + TaskStatus

**Files:**
- Create: `abt/src/service/scheduled_task_service.rs`
- Modify: `abt/src/service/mod.rs`

- [ ] **Step 1: Create the scheduled_task_service.rs file**

```rust
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

    /// 执行一次任务
    async fn run_once(&self) -> Result<TaskRunResult>;
}
```

- [ ] **Step 2: Register module in service/mod.rs**

Add `mod scheduled_task_service;` and `pub use scheduled_task_service::{ScheduledTask, TaskRunResult, TaskStatus};` to `abt/src/service/mod.rs`.

In `abt/src/service/mod.rs`, after `mod product_watcher_service;` add:

```
mod scheduled_task_service;
```

And after `pub use product_watcher_service::ProductWatcherService;` add:

```
pub use scheduled_task_service::{ScheduledTask, TaskRunResult, TaskStatus};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo clippy -p abt 2>&1 | tail -5`
Expected: compiles with only pre-existing warnings

- [ ] **Step 4: Commit**

```bash
git add abt/src/service/scheduled_task_service.rs abt/src/service/mod.rs
git commit -m "feat(service): add ScheduledTask trait and TaskStatus/TaskRunResult types"
```

---

### Task 2: TaskScheduler Implementation

**Files:**
- Create: `abt/src/implt/task_scheduler.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

- [ ] **Step 1: Create task_scheduler.rs**

```rust
//! 定时任务调度器

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::service::{ScheduledTask, TaskRunResult, TaskStatus};

pub struct TaskScheduler {
    tasks: Vec<Box<dyn ScheduledTask>>,
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

    /// 注册任务
    pub fn register(&mut self, task: Box<dyn ScheduledTask>) {
        let interval = task.interval_secs();
        let name = task.name().to_string();
        self.statuses.lock().await_insert(name.clone(), TaskStatus {
            name,
            is_running: false,
            last_run_at: None,
            last_elapsed_ms: None,
            last_result: None,
            last_error: None,
            total_runs: 0,
            interval_secs: interval,
        });
        self.tasks.push(task);
    }

    /// 启动所有已注册任务
    pub fn start(&self) {
        for task in &self.tasks {
            let task: Box<dyn ScheduledTask> = task.clone_box();
            let statuses = self.statuses.clone();
            let shutdown = self.shutdown.clone();

            tokio::spawn(async move {
                run_task_loop(task, statuses, shutdown).await;
            });

            tracing::info!(
                task = task.name(),
                interval_secs = task.interval_secs(),
                "TaskScheduler: task started"
            );
        }
    }

    /// 手动触发某个任务
    pub async fn trigger(&self, name: &str) -> anyhow::Result<TaskRunResult> {
        let task = self.tasks.iter().find(|t| t.name() == name).ok_or_else(|| {
            anyhow::anyhow!("task not found: {}", name)
        })?;

        let result = task.run_once().await?;
        Ok(result)
    }

    /// 查询所有任务状态
    pub async fn list_statuses(&self) -> Vec<TaskStatus> {
        let statuses = self.statuses.lock().await;
        let mut list: Vec<TaskStatus> = statuses.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}
```

Wait — the above has two issues: (1) `register` uses `lock().await` but is a sync function, and (2) `ScheduledTask` needs `clone_box()` which isn't defined. Let me write the correct version:

```rust
//! 定时任务调度器

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;

use crate::service::{ScheduledTask, TaskRunResult, TaskStatus};

const TASK_TIMEOUT_SECS: u64 = 60;

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

    /// 注册任务
    pub fn register(&mut self, task: impl ScheduledTask + 'static) {
        let name = task.name().to_string();
        let interval = task.interval_secs();
        let status = TaskStatus {
            name,
            is_running: false,
            last_run_at: None,
            last_elapsed_ms: None,
            last_result: None,
            last_error: None,
            total_runs: 0,
            interval_secs: interval,
        };
        // Pre-insert status; we lock in start() via blocking approach
        // Actually we need async — use a two-phase approach
        // Store status now, let start() init the HashMap
        self.tasks.push(Arc::new(task));
        // statuses will be initialized in start() which is async
    }

    /// 启动所有已注册任务（async because we write to Mutex）
    pub async fn start(&self) {
        for task in &self.tasks {
            let name = task.name().to_string();
            let interval = task.interval_secs();
            self.statuses.lock().await.insert(name.clone(), TaskStatus {
                name,
                is_running: false,
                last_run_at: None,
                last_elapsed_ms: None,
                last_result: None,
                last_error: None,
                total_runs: 0,
                interval_secs: interval,
            });
        }

        for task in &self.tasks {
            let task = Arc::clone(task);
            let statuses = self.statuses.clone();
            let shutdown = self.shutdown.clone();
            let name = task.name().to_string();
            let interval = task.interval_secs();

            tokio::spawn(async move {
                tracing::info!(
                    task = name.as_str(),
                    interval_secs = interval,
                    "TaskScheduler: task started"
                );
                run_task_loop(&*task, statuses, shutdown).await;
            });
        }
    }

    /// 手动触发某个任务
    pub async fn trigger(&self, name: &str) -> anyhow::Result<TaskRunResult> {
        let task = self.tasks.iter().find(|t| t.name() == name).ok_or_else(|| {
            anyhow::anyhow!("task not found: {}", name)
        })?;
        let result = Arc::clone(task).run_once().await?;
        Ok(result)
    }

    /// 查询所有任务状态
    pub async fn list_statuses(&self) -> Vec<TaskStatus> {
        let statuses = self.statuses.lock().await;
        let mut list: Vec<TaskStatus> = statuses.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}

async fn run_task_loop(
    task: &dyn ScheduledTask,
    statuses: Arc<Mutex<HashMap<String, TaskStatus>>>,
    shutdown: Arc<AtomicBool>,
) {
    let name = task.name().to_string();
    let interval = task.interval_secs();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            tracing::info!(task = name.as_str(), "task shutting down");
            return;
        }

        // Mark running
        {
            let mut map = statuses.lock().await;
            if let Some(s) = map.get_mut(&name) {
                s.is_running = true;
            }
        }

        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(TASK_TIMEOUT_SECS),
            task.run_once(),
        )
        .await;
        let elapsed = start.elapsed().as_millis() as u64;

        // Update status
        {
            let mut map = statuses.lock().await;
            if let Some(s) = map.get_mut(&name) {
                s.is_running = false;
                s.last_run_at = Some(Utc::now().to_rfc3339());
                s.last_elapsed_ms = Some(elapsed);
                s.total_runs += 1;
                match result {
                    Ok(Ok(r)) => {
                        s.last_result = Some(format!("processed={}, succeeded={}", r.processed, r.succeeded));
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
                        s.last_result = None;
                        s.last_error = Some(e.to_string());
                        tracing::error!(
                            task = name.as_str(),
                            elapsed_ms = elapsed,
                            error = %e,
                            "task failed"
                        );
                    }
                    Err(_) => {
                        s.last_result = None;
                        s.last_error = Some(format!("timed out after {}s", TASK_TIMEOUT_SECS));
                        tracing::error!(
                            task = name.as_str(),
                            elapsed_ms = elapsed,
                            "task timed out"
                        );
                    }
                }
            }
        }

        // Sleep with shutdown check
        for _ in 0..interval {
            if shutdown.load(Ordering::Relaxed) {
                tracing::info!(task = name.as_str(), "task shutting down during sleep");
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
```

- [ ] **Step 2: Update implt/mod.rs**

Replace `mod stock_alert_worker;` and `pub use stock_alert_worker::StockAlertWorker;` with:

```rust
mod task_scheduler;

pub use task_scheduler::TaskScheduler;
```

Note: `stock_alert_task` will be added in Task 3.

- [ ] **Step 3: Verify compilation**

Run: `cargo clippy -p abt 2>&1 | tail -5`
Expected: compiles (StockAlertWorker references in server.rs will break — that's fine, fixed in Task 5)

- [ ] **Step 4: Commit**

```bash
git add abt/src/implt/task_scheduler.rs abt/src/implt/mod.rs
git commit -m "feat(implt): add TaskScheduler with per-task spawn, status tracking, and timeout"
```

---

### Task 3: Refactor StockAlertWorker → StockAlertTask

**Files:**
- Create: `abt/src/implt/stock_alert_task.rs`
- Delete: `abt/src/implt/stock_alert_worker.rs`
- Modify: `abt/src/implt/mod.rs`

- [ ] **Step 1: Create stock_alert_task.rs**

This is the existing `scan_once` logic wrapped in `ScheduledTask`. Remove the timeout (scheduler provides it), the run loop, and the shutdown handling.

```rust
//! 库存告警定时任务

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::CreateNotificationRequest;
use crate::repositories::{NotificationRepo, ProductWatcherRepo};
use crate::service::{ScheduledTask, TaskRunResult};

const NOTIFICATION_TYPE_STOCK_ALERT: &str = "stock_alert";
const RELATED_TYPE_PRODUCT: &str = "product";

pub struct StockAlertTask {
    pool: Arc<PgPool>,
}

impl StockAlertTask {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScheduledTask for StockAlertTask {
    fn name(&self) -> &str {
        "stock_alert"
    }

    fn interval_secs(&self) -> u64 {
        std::env::var("STOCK_ALERT_SCAN_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300)
    }

    async fn run_once(&self) -> anyhow::Result<TaskRunResult> {
        let low_stock_products =
            ProductWatcherRepo::find_watched_low_stock_products(&self.pool).await?;
        let scanned = low_stock_products.len();
        let mut alerts_sent = 0usize;

        for product in &low_stock_products {
            let current = product.current_quantity;
            let threshold = product.effective_safety_stock;
            let pid = product.product_id;

            let watchers = ProductWatcherRepo::find_watchers_by_product(&self.pool, pid).await?;
            if watchers.is_empty() {
                continue;
            }

            let watcher_ids: Vec<i64> = watchers.iter().map(|w| w.user_id).collect();
            let users_with_unread = NotificationRepo::batch_has_unread_alert(
                &self.pool,
                &watcher_ids,
                NOTIFICATION_TYPE_STOCK_ALERT,
                RELATED_TYPE_PRODUCT,
                pid,
            )
            .await?;

            for watcher in &watchers {
                if users_with_unread.contains(&watcher.user_id) {
                    continue;
                }

                let metadata = serde_json::json!({
                    "current_quantity": current.to_string(),
                    "safety_stock": threshold.to_string(),
                    "product_name": product.product_name,
                });

                let req = CreateNotificationRequest {
                    user_id: watcher.user_id,
                    notification_type: NOTIFICATION_TYPE_STOCK_ALERT.to_string(),
                    title: format!("库存告警: {} 库存不足", product.product_name),
                    content: Some(format!(
                        "产品「{}」当前库存 {}，低于安全库存 {}",
                        product.product_name, current, threshold
                    )),
                    related_type: Some(RELATED_TYPE_PRODUCT.to_string()),
                    related_id: Some(pid),
                    metadata: Some(metadata),
                };

                match NotificationRepo::insert(&self.pool, &req).await {
                    Ok(_) => alerts_sent += 1,
                    Err(e) => {
                        tracing::error!(
                            product_id = pid,
                            user_id = watcher.user_id,
                            error = %e,
                            "Failed to create stock alert notification"
                        );
                    }
                }
            }
        }

        Ok(TaskRunResult {
            processed: scanned,
            succeeded: alerts_sent,
            message: format!("扫描 {} 个低库存产品，发送 {} 条告警", scanned, alerts_sent),
        })
    }
}
```

- [ ] **Step 2: Update implt/mod.rs**

Add `mod stock_alert_task;` and `pub use stock_alert_task::StockAlertTask;`. The `stock_alert_worker` module was already removed in Task 2.

Final `abt/src/implt/mod.rs` should include:

```rust
mod task_scheduler;
mod stock_alert_task;

pub use task_scheduler::TaskScheduler;
pub use stock_alert_task::StockAlertTask;
```

Plus all the existing modules that weren't touched.

- [ ] **Step 3: Delete old worker file**

Delete `abt/src/implt/stock_alert_worker.rs`.

- [ ] **Step 4: Verify compilation**

Run: `cargo clippy -p abt 2>&1 | tail -5`
Expected: compiles with only pre-existing warnings

- [ ] **Step 5: Commit**

```bash
git add abt/src/implt/stock_alert_task.rs abt/src/implt/mod.rs
git rm abt/src/implt/stock_alert_worker.rs
git commit -m "refactor: replace StockAlertWorker with StockAlertTask implementing ScheduledTask"
```

---

### Task 4: Proto Definition + gRPC Handler

**Files:**
- Create: `proto/abt/v1/task_scheduler.proto`
- Create: `abt-grpc/src/handlers/task_scheduler.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`

- [ ] **Step 1: Create proto file**

Create `proto/abt/v1/task_scheduler.proto`:

```protobuf
syntax = "proto3";
package abt.v1;

option go_package = "abt/v1";

import "google/protobuf/empty.proto";

service AbtTaskSchedulerService {
  rpc ListTasks(google.protobuf.Empty) returns (ListTasksResponse);
  rpc TriggerTask(TriggerTaskRequest) returns (TriggerTaskResponse);
}

message TaskStatusProto {
  string name = 1;
  bool is_running = 2;
  optional string last_run_at = 3;
  optional uint64 last_elapsed_ms = 4;
  optional string last_result = 5;
  optional string last_error = 6;
  uint64 total_runs = 7;
  uint64 interval_secs = 8;
}

message ListTasksResponse {
  repeated TaskStatusProto tasks = 1;
}

message TriggerTaskRequest {
  string name = 1;
}

message TriggerTaskResponse {
  uint64 processed = 1;
  uint64 succeeded = 2;
  string message = 3;
}
```

- [ ] **Step 2: Build to regenerate generated code**

Run: `cargo build -p abt-grpc 2>&1 | tail -5`
Expected: compiles and regenerates `abt-grpc/src/generated/abt.v1.rs`

- [ ] **Step 3: Create handler file**

Create `abt-grpc/src/handlers/task_scheduler.rs`:

```rust
//! Task Scheduler gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_task_scheduler_service_server::AbtTaskSchedulerService as GrpcTaskSchedulerService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

pub struct TaskSchedulerHandler;

impl TaskSchedulerHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskSchedulerHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcTaskSchedulerService for TaskSchedulerHandler {
    async fn list_tasks(
        &self,
        _request: Request<()>,
    ) -> GrpcResult<ListTasksResponse> {
        let state = AppState::get().await;
        let statuses = state.task_scheduler().list_statuses().await;

        Ok(Response::new(ListTasksResponse {
            tasks: statuses.into_iter().map(status_to_proto).collect(),
        }))
    }

    async fn trigger_task(
        &self,
        request: Request<TriggerTaskRequest>,
    ) -> GrpcResult<TriggerTaskResponse> {
        let req = request.into_inner();
        if req.name.is_empty() {
            return Err(error::validation("name", "任务名称不能为空"));
        }

        let state = AppState::get().await;
        let result = state
            .task_scheduler()
            .trigger(&req.name)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(TriggerTaskResponse {
            processed: result.processed as u64,
            succeeded: result.succeeded as u64,
            message: result.message,
        }))
    }
}

fn status_to_proto(s: abt::TaskStatus) -> TaskStatusProto {
    TaskStatusProto {
        name: s.name,
        is_running: s.is_running,
        last_run_at: s.last_run_at,
        last_elapsed_ms: s.last_elapsed_ms,
        last_result: s.last_result,
        last_error: s.last_error,
        total_runs: s.total_runs,
        interval_secs: s.interval_secs,
    }
}
```

- [ ] **Step 4: Update handlers/mod.rs**

Add `pub mod task_scheduler;` after `pub mod notification;`.

Add to the `pub use crate::generated::abt::v1` block:

```
abt_task_scheduler_service_server::AbtTaskSchedulerServiceServer,
```

- [ ] **Step 5: Verify compilation**

Run: `cargo clippy 2>&1 | tail -10`
Expected: compiles (server.rs references to old `StockAlertWorker` still broken — fixed in Task 5)

- [ ] **Step 6: Commit**

```bash
git add proto/abt/v1/task_scheduler.proto abt-grpc/src/handlers/task_scheduler.rs abt-grpc/src/handlers/mod.rs abt-grpc/src/generated/abt.v1.rs
git commit -m "feat(handler): add TaskScheduler gRPC service with ListTasks and TriggerTask"
```

---

### Task 5: Integration — server.rs, AppState, lib.rs

**Files:**
- Modify: `abt-grpc/src/server.rs`
- Modify: `abt/src/lib.rs`

- [ ] **Step 1: Add task_scheduler() method to AppState**

In `abt-grpc/src/server.rs`, in `AppState` struct, add field:

```rust
use std::sync::atomic::AtomicBool;

pub struct AppState {
    abt_context: &'static abt::AppContext,
    task_scheduler: Arc<abt::implt::TaskScheduler>,
}
```

Add method:

```rust
pub fn task_scheduler(&self) -> Arc<abt::implt::TaskScheduler> {
    Arc::clone(&self.task_scheduler)
}
```

- [ ] **Step 2: Update AppState::init() to create scheduler**

Replace the current init body. After creating the pool and initializing context, build the scheduler:

```rust
pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    let config = get_config();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.max_connection)
        .connect(&config.database_url)
        .await?;

    abt::init_context_with_pool(pool).await;

    let ctx = abt::get_context().await;
    let pool_arc = std::sync::Arc::new(ctx.pool().clone());
    let shutdown = std::sync::Arc::new(AtomicBool::new(false));

    let mut scheduler = abt::implt::TaskScheduler::new(shutdown);
    scheduler.register(abt::implt::StockAlertTask::new(pool_arc));
    scheduler.start().await;

    let state = Arc::new(AppState {
        abt_context: ctx,
        task_scheduler: Arc::new(scheduler),
    });

    APP_STATE
        .set(state)
        .map_err(|_| "AppState already initialized")?;

    Ok(())
}
```

- [ ] **Step 3: Remove old worker spawn block in start_server()**

Delete the entire block:

```rust
// Start stock alert worker
{
    let state = AppState::get().await;
    let pool = state.pool();
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = abt::implt::StockAlertWorker::new(std::sync::Arc::new(pool), shutdown);
    tokio::spawn(async move {
        worker.run().await;
    });
    tracing::info!("StockAlertWorker spawned");
}
```

This is now handled inside `AppState::init()`.

- [ ] **Step 4: Register TaskScheduler gRPC service**

In the `Server::builder()` chain in `start_server()`, add before `.serve(addr)`:

```rust
.add_service(crate::handlers::AbtTaskSchedulerServiceServer::with_interceptor(
    crate::handlers::task_scheduler::TaskSchedulerHandler::new(), auth_interceptor,
))
```

- [ ] **Step 5: Update lib.rs public API**

The `TaskScheduler` needs to be accessible from `abt-grpc`. In `abt/src/lib.rs`, the `pub use implt::*;` pattern won't work because `implt` only exports specific types. Verify that `TaskScheduler` and `StockAlertTask` are in `pub use` from `implt/mod.rs` (they are from Task 2 and Task 3).

Also ensure `TaskStatus` and `TaskRunResult` are re-exported via `pub use service::*;` (they are from Task 1).

- [ ] **Step 6: Verify full compilation**

Run: `cargo clippy 2>&1 | tail -10`
Expected: compiles with only pre-existing warnings

- [ ] **Step 7: Commit**

```bash
git add abt-grpc/src/server.rs abt/src/lib.rs
git commit -m "feat(server): integrate TaskScheduler into AppState, replace worker spawn"
```
