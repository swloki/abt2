---
title: "feat: Product Watchlist + Notification Center"
type: feat
status: active
date: 2026-05-07
origin: docs/superpowers/specs/2026-05-06-product-watchlist-notification-design.md
---

# feat: Product Watchlist + Notification Center

## Summary

为 ABT 系统实现产品关注列表和通用通知中心。用户可关注产品并设置自定义告警阈值（safety_stock_override），后台 Worker 每 5 分钟扫描库存低于阈值的产品，通过"回升再跌"去重策略推送告警通知到通用 notifications 表。通知中心支持分页查询、按类型过滤、标记已读、未读计数。关注相关 RPC 追加到现有 ProductService，通知中心为独立 gRPC 服务。

---

## Requirements

- R1. 关注 CRUD：用户关注/取消关注产品，查询自己的关注列表（含实时库存和告警状态）
- R2. 自定义阈值：用户可为单个关注产品设置 safety_stock_override，覆盖默认 safety_stock
- R3. 通用通知中心：notifications 表支持多种 type（stock_alert / system / 后续 approval 等）
- R4. 通知 CRUD：分页查询（支持 type/is_read/时间范围 filter）、标记已读（含 read_at）、全部已读（支持按 type 过滤）、未读计数（按 type 分组）
- R5. 告警 Worker：每 5 分钟扫描，库存低于 COALESCE(override, safety_stock) 时创建通知，metadata 含当前库存、阈值、产品名
- R6. 告警去重：回升再跌策略 + 未读去重，防抖动

**Origin document:** docs/superpowers/specs/2026-05-06-product-watchlist-notification-design.md

---

## Scope Boundaries

- 通知推送（邮件/消息）— 后续集成外部通知服务
- 通知偏好设置 — V2
- 批量关注/取消关注
- 扫描分片 / Redis 缓存

---

## Context & Research

### Relevant Code and Patterns

- **Repo 模式**: `abt/src/repositories/term_repo.rs` — `pub struct XxxRepo;` 无字段，静态方法，`Executor<'_>` 用于写操作，`&PgPool` 用于读操作
- **Service 模式**: `abt/src/service/term_service.rs` + `abt/src/implt/term_service_impl.rs` — `#[async_trait]` trait + `Arc<PgPool>` impl
- **Handler 模式**: `abt-grpc/src/handlers/term.rs` — `AppState::get().await` → service 调用，`#[require_permission]` 注解
- **Proto 模式**: `proto/abt/v1/term.proto` — `package abt.v1`, `AbtXxxService` 命名
- **Model 模式**: `abt/src/models/term.rs` — derive Serialize/Deserialize, 手动 FromRow 用于 JSONB 列
- **工厂方法**: `abt/src/lib.rs` — `get_*_service(ctx)` 函数
- **服务注册**: `abt-grpc/src/server.rs` — `AbtXxxServiceServer::with_interceptor(handler, interceptor)`
- **模块导出**: `mod xxx; pub use xxx::Xxx;` 统一模式

### Institutional Learnings

- **读后写加锁**: `SELECT FOR UPDATE` 防竞态（`docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`）
- **三层错误处理**: `err_to_status()` / `validation()` / `business_error()`（`docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`）
- **不用外键**: 应用层做引用检查

---

## Key Technical Decisions

- **关注 RPC 追加到 ProductService**: 关注是产品的附属操作，不单独建 service。关注列表查询 JOIN inventory 返回实时库存
- **通知为独立 gRPC 服务**: 通知中心有独立的 proto 和 handler，因为它是通用基础设施，不绑定产品
- **Worker 不用 CancellationToken**: 用 `std::sync::atomic::AtomicBool` + `std::sync::Arc` 做关闭信号，避免为单个 worker 新增 tokio-util 依赖
- **去重策略**: Worker 内部维护 `HashMap<(i64, i64), bool>` 跟踪产品是否曾回升，实现"回升再跌"去重
- **metadata JSONB**: 告警通知的 metadata 存放 `{current_quantity, safety_stock, product_name}`，前端无需额外查询

---

## Open Questions

### Resolved During Planning

