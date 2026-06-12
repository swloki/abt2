---
date: 2026-06-12
issue: 14
status: approved
---

# 销售订单详情页新增"库存数量"列

## 需求

Issue #14：在销售订单详情页商品列表中增加"库存数量"展示列，方便用户对比订单量与库存量。

## 方案

页面层聚合库存，只修改 `abt-web/src/pages/sales_order_detail.rs`。

### 改动内容

1. 在页面函数中，复用已有 `product_ids`，遍历调用 `inventory_svc.get_by_product()` 获取每个产品的库存详情，聚合为 `HashMap<i64, Decimal>`（product_id → 合计库存量）
2. 表格头部：在"数量"列之后插入 `th class="num-right" { "库存数量" }`
3. 表格行：在对应位置插入 `td`，从 HashMap 取值显示，无库存时显示 `-`

### 不改动的部分

- Service trait / model / repo 层不变
- 不修改 `SalesOrderItem` 实体

## 成功标准

- 详情页商品列表显示"库存数量"列，位于"数量"和"单价"之间
- 数值为该产品所有仓库库存合计
- 无库存产品显示 `-`
