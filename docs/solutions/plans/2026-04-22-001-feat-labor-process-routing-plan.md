---
title: "feat: Labor Process Routing & Auto-Routing on Import"
type: feat
status: active
date: 2026-04-22
origin: docs/brainstorms/2026-04-22-auto-routing-on-import-requirements.md
---

# feat: Labor Process Routing & Auto-Routing on Import

## Overview

引入工序字典、工艺路线、路线工序明细、BOM 路线映射四个新实体，以及在 Excel 导入时自动匹配/创建路线并绑定到产品的功能。解决人工成本管理中工序遗漏的问题。

## Problem Frame

当前 `bom_labor_process` 是扁平模型，每个产品独立维护工序列表。Excel 导入和手动录入都可能出现工序遗漏，且缺乏校验基准来判断一个产品的工序是否完整。需要引入标准化的工序定义和可复用的工艺路线模板，并在导入时自动建立校验基准。

## Requirements Trace

- R1. 全局工序字典 CRUD（`labor_process_dict` 表，工序编码 + 名称）
- R2. 工艺路线 CRUD（`routing` 表 + `routing_step` 工序明细，支持复用）
- R3. BOM 路线绑定（`bom_routing` 表，product_code → routing_id）
- R4. `bom_labor_process` 增加 `process_code` 列
- R5. Excel 导入增加"工序编码"列，校验编码是否存在于字典
- R6. 导入时若产品未绑定路线，自动匹配或创建路线并绑定
- R7. 导入响应增加 auto-routing 透明化信息
- R8. 向后兼容：未绑定路线的产品不做校验，导入照常进行

## Scope Boundaries

- 路线审核/审批流程 — 不包含
- 路线模板继承 — 不包含（独立需求）
- 路线变更迁移工具 — 不包含（独立需求）
- 手动录入时的路线校验提示 — 不包含（后续迭代）

## Context & Research

### Relevant Code and Patterns

- 分层架构：Proto → Model → Repository → Service trait → Service impl → Handler
- 工厂函数：`abt/src/lib.rs` 中 `get_*_service()` 模式
- 服务注册：`abt-grpc/src/server.rs` 中 `add_service()` 模式
- 现有导入流程：`abt/src/implt/labor_process_service_impl.rs` → `import_from_excel()`
- 权限宏：`#[require_permission(Resource::*, Action::*)]`，需处理 `Box::pin` 穿透

### Institutional Learnings

- 避免数据库外键，使用应用层引用检查（可返回友好中文错误）
- `sqlx::QueryBuilder::push_values` 闭包只有 `push_bind`，需原始 SQL 时用 `query_as`
- 三层错误处理：`err_to_status()` / `validation()` / `business_error()`
- 先读后写用 `SELECT ... FOR UPDATE` 防并发
- 编写回滚迁移

### External References

- 无需外部研究——本地模式充足

## Key Technical Decisions

- **不使用数据库外键**：所有关联为应用层逻辑，与项目约定一致
- **完全匹配路线**：路线匹配基于工序编码集合完全相等（不考虑顺序、is_required）
- **自动创建的路线命名**：`Auto-{product_code}-{YYYYMMDD}`
- **自动创建的步骤**：`is_required = true`，`step_order` 按 Excel 中出现顺序
- **导入响应扩展**：在 `ImportLaborProcessesResponse` 中增加可选字段，向后兼容

## Open Questions

### Deferred to Implementation

- 路线完全匹配的 SQL 查询性能优化策略（数据量小，初期可用子查询）
- Excel 模板中"工序编码"列的位置（建议放在第一列或第二列）

## Implementation Units

- [ ] **Unit 1: Database Migration**

**Goal:** 创建所有新表，为现有表添加 process_code 列

**Requirements:** R1, R2, R3, R4

**Dependencies:** None

**Files:**
- Create: `abt/migrations/026_add_labor_process_routing.sql`
- Create: `abt/migrations/027_rollback_labor_process_routing.sql`

**Approach:**
- 新建 `labor_process_dict`（工序字典）、`routing`（工艺路线）、`routing_step`（路线工序明细）、`bom_routing`（BOM 路线映射）四张表
- `bom_labor_process` 表增加 `process_code VARCHAR(50)` 列（允许 NULL）
- 编写回滚迁移

**Test scenarios:**
- Test expectation: none — 纯 SQL 迁移，通过后续单元测试验证

**Verification:**
- `cargo build` 通过，sqlx 编译时查询检查不报错

---

- [ ] **Unit 2: Proto Definitions**

**Goal:** 定义所有新服务和消息类型

**Requirements:** R1, R2, R3, R5, R7

**Dependencies:** None（可与 Unit 1 并行）

**Files:**
- Create: `proto/abt/v1/labor_process_dict.proto`
- Create: `proto/abt/v1/routing.proto`
- Modify: `proto/abt/v1/labor_process.proto` — 增加导入响应字段

