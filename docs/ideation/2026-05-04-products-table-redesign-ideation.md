---
date: 2026-05-04
topic: products-table-redesign
focus: 重新设计 products 表，从 JSONB meta 字段迁移到规范化列
mode: repo-grounded
---

# Ideation: Products 表重新设计

## Grounding Context

**Codebase Context:** products 表仅有 3 列：product_id (PK)、pdt_name (VARCHAR)、meta (JSONB)。meta 包含 8 个业务字段：category, subcategory, product_code, specification, unit, acquire_channel, loss_rate, old_code，加上 price 字段。`meta->>'field'` 访问模式散布在 8+ 个 repository 文件（product_repo, inventory_repo, bom_repo, product_price_repo, routing_repo, export/import handlers, labor_process_repo），约 36 处引用。

**Past Learnings:** bom_detail JSONB 已成功迁移到 bom_nodes 表（migration 031/032）。团队惯例：前向/回滚迁移成对、无 FK 约束（应用层检查）、归档而非 DROP 表。迁移安全模式：INSERT...ON CONFLICT、ALTER TABLE RENAME TO _archived。

**External Context:** 外部研究不可用（API 错误），但通用原则已记录。

## Selected Ideas (4 of 6)

### 1. 反向归一化：消除 product_code 作为跨表 JOIN 键

**Description:** 将 routing/labor_process 表的 JOIN 键从 product_code（JSONB 中的业务字符串）改为 product_id（数据库主键）。product_code 变为仅显示用途。

**Warrant:** `direct:` routing_repo.rs:382 用 `JOIN products p ON p.meta->>'product_code' = br.product_code`；labor_process_repo.rs:257 用 `WHERE blp.product_code = p.meta->>'product_code'`。这是用非规范化业务键做关联的典型反模式。

**Rationale:** 即使将 meta 全部拆为列，product_code 作为 JOIN 键的架构问题依然存在——product_code 变动导致下游引用静默断裂。改为 product_id 后，products 表内部存储格式变成纯实现细节，下游模块不再依赖它。这也大幅减少需要迁移的 `meta->>'product_code'` 访问点（从 21 处降至约 5 处）。

**Downsides:** 需同时修改 routing/labor_process 表结构和代码；影响 Excel 导入/导出接口；可能需更新 proto 定义。

**Confidence:** 85%
**Complexity:** High
**Status:** Unexplored

### 2. 分阶段双写迁移策略

**Description:** 遵循 bom_detail → bom_nodes 先例（migration 031/032），三阶段迁移：(1) 新增关系列，保留 meta JSONB；(2) 数据回填 + 双写；(3) 确认稳定后移除 JSONB 列。每阶段配有 rollback migration。

**Warrant:** `direct:` 031_create_bom_nodes_table.sql 创建 bom_nodes 表，032_rollback_create_bom_nodes_table.sql 为回滚。CLAUDE.md 记载 "always create rollback migration pairs" 和 "archive tables instead of DROP"。

**Rationale:** products 表是系统核心表——几乎所有业务查询都涉及它。双写过渡比大爆炸切换安全得多。团队已有成功先例。sqlx 编译时检查自然引导需要更新的查询。每阶段可独立部署和回滚。

**Downsides:** 双写期间写入 latency 略增；需要管理过渡期代码复杂度；需数据一致性验证。

**Confidence:** 90%
**Complexity:** Medium
**Status:** Unexplored

### 3. price 独立提取为 product_prices 表

**Description:** price 在 meta JSONB 中但 ProductMeta struct 不包含它。价格读写走独立路径（product_price_repo.rs 用 jsonb_set），且有独立的 product_price_log 审计表。将 price 提取为 product_prices (product_id PK, current_price DECIMAL(10,6), updated_at) 表。

**Warrant:** `direct:` product_price_repo.rs:36 用 `jsonb_set(meta, '{price}', ...)` 更新；product_without_price_export.rs 用三个 OR 条件判断"无价格"。ProductMeta struct（product.rs:38-55）不含 price 字段。

**Rationale:** price 是唯一拥有独立审计表的 meta 字段，有独立生命周期（频繁更新 vs 其他字段几乎不变）。jsonb_set 模式存在隐性数据丢失风险：meta 为 NULL 时更新会丢失所有其他属性。提取后 NULL（无价格）vs 零值（价格为零）语义更清晰。

**Downsides:** 需更新所有价格读取路径；product_without_price_export 逻辑需重写为 LEFT JOIN；需明确新表与 price_log 表的关系。

**Confidence:** 80%
**Complexity:** Medium
**Status:** Unexplored

### 6. category/subcategory 统一到 term 关系体系

**Description:** 分类以纯文本存在 meta 中，但系统已有 terms 表和 term_relation 关联表。统一为通过 term_relation 的标准关联，products 表增加 category_id / subcategory_id 列。

**Warrant:** `direct:` inventory_repo.rs:312 用 `p.meta->>'category' = term_id.to_string()` 做整数到字符串的隐式转换。product_repo.rs:89-92 通过 term_relation JOIN 过滤——两种分类查询模式并存，语义不统一。

**Rationale:** 分类改名后 products.meta 中的旧文本不会更新，导致过滤遗漏。统一为 ID 引用后分类变更自动生效。可与列拆分合并进行——category_id 替代 category 文本列。

**Downsides:** 需确认现有 category 存的是名称还是 ID；需建立映射表；影响前端分类筛选逻辑。

**Confidence:** 70%
**Complexity:** Medium
**Status:** Unexplored

## Cross-Cutting Combination

反向归一化 + 分阶段迁移的协同：先将 routing/labor_process 的 JOIN 键从 product_code 改为 product_id（消除根因，减少约 16 处 meta 访问），再做 meta 字段迁移时只需处理约 20 处而非 36 处。两个方向可并行推进。

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | EAV 模型替代 products 表 | 过度工程，8 字段用 EAV 需 8 次 JOIN |
| 2 | 元迁移框架抽象 | 过早抽象，只有 2-3 次使用场景 |
| 3 | 视图层隔离 | 半吊子方案，不解决类型安全和约束缺失 |
| 4 | 保留 JSONB + GIN + CHECK | task.md 明确要求不使用 jsonb |
| 5 | GIN 表达式索引 | 不迁移列的权宜之计 |
| 6 | 先加唯一约束 | 迁移前置步骤，非独立设计方向 |
| 7 | 城市规划分区（按稳定性） | 被分批迁移方案覆盖 |
| 8 | 双重地址簿模式（触发器） | 与双写重复，触发器违反项目惯例 |
| 9 | 数据审计（迁移前） | 迁移前提条件，非设计想法 |
| 10 | sqlx 迁移成本评估 | 迁移计划准备，非设计产出 |
| 11 | 消除 COALESCE 防御 | 列迁移的自然结果 |
| 12 | loss_rate f64→Decimal | 太小，融入列迁移子项 |
| 13 | Generated Column Bridge | 双源真相，不如双写干净 |
| 14 | product_code 辅助表 | 过度规范化，增加 JOIN |
| 15 | Rust 模型层重构 | 列迁移的伴随工作，非独立方向 |
| 16 | 全量列迁移（8 字段平铺） | 变更面过大，与分阶段迁移冲突 |
| 17 | 增量提取只提 product_code | 导致混合访问模式 |