- **阈值策略**: COALESCE(safety_stock_override, safety_stock)，用户可覆盖
- **去重策略**: 回升再跌（内存标记）+ 未读去重（DB 查询）
- **Worker 生命周期**: AtomicBool shutdown signal，不引入 tokio-util

### Deferred to Implementation

- **Worker 扫描间隔**: 环境变量 `STOCK_ALERT_SCAN_INTERVAL_SECS`，默认 300
- **metadata 中具体字段命名**: 实现时确定 JSON key

---

## Implementation Units

### Phase 1: Foundation

- U1. **Database Migration**

**Goal:** 创建 notifications 和 product_watchers 两张表及索引

**Requirements:** R1, R3, R4, R5

**Dependencies:** None

**Files:**
- Create: `abt/migrations/039_create_notifications.sql`
- Create: `abt/migrations/040_create_product_watchers.sql`

**Approach:**
- notifications 表：BIGSERIAL PK, user_id, type VARCHAR(32), title VARCHAR(256), content TEXT, related_type VARCHAR(64), related_id BIGINT, is_read BOOLEAN, read_at TIMESTAMPTZ, created_at TIMESTAMPTZ, metadata JSONB
- product_watchers 表：联合 PK (user_id, product_id), safety_stock_override BIGINT NULL, created_at, updated_at
- 3 个索引：product_watchers(product_id), notifications(user_id, is_read, created_at DESC), notifications(user_id, created_at DESC)
- 使用 CREATE TABLE IF NOT EXISTS + CREATE INDEX IF NOT EXISTS

**Patterns to follow:** `abt/migrations/003_create_warehouse_table.sql`

**Test scenarios:**
- Happy path: migration 应用成功，2 张表和 3 个索引存在
- 幂等性: 重复运行不报错

**Verification:** `cargo build` 通过

---

- U2. **Proto Definitions + Models**

**Goal:** 定义 notification.proto gRPC 服务，在 product.proto 追加关注 RPC，创建 Rust 数据模型

**Requirements:** R1, R3, R4

**Dependencies:** U1

**Files:**
- Create: `proto/abt/v1/notification.proto`
- Modify: `proto/abt/v1/product.proto` — 追加 WatchProduct / UnwatchProduct / ListWatchedProducts RPC
- Create: `abt/src/models/notification.rs`
- Modify: `abt/src/models/mod.rs`

**Approach:**
- notification.proto 定义 AbtNotificationService（ListNotifications / MarkAsRead / MarkAllAsRead / GetUnreadCount）
- product.proto 追加 3 个 RPC 到 AbtProductService
- WatchProductRequest 含可选 safety_stock_override 字段
- ListWatchedProductsResponse 返回 WatchedProduct（含 current_quantity, effective_safety_stock, is_alerting）
- GetUnreadCountResponse 含 total + by_type map
- MarkAllAsReadRequest 含可选 type 字段
- Notification 模型：Notification struct + NotificationType enum + NotificationQuery
- 手动 FromRow 处理 metadata JSONB 列

**Patterns to follow:**
- Proto: `proto/abt/v1/term.proto`
- Model: `abt/src/models/term.rs`

**Test scenarios:**
- Happy path: Notification 模型从 JSONB 行正确反序列化（metadata 为 JSON 对象）
- Happy path: NotificationType FromStr/Display 转换
- Edge case: metadata 为 NULL 时反序列化

**Verification:** `cargo build` 通过，proto 编译成功

---

- U3. **Repositories**

**Goal:** 实现 notification_repo 和 product_watcher_repo

**Requirements:** R1, R2, R4, R5

**Dependencies:** U1, U2

