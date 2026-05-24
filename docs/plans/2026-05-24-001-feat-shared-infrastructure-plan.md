---
title: "feat: 共享基础设施层实现"
created: 2026-05-24
status: active
depth: deep
origin: docs/superpowers/specs/2026-05-24-shared-infrastructure-design.md
---

# feat: 共享基础设施层实现

## Problem Frame

ABT 系统需要在 `abt-core` crate 中实现 10 个共享基础设施组件（文档编号、文档关联、库存预留、成本累积、事件总线、状态机、审计日志、幂等去重、死信队列、身份认证），为后续所有业务模块（Sales/Purchase/WMS/MES/QMS/FMS/OM）提供统一的基础服务。

当前 `abt-core` crate 已有完整的目录骨架，但所有内容为空占位符。实现必须严格遵循 `docs/uml-design/00-shared-infrastructure.html` 设计文档，不修改 `abt` crate 代码。

**数据库策略**：`abt-core` 使用独立数据库 `abt_v2`（环境变量 `ABT_CORE_DATABASE_URL`），与旧 `abt` 数据库并行运行，互不干扰。迁移文件放在 `abt-core/migrations/` 下。

## Scope Boundaries

### In Scope

- `abt-core/src/shared/` 下全部 10 个组件的完整实现（types, enums, 8 个共享服务, identity 模块）
- `abt-core/Cargo.toml` 添加所需依赖（thiserror, argon2, jsonwebtoken）
- `abt-core/migrations/` 新建所有共享基础设施表（独立数据库 `abt_v2`）
- `abt-core/src/shared/mod.rs` 更新模块声明
- 单元测试覆盖每个服务的核心逻辑

### Out of Scope

- 修改 `abt` crate 代码（任何文件）
- Proto 定义和 gRPC handler（后续独立任务）
- 将 `abt-grpc` 接入 `abt-core` 的共享服务
- 迁移 `abt` crate 现有服务到 `abt-core`
- 前端代码变更
- 旧库与新库之间的数据同步策略

### Deferred to Follow-Up Work

- gRPC handler 层为共享服务暴露 Proto API
- `abt-grpc` server 从 `abt` 切换到 `abt-core` 共享服务
- 业务模块（sales/purchase/wms/mes）集成共享服务
- Phase 2 DataScope 行级数据权限实现
- `PERMISSION_ROUTE_MAP` gRPC 路由权限表

## Summary

在 `abt-core` 中实现完整的共享基础设施层：基础类型 + 枚举（批次 1）、独立服务（批次 2-3）、事件系统（批次 4）、状态机（批次 5）、身份认证（批次 6）、数据库迁移（批次 7）。共 8 个实现单元，约 80 个文件（含所有 mod.rs），实际新写约 60 个有内容的文件，严格遵循设计文档 `00-shared-infrastructure.html`。

## Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| 枚举 DB 存储 SMALLINT | 兼容 sqlx 和 Proto，比字符串节省空间，比 i32 更紧凑 |
| ServiceImpl 持有 Arc\<PgPool\> | 与 abt crate 工厂模式一致，允许跨方法事务管理 |
| DomainError 使用 thiserror | 自动 Display + Error 派生，`#[from] anyhow` 兼容老代码 |
| LISTEN/NOTIFY + 30s 轮询 | NOTIFY 低延迟驱动，轮询兜底防丢失（设计文档明确要求） |
| 独立数据库 abt_v2 | 新库与旧库完全隔离，无表结构冲突、无双写风险 |
| 迁移放 abt-core/migrations/ | abt-core 自管理数据库生命周期，不依赖 abt crate 的迁移目录 |
| 不使用外键约束 | 多态引用无法用 FK、双库架构跨库 FK 不可行、事件驱动与强 FK 矛盾，引用完整性由程序保证 |
| 权限缓存 fail-closed | `expect()` 硬失败，空缓存拒绝启动（历史安全事故教训） |

