# 定时任务调度框架设计

## 背景

ABT 系统目前有一个 `StockAlertWorker` 作为独立后台任务运行，没有统一的任务调度抽象。随着系统发展，会有更多定时任务需求（数据清理、报表生成等），需要一个通用的调度框架。

## 核心抽象

### ScheduledTask Trait

```rust
// abt/src/service/scheduled_task_service.rs

pub struct TaskRunResult {
    pub processed: usize,
    pub succeeded: usize,
    pub message: String,
}

#[async_trait]
pub trait ScheduledTask: Send + Sync {
    fn name(&self) -> &str;
    fn interval_secs(&self) -> u64 { 300 }
    async fn run_once(&self) -> anyhow::Result<TaskRunResult>;
}
```

- `name()` — 全局唯一标识，用于 gRPC 按名称触发/查询
- `interval_secs()` — 默认 300 秒（5 分钟），任务可覆盖
- `run_once()` — 纯业务逻辑，不管调度和生命周期

### TaskStatus（内存状态）

```rust
pub struct TaskStatus {
    pub name: String,
    pub is_running: bool,
    pub last_run_at: Option<String>,      // RFC3339
    pub last_elapsed_ms: Option<u64>,
    pub last_result: Option<String>,      // Ok 时的 message
    pub last_error: Option<String>,       // Err 时的错误信息
    pub total_runs: u64,
    pub interval_secs: u64,
}
```

## TaskScheduler 调度器

```rust
// abt/src/implt/task_scheduler.rs

pub struct TaskScheduler {
    tasks: Vec<Box<dyn ScheduledTask>>,
    statuses: Arc<Mutex<HashMap<String, TaskStatus>>>,
    shutdown: Arc<AtomicBool>,
    pool: Arc<PgPool>,
}
```

### 方法

| 方法 | 职责 |
|------|------|
| `register(task)` | 注册任务，初始化 status |
| `start()` | 为每个任务 spawn 独立 tokio task |
| `trigger(name)` | 按名称手动执行一次，同步等待结果 |
| `list_statuses()` | 返回所有任务当前状态 |

### 调度逻辑

每个任务 spawn 独立 tokio task：

```
loop {
    if shutdown → return
    match timeout(60s, task.run_once()) {
        Ok(Ok(result)) → update_status(success, processed, succeeded)
        Ok(Err(e))     → update_status(error, e.to_string())
        Err(_)         → update_status(timeout, "timed out after 60s")
    }
    sleep_with_shutdown_check(task.interval_secs())
}
```

- 每个任务互不干扰
- 60 秒超时保护，防止任务卡死
- shutdown 检查贯穿 sleep 期间（每秒检查一次）

## gRPC 管理接口

```protobuf
// proto/abt/v1/task_scheduler.proto

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

- 需要 auth interceptor（JWT 认证）
- `TriggerTask` 对不存在的 name 返回 NOT_FOUND

## StockAlertTask 重构

现有 `stock_alert_worker.rs` → `stock_alert_task.rs`

```rust
pub struct StockAlertTask {
    pool: Arc<PgPool>,
}

#[async_trait]
impl ScheduledTask for StockAlertTask {
    fn name(&self) -> &str { "stock_alert" }
    fn interval_secs(&self) -> u64 {
        std::env::var("STOCK_ALERT_SCAN_INTERVAL_SECS")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(300)
    }

    async fn run_once(&self) -> anyhow::Result<TaskRunResult> {
        // 现有 scan_once 逻辑，去掉 timeout 包装（scheduler 已提供）
    }
}
```

## server.rs 集成

```rust
// 替换现有 worker spawn 代码
let shutdown = Arc::new(AtomicBool::new(false));
let mut scheduler = TaskScheduler::new(pool.clone(), shutdown);
scheduler.register(Box::new(StockAlertTask::new(pool.clone())));
// 未来: scheduler.register(Box::new(DataCleanupTask::new(pool.clone())));
scheduler.start();
```

`AppState` 新增 `task_scheduler: Arc<TaskScheduler>` 字段和 `task_scheduler_service()` 方法。

## 文件变更清单

### 新增

| 文件 | 职责 |
|------|------|
| `abt/src/service/scheduled_task_service.rs` | trait + TaskRunResult + TaskStatus |
| `abt/src/implt/task_scheduler.rs` | TaskScheduler 实现 |
| `abt-grpc/src/handlers/task_scheduler.rs` | gRPC handler |
| `proto/abt/v1/task_scheduler.proto` | proto 定义 |

### 修改

| 文件 | 变更 |
|------|------|
| `abt/src/implt/stock_alert_worker.rs` → `stock_alert_task.rs` | 重构为实现 ScheduledTask |
| `abt/src/implt/mod.rs` | 替换 worker 为 task + 新增 task_scheduler |
| `abt/src/service/mod.rs` | 新增 scheduled_task_service |
| `abt/src/lib.rs` | 新增 get_task_scheduler 工厂函数 |
| `abt-grpc/src/handlers/mod.rs` | 新增 handler 导出 |
| `abt-grpc/src/server.rs` | AppState 集成，替换 worker spawn，注册 gRPC 服务 |

### 不需要

- 无新数据库迁移（状态纯内存）
- 无新 model 文件（TaskStatus 在 service 层定义）

## 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 触发方式 | 固定间隔 | 满足当前需求，实现简单 |
| 状态存储 | 内存 HashMap | 重启后重置可接受，避免额外表 |
| 管理接口 | 列表 + 手动触发 | 满足运维需求 |
| 任务注册 | 启动时静态注册 | 当前不需要动态增删 |
| 并发模型 | 每任务独立 tokio task | 任务间互不干扰 |
| 超时保护 | 60 秒 | scheduler 统一提供，任务本身不需要 |
