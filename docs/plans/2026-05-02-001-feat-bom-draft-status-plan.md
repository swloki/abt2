---
title: BOM Draft Status Feature
type: feat
status: active
date: 2026-05-02
origin: docs/superpowers/specs/2026-05-02-bom-draft-status-design.md
---

# BOM 草稿状态功能实现计划

## Summary

在 `bom` 表新增 `status` / `published_at` / `published_by` 列，贯穿 proto → model → repo → service → handler 四层实现草稿/已发布逻辑。通过 SQL 层可见性过滤保证分页正确性，通过统一的 `require_creator_or_published` helper 保证所有读/写路径的归属检查一致性。关闭 5 个侧信道泄漏路径。

---

## Problem Frame

当前 BOM 一经创建即对所有有 `Bom:Read` 权限的用户可见。用户需要在编辑 BOM 时保持私密（草稿），仅在点击"发布"后才对他人可见。已有 BOM 数据需自动视为已发布以保持向后兼容。

---

## Requirements

- R1. 新建 BOM 默认为草稿状态，仅创建者可见和可操作
- R2. 发布操作将草稿转为已发布，对所有有权限用户可见
- R3. 已有 BOM 数据自动视为已发布，向后兼容
- R4. 列表查询中创建者看到自己的草稿+已发布，他人仅看到已发布
- R5. 分页正确——SQL 层过滤草稿，`query_count` 与实际返回行数一致
- R6. 所有读路径（GetBom / ExportBom / DownloadBom / GetBomCostReport / GetLeafNodes）统一使用归属检查，非创建者访问草稿返回 NotFound
- R7. 名称查重、产品使用查询排除他人草稿
- R8. Mutation WHERE 子句含状态检查，防止发布/编辑并发竞态
- R9. 发布操作记录审计信息（published_at, published_by）

---

## Scope Boundaries

- 撤销发布（UnpublishBom）——后续迭代
- 协作者/共享草稿——后续迭代
- 版本历史/不可变快照——后续迭代
- 草稿自动过期/清理——后续迭代
- 批量发布——后续迭代
- 乐观锁/版本号——单独关注点

### Deferred to Follow-Up Work

- `Bom` 结构体顶层 `created_by` 与 JSONB `bom_detail.created_by` 的最终合并清理——待 bom_nodes 重构稳定后统一处理

---

## Context & Research

### Relevant Code and Patterns

- `WarehouseStatus` enum（`abt/src/models/warehouse.rs`）——`BomStatus` 的实现模板：`#[serde(rename_all = "lowercase")]`、自定义 `FromRow`、`Display` trait
- `BomRepo::build_query_filter`（`abt/src/repositories/bom_repo.rs:182`）——动态 SQL 构建器，扩展 status + caller_id 过滤
- `BomServiceImpl` 已有 helper 模式（`build_children_map`、`get_all_descendants`、`collect_invalid_ids`）——`require_creator_or_published` 遵循同一模式
- `#[require_permission]` 宏无法覆盖创建者所有权检查——归属逻辑在 service/repo 层实现
- 迁移配对惯例：`029_create_bom_nodes_table.sql` / `030_rollback_create_bom_nodes_table.sql`

### Institutional Learnings

- 权限检查必须 fail-closed：草稿可见性检查中若 `created_by` 为 NULL 或无法关联有效用户，拒绝访问而非默认公开（来源：`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）
- 创建者可见性需业务层逻辑，`#[require_permission]` 仅处理 RBAC 层（来源：`docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md`）
- 迁移需双向（正向 + 回滚），遵循 `029` / `030` 配对模式

### External References

- Google AIP-136：推荐独立 Publish/Unpublish RPC，只接受实体 ID 不允许附带其他可变字段
- PostgreSQL CHECK 约束优于原生 ENUM（Crunchy Data, Close Engineering）：`VARCHAR + CHECK` 适合可能变更的值集合
- sqlx enum 映射：`#[derive(sqlx::Type)]` with `#[sqlx(type_name = "text", rename_all = "snake_case")]`

