---
title: "feat: H3Yun ERP Sync — 产品和库存单向同步"
type: feat
status: active
date: 2026-05-10
origin: docs/superpowers/specs/2026-05-08-h3yun-sync-design.md
---

# feat: H3Yun ERP Sync — 产品和库存单向同步

## Summary

在 `abt` crate 新增自包含的 `h3yun` 模块，通过 tokio channel 统一定时/手动/实时三种触发方式，使用独立映射表 `h3yun_sync_state` 跟踪同步状态（ObjectId 映射 + 水印），将产品和库存数据单向同步到 H3Yun ERP。不修改现有表结构，删除功能时直接移除模块和映射表。

---

## Requirements

- R1. 产品信息单向同步 ABT → H3Yun（schema `D000119Product_sale`），支持创建和更新
- R2. 库存记录单向同步 ABT → H3Yun（schema `D000119warehouse`），支持创建和更新
- R3. 独立 `h3yun_sync_state` 映射表存储 ObjectId 映射和水印，不修改现有表
- R4. tokio::sync::mpsc channel 统一三种触发方式：实时（High）、手动 gRPC（Normal）、定时 5 分钟（Low）
- R5. 单个 sync_worker 消费 channel，逐记录错误隔离
- R6. SyncError 分类：Transient（重试）、ValidationError（跳过）、FatalError（中止批次）
- R7. 利用 ScheduledTaskService 注册定时任务，增量同步未同步实体
- R8. gRPC AbtSyncService：SyncProduct、SyncAllProducts、SyncInventory、Reconcile
- R9. 删除同步：ABT 删除产品时同步调用 H3Yun RemoveBizObject
- R10. 对账：从 H3Yun 读回数据与 ABT 状态对比，检测漂移
- R11. 启动时凭证验证 fail-closed，缺失凭证不静默继续
- R12. 同步功能计划后续删除，所有代码集中在 `h3yun` 模块

---

## Scope Boundaries

- 不修改现有表结构（products、inventory 等不加列）
- BOM 同步暂不实现（schema `D000119BomNodes`）
- 字段映射硬编码在 Rust 代码中（不做声明式配置）
- 不做双向同步或 CRDT
- 不做 rate limiting / circuit breaker
- 不做 Prometheus metrics 集成
- 不做 dry-run 模式

### Deferred to Follow-Up Work

- BOM 数据同步：二期实现，需新增字段映射和同步逻辑
- 字段映射配置化：当 H3Yun schema 变更频繁时再引入
- 自适应同步频率（蚂蚁信息素模式）：当前固定 5 分钟间隔足够

---

## Context & Research

### Relevant Code and Patterns

- **模块注册**: `abt/src/lib.rs` — 工厂函数模式 `get_xxx_service(ctx)` 返回 `impl Trait`
- **Service trait**: `abt/src/service/` — `#[async_trait]` + `Executor<'_>` 参数
- **Repository**: `abt/src/repositories/` — `PgExecutor<'a> = &'a mut PgConnection`，sqlx query_as
- **Model**: `abt/src/models/` — `FromRow` + JSONB meta 字段
- **gRPC handler**: `abt-grpc/src/handlers/` — `#[require_permission]` + `AppState::get().await`
- **Server**: `abt-grpc/src/server.rs` — `AppState` + `TaskScheduler` 注册 + `auth_interceptor`
- **ScheduledTask**: `abt/src/service/scheduled_task_service.rs` — trait with `name()`, `interval_secs()`, `timeout_secs()`, `run_once()`
- **TaskScheduler**: `abt/src/implt/task_scheduler.rs` — `RunningGuard` 防并发，`shutdown` 信号
- **Proto**: `proto/abt/v1/*.proto` — package `abt.v1`，`abt-grpc/build.rs` 自动编译
- **Migration**: `abt/migrations/041_*` 为最新，新迁移为 `042`
- **Product model**: `product_id: i64`，`meta: ProductMeta`（JSONB）
- **Handler mod.rs**: `abt-grpc/src/handlers/mod.rs` — pub mod + pub use generated types

### Institutional Learnings

