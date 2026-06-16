# ABT 采购模块深度分析报告

> 日期：2026-06-15  
> 参考系统：Odoo (addons/purchase)、ERPNext (buying + stock + accounts)  
> 分析范围：abt-core/src/purchase/ 全部 7 个子模块 + abt-web 采购页面 + arrival_handler

---

## 一、ABT 采购模块现状总览

### 1.1 模块结构

| 子模块 | 路径 | 职责 | 状态机 |
|---|---|---|---|
| PurchaseQuotation | `purchase/quotation/` | 供应商报价记录（比价） | Draft → Active → Expired / Cancelled |
| PurchaseOrder | `purchase/order/` | 采购订单 | Draft → Confirmed → PartiallyReceived → Received → Closed / Cancelled |
| PurchaseReturn | `purchase/return_order/` | 采购退货 | Draft → Confirmed → Shipped → Settled / Cancelled |
| PurchaseReconciliation | `purchase/reconciliation/` | 供应商对账 | Draft → Confirmed → Settled |
| PaymentRequest | `purchase/payment/` | 付款申请 | Draft → Approved → Paid / Cancelled |
| MiscellaneousRequest | `purchase/misc_request/` | 零星请购 | Draft → Approved → Purchasing → Received → Closed / Cancelled |
| PurchaseDemandService | `purchase/demand_handler/` | 采购需求池（聚合 → 转 PO） | 需求状态机 |

### 1.2 数据流

```
需求池/MiscRequest → Quotation(比价) → PurchaseOrder → 到货通知(ArrivalNotice)
    → [WMS收货] → 质检 → PO状态自动变更为 Received
    → Reconciliation(对账) → PaymentRequest(付款)
    → PurchaseReturn(退货) ← 从已收货 PO 创建
```

### 1.3 PO Item 实体字段（当前）

```rust
// abt-core/src/purchase/order/model.rs:30-45
pub struct PurchaseOrderItem {
    pub id, order_id, line_no, product_id, description,
    pub quantity, unit_price, amount,           // 金额计算
    pub received_qty, inspected_qty, returned_qty,  // 收退货追踪
    pub quotation_item_id: Option<i64>,         // 报价关联
    pub expected_delivery_date: Option<NaiveDate>,
}
```

**缺失字段**（对标 Odoo/ERPNext）：`tax_ids`、`discount`、`currency_id`、`currency_rate`、`qty_invoiced`、`payment_term_id`、`incoterm`、`fiscal_position`。

---

## 二、Bug 清单（代码级，需立即修复）

### BUG-001: `create()` 校验在 INSERT 之后 — 脏数据残留风险

**文件**：`abt-core/src/purchase/order/implt.rs:88-117`

**问题**：先 insert 主表+明细（第 88-103 行），然后才校验 quantity/unit_price（第 106-117 行）。如果校验失败：

- 若调用方传入的是事务连接 `PgExecutor::Tx`，事务回滚不影响（调用方 rollback 即可）。
- 若调用方传入的是连接池 `PgExecutor::Pool`，**脏数据已持久化**——insert 的主表和明细行不会被回滚。

**修复**：将校验移到 insert 之前，第 84 行（计算 total_amount）之后立即校验。

```rust
// 2. 计算总金额
let total_amount: Decimal = req.items.iter().map(|i| i.quantity * i.unit_price).sum();

// 2.5 校验明细（移到这里！）
for (i, item) in req.items.iter().enumerate() {
    if item.quantity <= Decimal::ZERO {
        return Err(DomainError::validation(format!("订单明细第 {} 行数量必须大于 0", i + 1)));
    }
    if item.unit_price <= Decimal::ZERO {
        return Err(DomainError::validation(format!("订单明细第 {} 行单价必须大于 0", i + 1)));
    }
}

// 3. 插入主表
let id = PurchaseOrderRepo::insert(...)...
```

### BUG-002: `update()` 校验在 DELETE + INSERT 之后

**文件**：`abt-core/src/purchase/order/implt.rs:460-481`

**问题**：先 `update_fields` → `delete_by_order_id` → `insert_items`（第 460-467 行），然后才校验（第 469-481 行）。如果校验失败，**旧明细已被删除，新明细虽已插入但未校验**，订单明细被破坏。

**修复**：校验移到第 460 行之前。

### BUG-003: `create_from_quotation()` 完全不校验 + 数量默认值问题

**文件**：`abt-core/src/purchase/order/implt.rs:131-252`