**Files:**
- Create: `abt/src/repositories/notification_repo.rs`
- Create: `abt/src/repositories/product_watcher_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

**Approach:**
- NotificationRepo：
  - insert（创建通知，含 metadata JSONB）
  - find_by_user（分页 + filter by type/is_read/时间范围）
  - mark_as_read（设置 is_read=true + read_at=now）
  - mark_all_as_read（支持可选 type 过滤）
  - count_unread_by_user（返回总数 + 按 type 分组）
- ProductWatcherRepo：
  - upsert（INSERT ON CONFLICT DO UPDATE safety_stock_override + updated_at，返回是否新建）
  - delete（DELETE WHERE user_id + product_id）
  - find_by_user（分页查询，JOIN products 获取 product_code/pdt_name）
  - find_by_user_with_inventory（JOIN inventory 获取实时库存，计算 effective_safety_stock 和 is_alerting）
  - find_watched_low_stock_products（Worker 专用：COALESCE 查询低库存产品）
- 导出所有 repo struct 和关键行类型到 mod.rs

**Patterns to follow:**
- `abt/src/repositories/term_repo.rs` — unit struct + static methods
- `abt/src/repositories/inventory_cascade_repo.rs` — JSONB 处理模式
- `abt/src/repositories/mod.rs` — mod + pub use 导出

**Test scenarios:**
- Happy path: upsert 新关注 → 返回 is_new=true
- Happy path: upsert 已有关注（更新 override）→ 返回 is_new=false
- Happy path: delete 取消关注 → 行不存在也不报错
- Happy path: notification insert → mark_as_read → read_at 非空
- Happy path: count_unread_by_user → 返回按 type 分组计数
- Edge case: find_by_user_with_inventory → 库存为 NULL 的产品

**Verification:** `cargo clippy` 通过

---

### Phase 2: Service + Handler

- U4. **Service Traits + Implementations**

**Goal:** 定义 NotificationService trait、ProductWatcherService trait 及其实现

**Requirements:** R1, R2, R4

**Dependencies:** U2, U3

**Files:**
- Create: `abt/src/service/notification_service.rs`
- Create: `abt/src/service/product_watcher_service.rs`
- Modify: `abt/src/service/mod.rs`
- Create: `abt/src/implt/notification_service_impl.rs`
- Create: `abt/src/implt/product_watcher_service_impl.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs` — 添加工厂方法

**Approach:**
- NotificationService trait：list_notifications, mark_as_read, mark_all_as_read, get_unread_count
- ProductWatcherService trait：watch_product（返回 bool is_new）, unwatch_product, list_watched_products
- Impl 持有 `Arc<PgPool>`，直接委托 repo
- watch_product 调用 repo.upsert 返回 is_new
- list_watched_products 调用 repo.find_by_user_with_inventory
- 工厂方法：get_notification_service + get_product_watcher_service

**Patterns to follow:**
- Service trait: `abt/src/service/term_service.rs`
- Service impl: `abt/src/implt/term_service_impl.rs`
- Factory: `abt/src/lib.rs` — `get_*_service(ctx)` 模式

**Test scenarios:**
- Happy path: watch_product 新关注 → is_new=true
- Happy path: watch_product 已关注（更新 override）→ is_new=false
- Happy path: unwatch_product 成功
- Happy path: list_watched_products → 包含实时库存和 is_alerting
- Happy path: notification CRUD 全流程

**Verification:** `cargo clippy` 通过

---

- U5. **gRPC Handlers**

**Goal:** 实现 notification handler，在 product handler 中追加关注 RPC

**Requirements:** R1, R4

**Dependencies:** U4

**Files:**
- Create: `abt-grpc/src/handlers/notification.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/src/handlers/product.rs` — 追加 Watch/Unwatch/ListWatched 实现
- Modify: `abt-grpc/src/server.rs` — 注册 AbtNotificationService

**Approach:**
- NotificationHandler：unit struct，实现 AbtNotificationService trait
  - ListNotifications → notification_service.list_notifications → proto 转换
  - MarkAsRead → notification_service.mark_as_read
  - MarkAllAsRead → notification_service.mark_all_as_read（支持 type 过滤）
  - GetUnreadCount → notification_service.get_unread_count → 返回 total + by_type map
- ProductHandler 追加 3 个方法：
  - WatchProduct → product_watcher_service.watch_product → 返回 is_new
  - UnwatchProduct → product_watcher_service.unwatch_product
  - ListWatchedProducts → product_watcher_service.list_watched_products
- 所有操作校验 user_id = 当前登录用户（从 auth context 取）
- 错误映射：anyhow → err_to_status，业务校验 → business_error

**Patterns to follow:**
- Handler: `abt-grpc/src/handlers/term.rs`
- Server registration: `abt-grpc/src/server.rs`

**Test scenarios:**
- Happy path: WatchProduct → 返回 is_new
- Happy path: ListWatchedProducts → 包含库存信息
- Happy path: ListNotifications 分页 → 返回通知列表
- Happy path: MarkAllAsRead + type 过滤 → 只标记指定类型
- Happy path: GetUnreadCount → 返回分组计数
- Error path: 操作他人通知 → PERMISSION_DENIED

**Verification:** `cargo clippy` 通过，gRPC 服务可启动

---

### Phase 3: Worker

- U6. **Stock Alert Worker**

**Goal:** 实现后台库存告警 Worker，每 5 分钟扫描，回升再跌去重

**Requirements:** R5, R6

**Dependencies:** U3, U4

**Files:**
- Create: `abt/src/implt/stock_alert_worker.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt-grpc/src/server.rs` — 启动 Worker

**Approach:**
- StockAlertWorker struct：持有 `Arc<PgPool>` + `Arc<AtomicBool>`（shutdown signal）
- run() 循环：
  1. 检查 shutdown signal，收到则退出
  2. 调用 repo.find_watched_low_stock_products 获取低库存产品列表
  3. 对每个产品，检查回升标记：
     - 维护 `HashMap<(i64), bool>`（product_id → 是否曾回升）
     - 当前库存 >= 阈值 → 标记"已回升"，跳过
     - 当前库存 < 阈值 + 有"已回升"标记 → 需要告警，清除标记
     - 当前库存 < 阈值 + 无"已回升"标记 → 检查是否有未读 stock_alert，有则跳过
  4. 对需要告警的产品，查找关注者，创建 notification（type='stock_alert'，metadata 含 current_quantity/safety_stock/product_name）
  5. 记录日志：扫描数、低库存数、发送数、耗时
  6. sleep scan_interval
- scan_interval 从环境变量读取，默认 300 秒
- 在 server.rs 的 start_server 中 tokio::spawn 启动，传入 shutdown signal

**Patterns to follow:**
- `abt-grpc/src/handlers/mod.rs:88` — tokio::spawn 示例
- `abt/src/implt/inventory_cascade_service_impl.rs` — 查询 + 处理模式

**Test scenarios:**
- Happy path: 产品库存 < 阈值 + 曾回升 → 创建告警通知
- Happy path: 产品库存 < 阈值 + 未回升 + 有未读告警 → 不创建
- Happy path: 产品库存 >= 阈值 → 标记回升，不创建告警
- Happy path: metadata JSON 含 current_quantity/safety_stock/product_name
- Edge case: 无关注产品 → Worker 正常空转
- Edge case: shutdown signal → Worker 退出循环

**Verification:** Worker 启动后可扫描并创建告警通知

---

## System-Wide Impact

- **Interaction graph:** 关注列表和通知中心是新子系统，通过 JOIN inventory 与现有库存表交互。不修改现有产品和库存的写入路径
- **Error propagation:** anyhow 错误通过 service → handler，按三层错误约定映射 gRPC status code
- **State lifecycle risks:** Worker 是首个后台任务，需确保 shutdown 时不丢失正在处理的告警
- **API surface parity:** 新增独立 notification.proto 服务 + product.proto 追加 3 个 RPC，不影响现有 API
- **Unchanged invariants:** 现有所有 CRUD 服务、gRPC handler、数据库表不受影响

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Worker 首次引入后台任务模式 | 使用 AtomicBool shutdown signal，代码库已有 tokio::spawn 示例 |
| 关注量大时 Worker 扫描慢 | V1 关注量预期较小；find_watched_low_stock_products 用 JOIN 一次过滤 |
| 回升标记在 Worker 重启后丢失 | 重启后无标记 = 退化为未读去重策略，行为安全 |
| metadata JSONB 字段膨胀 | 每条通知 metadata 不超过 200 字节 |

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-06-product-watchlist-notification-design.md](docs/superpowers/specs/2026-05-06-product-watchlist-notification-design.md)
- Institutional learning: `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`
- Institutional learning: `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`