## System-Wide Impact

- **Database**: 新建独立数据库 `abt_v2`，包含全部共享基础设施表（document_sequences, document_links, inventory_reservations, cost_entries, domain_events, state_definitions, state_transition_defs, entity_state_logs, audit_logs, idempotency_records, users, roles, permissions, departments 等）
- **Environment**: 新增 `ABT_CORE_DATABASE_URL` 环境变量（abt_v2 连接串），与 `DATABASE_URL`（旧库）独立
- **Dependencies**: `abt-core` 新增 `thiserror`, `argon2`, `jsonwebtoken` 依赖
- **No abt crate impact**: 所有改动限于 `abt-core`，`abt` 编译和运行不受影响
- **Future consumers**: 所有业务模块（sales/purchase/wms 等）将依赖这些共享服务

---

## Output Structure

```
abt-core/src/shared/
  types/
    mod.rs
    context.rs              ← ServiceContext<'a>, DataScope
    error.rs                ← DomainError（tonic 映射在 abt-grpc 层）
    pagination.rs           ← PaginatedResult<T>, PageParams
    batch.rs                ← BatchResult, BatchMode, BatchFailure
    transaction.rs          ← TransactionMode
  enums/
    mod.rs
    document_type.rs        ← DocumentType (33 variants)
    link_type.rs            ← LinkType (7 variants)
    reservation.rs          ← ReservationType, ReservationStatus
    cost.rs                 ← CostType, CostEntityType
    event.rs                ← DomainEventType (19 variants), EventStatus
    audit.rs                ← AuditAction
    side_effect.rs          ← SideEffect
    sequence_strategy.rs    ← SequenceStrategy
  document_sequence/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  document_link/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  inventory_reservation/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  cost_entry/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  event_bus/
    mod.rs, model.rs, service.rs, implt/mod.rs
    registry.rs             ← EventHandlerRegistry + EventHandler trait
    processor.rs            ← EventProcessor
    dead_letter.rs          ← DeadLetterService trait + impl
  state_machine/
    mod.rs, model.rs, service.rs, implt/mod.rs
  audit_log/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  idempotency/
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  identity/
    mod.rs, model.rs, repo.rs, permission_cache.rs
    user_service.rs, role_service.rs, auth_service.rs
    permission_service.rs, department_service.rs
    implt/
      mod.rs, user_service_impl.rs, role_service_impl.rs
      auth_service_impl.rs, permission_service_impl.rs
      department_service_impl.rs
```

---

### U1. 基础类型和枚举

**Goal:** 建立所有共享服务依赖的基础类型层 — DomainError、ServiceContext、PaginatedResult、BatchResult、TransactionMode，以及全部共享枚举。

**Dependencies:** 无

**Files:**
- `abt-core/Cargo.toml` — 添加 thiserror, argon2, jsonwebtoken 依赖
- `abt-core/src/shared/types/mod.rs` — 创建
- `abt-core/src/shared/types/context.rs` — 创建
- `abt-core/src/shared/types/error.rs` — 创建
- `abt-core/src/shared/types/pagination.rs` — 创建
- `abt-core/src/shared/types/batch.rs` — 创建
- `abt-core/src/shared/types/transaction.rs` — 创建
- `abt-core/src/shared/enums/mod.rs` — 创建
- `abt-core/src/shared/enums/document_type.rs` — 创建
- `abt-core/src/shared/enums/link_type.rs` — 创建
- `abt-core/src/shared/enums/reservation.rs` — 创建
- `abt-core/src/shared/enums/cost.rs` — 创建
- `abt-core/src/shared/enums/event.rs` — 创建
- `abt-core/src/shared/enums/audit.rs` — 创建
- `abt-core/src/shared/enums/side_effect.rs` — 创建
- `abt-core/src/shared/enums/sequence_strategy.rs` — 创建
- `abt-core/src/shared/mod.rs` — 更新，添加 types 和 enums 模块

**Approach:**

