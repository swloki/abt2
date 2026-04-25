---
title: "feat: BOM 物料替换功能"
type: feat
status: active
date: 2026-04-25
origin: docs/brainstorms/2026-04-25-bom-product-substitution-requirements.md
---

# feat: BOM 物料替换功能

## Overview

新增 BOM 物料替换 RPC，支持将指定物料（product_id）在单个或所有 BOM 中替换为新物料，并可选择性覆盖节点属性。BOM 节点存储在 JSONB 中，替换采用 load-modify-save 模式。

---

## Problem Frame

当物料停产、缺货或需要升级时，用户需要将 BOM 中的某个物料替换为另一个。目前只能逐个手动编辑 BOM 节点，效率低且容易遗漏。（参见 origin: `docs/brainstorms/2026-04-25-bom-product-substitution-requirements.md`）

---

## Requirements Trace

- R1. 将 BOM 中指定的旧物料替换为新物料，同时更新 product_code
- R2. 替换时可选择性覆盖节点属性（quantity、loss_rate、unit、remark、position、work_center、properties）
- R3. 同一物料在同一 BOM 中出现多次时，替换所有出现位置
- R4. 支持指定单个 BOM 进行替换
- R5. 支持不指定 BOM（替换所有使用了该物料的 BOM）
- R6. 返回替换结果摘要：受影响的 BOM 数量、替换的节点数量

---

## Scope Boundaries

- 不记录替换历史（无审计日志）
- 不需要预览/确认步骤
- 不涉及替代料管理
- 不涉及库存联动
- 不涉及 BOM 版本管理

---

## Context & Research

### Relevant Code and Patterns

- **BOM 节点存储方式**：`bom_detail` 是 JSONB 列，内含 `nodes: Vec<BomNode>`。节点修改采用 load-modify-save 模式（加载整个 BOM → 修改内存中的 nodes vec → 写回整个 JSONB）
- **现有节点修改模式**：`abt/src/implt/bom_service_impl.rs` 中 `update_node` 方法 — `find_by_id` → 修改 `nodes` → `update`
- **查找使用物料的 BOM**：`abt/src/repositories/bom_repo.rs` 中 `find_boms_using_product` — 使用 `jsonb_array_elements` + `EXISTS` 子查询。但只返回 `BomReference`（bom_id + bom_name），不返回完整 BOM 数据
- **Proto optional 约定**：代码库使用 `optional` 关键字表示可选字段，`convert.rs` 中 `empty_to_none()` 处理空字符串
- **Handler 事务模式**：`state.begin_transaction()` → service 调用 → `tx.commit()`
- **权限**：所有 BOM 写操作使用 `#[require_permission(Resource::Bom, Action::Write)]`

### Institutional Learnings

- 无直接相关的历史经验。并发写入 BOM 时可参考 `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md` 中的 SELECT FOR UPDATE 模式（本次替换功能每次只修改一个 BOM 的 JSONB，并发风险较低）

---

## Key Technical Decisions

- **Proto 属性覆盖使用 `optional` 字段**：每个可覆盖属性（quantity、loss_rate 等）定义为 `optional`，字段存在表示覆盖，不存在表示保持原值。符合代码库现有约定。（解决 origin 文档中的待定问题）
- **新增仓库方法返回完整 BOM 列表**：`find_boms_using_product` 只返回 `BomReference`，批量替换需要加载完整 BOM 数据。新增 `find_all_boms_using_product` 方法直接返回 `Vec<Bom>`，避免 N+1 查询。（解决 origin 文档中的待定问题）
- **单次替换在事务内完成**：所有 BOM 的修改在同一个事务中执行，保证原子性。如果某个 BOM 修改失败，整个操作回滚。
- **product_code 从新物料获取**：替换时自动查询新物料的 product_code 并更新节点，无需用户手动指定。

---

## Open Questions

### Resolved During Planning