---

## Key Technical Decisions

- **VARCHAR + CHECK 约束而非 PostgreSQL 原生 ENUM**：允许通过 `DROP CONSTRAINT + ADD CONSTRAINT` 原子扩展状态值集合，无 ENUM 的事务限制
- **独立 `PublishBom` RPC 而非 `UpdateBom` 状态字段**：Google AIP-136 模式，防止状态转换的副作用，权限控制更清晰
- **SQL 层可见性过滤而非 service 层过滤**：保证 `query_count` 与分页结果一致性，避免"共 50 条但只有 3 条可见"的分页 Bug
- **`BomQuery` 新增 `caller_id` 字段（不暴露在 proto 中）**：由 handler 从 auth 注入，repo 层消费，保持 query 参数与 auth 上下文的分离
- **统一 helper 而非各方法分散检查**：7+ 个方法共用 `require_creator_or_published`，一处测试一处审计，未来新增状态仅改 helper

---

## Implementation Units

### U1. Database Migration

**Goal:** 在 `bom` 表新增 `status`、`published_at`、`published_by` 列，含 CHECK 约束和回滚脚本

**Requirements:** R3, R9

**Dependencies:** None

**Files:**
- Create: `abt/migrations/031_add_bom_status.sql`
- Create: `abt/migrations/032_rollback_add_bom_status.sql`

**Approach:**
- 正向迁移：ALTER TABLE 新增三列，CHECK 约束限制 status 值，回填已有数据的审计列
- 回滚迁移：按逆序 DROP COLUMN + DROP CONSTRAINT
- 编号接续 `030_rollback_create_bom_nodes_table.sql`

**Patterns to follow:**
- `abt/migrations/029_create_bom_nodes_table.sql` 和 `030_rollback_create_bom_nodes_table.sql` 的文件命名和结构

**Test scenarios:**
- Happy path: 迁移执行后 `bom` 表包含新列，已有行 status = 'published'，published_at / published_by 非 NULL
- Edge case: 插入 status = 'draft' 成功，插入 status = 'invalid' 被 CHECK 约束拒绝
- Edge case: 新建 BOM 的 published_at / published_by 为 NULL

**Verification:**
- 迁移可正向执行和回滚执行
- 已有 BOM 数据 status = 'published' 且审计列已回填

---

### U2. Proto Definition Changes

**Goal:** 新增 `BomStatus` enum、`PublishBom` RPC 和相关 message，扩展 `BomResponse` 和 `ListBomsRequest`

**Requirements:** R1, R2, R4

**Dependencies:** None

**Files:**
- Modify: `proto/abt/v1/bom.proto`

**Approach:**
- 新增 `BomStatus` enum：`BOM_STATUS_UNSPECIFIED = 0; BOM_STATUS_DRAFT = 1; BOM_STATUS_PUBLISHED = 2;`
- `BomResponse` 新增字段：`BomStatus status = 8;`、`int64 published_at = 9;`、`int64 published_by = 10;`
- 新增 `PublishBomRequest { int64 bom_id = 1; }`、`PublishBomResponse { BomResponse bom = 1; }`
- `ListBomsRequest` 新增 `optional BomStatus status = 8;`
- `service AbtBomService` 新增 `rpc PublishBom(PublishBomRequest) returns (PublishBomResponse);`
- `cargo build` 自动重新生成 `abt-grpc/src/generated/` 中的 Rust 代码

**Patterns to follow:**
- Proto enum 定义风格参考现有的 proto 文件中的 enum 模式
- Message 字段编号不冲突（检查现有字段最大编号为 7，新字段从 8 开始）

**Test scenarios:**
- Test expectation: none — proto 编译由 `cargo build` 自动验证，生成代码的类型正确性由后续单元的编译通过来保证

**Verification:**
- `cargo build` 成功，生成的 Rust 代码包含 `BomStatus` enum 和 `PublishBomRequest` / `PublishBomResponse`

---

