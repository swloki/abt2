---
title: "refactor: BOM Detail JSONB → bom_nodes 独立表迁移"
type: refactor
status: active
date: 2026-05-01
origin: docs/superpowers/specs/2026-04-30-bom-nodes-table-design.md
---

# refactor: BOM Detail JSONB → bom_nodes 独立表迁移

## Summary

将 `bom` 表的 `bom_detail` JSONB 列中的 BOM 节点数据迁移到独立的 `bom_nodes` 关系表中，每行对应一个节点。涉及数据库 migration、TypeScript 数据迁移脚本、Rust 全层代码重写（Model → Repository → Service → Handler），同时保持 gRPC 接口不变。

---

## Problem Frame

当前 BOM 节点数据以 JSONB 方式存储在 `bom.bom_detail` 列中。所有节点查询（按产品查找、根节点过滤、子树删除）都依赖 `jsonb_array_elements()` 全表扫描，无法利用索引。树遍历操作（叶子节点、后代收集）在 Rust 内存中执行。业务需要将节点拆分到独立表以支持高效的关联查询和未来扩展（成本聚合、版本控制等）。

---

## Requirements

- R1. 创建 `bom_nodes` 表，字段与当前 `BomNode` 结构体一一对应，加 `bom_id` 关联
- R2. `bom` 表添加 `created_by BIGINT` 列，从 JSONB 中迁移
- R3. TypeScript 脚本将历史 JSONB 数据迁移到 `bom_nodes`，处理 `parent_id=0` → `NULL`
- R4. Rust 代码全面重写：Model、Repository、Service、Handler 改为操作 `bom_nodes` 表
- R5. 移除所有 `jsonb_array_elements()` 查询，替换为标准 SQL 关联
- R6. gRPC proto 接口保持不变，外部客户端无感知
- R7. 保留 `bom_detail` JSONB 列作为过渡，不删除

---

## Scope Boundaries

- 不修改 proto 定义文件
- 不删除 `bom_detail` JSONB 列（后续稳定后再处理）
- 不添加外键约束（遵循团队惯例）
- 不添加 `created_at`/`updated_at` 到 `bom_nodes`（时间戳在 `bom` 表管理）
- 不涉及 BOM 版本控制或变更历史（未来工作）

### Deferred to Follow-Up Work

- 删除 `bom_detail` JSONB 列：稳定运行后单独处理
- BOM 版本控制 / 变更历史：依赖本次 `bom_nodes` 表作为基础

---

## Context & Research

### Relevant Code and Patterns

- `abt/src/models/bom.rs` — Bom / BomDetail / BomNode 结构体，自定义 `FromRow` 通过 JSONB 反序列化
- `abt/src/repositories/bom_repo.rs` — 8+ 处 `jsonb_array_elements()` 使用，`find_by_id_for_update` 锁模式
- `abt/src/implt/bom_service_impl.rs` — 3 处重复的树遍历模式（children_map + get_all_descendants）
- `abt/src/repositories/routing_repo.rs:383,396` — `jsonb_build_object` 包含查询匹配根节点
- `abt/src/repositories/labor_process_repo.rs:253` — `jsonb_array_elements` 查找无人工成本的 BOM
- `abt-grpc/src/handlers/convert.rs` — `BomNode` ↔ proto 转换，`bom_id = 0` 占位
- `abt/src/implt/excel/bom_export.rs` — BFS 遍历构建导出数据，依赖 `node.id` 和 `node.parent_id`
- 团队 migration 惯例：`BEGIN; ... COMMIT;` 包裹，配对 rollback 文件

### Institutional Learnings

- **Never TRUNCATE before INSERT** — 使用 `INSERT ... ON CONFLICT DO NOTHING` 保证幂等（见 permission migration）
- **Archive via RENAME, not DROP** — `ALTER TABLE old RENAME TO old_archived` 提供人工验证窗口
- **SELECT FOR UPDATE for read-then-write** — 并发修改必须加行锁
- **sqlx::QueryBuilder push_values only exposes push_bind** — 批量插入需要子查询时用 `query_as` 手写 SQL
- **N+1 prevention** — 用 JOIN 而非多次查询加载关联数据

---

## Key Technical Decisions