DomainError 使用 `thiserror` 派生 8 个变体（NotFound, Duplicate, PermissionDenied, BusinessRule, Validation, ConcurrentConflict, InvalidStateTransition, Internal）。abt-core 不依赖 tonic，`From<DomainError> for tonic::Status` 映射放在 abt-grpc 层。Internal 变体的完整错误链只记录 `tracing::error!` 不返回客户端。

ServiceContext 包装 PgExecutor + operator_id + department_id + DataScope + trace_id + request_id。DataScope 为三变体枚举（All/Department/Self）。

所有枚举使用 `i16` discriminant，实现 `sqlx::Type<Postgres>` 通过 `#[sqlx(type_name = "smallint")]`。同时实现 `From<i16>` 和 `Into<i16>` 用于数据库读写。枚举还需 derive Clone, Copy, PartialEq, Eq, Hash, Debug。

DocumentType 的 `prefix()` 方法返回各单据类型的前缀字符串（如 Quotation→"QUO", SalesOrder→"SO"）。SequenceStrategy 用于 DocumentSequence。

**Patterns to follow:** `common/src/lib.rs` 的 PgExecutor 类型别名; `common/src/error.rs` 的错误映射模式

**Test scenarios:**
- DomainError 各变体的 Display 输出格式正确
- DomainError → tonic::Status 映射各变体的 Code 正确（在 abt-grpc 层测试）
- PaginatedResult::new() 计算 total_pages 正确（含边界值 page_size=0, total=0）
- BatchResult::from_atomic() 全成功返回空 failed_items
- DocumentType::from_i16() 已知值返回正确变体，未知值返回 None
- DocumentType::prefix() 各变体返回设计文档中的前缀
- 所有枚举 i16 round-trip 正确

**Verification:** `cargo clippy -p abt-core` 无警告；所有测试通过

---

### U2. DocumentSequenceService 实现

**Goal:** 实现单据编号生成服务，支持 Sequential（PREFIX-YYYY-MM-SEQ）和 Timestamp（x{unix_ts}）两种策略。

**Dependencies:** U1

**Files:**
- `abt-core/src/shared/document_sequence/mod.rs` — 更新
- `abt-core/src/shared/document_sequence/model.rs` — 填充
- `abt-core/src/shared/document_sequence/repo.rs` — 填充
- `abt-core/src/shared/document_sequence/service.rs` — 更新 trait
- `abt-core/src/shared/document_sequence/implt/mod.rs` — 填充实现

**Approach:**

Model 定义 `DocumentSequence` 结构体（id, prefix, current_value, seq_date, padding_len, strategy）。

Service trait 单方法 `next_number(ctx, doc_type) -> Result<String, DomainError>`。

Repo 使用 `DocumentType::prefix()` 映射到 prefix 列。Sequential 策略使用 `INSERT INTO document_sequences (prefix, seq_date, current_value, padding_len, strategy) VALUES ($1, CURRENT_DATE, 1, $2, 'sequential') ON CONFLICT (prefix, seq_date) DO UPDATE SET current_value = document_sequences.current_value + 1 RETURNING current_value` 原子操作。结果格式化为 `{prefix}-{YYYY}-{MM}-{seq:0>padding}`。`document_sequences` 表在 `abt_v2` 中按设计文档定义创建（prefix + seq_date UNIQUE）。

Timestamp 策略直接生成 `x{unix_timestamp}`，不访问数据库。

ServiceImpl 持有 `Arc<PgPool>`，`next_number` 从 pool 获取连接执行。

**Patterns to follow:** `abt/src/implt/document_sequence_service_impl.rs` 的服务实现结构; `abt-core/CLAUDE.md` 的模块内部结构约定

**Test scenarios:**
- Sequential 策略首次调用生成 `{PREFIX}-{年}-{月}-001`
- Sequential 策略第二次调用序号递增为 002
- 跨月调用序号从 001 重新开始
- Timestamp 策略生成 `x` 前缀 + Unix 时间戳格式
- 不支持的 DocumentType 返回 Validation 错误

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U3. AuditLogService + IdempotencyService 实现