- **Fail-closed on init**: `OnceLock` 单例必须 `.expect()` 初始化（`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）
- **三层错误处理**: `err_to_status()`（基础设施）、`validation()`（字段验证）、`business_error()`（业务规则，零日志）（`docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`）

### External References

- reqwest Client 自带连接池和 keep-alive，`Client` 内部使用 `Arc`
- H3Yun OpenApi 单端点 `Invoke`，4 种 ActionName

---

## Key Technical Decisions

- **entity_id 使用 BIGINT 而非 UUID**: 产品 ID 是 `i64`，映射表 `entity_id` 必须匹配
- **reqwest 仅添加到 abt/Cargo.toml**: HTTP 客户端逻辑在 abt crate，abt-grpc 通过依赖传递获取
- **channel bounded capacity 1000**: 平衡背压和吞吐，H3Yun 宕机时不炸内存
- **v1 不做优先级排序**: channel FIFO 足够，实时事件自然先于定时批量事件
- **定时任务仅查找 `last_synced_at IS NULL`**: products 表无 `updated_at` 列，无法做增量变更检测；依赖 channel 实时事件捕获变更
- **sync_worker 在 server.rs 启动时 spawn**: 生命周期与 gRPC 服务一致
- **Sender 通过 OnceLock 全局存储**: 与 Excel 单例模式一致，各触发源通过 `get_sync_event_sender()` 获取

---

## Open Questions

### Resolved During Planning

- **如何不污染现有表**: 使用独立 `h3yun_sync_state` 表，不修改 products/inventory 表
- **水印持久化**: 映射表的 `last_synced_at` 列跨重启保留
- **存在性查询**: 映射表存 ObjectId，有则 update 无则 create

### Deferred to Implementation

- **H3Yun API 响应格式**: 文档有限，实际响应结构需在对接时调整
- **LoadBizObjects 分页限制**: 不确定单次查询返回上限，实现时需处理分页
- **channel 去重策略**: 实现时决定是 worker 端去重还是 sender 端去重

---

## Output Structure

```
abt/src/h3yun/
  mod.rs              # 模块入口，OnceLock<Sender>，re-export
  models.rs           # SyncState, SyncEvent, SyncError, H3Yun request/response types
  client.rs           # H3YunClient — reqwest HTTP 封装
  sync_state.rs       # SyncStateRepo — 映射表 CRUD
  product_sync.rs     # sync_product() — 产品字段映射和同步流程
  inventory_sync.rs   # sync_inventory() — 库存字段映射和同步流程
  sync_worker.rs      # SyncWorker — channel 消费者 + 错误隔离
  scheduled.rs        # H3YunSyncTask — ScheduledTask 实现

proto/abt/v1/
  sync.proto          # AbtSyncService 定义

abt-grpc/src/handlers/
  sync_handler.rs     # gRPC handler

abt/migrations/
  042_create_h3yun_sync_state.sql
```

---

## High-Level Technical Design

> *方向性指导，非实现规范。实现 agent 应将其作为上下文而非要复制的代码。*

### 同步流程

```
触发源                     Channel                  Worker
─────────────────────     ──────────────           ────────────────────
CRUD (create/update) ──→  mpsc::channel    ──→  sync_worker.run()
gRPC (SyncProduct)   ──→  (bounded 1000)   ──→    ├─ query sync_state
ScheduledTask        ──→                   ──→    ├─ has ObjectId? → UpdateBizObject
                                                  └─ no ObjectId?  → CreateBizObject + save mapping
```

### 错误隔离

```
for each record:
  match result:
    Ok → update sync_state (last_synced_at, ObjectId)
    Transient → retry up to 3 times with backoff
    ValidationError → log warn, skip, continue
    FatalError → abort batch, report