**问题 A**：整个方法没有任何 quantity/unit_price 校验。

**问题 B**：第 170 行 `quantity: qi.min_order_qty.unwrap_or(Decimal::ONE)`——当报价明细没有 min_order_qty 时，数量默认为 1。这意味着从报价单创建的 PO 行数量可能不正确（应该用报价单明细的实际报价数量，或者要求用户手动指定）。

**修复**：
- 增加 unit_price > 0 校验
- min_order_qty 语义改为"最小起订量"，数量应从报价明细的 quantity 字段或由用户在 UI 上指定，不应默认为 1

### BUG-004: 状态机初始转换错误被 `.ok()` 吞掉

**文件**（3 处）：
- `abt-core/src/purchase/payment/implt.rs:123-126`
- `abt-core/src/purchase/reconciliation/implt.rs:125-128`
- `abt-core/src/purchase/return_order/implt.rs:133-136`

**问题**：
```rust
new_state_machine_service(self.pool.clone())
    .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
    .await
    .ok();  // ← 错误被静默丢弃
```

这违反了 AGENTS.md 的明确规定：**"Never silently discard errors — no `let _ = expr.await;` or `let _ = result;`"**。

如果状态机转换失败（如 DB 连接问题），单据已创建但无初始状态记录，后续状态转换会失败。

**修复**：去掉 `.ok()`，用 `?` 传播错误：
```rust
.await?;
```

> 注：`order/implt.rs:124-126` 和 `misc_request/implt.rs` 中已正确使用 `?`，说明这是部分模块遗漏的一致性问题。

### BUG-005: 退货单不校验退货数量 ≤ 已收货数量

**文件**：`abt-core/src/purchase/return_order/implt.rs:43-139`

**问题**：`create()` 只校验订单状态允许退货（Confirmed/PartiallyReceived/Received），但不校验 `returned_qty` 是否超过 `received_qty`。用户可以退回比已收货更多的数量，导致负库存或数据不一致。

**Odoo 做法**：退货通过 stock.picking 的 reverse move 实现，系统自动限制退货数量 ≤ 已收货数量。

**ERPNext 做法**：通过 `over_delivery_receipt_allowance` 配置容差百分比。

**修复**：
```rust
// 校验退货数量
for item in &req.items {
    let po_item = po_items.iter().find(|p| p.id == item.order_item_id)
        .ok_or_else(|| DomainError::validation("退货明细未关联有效的订单行"))?;
    let max_returnable = po_item.received_qty - po_item.returned_qty;
    if item.returned_qty > max_returnable {
        return Err(DomainError::validation(format!(
            "退货数量 {} 超过可退数量 {}（已收 {} - 已退 {}）",
            item.returned_qty, max_returnable, po_item.received_qty, po_item.returned_qty
        )));
    }
}
```

### BUG-006: 对账单查询全部已收货明细，不按期间过滤

**文件**：`abt-core/src/purchase/reconciliation/implt.rs:61-68`

**问题**：`list_received_by_supplier(supplier_id)` 查询该供应商**所有**已收货明细，传入的 `period` 参数只用于写入主表，不用于过滤查询。这意味着：
- 如果供应商有多期收货，每次创建对账单都会把全部已收货明细拉进来。
- 同一个订单明细可能出现在多张对账单中（重复对账）。

**修复**：
- `list_received_by_supplier` 增加 `period_start` / `period_end` 参数
- 或者排除已关联到其他已确认对账单的明细（`NOT EXISTS` 子查询）

### BUG-007: 到货处理无超收容差

**文件**：`abt-core/src/purchase/arrival_handler.rs:92-94`

**问题**：
```rust
let all_received = po_items.iter().all(|item| item.received_qty >= item.quantity);
```

如果超收（received_qty > quantity），`>=` 仍然成立，会判定为全部收货。但没有校验超收是否在合理容差范围内。ERPNext 通过 `over_delivery_receipt_allowance` 配置允许的超收百分比（如 5%），超过则拒绝收货。

**当前行为**：不限制超收，只要有收货就更新状态。这可能导致严重问题——供应商发 200 个，PO 订 100 个，系统照样标记为 Received。

**建议**：增加可配置的超收容差参数，超过容差拒绝收货或给出警告。

### BUG-008: 取消订单不释放已分配的库存预留

**文件**：`abt-core/src/purchase/order/implt.rs:387-442`

