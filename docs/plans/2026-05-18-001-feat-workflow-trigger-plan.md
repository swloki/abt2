---
title: "feat: Workflow Trigger System"
created: 2026-05-18
status: active
plan-depth: lightweight
origin: docs/superpowers/specs/2026-05-18-workflow-trigger-design.md
---

# feat: Workflow Trigger System

## Problem

业务代码直接调 `start_instance()`，前端不知道有哪些触发事件可用，也无法配置事件与模板的绑定关系。

## Summary

在 `workflow_templates` 表加 `trigger_event` 列，后端代码静态定义触发事件（含 name/label/description），新增 `ListTriggerEvents` RPC 暴露给前端，新增 `trigger()` 方法供业务代码调用。

## Scope Boundaries

**In scope:**
- 数据库迁移：`trigger_event` 列 + 索引
- 后端 `TriggerEventDef` 注册表
- `ListTriggerEvents` RPC
- `trigger(event, entity_id, initiator_id)` 方法
- 创建/更新模板支持 `trigger_event` 字段

**Out of scope:**
- 前端 UI
- 一个触发器绑定多个模板
- 触发器条件过滤
- 触发器启用/禁用开关

---

## Implementation Units

### U1. 数据库迁移 — 添加 trigger_event 列

**Goal:** 为 `workflow_templates` 添加 `trigger_event` 字段和条件索引。

**Dependencies:** None

**Files:**
- Create: `abt/migrations/044_add_trigger_event.sql`

**Approach:**
- `ALTER TABLE workflow_templates ADD COLUMN trigger_event VARCHAR(100)`
- 条件索引 `WHERE status = 'active' AND deleted_at IS NULL`
- 同步更新 TS 迁移脚本 `scripts/create-workflow-tables.ts`

**Test expectation:** none — pure schema change, verified by subsequent units.

**Verification:** 本地执行迁移 SQL 无报错，`trigger_event` 列存在。

---

### U2. Model + Repository 层

**Goal:** 新增 `TriggerEventDef` 结构体、`all_trigger_events()` 函数、`find_active_by_trigger` 查询。

**Dependencies:** U1

**Files:**
- Modify: `abt/src/models/workflow.rs` — 新增 `TriggerEventDef`、`all_trigger_events()`
- Modify: `abt/src/repositories/workflow_template_repo.rs` — 新增 `find_active_by_trigger`
- Test: `abt/src/models/workflow.rs` (inline tests)

**Approach:**
- `TriggerEventDef { name, label, description }` 定义在 `workflow.rs`
- `all_trigger_events()` 返回 `&'static [TriggerEventDef]`，初始包含 `inventory_updated`
- `find_active_by_trigger(pool, trigger_event)` 查 `WHERE trigger_event = $1 AND status = 'active' AND deleted_at IS NULL`

**Patterns to follow:** `event_type` 常量模块模式（同文件），`WorkflowTemplateRepo` 现有查询模式。

**Test scenarios:**
- `all_trigger_events()` 返回非空，每项 name/label/description 非空
- `find_active_by_trigger` 查到活跃模板返回 Some
- `find_active_by_trigger` 无匹配返回 None

**Verification:** `cargo test -p abt -- workflow` 全通过。

---

### U3. Service trait + trigger() 实现

**Goal:** `WorkflowService` trait 新增 `trigger` 和 `list_trigger_events`，`WorkflowEngine` 实现之。

**Dependencies:** U2

**Files:**
- Modify: `abt/src/service/workflow_service.rs` — trait 新增两个方法
- Modify: `abt/src/implt/workflow_engine.rs` — 实现 `trigger()`、`list_trigger_events()`

**Approach:**
- `trigger(event, entity_id, initiator_id) -> Result<Option<i64>>`：按 trigger_event 查活跃模板，有则启动实例，无则返回 None
- `list_trigger_events() -> Result<Vec<TriggerEventDef>>`：返回 `all_trigger_events()` 并附带每个事件已绑定的活跃模板信息
- `trigger()` 复用现有 `start_instance` 的内部逻辑（创建实例 + 处理节点 + 记录 history）

**Patterns to follow:** 现有 `start_instance` 实现的事务模式。

**Test scenarios:**
- `trigger` 有绑定模板时返回 `Some(instance_id)`
- `trigger` 无绑定模板时返回 `None`（不报错）
- `list_trigger_events` 返回所有定义的事件及其绑定状态

**Verification:** `cargo test -p abt` 通过，`cargo clippy -p abt` 无错误。

---

### U4. Proto + gRPC Handler

**Goal:** 新增 `ListTriggerEvents` RPC，创建/更新模板支持 `trigger_event`。

**Dependencies:** U3

**Files:**
- Modify: `proto/abt/v1/workflow.proto` — 新增 RPC、消息、字段
- Modify: `abt-grpc/src/handlers/workflow.rs` — handler 实现
- Auto-generated: `abt-grpc/src/generated/abt.v1.rs`（cargo build 自动生成）

**Approach:**
- proto 新增 `ListTriggerEvents` RPC、`TriggerEventDef` 消息（name/label/description/bound_template_id/bound_template_name）
- `CreateWorkflowTemplateRequest` 加 `trigger_event` 字段
- `UpdateWorkflowTemplateRequest` 加 `trigger_event` 字段
- `WorkflowTemplateResponse` 加 `trigger_event` 字段
- Handler 的 `list_trigger_events` 调用 service 层
- Handler 的 create/update 传递 `trigger_event` 到 repo

**Patterns to follow:** 现有 `WorkflowHandler` 实现模式（extract_auth、GrpcResult、json_to_string）。

**Test scenarios:**
- `ListTriggerEvents` 返回事件列表 + 绑定模板信息
- `CreateTemplate` 带 `trigger_event` 可保存
- `UpdateTemplate` 带 `trigger_event` 可更新
- `GetTemplate` 返回 `trigger_event` 字段

**Verification:** `cargo build` 通过，`cargo clippy -p abt-grpc` 无错误。

---

## Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| trigger_event 可为空 | 是 | 不绑定触发器的模板仍可手动启动 |
| trigger 无绑定时 | 返回 None | 业务代码不需要 try-catch |
| TriggerEventDef 存储位置 | `workflow.rs` inline | 与其他 workflow model 保持一致 |
| list_trigger_events 鉴权 | 需要 auth interceptor | 只有登录用户才能查看触发器 |

## Deferred to Implementation

- `trigger()` 是否需要复用 `start_instance` 的内部逻辑还是直接调用——取决于代码结构
- TS 迁移脚本的同步更新细节