**Goal:** 实现操作审计日志和幂等去重两个独立服务。

**Dependencies:** U1

**Files:**
- `abt-core/src/shared/audit_log/mod.rs` — 更新
- `abt-core/src/shared/audit_log/model.rs` — 填充
- `abt-core/src/shared/audit_log/repo.rs` — 填充
- `abt-core/src/shared/audit_log/service.rs` — 更新 trait
- `abt-core/src/shared/audit_log/implt/mod.rs` — 填充
- `abt-core/src/shared/idempotency/mod.rs` — 更新
- `abt-core/src/shared/idempotency/model.rs` — 填充
- `abt-core/src/shared/idempotency/repo.rs` — 填充
- `abt-core/src/shared/idempotency/service.rs` — 更新 trait
- `abt-core/src/shared/idempotency/implt/mod.rs` — 填充

**Approach:**

**AuditLog**: Model 定义 AuditLog 结构体 + AuditLogQuery（entity_type, operator_id, action, time_range 过滤）。Service trait 两个方法：record（同事务内写入）和 query_logs（分页查询）。record 接收 changes: Option\<Value\>，内部扫描 `sensitive: true` 标记并自动脱敏。Append-only，不提供删除或更新方法。

**Idempotency**: Model 定义 IdempotencyRecord。Service trait 三个方法：check_and_mark（INSERT ON CONFLICT DO NOTHING，返回 true=首次/false=重复）、mark_processed（更新状态和结果）、cleanup_expired（删除过期记录）。idempotency_key 格式 `{event_id}:{handler_name}`。

两个服务均持有 `Arc<PgPool>`。

**Patterns to follow:** 与 U2 相同的服务实现模式

**Test scenarios:**

AuditLog:
- record 写入成功返回 id
- record 的 changes 中 sensitive 字段被脱敏为 "***"
- query_logs 按 entity_type 过滤正确
- query_logs 按 time_range 过滤正确
- query_logs 分页参数正确（total, page, total_pages）

Idempotency:
- check_and_mark 首次调用返回 true
- check_and_mark 重复调用返回 false
- mark_processed 更新状态为 Processed
- cleanup_expired 删除 created_at < before 的记录

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U4. DocumentLinkService + InventoryReservationService + CostEntryService 实现

**Goal:** 实现三个实体型共享服务 — 文档关联图谱、库存预留、成本累积。

**Dependencies:** U1

**Files:**
- `abt-core/src/shared/document_link/mod.rs` — 更新
- `abt-core/src/shared/document_link/model.rs` — 填充
- `abt-core/src/shared/document_link/repo.rs` — 填充
- `abt-core/src/shared/document_link/service.rs` — 更新 trait
- `abt-core/src/shared/document_link/implt/mod.rs` — 填充
- `abt-core/src/shared/inventory_reservation/mod.rs` — 更新
- `abt-core/src/shared/inventory_reservation/model.rs` — 填充
- `abt-core/src/shared/inventory_reservation/repo.rs` — 填充
- `abt-core/src/shared/inventory_reservation/service.rs` — 更新 trait
- `abt-core/src/shared/inventory_reservation/implt/mod.rs` — 填充
- `abt-core/src/shared/cost_entry/mod.rs` — 更新
- `abt-core/src/shared/cost_entry/model.rs` — 填充
- `abt-core/src/shared/cost_entry/repo.rs` — 填充
- `abt-core/src/shared/cost_entry/service.rs` — 更新 trait
- `abt-core/src/shared/cost_entry/implt/mod.rs` — 填充

**Approach:**

**DocumentLink**: LinkRequest 结构体（source_type, source_id, target_type, target_id, link_type）。create_links 固定 Atomic 模式 — 使用事务批量 INSERT。path 物化路径在 repo 层自动计算。find_linked 支持双向查询（source→target 和 target→source）。