- **全局 BIGSERIAL id**：节点 id 全局自增，不再按 BOM 内局部编号。需要两阶段插入处理 parent_id 映射（用户确认）
- **无外键约束**：只用索引保证查询性能，引用完整性在应用层维护（团队惯例，用户确认）
- **parent_id 根节点为 NULL**：旧数据中 `parent_id = 0` 迁移时转为 `NULL`
- **保留 JSONB 列**：迁移后 `bom_detail` 不删除，作为过渡期回退保障（用户确认）
- **TypeScript 迁移脚本**：独立脚本执行数据迁移，与 Rust 代码部署解耦（用户确认）
- **created_by 移至 bom 表**：从 JSONB 提取到 `bom.created_by` 列，不属于单个节点（用户确认）

---

## Open Questions

### Resolved During Planning

- 迁移方式：一次性重写（方案 A），不做双写过渡
- 字段保留：所有 BomNode 字段 1:1 迁移到 bom_nodes 表

### Deferred to Implementation

- TypeScript 脚本的具体错误处理策略（空 JSONB、格式异常、超大数据量）
- 是否使用递归 CTE 替代 Rust 内存中的树遍历（可在实现时根据性能测试决定）
- `quantity`/`loss_rate` 在 Rust 端是否从 `f64` 迁移到 `Decimal`（与数据库 DECIMAL 对齐）

---

## Implementation Units

- U1. **Database Migration — 创建 bom_nodes 表 + bom 表变更**

**Goal:** 创建 `bom_nodes` 表和 `bom` 表的 `created_by` 列，配对 rollback migration

**Requirements:** R1, R2

**Dependencies:** None

**Files:**
- Create: `abt/migrations/029_create_bom_nodes_table.sql`
- Create: `abt/migrations/030_rollback_create_bom_nodes_table.sql`

**Approach:**
- Forward migration：CREATE TABLE bom_nodes（按设计文档 schema），ALTER TABLE bom ADD COLUMN created_by BIGINT
- 使用 `BEGIN; ... COMMIT;` 包裹，遵循团队惯例
- Rollback migration：DROP TABLE IF EXISTS bom_nodes，ALTER TABLE bom DROP COLUMN IF EXISTS created_by
- 编号 029（当前最大为 028）

**Patterns to follow:**
- `abt/migrations/021_labor_process_redesign.sql` + `022_rollback_labor_process_redesign.sql` 的配对模式

**Test scenarios:**
- Happy path: migration 执行成功，bom_nodes 表存在且 schema 正确（字段类型、索引）
- Happy path: rollback 执行成功，表和列被清理
- Edge case: 重复执行 forward migration 不报错（幂等检查）
- Edge case: bom 表已有数据时添加 created_by 列不影响现有行

**Verification:**
- `cargo test -p abt` 通过
- 手动验证 `\d bom_nodes` 输出与设计文档一致

---

- U2. **BomNode Model + BomNodeRepo — 新增模型和仓库层**

**Goal:** 创建 BomNode 相关的 Rust 结构体和 BomNodeRepo 仓库，提供 CRUD 操作

**Requirements:** R1, R4

**Dependencies:** U1

**Files:**
- Create: `abt/src/models/bom_node.rs`
- Create: `abt/src/repositories/bom_node_repo.rs`
- Modify: `abt/src/models/mod.rs` — 注册新模块
- Modify: `abt/src/repositories/mod.rs` — 注册新模块
- Test: `abt/src/models/bom_node.rs` (inline tests)

**Approach:**
- 新增 `BomNode` 结构体映射 bom_nodes 表行，使用 `sqlx::FromRow` derive
- 新增 `NewBomNode`（插入用，无 id）和 `UpdateBomNode`（更新用，所有字段 Option）
- `BomNodeRepo` 提供：insert、batch_insert、find_by_bom_id、find_by_id、update、delete、find_by_product_id、delete_by_bom_id
- `find_by_bom_id` 返回 `Vec<BomNode>`，按 `order` 排序
- `delete` 使用递归 CTE (`WITH RECURSIVE`) 查找所有后代 ID，一次性删除
- 工厂函数注册在 `abt/src/lib.rs`

**Technical design:**

> 递归 CTE 删除子树示例（方向性指导）:
> ```sql
> WITH RECURSIVE descendants AS (
>     SELECT id FROM bom_nodes WHERE id = $1
>     UNION ALL
>     SELECT n.id FROM bom_nodes n JOIN descendants d ON n.parent_id = d.id
> )
> DELETE FROM bom_nodes WHERE id IN (SELECT id FROM descendants)
> ```

**Patterns to follow:**
- `abt/src/repositories/bom_repo.rs` — sqlx query_as 模式
- `abt/src/repositories/labor_process_repo.rs` — 批量操作模式