- `find_boms_using_product` 不满足批量替换需求（只返回 BomReference）→ 新增 `find_all_boms_using_product` 仓库方法
- 属性覆盖的 proto 表达 → 使用 `optional` 字段，每个属性独立可选
- product_code 获取方式 → 使用 `ProductRepo::find_by_ids(&self.pool, &[new_product_id])` 查询产品，取出 product.meta.product_code（与现有 `load_bom_with_products` 方法模式一致）

### Deferred to Implementation

- 具体的 JSONB 替换 SQL 是否可行（直接在 SQL 层面用 `jsonb_set` 替换，还是保持 load-modify-save 模式）
- `find_all_boms_using_product` 方法签名：接受 `pool` 还是 `Executor`（影响是否在事务内读取）
- U4 事务范围：product_code 查询是否应包含在事务内

---

## Implementation Units

- [x] U1. **Proto 定义**

**Goal:** 定义 `SubstituteProduct` RPC 及其请求/响应消息

**Requirements:** R1, R2, R4, R5, R6

**Dependencies:** 无

**Files:**
- Modify: `proto/abt/v1/bom.proto`

**Approach:**
- 在 `AbtBomService` 中添加 `rpc SubstituteProduct(SubstituteProductRequest) returns (SubstituteProductResponse)`
- `SubstituteProductRequest`：`old_product_id`（必填）、`new_product_id`（必填）、`bom_id`（optional，不填表示所有 BOM）、可选属性覆盖字段（`optional double quantity`、`optional double loss_rate`、`optional string unit`、`optional string remark`、`optional string position`、`optional string work_center`、`optional string properties`）
- `SubstituteProductResponse`：`affected_bom_count`（int64）、`replaced_node_count`（int64）

**Patterns to follow:**
- `proto/abt/v1/bom.proto` 中 `BomResponse` 使用 `optional int64 bom_category_id` 的模式
- 其他 proto 文件中 `optional` 关键字的用法

**Test scenarios:**
- Test expectation: none — proto 定义通过 `cargo build` 编译验证

**Verification:**
- `cargo build -p abt-grpc` 编译通过，生成的 proto 文件包含新 RPC

---

- [x] U2. **仓库层：新增查询方法**

**Goal:** 新增仓库方法，查找所有包含指定 product_id 的完整 BOM 列表

**Requirements:** R5

**Dependencies:** 无

**Files:**
- Modify: `abt/src/repositories/bom_repo.rs`

**Approach:**
- 新增 `find_all_boms_using_product(pool, product_id) -> Result<Vec<Bom>>` 方法
- SQL 使用 `jsonb_array_elements` + `EXISTS` 模式（与现有 `find_boms_using_product` 一致），但返回完整 BOM 行而非仅 bom_id/bom_name
- 必须使用 `sqlx::query_as::<_, Bom>(...)`（运行时查询），因为 `Bom` 有自定义 `FromRow` impl，需要 `bom_detail::text` 类型转换。不能使用 `sqlx::query_as!` 宏
- 无分页（物料替换需要获取全部），但可以考虑加 LIMIT 保护

**Patterns to follow:**
- `abt/src/repositories/bom_repo.rs` 中 `find_boms_using_product` 的 SQL 模式
- `find_by_id` 的 `FromRow` 反序列化模式

**Test scenarios:**
- Test expectation: none — 通过集成测试验证（本项目无单元测试框架用于 repository 层）

**Verification:**
- `cargo build -p abt` 编译通过

---

- [x] U3. **Service trait 与实现**

**Goal:** 定义 `substitute_product` 服务接口并实现替换逻辑

**Requirements:** R1, R2, R3, R4, R5, R6

**Dependencies:** U2

**Files:**
- Modify: `abt/src/service/bom_service.rs`
- Modify: `abt/src/implt/bom_service_impl.rs`