**问题**：`cancel()` 只做状态转换 + 事件发布 + 审计日志，不检查和释放可能已关联的库存预留（如果 PO 创建时通过 `inventory_reservation` 做了预留分配）。虽然当前采购流程似乎不直接预留库存（预留主要在销售端），但如果未来扩展或已有 demand_handler 做了预留关联，取消时应释放。

---

## 三、功能差距分析（对标 Odoo / ERPNext）

### 3.1 价格与税务管理

| 能力 | ABT | Odoo | ERPNext | 差距 |
|---|---|---|---|---|
| **行级折扣** | 无 | `discount` Float 字段，百分比折扣 | `discount_percentage` + `discount_amount` | ABT 的 PO Item 无 discount 字段，无法记录折扣 |
| **税额计算** | 无 | `tax_ids` M2M → account.tax；支持含税/不含税价；fiscal_position 自动映射 | `taxes_and_charges` 模板 + `Purchase Taxes and Charges` 子表 | ABT 完全缺失税务模型，无法处理增值税/进项税 |
| **价格表** | 无 | `product.supplierinfo`（供应商-产品价格关联，含最小起订量、交货周期） | `Price List`（独立价格表实体，可关联供应商） | ABT 报价单是静态记录，无法自动获取供应商的最新价格 |
| **定价规则** | 无 | 基于数量的价格阶梯（supplierinfo 中的 min_qty） | `Pricing Rule`（动态定价引擎，支持批量折扣、季节性折扣） | ABT 无法自动应用批量折扣或动态定价 |
| **多币种** | 仅 quotation_item 有 `currency: String` 字段 | `currency_id` + `currency_rate`（存储汇率快照） | `currency` + `conversion_rate` + `plc_conversion_rate` | ABT 无汇率快照，多币种不可靠 |

### 3.2 采购流程管理

| 能力 | ABT | Odoo | ERPNext | 差距 |
|---|---|---|---|---|
| **RFQ 询价流程** | Quotation 是静态记录 | RFQ → 发送给供应商(portal/email) → 供应商报价 → 比较 → 选标 → PO | Material Request → RFQ → Supplier Quotation → PO | ABT 缺少标准化的 RFQ 询价→比价→选标闭环 |
| **多阶段审批** | 无审批阶段 | `to approve` 状态 + 金额阈值双审批 | `Authorization Control`（按金额配置审批层级） | ABT 的 PO 直接从 Draft → Confirmed，无审批拦截 |
| **PO 锁定** | 无 | `locked` 字段 + `lock_confirmed_po` | 无（通过文档提交状态实现） | ABT 的 Confirmed PO 可被后续操作意外修改 |
| **Confirmed 后修改明细** | 不支持 | 支持（message_post 追踪变更） | `can_update_items()` 支持已提交 PO 修改明细 | ABT 的 Confirmed PO 完全不可修改，实际业务中需要追加/调整 |
| **合并/拆分 PO** | 不支持 | `action_merge()`（多 RFQ 按供应商合并） | mapper 支持跨单据合并 | ABT 无法合并多个需求/请购到同一张 PO |
| **超量/少量收货容差** | 无 | 基于 qty_received 与 product_qty 比较 | `over_order_allowance` / `over_delivery_receipt_allowance` | ABT 不限制超收，也不配置容差 |
| **三向匹配** | 仅付款时做金额容差校验 | PO ↔ Receipt ↔ Bill 三向匹配（`purchase.bill.line.match`） | PO ↔ Purchase Receipt ↔ Purchase Invoice 三向匹配 | ABT 付款仅校验金额偏差，不匹配数量和明细行 |
| **一揽子采购(Blanket Order)** | 无 | `purchase.requisition` | `Blanket Order` | ABT 无法处理长期框架协议下的分批下单 |

### 3.3 供应商管理

| 能力 | ABT | Odoo | ERPNext | 差距 |
|---|---|---|---|---|
| **供应商评分卡** | 仅 Blacklisted/Disqualified 状态 | 无（依赖第三方模块） | `Supplier Scorecard`（完整评分体系：周期、标准、权重、等级） | ABT 无供应商绩效评估能力 |
| **供应商-产品关联** | 无 | `product.supplierinfo`（产品码、供应商 SKU、交货周期、最小起订量） | `Item Supplier` 子表 | ABT 无法记录"某供应商供某产品，交期 X 天，起订量 Y" |
| **交货周期(Lead Time)** | 无 | supplierinfo.delay（天数）→ 自动计算 date_planned | Item.lead_time_days → 自动计算 schedule_date | ABT 的 expected_delivery_date 纯手工输入 |
| **上次采购价参考** | 无 | `last_purchase_price` 自动显示 | `get_last_purchase_rate()` 自动获取 | ABT 创建 PO 时不显示历史采购价，采购员无法比价 |