### U3. Model Changes

**Goal:** 新增 `BomStatus` enum，扩展 `Bom` struct（含 `created_by` 顶层字段）和 `BomQuery`

**Requirements:** R1, R4

**Dependencies:** U2（需 proto 生成的类型编译通过）

**Files:**
- Modify: `abt/src/models/bom.rs`

**Approach:**
- 新增 `BomStatus` enum，派生 `Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type`
- `as_str()` / `from_str()` 方法与 `WarehouseStatus` 模式一致；`from_str` 对无效值返回 `anyhow::Error`
- `Bom` struct 新增字段：`status: BomStatus`、`published_at: Option<DateTime<Utc>>`、`published_by: Option<i64>`、`created_by: Option<i64>`（顶层，从 `bom` 表列直接读取）
- 更新 `FromRow` impl：从行中读取新列；`created_by` 从 `bom` 表列读取而非 JSONB
- `BomQuery` 新增字段：`status: Option<BomStatus>`、`caller_id: Option<i64>`（不暴露在 proto 中，由 handler 注入）

**Patterns to follow:**
- `WarehouseStatus` enum 的 `as_str()` / `Display` / `FromRow` 模式（`abt/src/models/warehouse.rs`）

**Test scenarios:**
- Happy path: `BomStatus::from_str("draft")` → `Ok(BomStatus::Draft)`，`BomStatus::from_str("published")` → `Ok(BomStatus::Published)`
- Error path: `BomStatus::from_str("invalid")` → `Err`
- Happy path: `BomStatus::Draft.as_str()` → `"draft"`，`BomStatus::Published.as_str()` → `"published"`
- Happy path: `BomQuery` 默认值中 `status` 和 `caller_id` 均为 `None`

**Verification:**
- `cargo test -p abt` 通过
- `BomQuery { status: Some(BomStatus::Draft), ..Default::default() }` 编译通过

---

### U4. Repository Changes

**Goal:** 扩展 `BomRepo` —— insert 带 status、新增 update_status、build_query_filter 增加可见性过滤、修复侧信道泄漏

**Requirements:** R1, R4, R5, R7, R8

**Dependencies:** U3

**Files:**
- Modify: `abt/src/repositories/bom_repo.rs`

**Approach:**

**`insert`** — 新增 `status: &str` 参数，INSERT 语句增加 `status` 列，创建时传入 `"draft"`

**`update_status`** — 新方法：
```sql
UPDATE bom SET status = $1, published_at = $2, published_by = $3, update_at = NOW()
WHERE bom_id = $4
```
使用 `sqlx::query`（uncheckable，与 `find_by_id` 一致）

**`build_query_filter`** — 新增两个过滤维度：
- `status` 过滤：`AND status = $N`
- `caller_id` 可见性过滤：仅当 `caller_id` 有值时生效
  - 注入 `AND (status = 'published' OR created_by = $N)`
  - 这同时处理了"非创建者只看已发布"和"创建者看自己的草稿+已发布"（创建者列表查询不传 caller_id 过滤，走"我的草稿"tab 时才传 status=Draft + caller_id）

**`update`** — 两个分支的 WHERE 子句均增加 `AND (status = 'published' OR created_by = $N)`（需新增 caller_id 参数）

**`delete`** — WHERE 子句增加 `AND (status = 'published' OR created_by = $N)`（需新增 caller_id 参数）

**`find_boms_using_product`** — 查询增加 `AND bom.status = 'published'`，排除草稿 BOM（产品删除守卫中不泄露草稿名称）

**`exists_name`** — 新增 `caller_id: Option<i64>` 参数，查询改为：
```sql
SELECT EXISTS(
  SELECT 1 FROM bom WHERE bom_name = $1
  AND (status = 'published' OR created_by = $2)
)
```

**Patterns to follow:**
- `build_query_filter` 现有的 `if let Some(...)` + `query.push(" AND ...")` 模式
- uncheckable SQL 使用 `sqlx::query` / `sqlx::query_scalar`（参考 `find_by_id` 和 `exists_name`）