**Test scenarios:**
- Happy path: insert 单个节点，find_by_id 返回正确数据
- Happy path: batch_insert 多个节点，find_by_bom_id 返回按 order 排序
- Happy path: update 节点字段，验证更新成功
- Happy path: delete 叶节点，仅删除该节点
- Happy path: delete 非叶节点，递归删除所有后代
- Edge case: find_by_bom_id 对空 BOM 返回空 Vec
- Edge case: delete 不存在的 id 不报错

**Verification:**
- `cargo test -p abt` 通过
- BomNodeRepo 的所有方法可通过集成测试验证

---

- U3. **BomService 重写 — 核心业务逻辑切换到 bom_nodes**

**Goal:** 重写 BomServiceImpl 中所有节点操作，从 JSONB 读-改-写改为 BomNodeRepo 调用

**Requirements:** R4, R5

**Dependencies:** U2

**Files:**
- Modify: `abt/src/implt/bom_service_impl.rs` — 全部节点操作重写
- Modify: `abt/src/service/bom_service.rs` — service trait 签名不变，但可能需要调整内部类型
- Modify: `abt/src/lib.rs` — 工厂函数注入 BomNodeRepo

**Approach:**

按方法逐一重写：

| 方法 | 当前模式 | 迁移后模式 |
|------|---------|-----------|
| `create` | 创建空 BomDetail JSONB | 仅创建 bom 行，不插入节点 |
| `delete` | 删 bom 行 | 删 bom 行 + `delete_by_bom_id` |
| `add_node` | 读 JSONB → push node → 写回 JSONB | `BomNodeRepo::insert` 单行插入 |
| `update_node` | 读 JSONB → find node → mutate → 写回 | `BomNodeRepo::update` 单行更新 |
| `delete_node` | 读 JSONB → BFS 收集后代 → 从 vec 移除 → 写回 | `BomNodeRepo::delete`（含递归 CTE） |
| `swap_node_position` | 读 JSONB → swap order → 写回 | 两次 `UPDATE bom_nodes SET order` |
| `get_leaf_nodes` | 读 JSONB → 构建 parent_ids HashSet 过滤 | SQL `NOT EXISTS (SELECT 1 FROM bom_nodes WHERE parent_id = n.id)` |
| `save_as` | clone BomDetail → insert 新 bom | INSERT INTO bom_nodes SELECT ... FROM bom_nodes WHERE bom_id = source |
| `get_product_code` | 读 JSONB → 找根节点 | `SELECT product_code FROM bom_nodes WHERE bom_id = $1 AND parent_id IS NULL ORDER BY sort_order LIMIT 1` |
| `substitute_product` | 读 JSONB → 批量修改 → 写回 | `UPDATE bom_nodes SET product_id = $1 WHERE product_id = $2` |
| `get_bom_cost_report` | 读 JSONB → 构建 children_map → 遍历 | SQL 查叶子节点 + JOIN 产品/价格 |

关键变化：
- `find` 方法：查 bom 行 + `find_by_bom_id` 获取节点，组装成 Bom（仍包含 BomDetail 以兼容 proto）
- `BomDetail` 结构体保留但仅作为 proto 序列化的中间层，从 bom_nodes 查询结果构建
- 树遍历（children_map、get_all_descendants）尽量下推到 SQL，减少内存操作

**Execution note:** 建议逐方法重写并测试，不要一次性全部改完再验证

**Patterns to follow:**
- `abt/src/implt/bom_service_impl.rs` — 现有的事务管理和错误处理模式

**Test scenarios:**
- Happy path: create 空 BOM，无节点
- Happy path: add_node 添加根节点（parent_id=NULL），再添加子节点
- Happy path: update_node 修改 quantity、remark 等字段
- Happy path: delete_node 叶节点，仅删该节点
- Happy path: delete_node 中间节点，级联删除所有后代
- Happy path: swap_node_position 交换同级节点顺序
- Happy path: get_leaf_nodes 返回所有叶节点
- Happy path: save_as 创建 BOM 副本，包含所有节点和正确的 parent_id
- Happy path: substitute_product 替换产品，影响所有相关 BOM
- Edge case: delete 空的 BOM（无节点）
- Edge case: substitute_product 指定特定 bom_id 时只影响该 BOM
- Integration: add_node → get_leaf_nodes → delete_node 完整流程

**Verification:**
- `cargo test -p abt` 通过
- `cargo test -p abt-grpc` 通过

---

- U4. **BomRepo 清理 + 跨仓库 JSONB 移除**