### 3.4 付款与对账

| 能力 | ABT | Odoo | ERPNext | 差距 |
|---|---|---|---|---|
| **付款条款结构化** | `payment_terms: Option<String>` 纯文本 | `payment_term_id` → `account.payment.term` | `payment_terms_template` + `payment_schedule` 子表 | ABT 的付款条款是文本，无法生成结构化的分期付款计划 |
| **预付款** | 无 | `is_downpayment` + `_create_downpayments()` | `advance_paid` + `advance_payment_status` | ABT 无预付款管理 |
| **到岸成本(Landed Cost)** | 无 | 通过 stock.landed.cost 模块分摊运输/关税 | `set_landed_cost_based_on_purchase_invoice_rate` | ABT 无法将运费/关税分摊到物料成本 |
| **发票状态追踪** | 无 | `invoice_status`（no/to invoice/invoiced）每行 `qty_invoiced` / `qty_to_invoice` | `per_billed` 百分比 | ABT 的 PO 不追踪已开票数量和开票状态 |
| **付款计划** | 无 | `payment_term_id` 自动生成付款计划行 | `Payment Schedule` 子表（日期 + 金额百分比 + 状态） | ABT 无法表达"30% 预付，70% 货到付款"这类分期 |

---

## 四、改进建议（按优先级排序）

### P0 — 立即修复（Bug 修复，不改数据模型）

| 编号 | 改进项 | 影响范围 | 工作量 |
|---|---|---|---|
| P0-1 | BUG-001/002: 校验前置（create + update） | `order/implt.rs` | 0.5h |
| P0-2 | BUG-003: create_from_quotation 增加校验 + 修复数量默认值 | `order/implt.rs` | 0.5h |
| P0-3 | BUG-004: 去掉 `.ok()` 吞错误（3 处） | `payment/`, `reconciliation/`, `return_order/` | 0.5h |
| P0-4 | BUG-005: 退货单增加退货数量上限校验 | `return_order/implt.rs` | 1h |
| P0-5 | BUG-006: 对账单按期间过滤 + 防重复 | `reconciliation/implt.rs` + `repo.rs` | 2h |

### P1 — 核心功能补齐（改数据模型，迁移 + Service 扩展）

| 编号 | 改进项 | 对标 | 影响范围 | 工作量 |
|---|---|---|---|---|
| P1-1 | **PO 行增加 tax + discount 字段** | Odoo `tax_ids`, `discount` | migration + model + repo + implt + 页面 | 4h |
| P1-2 | **PO 增加多币种 + 汇率快照** | Odoo `currency_id`, `currency_rate` | migration + model + repo + implt | 4h |
| P1-3 | **付款条款结构化** | ERPNext `payment_schedule` 子表 | 新建 migration + model + 页面 | 6h |
| P1-4 | **超收容差配置** | ERPNext `over_delivery_receipt_allowance` | buying_settings 表 + arrival_handler 修改 | 3h |
| P1-5 | **PO 确认后修改明细** | Odoo/ERPNext confirmed PO edit | order service 增加 `update_items()` + 状态校验 | 4h |
| P1-6 | **发票/开票状态追踪** | Odoo `qty_invoiced`, `invoice_status` | PO item 增加 `qty_invoiced` 字段 + 对账/付款联动 | 6h |

### P2 — 流程增强（跨模块改动）

| 编号 | 改进项 | 对标 | 影响范围 | 工作量 |
|---|---|---|---|---|
| P2-1 | **多阶段审批** | Odoo `to approve` + ERPNext `Authorization Control` | PO 状态机增加 PendingApproval + 审批配置表 | 6h |
| P2-2 | **供应商价格目录** | Odoo `product.supplierinfo` | 新建 supplier_price 表 + 报价/PO 自动取价 | 8h |
| P2-3 | **上次采购价参考** | ERPNext `get_last_purchase_rate` | PO 创建/编辑页面显示历史价格 | 3h |
| P2-4 | **合并 PO** | Odoo `action_merge` | 新建 merge 服务方法 | 4h |
| P2-5 | **三向匹配增强** | Odoo `purchase.bill.line.match` | 付款校验增加数量级匹配 | 6h |