**Test scenarios:**
- Happy path: `insert` with status `"draft"` → 新行 status = 'draft'
- Happy path: `update_status(bom_id, "published", now, user_id)` → 行 status = 'published' 且审计列已设置
- Happy path: `query` with `status = Some(Draft)` + `caller_id = Some(42)` → 仅返回 user 42 的草稿
- Happy path: `query` with `caller_id = Some(99)` (not creator) → 不返回任何草稿
- Edge case: `query` with `caller_id = None` → 不添加可见性过滤（管理员/向后兼容）
- Edge case: `find_boms_using_product` 仅返回已发布 BOM
- Edge case: `exists_name` with `caller_id = Some(99)` 对仅存在于他人草稿中的名称返回 false

**Verification:**
- `cargo test -p abt` 通过
- 需要数据库的测试：所有 BOM repo 测试通过

---

### U5. Service Layer Changes

**Goal:** 新增 `publish` trait 方法、`require_creator_or_published` helper，在所有操作方法中接入归属检查

**Requirements:** R1, R2, R6, R8, R9

**Dependencies:** U4

**Files:**
- Modify: `abt/src/service/bom_service.rs`
- Modify: `abt/src/implt/bom_service_impl.rs`

**Approach:**

**Trait 新增：**
- `async fn publish(&self, executor: Executor<'_>, bom_id: i64, operator_id: i64) -> Result<Bom>`

**`exists_name` 签名扩展：**
- 从 `exists_name(&self, name: &str)` 改为 `exists_name(&self, name: &str, caller_id: Option<i64>)`

**Helper 新增（`BomServiceImpl` 私有方法）：**
```rust
fn require_creator_or_published(
    bom: &Bom,
    user_id: i64,
    reveal_existence: bool,
) -> Result<()>
```
逻辑：`status == Published` → 放行；`created_by == Some(user_id)` → 放行；否则 → `Err`。`reveal_existence` 参数控制错误信息措辞（供 handler 映射到 NotFound vs PermissionDenied）。`created_by` 为 `None` 时（异常数据），拒绝访问（fail-closed）。

**`create`** — `BomRepo::insert` 传入 `"draft"` 作为 status

**`publish`** — 实现：
1. `BomRepo::find_by_id` 加载 BOM
2. `require_creator_or_published(bom, operator_id, true)` — 已发布时幂等直接返回
3. 若为草稿：`BomRepo::update_status(executor, bom_id, "published", Utc::now(), operator_id)`
4. 重新加载完整 BOM（含节点）返回

**`query`** — 从 `BomQuery.caller_id` 传递给 `build_query_filter`，逻辑不变（SQL 层已处理）

**接入 helper 的方法（在现有逻辑之前调用 `require_creator_or_published`）：**
- `find`、`get_leaf_nodes`、`get_product_code`、`get_bom_cost_report` → `reveal_existence = false`
- `update`、`delete`、`add_node`、`update_node`、`delete_node` → `reveal_existence = true`

**Mutation 方法额外传入 `caller_id` 到 repo 层：**
- `update` → `BomRepo::update(executor, bom_id, name, None, category_id, caller_id)`
- `delete` → `BomRepo::delete(executor, bom_id, caller_id)`
- `add_node` / `update_node` / `delete_node` 已通过 `require_creator_or_published` 前置检查

**`save_as`** — 新 BOM 的 `insert` 传入 `"draft"` 作为 status（与 create 行为一致）

**`exists_name`** — 透传 `caller_id` 到 `BomRepo::exists_name`

**Patterns to follow:**
- `BomServiceImpl` 中现有的私有 helper 方法模式
- Service trait 方法签名使用 `Executor<'_>`（与现有模式一致，handler 管理事务生命周期）

