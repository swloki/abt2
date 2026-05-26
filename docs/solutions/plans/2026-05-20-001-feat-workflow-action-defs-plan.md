---
title: "feat: 工作流 Action 定义查询接口"
type: feat
status: active
date: 2026-05-20
origin: docs/superpowers/specs/2026-05-19-workflow-action-defs-design.md
---

# 工作流 Action 定义查询接口

## Summary

在 `AbtWorkflowService` 新增 `ListActionDefs` RPC，返回 ActionRegistry 中已注册 action 的元数据（名称、标签、输入参数、输出字段）。改造 `ActionRegistry` 使其同时存储 `ActionDef` 元数据，前端据此引导用户配置 auto_task 节点。

---

## Requirements

- R1. 新增 `ListActionDefs` RPC，返回所有已注册 action 的定义列表
- R2. 每个定义包含 name、label、description、inputs（FieldDef 列表）、outputs（FieldDef 列表）
- R3. FieldDef 包含 name、label、field_type、required、description
- R4. `ActionRegistry::register()` 签名扩展，同时注册 action 实现和元数据
- R5. 只搭机制，不注册具体 action

---

## Scope Boundaries

- 不注册具体 action 实现
- 不修改 `AutoAction` trait
- 不修改 `execute()` 执行流程
- 不修改前端组件

---

## Context & Research

### Relevant Code and Patterns

- `abt/src/implt/workflow_actions.rs` — `ActionRegistry` 当前只存 `HashMap<String, Arc<dyn AutoAction>>`，需增加 `defs` 字段
- `abt/src/implt/workflow_engine.rs` — `WorkflowEngine` 持有 `Arc<ActionRegistry>`，需暴露 `action_registry()` 访问器
- `proto/abt/v1/workflow.proto` — 已有 `AbtWorkflowService` 定义，在末尾追加新 RPC
- `abt-grpc/src/handlers/workflow.rs` — 现有 handler 模式：`AppState::get().await` → `state.workflow_service()`
- 其他 model → proto 转换模式：各 handler 文件中的 `xxx_to_proto()` 函数

### Patterns to follow

- Proto message + `cargo build` 自动生成 Rust 代码
- Model struct 与 proto message 分离，handler 层做转换
- 现有 RPC handler 模式（参考 `list_templates`）

---

## Key Technical Decisions

- **元数据硬编码在 Rust struct 中而非数据库**：action 的输入输出由代码逻辑决定，运行时修改无意义，硬编码是正确的
- **`ActionDef` 作为独立 struct 放在 `workflow_actions.rs` 中**：与 `ActionRegistry` 紧密耦合，无需单独 model 文件
- **`ActionRegistry` 内部用 `Vec<ActionDef>` 而非 `HashMap` 存储 defs**：`list_defs()` 返回列表即可，不需要按名称查找元数据（`is_registered` 已有）

---

## Implementation Units

### U1. Proto 定义 + 编译验证

**Goal:** 在 workflow.proto 中新增 ListActionDefs RPC 和相关 message 定义

**Requirements:** R1, R2, R3

**Dependencies:** None

**Files:**
- Modify: `proto/abt/v1/workflow.proto`

**Approach:**
- 在 `AbtWorkflowService` 末尾追加 `rpc ListActionDefs(ListActionDefsRequest) returns (ActionDefListResponse)`
- 新增 message：`ListActionDefsRequest`（空）、`ActionDefListResponse`、`ActionDef`、`FieldDef`
- 字段定义与 spec 一致

**Patterns to follow:**
- 现有 proto message 命名和字段编号模式

**Test scenarios:**
- Test expectation: none — proto 定义由 `cargo build` 编译时验证

**Verification:**
- `cargo build` 编译通过，生成的 Rust 代码包含新 message 类型

---

### U2. ActionDef/FieldDef struct + ActionRegistry 改造

**Goal:** 定义元数据 struct，改造 ActionRegistry 支持 action 定义注册和查询

**Requirements:** R4, R5

**Dependencies:** U1

**Files:**
- Modify: `abt/src/implt/workflow_actions.rs`

**Approach:**
- 新增 `ActionDef` struct（name, label, description, inputs: Vec\<FieldDef\>, outputs: Vec\<FieldDef\>）
- 新增 `FieldDef` struct（name, label, field_type, required, description）
- `ActionRegistry` 增加 `defs: Vec<ActionDef>` 字段
- `register()` 签名改为 `register(name, def: ActionDef, action: Arc<dyn AutoAction>)`
- 新增 `list_defs() -> &[ActionDef]`
- `WorkflowEngine` 新增 `pub fn action_registry() -> &ActionRegistry` 访问器

**Patterns to follow:**
- 现有 `ActionRegistry` 的方法风格

**Test scenarios:**
- Happy path: register 一个 action 带 ActionDef → list_defs() 返回包含该定义
- Happy path: register 多个 action → list_defs() 返回全部
- Edge case: 无注册 action → list_defs() 返回空切片

**Verification:**
- `cargo clippy` 通过

---

### U3. gRPC Handler 实现

**Goal:** 实现 ListActionDefs handler，从 ActionRegistry 读取定义并返回 proto 响应

**Requirements:** R1

**Dependencies:** U2

**Files:**
- Modify: `abt-grpc/src/handlers/workflow.rs`

**Approach:**
- 新增 `list_action_defs` 方法
- 通过 `AppState::get().await` → `state.workflow_service()` 获取 engine
- 调用 `engine.action_registry().list_defs()` 获取定义列表
- 转换 ActionDef/FieldDef 为 proto message 返回
- 新增 `action_def_to_proto()` 转换函数

**Patterns to follow:**
- 现有 `list_templates` handler 模式
- 现有 `xxx_to_proto()` 转换函数模式

**Test scenarios:**
- Happy path: 无注册 action → 返回空列表
- Happy path: 有注册 action → 返回包含完整定义的列表（含 inputs/outputs）

**Verification:**
- `cargo clippy` 通过
- gRPC 调用返回正确响应

---

## System-Wide Impact

- **Interaction graph:** 新增只读 RPC，不影响任何现有流程
- **API surface:** `AbtWorkflowService` 新增一个 RPC，proto 变更向后兼容
- **Unchanged invariants:** `AutoAction` trait、`execute()` 流程、Graph Linter 均不变

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-19-workflow-action-defs-design.md](docs/superpowers/specs/2026-05-19-workflow-action-defs-design.md)
- **前端需求文档:** `E:\work\front\abt_front\docs\api-requirement-workflow-action-defs.md`
- Related code: `abt/src/implt/workflow_actions.rs`, `abt/src/implt/workflow_engine.rs`, `proto/abt/v1/workflow.proto`