### P3 — 高级能力（按需实施）

| 编号 | 改进项 | 对标 | 说明 |
|---|---|---|---|
| P3-1 | 供应商评分卡 | ERPNext `Supplier Scorecard` | 完整的供应商绩效评估体系 |
| P3-2 | 一揽子采购(Blanket Order) | Odoo `purchase.requisition` | 框架协议 + 分批释放 |
| P3-3 | 到岸成本(Landed Cost) | Odoo `stock.landed.cost` | 运费/关税分摊到物料成本 |
| P3-4 | 预付款管理 | Odoo `is_downpayment` | 预付款 → 抵扣后续发票 |
| P3-5 | RFQ 门户 | Odoo portal | 供应商在线报价 |

---

## 五、关键设计决策建议

### 5.1 税务模型设计（参考 Odoo）

```
新增表: purchase_tax_rates (税码 + 税率 + 类型[进项/销项])
PO Item 增加: tax_rate_id (FK), tax_amount (计算列)
PO 主表增加: tax_total, amount_total (含税)
```

Odoo 的税务模型更复杂（fiscal_position 自动映射、含税/不含税切换），ABT 可先实现简化版：PO 行直接挂税码，系统自动计算税额。

### 5.2 付款条款结构化（参考 ERPNext）

```
新增表: payment_terms_template (模板)
新增表: payment_schedule (PO 子表)
  - due_date (到期日)
  - payment_percent (付款百分比)
  - payment_amount (计算金额)
  - status (待付/已付)
```

在 PO 确认时，根据 `payment_terms_template` 自动生成 `payment_schedule` 行。后续 PaymentRequest 与 schedule 行匹配。

### 5.3 供应商价格目录（参考 Odoo `product.supplierinfo`）

```
新增表: supplier_product_price
  - supplier_id (FK)
  - product_id (FK)
  - min_order_qty (最小起订量)
  - price (单价)
  - currency (币种)
  - delay_days (交货周期天数)
  - valid_from / valid_until (有效期)
  - sequence (优先级)
```

PO 创建时，选择产品 + 供应商后自动匹配最优价格，计算 `expected_delivery_date = order_date + delay_days`。

### 5.4 超收容差（参考 ERPNext）

```
新增表: purchase_settings (采购参数配置)
  - over_delivery_allowance_pct (超收容差百分比，默认 0)
  - over_order_allowance_pct (超下单容差百分比)
  - maintain_same_rate (全链路价格一致性校验)
  - po_required_for_receipt (收货是否必须关联 PO)
```

在 `arrival_handler.rs` 中收货时校验 `received_qty <= quantity * (1 + tolerance)`。

---

## 六、ABT 与三系 ERP 设计哲学对比

| 维度 | ABT | Odoo | ERPNext |
|---|---|---|---|
| **灵活性** | 偏低（字段固定，流程刚性） | 极高（compute/store/inherit 机制，ORM 生态丰富） | 高（DocType 可自定义字段，定价规则引擎） |
| **校验严格度** | 不足（校验顺序错误，无容差配置） | 适中（基于 product_qty 和配置项） | 高（全链路校验，maintain_same_rate） |
| **状态机** | 显式状态机 + 乐观锁（设计较好） | Selection + button 方法（简单直接） | status_updater 联动更新上游单据 |
| **跨模块集成** | 事件驱动（event_bus + handler） | ORM 关联 + compute 联动 | status_updater + Bin 更新 |
| **可配置性** | 低（硬编码校验逻辑） | 中（res.config.settings） | 高（Buying Settings 独立 DocType） |

**ABT 的优势**：显式状态机 + 乐观锁 + 事件总线的架构设计是优于 Odoo/ERPNext 的。ABT 的问题在于**业务字段覆盖不足**（税、折扣、币种、付款条款等缺失）和**校验逻辑实现有 Bug**，而非架构方向错误。

---

## 七、下一步行动

1. **立即执行 P0**（Bug 修复）——不改数据模型，纯逻辑修复，风险可控
2. **规划 P1**（核心字段补齐）——需要 migration + 设计文档同步
3. **按需推进 P2/P3**——根据实际业务需求逐步迭代

每项 P1 改动需要遵循 ABT 强制流程：**Interface + Model Design → Review → Implementation → Design Doc Sync**。
