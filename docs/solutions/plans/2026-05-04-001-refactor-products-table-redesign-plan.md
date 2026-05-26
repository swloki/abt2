---
title: refactor: Products 表结构重新设计
type: refactor
status: active
date: 2026-05-04
origin: docs/brainstorms/2026-05-04-products-table-redesign-requirements.md
---

# refactor: Products 表结构重新设计

## Summary

重构 products 表：通过数据库迁移将 product_code 和 unit 提升为带约束的独立列，创建 product_price 历史表替代 product_price_log，移除 category/subcategory/loss_rate，meta 缩减为 specification/acquire_channel/old_code。然后更新 Rust 模型层、6 个 repository 文件、proto 定义和 gRPC handler。

---

## Problem Frame

products 表将所有业务属性存储在单一 JSONB `meta` 列中。product_code 作为跨模块 JOIN 键没有 UNIQUE 约束，价格生命周期与产品属性混在同一个 JSONB blob 中，分类文本与 term 体系脱节。本次重构将这些结构性问题一次性解决。（详见 origin 文档）

---

## Requirements

- R1. 创建 product_price 历史表，最新行即当前价格
- R2. 当前价格通过查询最新记录获取
- R3. 原 product_price_log 通过 ALTER TABLE RENAME TO product_price_log_archived 归档，并 DROP CONSTRAINT 移除原有 FK 约束
- R4. 所有价格读写路径从 jsonb_set 迁移到新表
- R5. product_code 提升为 NOT NULL UNIQUE 列
- R6. unit 提升为 NOT NULL 列
- R7. 新列数据从 meta 回填，完成后 meta 中移除对应字段
- R8. 分类统一用 term_relation 关联，inventory_repo 中现有 `meta->>'category' = term_id.to_string()` 改为 term_relation JOIN（同时修正 term_id 与文本比较的既有 bug）
- R9. category 从 meta 移除
- R10. subcategory 从 meta 移除
- R11. loss_rate 从 meta 移除
- R12. meta 仅保留 specification、acquire_channel、old_code
- R13. 前向/回滚迁移对
- R14. 迁移前执行数据完整性审计（product_code 重复/空值检查、meta 字段可提取性验证），审计作为 U1 的前置步骤而非推迟到实现阶段
- R15. 不添加 FK 约束

**Origin acceptance examples:** AE1 (price history), AE2 (product_code UNIQUE), AE3 (term_relation), AE4 (meta cleanup), AE5 (rollback)

---

## Scope Boundaries

- bom_routing/bom_labor_process/bom_nodes 反向归一化（product_id JOIN）作为后续独立任务
- loss_rate f64→Decimal 修正不在本次范围
- Excel 导入/导出适配在本次范围内

### Deferred to Follow-Up Work

- 反向归一化：将 bom_routing、bom_labor_process、bom_nodes 的 product_code 列改为 product_id 引用
- Proto 中 product_code/unit 从 ProductMeta 移到顶层 message 后，需确认前端客户端适配

---

## Context & Research

### Relevant Code and Patterns

- **JSONB 提取先例**：`abt/migrations/029_add_bom_draft_status.sql` — ALTER TABLE ADD COLUMN + UPDATE backfill from JSONB + 约束
- **表拆分先例**：`abt/migrations/031_create_bom_nodes_table.sql` / `032_rollback_create_bom_nodes_table.sql` — 新建表 + 回滚对
- **ProductMeta struct**：`abt/src/models/product.rs` — 8 字段，自定义 FromRow impl（手动 serde 反序列化）
- **Price 操作**：`abt/src/repositories/product_price_repo.rs` — 混合 sqlx::query! 宏和运行时 query

### Institutional Learnings