**Goal:** 移除 bom_repo.rs 中所有 `jsonb_array_elements()` 查询，替换为 bom_nodes 关联；修复 routing_repo.rs 和 labor_process_repo.rs 中的 JSONB 引用

**Requirements:** R5

**Dependencies:** U2, U3

**Files:**
- Modify: `abt/src/repositories/bom_repo.rs` — 替换 8+ 处 jsonb 查询
- Modify: `abt/src/repositories/routing_repo.rs` — 2 处 jsonb 根节点匹配
- Modify: `abt/src/repositories/labor_process_repo.rs` — 1 处 jsonb 无人工成本查询

**Approach:**

BomRepo 中需要替换的查询：

| 原始 JSONB 查询 | 替换为 |
|----------------|--------|
| `jsonb_array_elements(bom_detail->'nodes') WHERE product_id` | `EXISTS (SELECT 1 FROM bom_nodes WHERE bom_id = b.bom_id AND product_id = $1)` |
| `jsonb_array_elements WHERE parent_id=0 AND product_code` | `EXISTS (SELECT 1 FROM bom_nodes n JOIN products p WHERE n.bom_id = b.bom_id AND n.parent_id IS NULL AND ...)` |
| `bom_detail->>'created_by'` | `b.created_by` |
| `bom_detail @> jsonb_build_object('nodes', ...)` (routing_repo) | `JOIN bom_nodes n ON n.bom_id = b.bom_id AND n.product_id = p.product_id AND n.parent_id IS NULL` |
| `jsonb_array_elements WHERE parent_id=0` (labor_process_repo) | `JOIN bom_nodes n ON n.bom_id = b.bom_id AND n.parent_id IS NULL` |

BomRepo 的 `find_by_id` 和 `find_by_id_for_update` 不再需要自定义 `FromRow`（JSONB 反序列化），但 Bom 结构体中仍需携带节点数据用于 proto 转换。方案：`find_by_id` 返回 Bom 不含节点，调用方额外调用 `BomNodeRepo::find_by_bom_id` 获取节点。

BomRepo 的 `insert` / `update` 方法去掉 `bom_detail` JSONB 参数（但 SQL 中保留对 `bom_detail` 列的操作以兼容已有数据，可设为空对象 `{}`）。

**Patterns to follow:**
- 现有 `build_query_filter` 的动态 SQL 构建模式

**Test scenarios:**
- Happy path: 按 product_id 过滤 BOM 列表
- Happy path: 按 product_code（根节点）过滤 BOM
- Happy path: find_boms_using_product 返回正确计数和列表
- Happy path: find_product_codes_with_bom 返回正确的产品编码
- Integration: routing_repo 的 BOM 根节点匹配查询
- Integration: labor_process_repo 的无人工成本查询

**Verification:**
- `cargo test -p abt` 通过
- grep 验证：`jsonb_array_elements` 不再出现在 bom_repo.rs、routing_repo.rs、labor_process_repo.rs 中

---

- U5. **Handler & Convert 层更新**

**Goal:** 调整 gRPC handler 和 proto 转换逻辑，适配新的数据模型

**Requirements:** R6

**Dependencies:** U3, U4

**Files:**
- Modify: `abt-grpc/src/handlers/bom.rs` — handler 调整
- Modify: `abt-grpc/src/handlers/convert.rs` — model ↔ proto 转换更新

**Approach:**

Convert 层关键变化：
- `From<abt::Bom> for BomResponse`：`created_by` 从 `bom.created_by` 获取（不再从 `bom_detail.created_by`）
- `From<abt::BomNode> for BomNodeResponse`：`bom_id` 从 BomNode 的 `bom_id` 字段填充（不再硬编码为 0）
- `BomDetailProto` 的构建从 `bom_nodes` 查询结果组装，而非 JSONB 反序列化

Handler 层：
- `add_bom_node`：构造 `NewBomNode` 传给 service
- `update_bom_node`：构造 `UpdateBomNode` 传给 service
- 事务模式不变（`state.begin_transaction()` + `tx.commit()`）

**Patterns to follow:**
- `abt-grpc/src/handlers/bom.rs` — 现有的事务和错误处理模式
- `abt-grpc/src/handlers/convert.rs` — 现有的 model → proto 转换模式

**Test scenarios:**
- Happy path: get_bom 返回正确的 BomDetailProto（含节点列表）
- Happy path: add_bom_node 后 get_bom 返回包含新节点
- Happy path: BomNodeResponse 的 bom_id 字段正确填充
- Happy path: created_by 正确返回（不再从 JSONB 读取）
- Edge case: 空 BOM（无节点）返回空 nodes 列表