```

### 映射表生命周期

```
Create product → CreateBizObject → save (entity_id, h3yun_object_id) to sync_state
Update product → lookup ObjectId → UpdateBizObject → update last_synced_at
Delete product → lookup ObjectId → RemoveBizObject → delete sync_state row
```

---

## Implementation Units

- U1. **依赖和数据库迁移**

**Goal:** 添加 reqwest 依赖，创建 `h3yun_sync_state` 映射表

**Requirements:** R3, R11

**Dependencies:** None

**Files:**
- Modify: `abt/Cargo.toml`
- Create: `abt/migrations/042_create_h3yun_sync_state.sql`

**Approach:**
- 在 `abt/Cargo.toml` 的 `[dependencies]` 添加 `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`
- 创建迁移文件，`entity_id` 使用 `BIGINT`（匹配 `product_id: i64`）

**Test scenarios:**
- Test expectation: none — 依赖添加和迁移文件，`cargo build` 验证编译通过

**Verification:**
- `cargo build -p abt` 编译成功
- 迁移文件 SQL 语法正确

---

- U2. **Models — 数据类型定义**

**Goal:** 定义 H3Yun 同步所需的所有数据类型

**Requirements:** R3, R4, R5, R6

**Dependencies:** U1

**Files:**
- Create: `abt/src/h3yun/models.rs`
- Create: `abt/src/h3yun/mod.rs`

**Approach:**
- `SyncState` struct 映射到 `h3yun_sync_state` 表行，含 `FromRow`
- `EntityType` enum（Product, Inventory）
- `SyncEvent` struct（entity_type, entity_id, priority）
- `Priority` enum（High, Normal, Low）
- `SyncError` enum（Transient, ValidationError, FatalError）
- H3Yun 请求/响应类型（`H3YunRequest`, `H3YunResponse`）
- `mod.rs` 声明子模块，定义 `OnceLock<Sender<SyncEvent>>` 和 `get_sync_event_sender()` 函数

**Patterns to follow:**
- `abt/src/models/product.rs` 的 `FromRow` + JSONB 模式
- `abt/src/lib.rs` 的 `OnceLock` / `OnceCell` 全局单例模式

**Test scenarios:**
- Happy path: `SyncState::from_row` 正确解析数据库行
- Happy path: `SyncEvent` 序列化/反序列化
- Edge case: `SyncState` 的 `h3yun_object_id` 和 `last_synced_at` 为 None 时正确处理

**Verification:**
- `cargo clippy -p abt` 无警告

---

- U3. **Sync State Repository — 映射表 CRUD**

**Goal:** 实现映射表的数据库操作

**Requirements:** R3

**Dependencies:** U2

**Files:**
- Create: `abt/src/h3yun/sync_state.rs`

**Approach:**
- `SyncStateRepo` 提供静态方法（与现有 repo 模式一致）
- `upsert(pool, entity_type, entity_id, h3yun_object_id)` — 插入或更新映射
- `find(pool, entity_type, entity_id) -> Option<SyncState>` — 查询单条映射
- `find_unsynced(pool, entity_type, limit) -> Vec<SyncState>` — 查询未同步实体（`last_synced_at IS NULL`）
- `update_synced(pool, id, h3yun_object_id)` — 更新 ObjectId 和 last_synced_at
- `delete(pool, entity_type, entity_id)` — 删除映射行（删除同步用）

**Patterns to follow:**
- `abt/src/repositories/product_repo.rs` — 使用 `PgPool` 和 `Executor<'_>` 的查询模式
- `abt/src/repositories/mod.rs` — `PgExecutor as Executor` 类型别名

**Test scenarios:**
- Happy path: upsert 新映射 → find 返回记录
- Happy path: upsert 已有映射 → ObjectId 和 last_synced_at 更新
- Edge case: find 不存在的映射 → 返回 None
- Happy path: find_unsynced 返回 `last_synced_at IS NULL` 的记录
- Happy path: delete 后 find 返回 None

**Verification:**
- `cargo clippy -p abt` 无警告
- SQL 查询语法正确（sqlx 编译期检查）

---

- U4. **H3Yun API Client**

**Goal:** 封装 H3Yun REST API 调用

**Requirements:** R1, R2, R11

**Dependencies:** U2

**Files:**
- Create: `abt/src/h3yun/client.rs`

**Approach:**
- `H3YunClient` 持有 `reqwest::Client`、endpoint、engine_code、engine_secret
- `new()` 从环境变量读取凭证，缺失时使用设计文档中的默认值，日志 warn
- `create(schema_code, biz_object_json) -> Result<String>` — 返回 ObjectId
- `update(schema_code, object_id, biz_object_json) -> Result<()>`
- `delete(schema_code, object_id) -> Result<()>`
- `query_list(schema_code, filter_json) -> Result<Vec<Value>>` — 对账用
- 所有方法向同一端点 POST，通过 `ActionName` 区分操作
- 请求头带 EngineCode + EngineSecret

**Patterns to follow:**
- `abt/src/lib.rs` 的 `OnceLock` 模式用于全局客户端实例