- 迁移必须前向/回滚成对（见 `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）
- 不使用 REFERENCES FK 约束，应用层保证关联完整性
- 归档表用 ALTER TABLE RENAME TO _archived，而非 DROP
- 迁移包在 BEGIN; ... COMMIT; 事务中
- sqlx::QueryBuilder 只有 push_bind，无 push_raw

### Affected Files

| File | Impact | meta 访问点数 |
|------|--------|-------------|
| `abt/src/models/product.rs` | ProductMeta 缩减，FromRow 重写 | - |
| `abt/src/repositories/product_repo.rs` | product_code 列化 | 5 |
| `abt/src/repositories/product_price_repo.rs` | 整体重写到新表 | 6 |
| `abt/src/repositories/inventory_repo.rs` | product_code/unit/price/category/specification | 12 |
| `abt/src/repositories/bom_repo.rs` | product_code 列化 | 3 |
| `abt/src/repositories/routing_repo.rs` | product_code JOIN | 2 |
| `abt/src/repositories/labor_process_repo.rs` | product_code JOIN | 2 |
| `abt/src/implt/excel/product_inventory_import.rs` | price 写入路径 | 2 |
| `abt/src/implt/excel/product_without_price_export.rs` | price 读取路径 | 1 |
| `proto/abt/v1/product.proto` | ProductMeta 缩减，顶层字段 | - |
| `abt-grpc/src/handlers/convert.rs` | 双向转换更新 | - |
| `abt-grpc/src/handlers/product.rs` | handler 适配 | - |
| `abt/src/service/product_service.rs` | trait 签名变更（product_code/unit 参数） | - |
| `abt/src/implt/product_service_impl.rs` | 实现签名变更 | - |

---

## Key Technical Decisions

- **先迁移后改代码**：sqlx::query! 编译时检查需要新 schema 才能编译，因此必须先 apply migration 到 dev DB 再改 Rust 代码
- **部署顺序要求**：前向迁移会从 meta 中清除 product_code 等字段，旧代码的 `meta->>'product_code'` 查询将静默返回空结果。必须按以下顺序操作：停止运行中的服务 → 应用前向迁移 → 构建并部署新代码 → 启动服务。不可在迁移和代码部署之间有运行中的旧代码实例
- **迁移分两个文件**：036（前向：列提升 + product_price 表 + 数据回填 + meta 清理 + price_log 归档）+ 037（回滚）
- **price_log 数据迁移到 product_price**：将 price_log 中的历史记录迁移到新表，按 created_at 排序保证最新行正确；meta 中的当前价格仅在 price_log 无记录时作为补充来源
- **运行时查询升级为宏**：迁移 price 相关的运行时 query 为 sqlx::query! 宏，获得编译时保护
- **Proto reserved field numbers**：被删除的 ProductMeta 字段（1-category, 2-subcategory, 7-loss_rate）reserved，product_code(3) 和 unit(5) 移到顶层 message

---

## Open Questions

### Resolved During Planning

- 迁移编号：036/037（当前最新 035）
- FromRow 策略：重写为混合模式（新列直接读 + meta 只反序列化 3 字段）
- Proto 变更在范围内

### Deferred to Implementation

- [Affects R14] 数据审计具体阈值和清理脚本
- [Affects R4] price_log 中是否有 meta 中不存在的价格记录，需实际查询确认
- [Affects R3] product_price_log_archived 表最终删除时机

### From 2026-05-04 Review

- [Affects U5] bom_routing 和 bom_labor_process 表中的 product_code 字符串值是否需要与新 UNIQUE 列保持一致性检查——当前推迟到反向归一化任务，但字符串不一致会导致 JOIN 静默返回空结果
- [Affects U4] get_prices_by_ids 批量查询在产品无价格记录时的行为需明确：(a) HashMap 中不包含该产品（当前行为），(b) 使用 LEFT JOIN 包含所有请求的产品（price 为 None），或 (c) 迁移时为无价格产品插入 NULL 行

---

## Implementation Units

- U1. **Database Migrations**

**Goal:** 创建前向和回滚迁移文件，执行 schema 变更和数据迁移

**Requirements:** R1, R2, R3, R5, R6, R7, R8, R9, R10, R11, R12, R13, R15

**Dependencies:** None

**Files:**
- Create: `abt/migrations/036_products_table_redesign.sql`
- Create: `abt/migrations/037_rollback_products_table_redesign.sql`

**Approach:**
- 前向迁移包含：ALTER TABLE 添加 product_code/unit 列（nullable）→ 从 meta 回填数据 → 添加 NOT NULL/UNIQUE 约束和索引 → 创建 product_price 表 → 从 price_log + meta 迁移价格数据 → 清理 meta（移除 7 个字段）→ ALTER TABLE product_price_log RENAME TO product_price_log_archived → ALTER TABLE product_price_log_archived DROP CONSTRAINT 移除原有 FK
- 回滚迁移包含：恢复 meta 字段 → 从 product_price 回写 price 到 meta → DROP product_price → ALTER TABLE product_price_log_archived RENAME TO product_price_log（恢复原表名）→ 恢复 price_log FK 约束 → 删除新列。注意：前向迁移后新建的价格记录在回滚时会丢失，需在回滚前确认可接受
- 包裹在 BEGIN; ... COMMIT; 事务中。注意：单事务包含 ALTER TABLE + 全表 UPDATE 回填 + INSERT...SELECT 数据迁移，会在 products 表上持有 ACCESS EXCLUSIVE 锁。如果产品表行数较多（>1000），需评估锁持有时间是否可接受，或考虑分阶段迁移
- 迁移前必须执行数据完整性审计（R14）：检查 product_code 是否有重复或空值、meta 字段是否可正常提取
- 遵循 migration 029 的 JSONB 提取模式和 migration 031/032 的表创建模式

**Patterns to follow:**
- `abt/migrations/029_add_bom_draft_status.sql` — JSONB 提取到列
- `abt/migrations/031_create_bom_nodes_table.sql` — 新建表 + 回滚对

**Test scenarios:**
- Edge case: product_code 有重复值的行需要迁移前处理
- Edge case: meta 为 NULL 的产品行
- Edge case: price_log 中有但 meta 中没有的记录
- Edge case: meta->>'price' 不是合法 decimal 的行
- Integration: 前向迁移后数据完整（product_code/unit 列有值，product_price 有记录，meta 只有 3 个 key）
- Integration: Covers AE5. 执行 rollback 后 products 表恢复原状

**Verification:**
- 迁移后 products 表有 product_code (NOT NULL UNIQUE) 和 unit (NOT NULL) 列
- product_price 表存在且有从 price_log 迁移的数据
- meta JSONB 仅含 specification/acquire_channel/old_code
- product_price_log_archived 表存在（通过 ALTER TABLE RENAME 而非 DROP，原始 FK 约束已移除）

---

- U2. **Model Layer Updates**

**Goal:** 更新 Product struct、ProductMeta struct、FromRow 实现和相关测试

**Requirements:** R5, R6, R7, R9, R10, R11, R12

**Dependencies:** U1

**Files:**
- Modify: `abt/src/models/product.rs`
- Modify: `abt/src/service/product_service.rs`（trait 签名：create/update 需新增 product_code/unit 参数）
- Modify: `abt/src/implt/product_service_impl.rs`（实现签名同步更新）

**Approach:**
- Product struct 新增 `product_code: String` 和 `unit: String` 字段
- ProductMeta 缩减为仅含 specification、acquire_channel、old_code
- FromRow 重写：product_code 和 unit 从行直接读取，meta 仅反序列化剩余 3 字段
- ProductService trait 的 create/update 方法签名需新增 product_code 和 unit 参数（不再从 ProductMeta 中隐式传递）
- 更新所有相关类型定义和单元测试

**Test scenarios:**
- Happy path: 从数据库行构造 Product 时 product_code 和 unit 正确读取
- Happy path: ProductMeta 序列化/反序列化仅包含 3 字段
- Edge case: meta 中有旧字段（category 等）时反序列化不报错（serde deny_unknown_fields 状态需确认）
- Edge case: old_code 为 None 时序列化正确

**Verification:**
- `cargo test -p abt -- models::product` 通过
- ProductMeta 仅含 specification、acquire_channel、old_code

---

- U3. **Product Repository Updates**

**Goal:** 更新 product_repo.rs 中所有 meta 访问为新列

**Requirements:** R5, R7

**Dependencies:** U1, U2

**Files:**
- Modify: `abt/src/repositories/product_repo.rs`

**Approach:**
- insert/update 方法：新增 product_code 和 unit 作为独立绑定参数，meta 序列化时不再包含这两个字段
- exist_product_code、find_by_code、find_by_codes：从 `meta->>'product_code'` 改为直接列引用
- 查询方法（query/list）：SELECT 和 WHERE 中的 product_code 引用更新
- 运行时 query（exist_product_code、find_by_code、find_by_codes）升级为 sqlx::query! 宏

**Patterns to follow:**
- bom_repo.rs 中 bom_nodes 的列访问模式

**Test scenarios:**
- Happy path: 创建产品时 product_code 和 unit 写入独立列
- Happy path: 通过 product_code 查询使用列索引
- Edge case: 重复 product_code 插入被 UNIQUE 约束拒绝
- Edge case: 按 product_code 列表批量查询正确返回
- Integration: 更新产品时 meta 中不包含 product_code/unit（不覆盖）

**Verification:**
- `cargo test -p abt -- product_repo` 通过
- 无 `meta->>'product_code'` 残留

---

- U4. **Product Price Repository Rewrite**

**Goal:** 重写 product_price_repo.rs 使用新的 product_price 表

**Requirements:** R1, R2, R4

**Dependencies:** U1, U2

**Files:**
- Modify: `abt/src/repositories/product_price_repo.rs`

**Approach:**
- get_price：从 product_price 表查最新记录（ORDER BY created_at DESC LIMIT 1），替代 `(meta->>'price')::decimal`
- update_price：INSERT 到 product_price 表，替代 `jsonb_set(meta, '{price}', ...)`
- get_prices_by_ids：批量查询从 product_price 表获取每个产品的最新价格
- insert_price_log：合并到 update_price 中（新表每行既是记录也是历史）
- list_price_history / count_all_price_history / list_all_price_history：查询从 product_price_log_archived 改为 product_price 表
- 所有运行时 query 升级为 sqlx::query! 宏

**Test scenarios:**
- Happy path: Covers AE1. 首次设置价格后查询返回该价格；更新后再查询返回新价格
- Happy path: 批量查询多个产品价格正确返回
- Edge case: 产品无价格记录时返回 None/零值
- Integration: 价格历史列表包含所有变更记录

**Verification:**
- `cargo test -p abt -- product_price` 通过
- 无 `jsonb_set` 和 `meta->>'price'` 残留

---

- U5. **Downstream Repository Updates**

**Goal:** 更新 inventory/bom/routing/labor_process 仓库中的 meta 访问

**Requirements:** R4, R5, R6, R8, R9

**Dependencies:** U1, U2

**Files:**
- Modify: `abt/src/repositories/inventory_repo.rs`
- Modify: `abt/src/repositories/bom_repo.rs`
- Modify: `abt/src/repositories/routing_repo.rs`
- Modify: `abt/src/repositories/labor_process_repo.rs`

**Approach:**
- inventory_repo.rs：8 处 `meta->>'product_code'` → `product_code`，1 处 `meta->>'unit'` → `unit`，1 处 `(meta->>'price')::decimal` → JOIN product_price 取最新，1 处 `meta->>'category'` filter → 改用 term_relation JOIN（注意：当前代码 `meta->>'category' = term_id.to_string()` 是既有 bug，文本与数值比较永不相等，改为 term_relation JOIN 是正确性修复而非简单重构）
- bom_repo.rs：3 处 `meta->>'product_code'` → `product_code`
- routing_repo.rs：2 处 JOIN 条件中的 `p.meta->>'product_code'` → `p.product_code`
- labor_process_repo.rs：2 处 `p.meta->>'product_code'` → `p.product_code`

**Test scenarios:**
- Happy path: 库存列表查询显示正确的 product_code 和 unit
- Happy path: BOM 查询按 product_code 过滤正常
- Integration: Covers AE3. 分类查询通过 term_relation 工作正常
- Edge case: COALESCE 防御可以简化为直接列引用（列有 NOT NULL 约束）

**Verification:**
- `cargo test -p abt -- inventory` 通过
- `cargo test -p abt -- bom` 通过
- `cargo test -p abt -- routing` 通过
- `cargo test -p abt -- labor_process` 通过
- grep 确认无 `meta->>'product_code'` 或 `meta->>'unit'` 残留

---

- U6. **Proto, gRPC, and Excel Handler Updates**

**Goal:** 更新 proto 定义、gRPC 转换层和 Excel 处理器

**Requirements:** R4, R5, R6, R12

**Dependencies:** U2, U3, U4, U5

**Files:**
- Modify: `proto/abt/v1/product.proto`
- Modify: `abt-grpc/src/handlers/convert.rs`
- Modify: `abt-grpc/src/handlers/product.rs`
- Modify: `abt/src/service/product_service.rs`
- Modify: `abt/src/implt/product_service_impl.rs`
- Modify: `abt/src/implt/excel/product_inventory_import.rs`
- Modify: `abt/src/implt/excel/product_without_price_export.rs`
- Modify: `abt-grpc/src/generated/abt.v1.rs` (auto-generated by build.rs)

**Approach:**
- Proto: ProductMeta reserved 1,2,7；移除 product_code(3) 和 unit(5)，在 ProductResponse 等顶层 message 新增这两个字段
- convert.rs：Product→Proto 转换中 product_code/unit 从 Product 顶层字段映射，ProductMeta 仅映射 3 字段；Proto→Model 反向转换同理
- product.rs handler：create/update 请求解析时从 proto 新字段提取 product_code/unit
- product_inventory_import.rs：价格写入改为 INSERT 到 product_price 表，old_price 读取来源从 `meta->>'price'` 改为从 product_price 表查询最新记录
- product_without_price_export.rs：无价格过滤改为从 product_price 子查询最新价格，过滤条件为 price IS NULL OR price = 0（保留对零价产品的排除逻辑，而非仅排除无记录的产品）
- `cargo build` 后 build.rs 自动重新生成 `abt-grpc/src/generated/abt.v1.rs`

**Test scenarios:**
- Happy path: gRPC 创建产品请求中 product_code 和 unit 正确映射
- Happy path: gRPC 查询产品响应包含顶层 product_code/unit 字段
- Integration: Covers AE4. 完整端到端流程：创建产品→设置价格→查询→meta 仅含 3 字段
- Edge case: Excel 导入设置价格写入 product_price 表而非 jsonb_set
- Edge case: 无价格产品导出查询正确

**Verification:**
- `cargo build` 通过（proto 编译 + sqlx 编译时检查）
- `cargo test -p abt-grpc` 通过
- `cargo test -p abt` 全量通过

---

## System-Wide Impact

- **Interaction graph:** 产品创建/更新影响 BOM 构建、库存查询、价格管理、Excel 导入/导出
- **Error propagation:** UNIQUE 约束违反从数据库传播到应用层，需要友好错误消息
- **State lifecycle risks:** 迁移期间 meta 和新列双源并存，迁移完成后需验证数据一致性
- **API surface parity:** gRPC ProductResponse message 结构变更，客户端需适配新字段位置
- **Integration coverage:** 全链路测试（创建产品→BOM 关联→库存查询→价格设置→Excel 导出）

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 迁移时 product_code 有重复值 | 迁移前数据审计脚本，发现重复则人工处理后再迁移 |
| price_log 数据与 meta 价格不一致 | 迁移逻辑优先取 price_log 数据，meta 仅作 fallback |
| gRPC 客户端不兼容新 message 结构 | Proto reserved 保留旧 field number，新字段用新 number |
| sqlx 编译时检查要求 dev DB 与迁移同步 | 迁移先 apply 到 dev DB，再改代码 |
| inventory_repo 中 category filter 行为变化 | 改用 term_relation JOIN 替代文本匹配，需确认数据覆盖 |

---

## Sources & References

- **Origin document:** [docs/brainstorms/2026-05-04-products-table-redesign-requirements.md](docs/brainstorms/2026-05-04-products-table-redesign-requirements.md)
- **Ideation doc:** [docs/ideation/2026-05-04-products-table-redesign-ideation.md](docs/ideation/2026-05-04-products-table-redesign-ideation.md)
- JSONB 提取先例: `abt/migrations/029_add_bom_draft_status.sql`
- 表创建先例: `abt/migrations/031_create_bom_nodes_table.sql`
- 迁移安全模式: `docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`