**Approach:**
- Service trait 新增方法：`substitute_product(old_product_id, new_product_id, bom_id: Option<i64>, overrides: Option<AttributeOverrides>, executor) -> Result<(affected_bom_count, replaced_node_count)>`
- `AttributeOverrides` 是一个辅助结构体，包含所有可选覆盖字段
- 实现逻辑：
  1. 如果指定 `bom_id`：`find_by_id` 加载该 BOM
  2. 如果未指定 `bom_id`：`find_all_boms_using_product` 加载所有匹配 BOM
  3. 使用 `ProductRepo::find_by_ids` 查询新物料的 product_code（与 `load_bom_with_products` 模式一致）
  4. 遍历每个 BOM 的 nodes，将匹配 `old_product_id` 的节点替换：
     - 更新 `product_id` 为新值
     - 更新 `product_code` 为新物料的 code
     - 属性覆盖逻辑：`AttributeOverrides` 中每个字段为 `Option<T>`，`None` 表示保持原值，`Some(value)` 表示覆盖为 value
  5. 对每个修改过的 BOM 调用 `update` 写回，同时累计 `affected_bom_count`（有节点被修改的 BOM 数）和 `replaced_node_count`（总替换节点数）
  6. 返回 `(affected_bom_count, replaced_node_count)`

**Patterns to follow:**
- `abt/src/implt/bom_service_impl.rs` 中 `update_node` 的 load-modify-save 模式
- 所有写操作通过 `executor` 参数支持事务

**Test scenarios:**
- Test expectation: none — 服务层无独立测试框架，通过 gRPC handler 端到端验证

**Verification:**
- `cargo build -p abt` 编译通过

---

- [x] U4. **gRPC Handler 与转换**

**Goal:** 添加 handler 方法，处理 proto 请求/响应转换，连接事务和权限

**Requirements:** R1, R2, R4, R5, R6

**Dependencies:** U1, U3

**Files:**
- Modify: `abt-grpc/src/handlers/bom.rs`

**Approach:**
- 在 `BomHandler` 上实现 `substitute_product` 方法
- 添加 `#[require_permission(Resource::Bom, Action::Write)]`
- 请求转换：从 proto 提取 old_product_id、new_product_id、optional bom_id、optional 属性覆盖
- 响应转换：构造 `SubstituteProductResponse` 包含 affected_bom_count 和 replaced_node_count
- 使用 `state.begin_transaction()` 包裹整个操作

**Patterns to follow:**
- `abt-grpc/src/handlers/bom.rs` 中 `update_bom_node` 的事务模式
- `abt-grpc/src/handlers/convert.rs` 中的 `empty_to_none()` 转换模式

**Test scenarios:**
- Test expectation: none — 通过 `cargo build` 编译验证，手动或客户端测试验证 RPC 行为

**Verification:**
- `cargo build -p abt-grpc` 编译通过
- gRPC 反射可见新 RPC 方法

---

## System-Wide Impact

- **Interaction graph:** 新增 RPC 不影响现有 BOM 操作。所有现有 RPC 保持不变。
- **Error propagation:** 查询不到匹配 BOM 时返回零计数（非错误）。事务失败时整体回滚。
- **State lifecycle risks:** 多 BOM 替换在同一事务内执行，保证原子性。大量 BOM 可能导致长事务，但物料替换是低频操作。
- **API surface parity:** 仅 gRPC API，无其他接口需同步。
- **Unchanged invariants:** 现有的 BOM CRUD、节点管理、导出等功能不受影响。

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 大量 BOM 导致长事务 | 替换是低频操作，可接受；后续可加分批处理 |
| 并发修改同一 BOM | load-modify-save 模式下后写覆盖，风险低且符合现有行为 |
| 新物料的 product_code 获取方式 | 实现时确认 product service 的查询方法 |

---

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-25-bom-product-substitution-requirements.md](docs/brainstorms/2026-04-25-bom-product-substitution-requirements.md)
- BOM 服务接口: `abt/src/service/bom_service.rs`
- BOM 仓库: `abt/src/repositories/bom_repo.rs`
- BOM Handler 模式: `abt-grpc/src/handlers/bom.rs`