**Verification:**
- `cargo test -p abt-grpc` 通过
- `cargo build -p abt-grpc` 编译通过

---

- U6. **TypeScript 数据迁移脚本**

**Goal:** 编写 TypeScript 脚本，将历史 JSONB 数据迁移到 bom_nodes 表

**Requirements:** R3

**Dependencies:** U1

**Files:**
- Create: `scripts/migrate-bom-detail.ts`

**Approach:**

脚本逻辑（按设计文档）：
1. 读取所有 `bom` 记录的 `bom_id` 和 `bom_detail`
2. 解析每个 `bom_detail.nodes` 数组
3. `created_by` 写入 `bom.created_by` 列
4. 两阶段插入 `bom_nodes`：
   - 第一轮：插入所有节点，`parent_id = NULL`，记录旧 id → 新 id 映射
   - 第二轮：UPDATE `parent_id`，旧 parent_id 替换为新的 id（根节点 `parent_id=0` → 保持 NULL）
5. 幂等设计：使用 `ON CONFLICT DO NOTHING` 或检查已有数据

注意事项：
- `created_by` 兼容性：JSONB 中可能是字符串或数字，需统一转换为 BIGINT
- 空 `bom_detail`：跳过或记录警告
- 事务安全：每个 BOM 的迁移在一个事务内完成
- 进度报告：输出每个 BOM 的迁移状态

**Test scenarios:**
- Happy path: 正常 BOM（多个节点，含父子关系）迁移成功
- Happy path: 根节点 `parent_id=0` → `parent_id=NULL`
- Edge case: 空 `bom_detail`（`{}`）跳过
- Edge case: `bom_detail` 为 NULL 跳过
- Edge case: `created_by` 为字符串 `"123"` → 转为数字 123
- Edge case: 重复执行不产生重复数据（幂等）

**Verification:**
- 迁移后 `SELECT COUNT(*) FROM bom_nodes` 与 JSONB 节点总数一致
- 抽样验证：随机 BOM 的节点在 bom_nodes 中完整且 parent_id 关系正确

---

## System-Wide Impact

- **Interaction graph:** `BomService` 是系统中被广泛引用的服务。Excel 导出（`bom_export.rs`）、成本报告、劳务工序、路由都依赖 BOM 数据
- **Error propagation:** 错误处理保持 `anyhow::Result` + gRPC status 转换，不变
- **State lifecycle risks:** 迁移窗口期间 JSONB 和 bom_nodes 共存，需确保 TypeScript 脚本完成后才部署新 Rust 代码
- **API surface parity:** gRPC 接口完全不变，外部客户端无感知
- **Integration coverage:** routing_repo 和 labor_process_repo 的 JSONB 引用必须同步更新，否则会报错
- **Unchanged invariants:** proto 定义、gRPC 服务方法、 BomService trait 签名不变

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 迁移脚本中断导致部分 BOM 数据不一致 | 每个 BOM 在独立事务中迁移；脚本幂等可重跑 |
| routing_repo / labor_process_repo 遗漏 JSONB 引用 | grep 全局搜索 `jsonb_array_elements` 和 `bom_detail` 确认无遗漏 |
| BomDetail 结构体移除导致 Excel 导出等模块编译失败 | BomDetail 保留作为 proto 中间层，从 bom_nodes 数据构建 |
| 两阶段 parent_id 映射 UPDATE 失败留下孤儿节点 | 在同一事务中执行，失败则整体回滚 |
| `quantity`/`loss_rate` f64 精度问题 | 数据库使用 DECIMAL(10,6)，Rust 端可后续优化为 Decimal |
| `created_by` 兼容性（字符串 vs 数字） | TypeScript 脚本中统一转换，处理异常值 |

---

## Documentation / Operational Notes

- 部署顺序：先跑 SQL migration（U1）→ 跑 TypeScript 迁移脚本（U6）→ 部署新 Rust 代码（U2-U5）
- 过渡期内 `bom_detail` JSONB 列保留但不再读写
- 后续稳定后需单独 migration 删除 `bom_detail` 列

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-04-30-bom-nodes-table-design.md](docs/superpowers/specs/2026-04-30-bom-nodes-table-design.md)
- Related code: `abt/src/models/bom.rs`, `abt/src/repositories/bom_repo.rs`, `abt/src/implt/bom_service_impl.rs`
- Related patterns: `abt/migrations/021_labor_process_redesign.sql` + `022_rollback_labor_process_redesign.sql`
- Learnings: `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`