**Test scenarios:**
- Happy path: 请求体正确构造（ActionName, SchemaCode, BizObject 字段）
- Edge case: 环境变量缺失时使用默认凭证并 warn
- Error path: HTTP 超时返回 `SyncError::Transient`
- Error path: HTTP 401 返回 `SyncError::FatalError`
- Error path: H3Yun 返回业务错误返回 `SyncError::ValidationError`

**Verification:**
- `cargo clippy -p abt` 无警告

---

- U5. **产品同步逻辑**

**Goal:** 实现产品字段映射和 create/update/delete 同步流程

**Requirements:** R1, R9

**Dependencies:** U3, U4

**Files:**
- Create: `abt/src/h3yun/product_sync.rs`

**Approach:**
- `sync_product(pool, client, product) -> Result<(), SyncError>` 函数
- 从 Product 构造 H3Yun payload：product_code→Procode, pdt_name→Proname, meta.specification→Prospec, unit→Unit, meta.acquire_channel→huoqu, 固定值→Fa5124b
- 分类字段需查询 TermService 获取三级分类路径
- 流程：查映射表 → 有 ObjectId 则 UpdateBizObject，无则 CreateBizObject → 更新映射表
- `delete_product_sync(pool, client, product_id) -> Result<()>` 函数
- 删除流程：查映射表 → 有 ObjectId 则 RemoveBizObject + 删除映射行，无则跳过
- H3Yun 删除失败 warn 但不返回错误

**Patterns to follow:**
- 字段映射参考设计文档中的映射表
- 错误隔离：每条记录的 sync 包裹在独立错误边界中

**Test scenarios:**
- Happy path: 新产品同步 → CreateBizObject → 映射表写入 ObjectId
- Happy path: 已有产品同步 → UpdateBizObject → last_synced_at 更新
- Happy path: 产品删除 → RemoveBizObject → 映射行删除
- Edge case: 删除未同步产品 → 跳过 H3Yun 调用
- Error path: CreateBizObject 失败 → Transient 重试 / ValidationError 跳过
- Edge case: 分类为空时 Pgroup/PgroupM/PgroupS 字段处理

**Verification:**
- `cargo clippy -p abt` 无警告

---

- U6. **库存同步逻辑**

**Goal:** 实现库存字段映射和 create/update 同步流程

**Requirements:** R2

**Dependencies:** U3, U4

**Files:**
- Create: `abt/src/h3yun/inventory_sync.rs`

**Approach:**
- `sync_inventory(pool, client, inventory, location, warehouse, product) -> Result<(), SyncError>` 函数
- 从关联数据构造 H3Yun payload：location_code→KW20201118, warehouse_name→WH20201118, product_code→Pcode20201118, product_code→Name, pdt_name→pname, "期初导入"→Size, quantity→stockqty, unit→unit
- 流程与产品同步相同：查映射表 → create or update → 更新映射表
- 库存无删除同步（设计文档未提及）

**Patterns to follow:**
- 与 U5 产品同步相同的映射表查询和 create/update 模式

**Test scenarios:**
- Happy path: 新库存同步 → CreateBizObject → 映射表写入
- Happy path: 已有库存同步 → UpdateBizObject → last_synced_at 更新
- Error path: CreateBizObject 失败 → SyncError 分类处理

**Verification:**
- `cargo clippy -p abt` 无警告

---

- U7. **Sync Worker — Channel 消费者**

**Goal:** 实现 channel 消费、逐记录错误隔离

**Requirements:** R4, R5, R6

**Dependencies:** U5, U6

**Files:**
- Create: `abt/src/h3yun/sync_worker.rs`
- Modify: `abt/src/h3yun/mod.rs`

**Approach:**
- `SyncWorker` struct 持有 `Receiver<SyncEvent>`, `PgPool`, `H3YunClient`
- `run()` 循环：从 channel recv 事件，根据 entity_type 调用 product_sync 或 inventory_sync
- 每条记录独立错误处理，记录 processed/succeeded/failed
- Transient 错误重试最多 3 次（指数退避）
- ValidationError 记录 warn 后继续
- FatalError 中止当前处理循环
- `start_sync_channel(pool, client) -> Sender<SyncEvent>` 工厂函数：创建 channel，spawn worker task，返回 sender

**Patterns to follow:**
- `tokio::sync::mpsc` bounded channel
- `abt-grpc/src/server.rs` 中的 tokio::spawn 模式

