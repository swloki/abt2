---
module: master_data
tags: [products, acquire_channel, schema-migration, data-migration, enum, postgres]
problem_type: schema-migration
date: 2026-06-12
---

# 产品采购渠道（acquire_channel）字段迁移

## 背景

`acquire_channel` 原本以**字符串**形式埋在 `products.meta` JSONB 中（`meta->>'acquire_channel'`），取值为 `self-made` / `purchase` / `outsourced` / `non-inventory` 及中文「自制 / 外购 / 委外 / 费用」等多种别名。

JSONB 内的字符串存在以下问题：
- 无法加 CHECK 约束做枚举校验，脏数据（如 `c采购`）可直接写入；
- 无法高效索引，按渠道统计/分流只能字符串匹配，脆弱且慢；
- 别名不统一（中英文混用），业务逻辑需枚举所有写法。

## 方案

迁移 `032_acquire_channel_enum.sql` 将 `acquire_channel` 提升为独立列：

- 新增 `products.acquire_channel SMALLINT NOT NULL DEFAULT 9`
- CHECK 约束 `acquire_channel IN (1, 2, 3, 4, 9)`
- 部分索引 `idx_products_acquire_channel`（`WHERE deleted_at IS NULL`）
- 数据回填：按中英文别名映射到枚举值

### 枚举语义

定义于 `abt-core/src/master_data/product/model.rs:66`（`AcquireChannel`）：

| 值 | Variant       | 含义                                              |
|----|---------------|---------------------------------------------------|
| 1  | SelfProduced  | 自制                                              |
| 2  | Purchased     | 外购                                              |
| 3  | Outsourced    | 委外（预留）                                      |
| 4  | NonInventory  | 费用 / 服务 / 虚拟件（跳过库存校验和补货）        |
| 9  | Legacy        | 历史遗留，**行为等同自制**，日志驱动数据清洗（临时态） |

回填映射词：`self-made / 自制 / 自产 → 1`；`purchase / 外购 / 采购 → 2`；`outsourced / 委外 → 3`；`non-inventory / 费用 / 服务 → 4`；其余 → `9`。

## 数据清洗（2026-06-12）

迁移后共 354 条未匹配（`acquire_channel = 9`），逐类处理：

| 原 `meta->>'acquire_channel'` | 数量 | 处理                                                 |
|-------------------------------|------|------------------------------------------------------|
| `''`（空字符串）              | 352  | 本身无渠道信息，保持 `9`（Legacy，待业务确认归属）   |
| `c采购`（product_id 9137）    | 1    | 脏数据，修正为 `2`，`meta` 同步改为「采购」          |
| `客供`（product_id 9138）     | 1    | 归并到 `2`（外购），`meta` 同步改为「采购」          |

**业务决策**：「客供料」暂不新增独立枚举（如 `5`），归并到外购(2)。若未来客供场景增多，再考虑独立枚举（届时需同步 CHECK 约束、`AcquireChannel` 枚举、迁移映射、前端选项、设计文档）。

清洗后渠道分布：自制 3625 / 外购 5877 / 委外 2648 / Legacy(空) 352。

## 注意事项

- **`Legacy(9)` 是临时态**：代码注释明确"行为等同自制，日志驱动数据清洗"。当前遗留的 352 条空值并非终态，应结合业务逐步清洗到正确渠道（多数应归自制 1）。新数据不应再写入 9。
- **同一枚举复用**：`acquire_channel` 字段同时出现在 `fulfillment_plan_lines`（迁移 033）和 `demands`（迁移 034）表，共用 `AcquireChannel` 枚举与 CHECK 约束。
- **写入校验**：新增/更新产品时由应用层（`ProductService`）保证 `acquire_channel` 取合法枚举值，勿依赖字符串别名。

## 关联

- 迁移：`abt-core/migrations/032_acquire_channel_enum.sql`、`033_sales_order_fulfillment.sql`、`034_demands.sql`
- 枚举：`abt-core/src/master_data/product/model.rs`（`AcquireChannel`、`from_i16` / `as_i16`）
- 设计文档：`docs/uml-design/09-master-data.html`