**Approach:**
- `labor_process_dict.proto`：定义 `AbtLaborProcessDictService`（List/Create/Update/Delete）及相应消息
- `routing.proto`：定义 `AbtRoutingService`（List/Create/Update/Delete/GetDetail）及消息，routing 包含 step 列表
- `labor_process.proto`：`ImportLaborProcessesResponse` 增加 `auto_created_routing`、`matched_existing_routing`、`routing_name`、`routing_id` 字段；Excel 增加"工序编码"相关说明

**Patterns to follow:**
- `proto/abt/v1/labor_process.proto` 的消息结构风格
- 分页查询使用 `page`/`page_size` 字段

**Test scenarios:**
- Test expectation: none — Proto 定义，通过 `cargo build` 验证编译

**Verification:**
- `cargo build` 通过，生成的 Rust 代码无编译错误

---

- [ ] **Unit 3: Process Dictionary Full Stack**

**Goal:** 实现工序字典的完整 CRUD 功能

**Requirements:** R1

**Dependencies:** Unit 1, Unit 2

**Files:**
- Create: `abt/src/models/labor_process_dict.rs`
- Create: `abt/src/repositories/labor_process_dict_repo.rs`
- Create: `abt/src/service/labor_process_dict_service.rs`
- Create: `abt/src/implt/labor_process_dict_service_impl.rs`
- Create: `abt-grpc/src/handlers/labor_process_dict.rs`
- Modify: `abt/src/models/mod.rs` — 导出新 model
- Modify: `abt/src/repositories/mod.rs` — 导出新 repo
- Modify: `abt/src/service/mod.rs` — 导出新 service trait
- Modify: `abt/src/implt/mod.rs` — 导出新 impl
- Modify: `abt/src/lib.rs` — 添加 `get_labor_process_dict_service()` 工厂函数
- Modify: `abt-grpc/src/handlers/mod.rs` — 导出新 handler
- Modify: `abt-grpc/src/server.rs` — 注册新服务
- Modify: `abt-grpc/src/app_state.rs` — 添加 `labor_process_dict_service()` 方法

**Approach:**
- Model：`LaborProcessDict` struct（id, code, name, description, sort_order, created_at, updated_at）
- Repo：CRUD + 关键字搜索（ILIKE code/name）
- Service：标准 CRUD，删除前检查是否被 `routing_step` 引用
- Handler：使用 `#[require_permission]` 宏，处理 `Box::pin` 穿透

**Patterns to follow:**
- `abt/src/models/labor_process.rs` — Model 结构
- `abt/src/repositories/labor_process_repo.rs` — Repo 查询模式
- `abt-grpc/src/handlers/labor_process.rs` — Handler 模式

**Test scenarios:**
- Happy path: 创建工序字典条目（code="C001", name="车削"）→ 成功返回 id
- Happy path: 按关键字搜索 → 返回匹配结果
- Edge case: 创建重复 code → 返回错误
- Edge case: 创建重复 name → 返回错误
- Error path: 删除被 routing_step 引用的工序 → 返回引用检查错误
- Error path: code 或 name 为空 → 校验失败

**Verification:**
- `cargo test -p abt` 和 `cargo test -p abt-grpc` 通过
- gRPC reflection 可看到新服务

---

- [ ] **Unit 4: Routing Full Stack**

**Goal:** 实现工艺路线（含工序明细）的完整 CRUD 功能

**Requirements:** R2, R3

**Dependencies:** Unit 1, Unit 2, Unit 3（routing_step 引用 process_dict）

**Files:**
- Create: `abt/src/models/routing.rs`
- Create: `abt/src/repositories/routing_repo.rs`
- Create: `abt/src/service/routing_service.rs`
- Create: `abt/src/implt/routing_service_impl.rs`
- Create: `abt-grpc/src/handlers/routing.rs`
- Modify: `abt/src/models/mod.rs` — 导出新 model
- Modify: `abt/src/repositories/mod.rs` — 导出新 repo
- Modify: `abt/src/service/mod.rs` — 导出新 service trait
- Modify: `abt/src/implt/mod.rs` — 导出新 impl
- Modify: `abt/src/lib.rs` — 添加 `get_routing_service()` 工厂函数
- Modify: `abt-grpc/src/handlers/mod.rs` — 导出新 handler
- Modify: `abt-grpc/src/server.rs` — 注册新服务
- Modify: `abt-grpc/src/app_state.rs` — 添加 `routing_service()` 方法

**Approach:**
- Model：`Routing`（id, name, description, steps, created_at, updated_at）、`RoutingStep`（id, routing_id, process_code, step_order, is_required, remark）、`BomRouting`（id, product_code, routing_id, created_at, updated_at）
- Repo：
  - routing CRUD + 分页搜索
  - routing_step 批量插入/删除（事务内）
  - bom_routing：按 product_code 查询/设置/删除
  - **路线完全匹配查询**：查找 routing_id WHERE 其 routing_step 的 process_code 集合与给定集合完全一致
- Service：
  - CRUD routing（含 steps 的整体创建/更新，事务包裹）
  - `find_matching_routing(process_codes: Vec<String>)` — 查找完全匹配的路线
  - `set_bom_routing(product_code, routing_id)` / `get_bom_routing(product_code)`
  - 删除路线前检查是否被 bom_routing 引用