**InventoryReservation**: ReserveRequest 结构体（product_id, warehouse_id, qty, reservation_type, source_type, source_id, source_line_id, priority, expires_at）。reserve 固定 ContinueOnError 模式 — 逐条尝试，失败不影响其他行。每条 reserve 使用 `SELECT ... FOR UPDATE` 锁定相关行防超卖。fulfill 将 status 改为 Fulfilled，cancel 改为 Cancelled。total_reserved 聚合 status=Active 的 reserved_qty。

**CostEntry**: EntryRequest 结构体（entity_type, entity_id, cost_type, debit_amount, credit_amount, cost_center, profit_center, period, source_type, source_id）。create_entries 固定 Atomic 模式 — 双层记账必须完整。find_by_entity 按 entity_type + entity_id 查询，支持分页。

三个服务均持有 `Arc<PgPool>`。

**Test scenarios:**

DocumentLink:
- create_links 单条创建成功，path 正确生成
- create_links 多条原子创建（一条失败全部回滚）
- find_linked 按 source 查询返回正确结果

InventoryReservation:
- reserve 单条预留成功
- reserve 部分失败继续（ContinueOnError），返回 BatchResult 含失败项
- fulfill 将 status 改为 Fulfilled
- cancel 将 status 改为 Cancelled
- total_reserved 聚合 status=Active 的预留量

CostEntry:
- create_entries 原子创建多条
- create_entries 一条失败全部回滚（Atomic）
- find_by_entity 分页查询正确

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U5. 事件系统 — DomainEventBus + Registry + EventProcessor + DeadLetter

**Goal:** 实现完整的事件驱动架构 — 事件发布、Handler 注册/分发、后台消费者（LISTEN/NOTIFY + 轮询兜底）、死信队列。

**Dependencies:** U1, U3（IdempotencyService）

**Files:**
- `abt-core/src/shared/event_bus/mod.rs` — 更新
- `abt-core/src/shared/event_bus/model.rs` — 填充（DomainEvent, EventPublishRequest, EventQuery）
- `abt-core/src/shared/event_bus/service.rs` — 更新 DomainEventBus trait
- `abt-core/src/shared/event_bus/implt/mod.rs` — DomainEventBusImpl
- `abt-core/src/shared/event_bus/registry.rs` — 新建（EventHandlerRegistry + EventHandler trait）
- `abt-core/src/shared/event_bus/processor.rs` — 新建（EventProcessor）
- `abt-core/src/shared/event_bus/dead_letter.rs` — 新建（DeadLetterService trait + impl）

**Approach:**

**DomainEventBus**: publish 方法 — 生成 idempotency_key（None 时自动生成 `{aggregate_type}:{aggregate_id}:{event_type}`）→ INSERT domain_events ON CONFLICT DO NOTHING → NOTIFY `domain_event, '{id}'` → 返回 event_id。mark_processed 批量更新 status=Processed。mark_failed 递增 retry_count 并设置 reason。find_events 支持按 aggregate_type、event_type、status、since 过滤，返回 PaginatedResult。

**EventHandler trait**: `handle(ctx, event) -> Result<(), DomainError>` + `name() -> &str`。EventHandlerRegistry: HashMap<DomainEventType, Vec<Arc<dyn EventHandler>>>，dispatch 按事件类型查找所有注册 handler 并顺序调用。

**EventProcessor**: 持有 Arc\<PgPool\>、Arc\<dyn EventHandlerRegistry\>、Arc\<dyn IdempotencyService\>、Arc\<dyn DeadLetterService\>。start() spawn 后台 tokio task：建立专用 LISTEN 连接 → 收到通知后 FETCH FOR UPDATE SKIP LOCKED → check_and_mark → dispatch → mark_processed/mark_failed。30s 轮询兜底：扫描 status=PENDING 且 created_at < now()-30s。指数退避重试（2^n 秒，上限 60s）。retry_count > max_retries(3) → 标记 DEAD_LETTER。stop() 设置 running=false，等待 task 结束。