**Test scenarios:**
- Happy path: 接收 Product 事件 → 调用 sync_product → 成功
- Happy path: 接收 Inventory 事件 → 调用 sync_inventory → 成功
- Error path: 单条记录 Transient 失败 → 重试 3 次后跳过，不影响后续记录
- Error path: 单条记录 ValidationError → 跳过，继续处理
- Edge case: channel 关闭 → worker 正常退出

**Verification:**
- `cargo clippy -p abt` 无警告

---

- U8. **Proto 定义 + 编译验证**

**Goal:** 定义 AbtSyncService proto，验证编译

**Requirements:** R8

**Dependencies:** None

**Files:**
- Create: `proto/abt/v1/sync.proto`

**Approach:**
- package `abt.v1`
- Service `AbtSyncService` with RPCs: SyncProduct, SyncAllProducts, SyncInventory, Reconcile
- Request/Response messages：SyncProductRequest（product_id）、SyncAllRequest（空）、SyncInventoryRequest（product_id）、SyncResponse（processed, succeeded, message）、ReconcileRequest（entity_type）、ReconcileResponse（drifts 列表）
- ReconcileResponse 包含 DriftItem message（entity_type, entity_id, drift_type, detail）
- 遵循现有 proto 命名约定（参考 `proto/abt/v1/product.proto` 等）

**Patterns to follow:**
- 现有 `proto/abt/v1/*.proto` 文件的 package、命名、message 结构

**Test scenarios:**
- Test expectation: none — proto 编译由 `cargo build -p abt-grpc` 验证

**Verification:**
- `cargo build -p abt-grpc` 编译成功，生成 `abt-grpc/src/generated/abt/v1/` 下的 sync 相关文件

---

- U9. **gRPC Handler + Server 注册**

**Goal:** 实现 gRPC handler 并注册到服务器

**Requirements:** R7, R8

**Dependencies:** U7, U8

**Files:**
- Create: `abt-grpc/src/handlers/sync_handler.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/server.rs`
- Modify: `abt/src/lib.rs`

**Approach:**
- `SyncHandler` 实现 `AbtSyncService` trait
- `SyncProduct`: 通过 `get_sync_event_sender()` 发送 High priority SyncEvent，立即返回 SyncResponse（异步处理）
- `SyncAllProducts`: 查询所有产品，批量发送 Normal priority SyncEvent
- `SyncInventory`: 查询指定产品的库存，发送 SyncEvent
- `Reconcile`: 调用 client.query_list，与映射表对比，返回 DriftItem 列表
- `server.rs`: 注册 `AbtSyncServiceServer::with_interceptor`，启动时调用 `start_sync_channel()` spawn worker
- `handlers/mod.rs`: 添加 `pub mod sync` 和 pub use generated type
- `lib.rs`: 添加 `pub mod h3yun` 和 `get_h3yun_client()` 工厂函数

**Patterns to follow:**
- `abt-grpc/src/handlers/product.rs` — handler 模式
- `abt-grpc/src/server.rs` — 服务注册模式
- `abt-grpc/src/handlers/mod.rs` — 模块导出模式

**Test scenarios:**
- Happy path: SyncProduct gRPC → event 发送到 channel → SyncResponse 返回
- Happy path: SyncAllProducts → 批量 event 发送
- Happy path: Reconcile → 返回漂移列表
- Edge case: channel full → SyncProduct 返回错误或等待
- Error path: H3Yun 凭证缺失 → 启动 fail-closed

**Verification:**
- `cargo clippy -p abt-grpc` 无警告
- 服务启动时 sync_worker task 正常 spawn

---

- U10. **定时任务 + 删除同步 + CRUD 集成**

**Goal:** 注册定时增量同步，在产品 CRUD 流程中注入 SyncEvent 发送

**Requirements:** R7, R9

**Dependencies:** U9

**Files:**
- Create: `abt/src/h3yun/scheduled.rs`
- Modify: `abt-grpc/src/server.rs`（注册定时任务）
- Modify: 产品删除流程（在 delete 方法中调用 delete_product_sync）

**Approach:**
- `H3YunSyncTask` 实现 `ScheduledTask` trait
  - `name()` → `"h3yun_sync"`
  - `interval_secs()` → `300`（5 分钟）
  - `timeout_secs()` → `600`（覆盖默认 60s）
  - `run_once()` → 查询 `find_unsynced`，批量发送 Low priority SyncEvent，返回 `TaskRunResult`