**Test scenarios:**
- Happy path: `publish` 将草稿转为已发布，`published_at` / `published_by` 已设置
- Happy path: `publish` 对已发布 BOM 幂等，不更新审计列
- Error path: `publish` 非创建者 → `Err`
- Happy path: `require_creator_or_published` 对已发布 BOM 放行（任意用户）
- Happy path: `require_creator_or_published` 对草稿 + 创建者放行
- Error path: `require_creator_or_published(reveal_existence=false)` 对草稿 + 非创建者 → `Err`（NotFound 语义）
- Error path: `require_creator_or_published(reveal_existence=true)` 对草稿 + 非创建者 → `Err`（PermissionDenied 语义）
- Edge case: `require_creator_or_published` with `created_by = None` → `Err`（fail-closed）
- Happy path: `exists_name("test", Some(99))` 对仅存在于 user 99 草稿中的名称返回 true
- Happy path: `create` 生成的 BOM status = Draft
- Happy path: `save_as` 生成的新 BOM status = Draft

**Verification:**
- `cargo test -p abt` 通过
- 所有 service 层测试覆盖 helper 和 publish 的核心逻辑

---

### U6. gRPC Handler Changes

**Goal:** 新增 `publish_bom` handler，在所有读/写 handler 中接入归属检查，修复侧信道泄漏

**Requirements:** R2, R4, R6, R7

**Dependencies:** U5

**Files:**
- Modify: `abt-grpc/src/handlers/bom.rs`
- Modify: `abt-grpc/src/handlers/product.rs`（若 `find_boms_using_product` 的 status 过滤在 handler 层无法实现则无需改；当前 repo 层已直接在 SQL 加 `AND status = 'published'`）

**Approach:**

**新增 `publish_bom` handler：**
- `#[require_permission(Resource::Bom, Action::Write)]`
- 提取 auth user_id → 开启事务 → `srv.publish(&mut tx, req.bom_id, user_id)` → commit → 返回 `BomResponse`

**`list_boms` —** 从 `extract_auth` 获取 user_id，注入 `BomQuery.caller_id`；若请求含 `status` 参数（"我的草稿"tab），注入 `BomQuery.status`

**`get_bom` —** 在 `srv.find()` 之后、`NotFound` 映射之前，`require_creator_or_published` 已在 service 层调用；handler 将 helper 错误映射为 gRPC `NotFound`

**`get_bom_cost_report` —** 增加 `require_creator_or_published` 检查（该 handler 使用 `BomCost:Read` 权限，不等于 `Bom:Read`）；service 层 `get_bom_cost_report` 已内置 helper 调用

**`export_bom` —** 在调用 exporter 之前，通过 `BomRepo::find_by_id_pool` 加载 BOM 并调用 helper 检查（该 handler 不使用事务）

**`download_bom` —** 同上，在调用 `exporter.export_with_name` 之前检查

**`exists_bom_name` —** 从 auth 提取 user_id，传入 `srv.exists_name(&req.name, Some(user_id))`

**`get_leaf_nodes` —** service 层已内置 helper 调用

**节点操作 / update / delete —** service 层已内置 helper 调用

**`get_product_code` —** service 层已内置 helper 调用（该 handler 使用 `Bom:Read` 权限）

**Patterns to follow:**
- 现有 handler 的 `extract_auth` + `begin_transaction` + `commit` 模式
- `#[require_permission]` 宏的权限声明模式
- 错误映射：`error::not_found()` / `error::err_to_status()`

**Test scenarios:**
- Happy path: `PublishBom` with draft owned by caller → success, response status = PUBLISHED
- Error path: `PublishBom` with draft owned by another user → `PermissionDenied`
- Happy path: `PublishBom` on already-published BOM → success (idempotent)
- Error path: `GetBom` on another user's draft → `NotFound`
- Error path: `GetBomCostReport` on another user's draft → `NotFound`（即使有 `BomCost:Read` 权限）
- Error path: `ExportBom` on another user's draft → `NotFound`
- Error path: `DownloadBom` on another user's draft → `NotFound`
- Happy path: `ExistsBomName` with name only in another user's draft → false
- Happy path: `ListBoms` as non-creator → only published BOMs returned, count correct

