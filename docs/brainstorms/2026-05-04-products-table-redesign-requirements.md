---
date: 2026-05-04
topic: products-table-redesign
---

# Products 表结构重新设计

## Summary

重构 products 表：将 price 提取到独立的 product_price 历史表（替代 product_price_log），将 product_code 和 unit 提升为带约束的独立列，分类改用已有的 term_relation 表关联，移除 subcategory 和 loss_rate，meta 缩减为仅保留 specification、acquire_channel、old_code。

---

## Problem Frame

products 表将所有业务属性存储在单一 JSONB `meta` 列中，导致以下问题：

- **无数据库约束**：product_code 作为跨模块关联的业务主键，没有 UNIQUE 约束，并发写入可能产生重复编码
- **无法索引**：`meta->>'product_code'` 的等值查询和 JOIN 全部走顺序扫描，8+ 个 repository 文件共 36 处引用
- **分类不一致**：category 以文本字符串存储在 meta 中，与系统已有的 terms/term_relation 分类体系脱节，分类改名后产品数据不同步
- **price 生命周期割裂**：price 通过 `jsonb_set` 写入 meta，但 ProductMeta 结构体不包含 price 字段，存在隐性数据丢失风险；价格审计依赖独立的 product_price_log 表
- **冗余字段**：subcategory 和 loss_rate 当前无实际业务用途

---

## Requirements

**价格独立**

- R1. 创建 product_price 表，存储价格变更历史记录，每行包含 product_id、price、operator_id、remark、created_at
- R2. 产品当前价格通过查询该产品最新一行记录获取，无需单独的 current_price 字段
- R3. 原 product_price_log 表归档后移除，其审计功能由新表涵盖
- R4. 所有价格写入和读取路径从 `jsonb_set(meta, '{price}', ...)` 迁移到新表

**列提升**

- R5. product_code 提升为 products 表的独立列，带 NOT NULL 和 UNIQUE 约束
- R6. unit 提升为 products 表的独立列，带 NOT NULL 约束
- R7. 新列数据从现有 meta JSONB 中回填，回填完成后 meta 中对应字段移除

**分类体系**

- R8. 产品分类统一使用已有的 term_relation 表进行关联，不再在 meta 中存储 category 文本
- R9. category 字段从 meta JSONB 中移除

**字段移除**

- R10. subcategory 从 meta JSONB 中移除，不再存储
- R11. loss_rate 从 meta JSONB 中移除，不再存储

**Meta 缩减**

- R12. 迁移后 meta JSONB 仅保留三个字段：specification、acquire_channel、old_code

**迁移安全**

- R13. 每个前向迁移配有对应的 rollback 迁移，遵循团队惯例
- R14. 迁移前执行数据完整性审计：检查 product_code 是否有重复或空值、meta 字段是否可正常提取
- R15. 不添加数据库级外键约束，关联完整性由应用层保证

---

## Acceptance Examples

- AE1. **Covers R1, R2.** 给定产品 ID=42 当前无价格记录，当创建一条 price=99.50 的记录后，查询该产品当前价格返回 99.50；再创建一条 price=120.00 的记录后，查询返回 120.00。
- AE2. **Covers R5.** 给定产品 A 的 product_code="SKU001"，当尝试创建另一个 product_code="SKU001" 的产品时，数据库拒绝插入并抛出唯一约束错误。
- AE3. **Covers R8, R9.** 给定产品关联了 term_id=5（分类"电子元件"），当通过分类查询产品时，结果包含该产品，且 meta 中不再有 category 字段。
- AE4. **Covers R10, R11, R12.** 迁移完成后，任意产品的 meta JSONB 仅包含 specification、acquire_channel、old_code 三个 key，不再包含 category、subcategory、loss_rate、product_code、unit、price。
- AE5. **Covers R13, R14.** 执行前向迁移后数据完整，执行对应 rollback 迁移后 products 表恢复到迁移前状态。

---

## Success Criteria

- products 表拥有 product_code（UNIQUE NOT NULL）和 unit（NOT NULL）独立列
- price 全部读写走 product_price 表，meta 中不再包含 price
- 产品分类通过 term_relation 查询，meta 中不再包含 category
- meta JSONB 仅含 specification、acquire_channel、old_code
- 所有现有测试通过，无功能回归
- migration/rollback 对可正常执行

---

## Scope Boundaries

- bom_routing、bom_labor_process、bom_nodes 的反向归一化（改为 product_id JOIN）作为后续独立任务
- loss_rate 的 DECIMAL 类型修正是 BOM 层面的事，不在本次范围内
- Rust 模型层重构（derive FromRow 等）随 schema 变更自然发生，不作为独立任务
- Proto 定义更新不在本次范围内，由实现阶段处理
- Excel 导入/导出中 product_code 相关逻辑的适配由实现阶段处理

---

## Key Decisions

- **product_price 采用历史记录表设计（最新行=当前价格）**：合并 price 和 price_log 的功能到一张表，简化数据模型，避免双源真相
- **分类用 term_relation 而非 products 表上的 category_id 列**：复用已有的关联表，避免冗余，与系统现有分类体系一致
- **meta 保留而非完全消除**：specification、acquire_channel、old_code 仍为低频、无约束需求的字段，不值得独立成列
- **subcategory 和 loss_rate 完全移除**：当前无业务需求，不保留无用数据
- **迁移遵循团队惯例**：前向/回滚迁移对、无 FK 约束、归档旧表而非直接删除

---

## Dependencies / Assumptions

- term_relation 表已有足够的产品-分类关联数据覆盖现有 category 文本映射
- 现有数据中 product_code 无重复值（或重复值可在迁移前人工处理）
- 现有 meta 中的 price 字段值均可转换为 DECIMAL 类型
- bom_nodes 表中的 unit 列与 products 的 unit 保持一致的值域

---

## Outstanding Questions

### Deferred to Planning

- [Affects R8] 现有 term_relation 数据是否完整覆盖了所有产品的分类？需要数据审计确认
- [Affects R14] 迁移前数据审计的具体阈值（允许空值的数量上限）由实现阶段确定
- [Affects R4] product_price_repo 中使用 `sqlx::query`（非宏）的查询路径需要逐一排查，编译器无法自动捕获遗漏