- Handler：使用 `#[require_permission]` 宏

**Patterns to follow:**
- `abt/src/implt/labor_process_service_impl.rs` — 事务使用模式
- `abt/src/repositories/labor_process_repo.rs` — 批量操作模式

**Test scenarios:**
- Happy path: 创建路线 + 多个工序步骤 → 成功
- Happy path: 更新路线步骤（增删改）→ 成功
- Happy path: 设置 BOM 路线绑定 → 成功
- Happy path: 查找完全匹配路线（已有路线包含 {C001, X002}，查询 {X002, C001}）→ 匹配成功
- Edge case: 同一路线中重复 process_code → UNIQUE 约束报错
- Edge case: 查找不匹配路线（查询 {C001, X002, M003}，只有 {C001, X002}）→ 返回 None
- Error path: 删除被 bom_routing 引用的路线 → 返回引用检查错误
- Integration: 创建路线 → 绑定到 BOM → 查询 BOM 路线 → 返回路线详情

**Verification:**
- `cargo test -p abt` 通过
- 路线 CRUD 和 BOM 绑定 API 可通过 gRPC 调用

---

- [ ] **Unit 5: Import Logic Modification**

**Goal:** 修改 Excel 导入逻辑，增加工序编码校验和自动路线匹配/创建

**Requirements:** R5, R6, R7, R8

**Dependencies:** Unit 3, Unit 4

**Files:**
- Modify: `abt/src/models/labor_process.rs` — BomLaborProcess 增加 process_code 字段
- Modify: `abt/src/repositories/labor_process_repo.rs` — 查询增加 process_code
- Modify: `abt/src/implt/labor_process_service_impl.rs` — 导入逻辑改造
- Modify: `abt-grpc/src/handlers/labor_process.rs` — 传递 auto-routing 信息

**Approach:**
- Model：`BomLaborProcess` 增加 `process_code: Option<String>`
- Excel 模板：增加"工序编码"列（放在"工序名称"之前或之后）
- 导入流程改造：
  1. 解析 Excel，提取每行的 process_code
  2. 校验所有 process_code 在 `labor_process_dict` 中存在 → 不存在则报错
  3. 检查 `bom_routing` 是否已有绑定
  4. 若未绑定：
     a. 提取不重复 process_code 集合
     b. 调用 `find_matching_routing()` 查找完全匹配路线
     c. 找到 → 绑定到已有路线
     d. 未找到 → 创建新路线 + 绑定
  5. 继续正常导入 bom_labor_process 记录（含 process_code）
  6. 返回导入结果 + auto-routing 信息
- 向后兼容：process_code 列允许为空，为空时跳过路线相关逻辑（保持原有行为）

**Patterns to follow:**
- `abt/src/implt/labor_process_service_impl.rs` — 现有导入流程
- `business_error()` 用于业务规则校验失败
- `SELECT ... FOR UPDATE` 用于先读后写场景

**Test scenarios:**
- Happy path: 导入含 process_code 的 Excel → 自动创建路线 + 成功导入
- Happy path: 导入含 process_code 的 Excel → 匹配到已有路线 + 成功导入
- Happy path: 产品已绑定路线 → 按现有校验逻辑执行
- Happy path: process_code 列为空 → 保持原有行为（向后兼容）
- Edge case: Excel 中有重复 process_code → 提取不重复集合进行匹配
- Error path: process_code 在字典中不存在 → 返回错误列出未知编码
- Integration: 首次导入 → 创建路线 → 再次导入同一产品 → 路线校验生效

**Verification:**
- `cargo test -p abt` 通过
- 手动测试：导入含工序编码的 Excel，验证路线自动创建和绑定

## System-Wide Impact

- **Interaction graph:** 导入流程现在依赖 RoutingService 和 LaborProcessDictService
- **Error propagation:** 未知 process_code 通过 `business_error()` 返回给前端
- **State lifecycle risks:** 路线创建和 BOM 绑定应在同一事务中完成，避免部分写入
- **API surface parity:** 导入响应增加可选字段，不影响现有客户端
- **Unchanged invariants:** 已绑定路线的产品导入行为不变；未绑定且无 process_code 的导入行为不变

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 路线匹配 SQL 性能 | 初期数据量小，使用子查询；后期可加缓存或 hash 列 |
| Excel 模板变更 | process_code 列允许为空，向后兼容旧模板 |
| 自动创建大量重复路线 | 完全匹配查询优先复用已有路线 |
| 并发导入同一产品 | 使用事务 + 行锁防止竞态 |

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-22-auto-routing-on-import-requirements.md](docs/brainstorms/2026-04-22-auto-routing-on-import-requirements.md)
- **Design spec:** [docs/superpowers/specs/2026-04-22-labor-process-routing-design.md](docs/superpowers/specs/2026-04-22-labor-process-routing-design.md)
- **Ideation:** [docs/ideation/2026-04-22-labor-process-routing-ideation.md](docs/ideation/2026-04-22-labor-process-routing-ideation.md)