**Verification:**
- `cargo build` 成功
- gRPC 集成测试：创建草稿 → 他人查不到 → 发布 → 他人可查到

---

### U7. Type Conversion Changes

**Goal:** 扩展 `Bom → BomResponse` 和 `BomStatus → proto BomStatus` 的转换

**Requirements:** R1, R9

**Dependencies:** U2, U3

**Files:**
- Modify: `abt-grpc/src/handlers/convert.rs`

**Approach:**
- `BomResponse` 转换新增：`status`（`BomStatus` → proto enum）、`published_at`（`DateTime<Utc>` → timestamp）、`published_by`（`Option<i64>` → `i64`，None 映射为 0）
- 新增 `From<BomStatus> for i32`（proto enum value）或直接在 `From<abt::Bom> for BomResponse` 中 match 映射

**Patterns to follow:**
- 现有 `From<abt::Warehouse> for WarehouseResponse` 的 enum 映射模式（`matches!(w.status, WarehouseStatus::Active)` → bool）

**Test scenarios:**
- Happy path: Draft Bom → BomResponse.status = BOM_STATUS_DRAFT, published_at = 0, published_by = 0
- Happy path: Published Bom → BomResponse.status = BOM_STATUS_PUBLISHED, published_at > 0, published_by = creator_id
- Edge case: published_at = None → BomResponse.published_at = 0

**Verification:**
- `cargo build` 成功
- 类型转换在集成测试中端到端验证（U6 的 gRPC 测试覆盖）

---

## System-Wide Impact

- **Interaction graph:** BomHandler（8 个 handler 方法受影响的读/写路径）→ BomService → BomRepo / BomNodeRepo；ProductHandler（`check_product_usage` 调用链）
- **Error propagation:** helper 错误 → service 层 `Result::Err` → handler 层映射为 gRPC `NotFound`（读）或 `PermissionDenied`（写）
- **State lifecycle risks:** 发布与并发编辑的竞态由 mutation WHERE 子句防护；`created_by = NULL` 的边缘情况由 helper 的 fail-closed 策略覆盖
- **Unchanged invariants:** 现有 BOM 的 `Bom:Read` / `Bom:Write` / `Bom:Delete` / `BomCost:Read` 权限检查保持不变；`#[require_permission]` 宏逻辑不变；proto 字段编号向后兼容（仅新增）

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| `Bom` struct 新增 `created_by` 顶层字段后，现有代码中直接读取 `bom_detail.created_by` 的路径可能出现不一致 | U3 中更新 `FromRow` 同时填充顶层 `created_by` 和 `bom_detail.created_by`；后续 deferred 工作中统一清理双重来源 |
| `build_query_filter` 传入 `caller_id` 改变方法签名，影响所有调用方 | `caller_id` 作为独立参数传入（不修改 `BomQuery` 的序列化语义），现有调用方传 `None` 保持原有行为 |
| 大量读路径需增加归属检查，遗漏任一路径即为安全漏洞 | U5 的 helper 集中化管理，code review 时 grep `require_creator_or_published` 确认所有读路径已覆盖 |

---

## Documentation / Operational Notes

- 迁移执行后 `published_at` / `published_by` 回填使用 `create_at` / `created_by`，审计日志中可区分迁移回填和实际发布操作（回填时间 = 迁移执行时间）
- gRPC 客户端需更新 proto 定义以使用新的 `BomStatus` 字段和 `PublishBom` RPC
- 前端需新增"发布"按钮和"我的草稿"tab

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-02-bom-draft-status-design.md](docs/superpowers/specs/2026-05-02-bom-draft-status-design.md)
- Related code: `abt/src/models/warehouse.rs` (WarehouseStatus pattern), `abt/src/repositories/bom_repo.rs` (build_query_filter), `abt/src/implt/bom_service_impl.rs` (helper pattern)
- Institutional: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`, `docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md`
- External: Google AIP-136, Crunchy Data "Enums vs Check Constraints in Postgres"