**DeadLetterService**: list_dead_letters 查询 status=DEAD_LETTER 分页返回。retry_one 将 status 重置为 PENDING、retry_count 重置为 0。archive 删除指定时间前的死信事件。

**Test scenarios:**

DomainEventBus:
- publish 写入事件并返回 id
- publish idempotency_key 去重（重复发布返回相同 id）
- mark_processed 批量更新状态
- find_events 按条件过滤正确

EventHandlerRegistry:
- register + dispatch 正确路由到 handler
- 未注册的事件类型 dispatch 不报错

EventProcessor:
- start/stop 生命周期管理
- 30s 轮询扫描超时事件
- 超过 max_retries 标记 DEAD_LETTER

DeadLetterService:
- list_dead_letters 查询死信事件
- retry_one 重置状态重新投递
- archive 删除过期死信

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U6. StateMachineService 实现

**Goal:** 实现统一状态机服务 — 状态定义、转换规则、SideEffect 执行、状态历史查询。

**Dependencies:** U1, U5（SideEffect 可触发事件发布）

**Files:**
- `abt-core/src/shared/state_machine/mod.rs` — 更新
- `abt-core/src/shared/state_machine/model.rs` — 填充（StateDefinition, StateTransitionDef, EntityStateLog, SideEffect）
- `abt-core/src/shared/state_machine/service.rs` — 更新 trait
- `abt-core/src/shared/state_machine/implt/mod.rs` — 填充

**Approach:**

Model 定义三个实体：StateDefinition（entity_type, state_name, label, is_initial, is_final）、StateTransitionDef（from_state, to_state, trigger_event, guard_condition, side_effects: Vec\<SideEffect\>）、EntityStateLog（entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark）。

SideEffect 枚举（PublishEvent, Notify, TriggerWorkflow, UpdateField）存为 JSONB，实现 serde Serialize/Deserialize。

StateMachineServiceImpl 持有 `Arc<PgPool>`。configure() 批量 UPSERT 状态定义和转换规则。transition() 五步校验：查当前状态 → 匹配转换规则 → 校验 guard（Phase 1 先跳过 guard 执行，仅检查是否存在）→ 插入 EntityStateLog → 执行 side_effects（Phase 1 仅 PublishEvent 通过 EventBus 实现）。get_current_state 从 entity_state_logs 取最新记录。get_allowed_transitions 从 state_transition_defs 查询。get_state_history 分页查询。

**Test scenarios:**
- configure 批量创建状态定义和转换规则
- transition 合法状态转换成功，写入 EntityStateLog
- transition 非法状态转换返回 InvalidStateTransition
- transition 不存在的实体返回 NotFound
- get_current_state 返回最新状态
- get_allowed_transitions 返回所有可转换目标状态
- get_state_history 分页查询正确

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U7. Identity & Access 模块

**Goal:** 实现完整的身份认证与权限管理 — User/Role/Department CRUD、JWT 认证、RBAC 权限校验、RolePermissionCache。

**Dependencies:** U1, U3（AuditLog 集成）; 需要 `abt-core/Cargo.toml` 添加 `argon2` 和 `jsonwebtoken` 依赖

**Files:**
- `abt-core/src/shared/identity/mod.rs` — 新建
- `abt-core/src/shared/identity/model.rs` — 新建
- `abt-core/src/shared/identity/repo.rs` — 新建
- `abt-core/src/shared/identity/permission_cache.rs` — 新建
- `abt-core/src/shared/identity/user_service.rs` — 新建
- `abt-core/src/shared/identity/role_service.rs` — 新建
- `abt-core/src/shared/identity/auth_service.rs` — 新建
- `abt-core/src/shared/identity/permission_service.rs` — 新建
- `abt-core/src/shared/identity/department_service.rs` — 新建
- `abt-core/src/shared/identity/implt/mod.rs` — 新建
- `abt-core/src/shared/identity/implt/user_service_impl.rs` — 新建
- `abt-core/src/shared/identity/implt/role_service_impl.rs` — 新建
- `abt-core/src/shared/identity/implt/auth_service_impl.rs` — 新建
- `abt-core/src/shared/identity/implt/permission_service_impl.rs` — 新建
- `abt-core/src/shared/identity/implt/department_service_impl.rs` — 新建
- `abt-core/src/shared/mod.rs` — 更新添加 identity 模块