- 在 `server.rs` 的 `AppState::init()` 中注册 `H3YunSyncTask`
- 产品删除流程：在删除前调用 `h3yun::delete_product_sync()`，失败时 warn 不阻塞

**Patterns to follow:**
- `abt/src/implt/task_scheduler.rs` 中 `StockAlertTask` 的注册模式
- `abt-grpc/src/server.rs` 中 `scheduler.register()` 调用

**Test scenarios:**
- Happy path: 定时任务触发 → 查询未同步实体 → 发送 events → TaskRunResult
- Happy path: 产品删除 → 映射表查到 ObjectId → H3Yun RemoveBizObject
- Edge case: 无未同步实体 → TaskRunResult { processed: 0, succeeded: 0 }
- Edge case: H3Yun 删除失败 → warn 日志，ABT 删除不阻塞
- Integration: 定时任务注册后 TaskScheduler list_statuses 包含 "h3yun_sync"

**Verification:**
- `cargo clippy` 全 workspace 无警告
- `cargo test` 全部通过

---

- U11. **对账功能**

**Goal:** 实现读回验证，检测 ABT 与 H3Yun 的数据漂移

**Requirements:** R10

**Dependencies:** U4, U9

**Files:**
- Modify: `abt-grpc/src/handlers/sync_handler.rs`（Reconcile handler 实现）

**Approach:**
- 在 `sync_handler.rs` 中实现 `Reconcile` RPC
- 调用 `client.query_list(schema_code)` 获取 H3Yun 所有记录
- 遍历 H3Yun 记录，对每条记录查 `sync_state` 映射
  - 映射存在但 ABT 实体已不存在 → "幽灵记录"
  - 映射存在但关键字段不匹配 → "数据漂移"
- 查 `sync_state` 中有映射但 H3Yun 中无对应记录 → "同步丢失"
- 返回 DriftItem 列表，不自动修复

**Test scenarios:**
- Happy path: ABT 和 H3Yun 数据一致 → 空 drift 列表
- Edge case: H3Yun 有记录但 ABT 已删除 → 幽灵记录
- Edge case: ABT 有映射但 H3Yun 无记录 → 同步丢失
- Edge case: H3Yun 字段值与 ABT 不同 → 数据漂移
- Error path: H3Yun API 调用失败 → 返回错误

**Verification:**
- `cargo clippy` 无警告

---

## System-Wide Impact

- **Interaction graph:** 产品 CRUD 流程中注入 SyncEvent 发送（仅增加 channel send，不影响原有逻辑）
- **Error propagation:** H3Yun 同步失败不影响 ABT CRUD 操作（channel 解耦 + 删除失败 warn 不阻塞）
- **State lifecycle risks:** 映射表孤儿记录（ABT 实体删除但映射行残留）需通过 Reconcile 检测
- **API surface parity:** 新增 AbtSyncService，不影响现有 service
- **Integration coverage:** 定时任务 + channel worker + gRPC handler 的端到端集成
- **Unchanged invariants:** 现有 products/inventory 表结构、CRUD 接口、权限系统均不变

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| H3Yun API 响应格式未知 | 实现时需实际对接调试，响应解析留有调整空间 |
| products 表无 updated_at | 定时任务仅检测未同步实体，变更依赖 channel 事件 |
| H3Yun rate limit 未知 | Transient 错误重试 + backoff；可后续添加 rate limiter |
| 映射表与 ABT 实体不同步 | Reconcile 检测孤儿记录和数据漂移 |
| 60s 定时任务超时不足 | H3YunSyncTask 覆盖 timeout_secs() 为 600s |
| 凭证硬编码在代码/文档中 | 环境变量优先，缺失时 warn + 使用默认值 |

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-08-h3yun-sync-design.md](docs/superpowers/specs/2026-05-08-h3yun-sync-design.md)
- Related ideation: [docs/ideation/2026-05-08-h3yun-sync-ideation.md](docs/ideation/2026-05-08-h3yun-sync-ideation.md)
- ScheduledTask pattern: `abt/src/service/scheduled_task_service.rs`
- TaskScheduler pattern: `abt/src/implt/task_scheduler.rs`
- Fail-closed learning: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`
- Error handling learning: `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`