**Approach:**

Model 定义 User（user_id, username, password_hash, display_name, is_active, is_super_admin）、Role（role_id, role_name, role_code, is_system_role, parent_role_id）、Department（department_id, department_name, department_code, description, is_active, is_default）、Claims（JWT payload）、AuthContext（gRPC 请求级）、ResourceActionDef（resource_code, action）。

**UserService**: create/update/delete/get/list/batch_assign_roles。密码使用 argon2 哈希。create 和 update 时调用 AuditLogService.record()。

**RoleService**: create/update/delete/list/assign_permissions/remove_permissions。assign_permissions 接受 `Vec<(String, String)>`（resource_code, action）元组。变更后触发 RolePermissionCache reload。

**AuthService**: login（验证密码 → 查角色部门 → 生成 JWT）、refresh_token、get_user_claims。JWT 使用 HMAC-SHA256（HS256），secret 从构造函数传入。Claims 只存 ID（sub, role_ids, department_ids），不存权限详情。

**PermissionService**: check_permission（单条）、batch_check_permissions（批量）、get_user_permissions。通过 RolePermissionCache 内存查询，不查数据库。super_admin 直接放行。

**DepartmentService**: create/update/delete/list/assign_departments/remove_departments。用户-部门多对多关系。

**RolePermissionCache**: `RwLock<HashMap<i64, HashSet<String>>>`。load() 启动时全量加载 role_permissions + DFS 解析 parent_role_id 继承，环检测 → hard fail。get_merged_permissions() 合并多角色权限。变更时 reload。

需要在 `abt-core/Cargo.toml` 添加 `argon2` 和 `jsonwebtoken` 依赖。

**Test scenarios:**

UserService:
- create_user 成功创建并返回 User（password 已哈希）
- create_user 重复 username 返回 Duplicate
- update_user 修改 display_name 成功
- delete_user 软删除（设 is_active=false）
- batch_assign_roles 分配多角色

RoleService:
- create_role 成功
- assign_permissions 分配权限后缓存 reload
- remove_permissions 移除权限后缓存 reload

AuthService:
- login 正确密码返回 JWT token
- login 错误密码返回 PermissionDenied
- refresh_token 刷新成功
- get_user_claims 返回 Claims 含 role_ids + department_ids

PermissionService:
- check_permission 有权限返回 true
- check_permission 无权限返回 false
- check_permission super_admin 返回 true
- batch_check_permissions 批量结果正确

DepartmentService:
- CRUD + assign/remove_departments 正确

RolePermissionCache:
- load() 启动加载成功
- get_merged_permissions 多角色合并正确
- 继承链解析正确
- 环检测 hard fail

**Verification:** `cargo clippy -p abt-core` 无警告；`cargo test -p abt-core` 通过

---

### U8. 数据库迁移

**Goal:** 创建独立数据库 `abt_v2` 的所有共享基础设施表和索引。

**Dependencies:** U1-U7（所有模型定义确定后）

**Files:**
- `abt-core/migrations/001_create_shared_infrastructure.sql` — 新建（全新数据库，从 001 开始编号）

**Approach:**

新数据库 `abt_v2` 完全独立，所有表按设计文档定义创建，无需考虑旧表兼容。

迁移开头需执行 `CREATE EXTENSION IF NOT EXISTS pg_trgm;` 以支持 document_links 的 trigram 索引。

表清单：
- `document_sequences` — prefix + seq_date UNIQUE，按设计文档定义
- `document_links` — source/target 双索引 + path trigram 索引
- `inventory_reservations` — product/warehouse/status 复合索引 + source 索引
- `cost_entries` — entity 复合索引 + period 索引
- `domain_events` — idempotency_key UNIQUE + status 部分索引 + aggregate 复合索引
- `state_definitions` — (entity_type, state_name) UNIQUE
- `state_transition_defs` — (entity_type, from_state, to_state) UNIQUE
- `entity_state_logs` — entity 复合索引
- `audit_logs` — entity 索引 + operator 索引 + created_at 索引
- `idempotency_records` — idempotency_key UNIQUE + (event_id, handler_name) 索引
- Identity 相关表（users, roles, permissions, departments, user_roles, user_departments, role_permissions）— 按设计文档定义

所有枚举列使用 SMALLINT（与 Rust 端 i16 对应）。JSONB 用于灵活字段（changes, payload, side_effects, guard_condition）。主键统一使用 `id`（BIGSERIAL）。

**不使用外键约束** — 引用完整性由程序层 Service trait 保证。原因：多态引用（entity_type + entity_id）无法用 FK 表达；双库架构跨库引用 FK 不可行；事件驱动最终一致性架构与强 FK 矛盾。

**Test scenarios:**
- 迁移文件 SQL 语法正确（通过 sqlx prepare 或手动验证）
- 所有索引不与现有索引冲突

**Verification:** 在 `abt_v2` 数据库上执行迁移成功

---

## Dependency Graph

```
U1 (types + enums)
├── U2 (DocumentSequence)
├── U3 (AuditLog + Idempotency)
├── U4 (DocLink + Reservation + CostEntry)
├── U5 (EventBus + Processor + DeadLetter) ← 依赖 U3 (Idempotency)
│   └── U6 (StateMachine) ← 依赖 U5 (SideEffect 可触发事件)
├── U7 (Identity) ← 依赖 U3 (AuditLog)
└── U8 (Migration) ← 依赖所有 U2-U7 的模型定义，在 abt_v2 新库上执行
```

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| 枚举 SQLX 映射编译错误 | Medium | Low | U1 先实现并验证 clippy 通过后再做后续 |
| LISTEN/NOTIFY 在 Windows 环境不工作 | Low | High | 30s 轮询兜底机制保证功能正常 |
| 双库并行期间数据一致性 | Medium | Medium | 后续制定同步策略，当前两个库完全独立 |
| 权限缓存启动失败阻塞整个服务 | Low | Critical | fail-closed 策略：.expect() 硬失败，明确错误信息 |
| abt-core 与 abt 类型冲突 | Low | Medium | 完全隔离，abt-core 不引用 abt 的任何类型，各自连各自的数据库 |

## Assumptions

- `abt-core` 使用独立数据库 `abt_v2`，通过 `ABT_CORE_DATABASE_URL` 环境变量连接
- `abt-core` 不需要引用 `abt` crate 的任何类型（依赖方向单向）
- `common` crate 的 `PgExecutor` 类型别名足够，不需要在 abt-core 中重新定义
- Phase 1 实现中 StateMachine 的 guard_condition 仅检查是否存在，不实际执行 AST 评估
- Phase 1 实现中 SideEffect 的 Notify 和 TriggerWorkflow 为空操作（后续集成通知和工作流）
- Identity 模块的 argon2 密码哈希和 JWT 生成是全新实现，读写 `abt_v2` 中的用户/角色表
- 新库表的主键统一为 `id`（BIGSERIAL），不使用外键约束，引用完整性由 Service trait 保证
- document_links 的 trigram 索引需要 `pg_trgm` 扩展，迁移文件中会 `CREATE EXTENSION IF NOT EXISTS pg_trgm`
- 旧库与新库之间的数据同步策略后续单独制定
