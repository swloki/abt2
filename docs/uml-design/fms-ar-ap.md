# 应收应付（AR/AP）台账设计

> 域：fms。实现：`abt-core/src/fms/ar_ap/`；前端 `abt-web/src/pages/fms_ar_ledger.rs` / `fms_ap_ledger.rs`

## 数据模型

- **`ar_ap_ledger`**：应收应付台账明细（每笔发票/收款一行）。关键字段：`party_type`(Customer/Supplier)、`party_id`、`direction`(Debit/Credit)、`amount`、`amount_applied`(已核销)、`due_date`、`source_doc_no`、`period`。
  - 未清余额 = `amount - amount_applied`（实体方法 `outstanding()`）
- **`ar_ap_settlements`**：核销记录（付款 ↔ 发票关联，含 `exchange_gain_loss` 汇兑损益）

## ArApService 接口（`abt-core/src/fms/ar_ap/service.rs`）

### 台账查询
- `list_ledger(filter, page) -> PaginatedResult<ArApLedgerRow>` — 分页台账（JOIN 往来方名称 + 科目 + 上游单号 + 产品聚合）
- `list_ledger_details(filter) -> Vec<ArApLedgerDetailRow>` — 台账明细（产品行项目级，导出明细表用，不分页）
- `ledger_summary(filter) -> LedgerSummary` — 台账汇总（顶部统计卡片用，按 due_date 聚合）

**`ArApLedgerFilter`**：`party_type`、`party_id`、`outstanding_only`、`period`、`start_date`/`end_date`、`keyword`（往来方名称）、`doc_no`（发生单号）、`product_code`/`product_name`（产品，EXISTS 三来源行项目）、`rep_name`（销售经理/采购员，EXISTS `users.display_name`）。条件拼装集中在 `build_filter_conditions`（`query_with_party`/`summary`/`query_details` 三处共用，`bind_filter!` 宏按 `FilterArg` 枚举绑定 `$N`），消除原先三处重复的条件拼装。

**前端筛选**（AR/AP 对称）：发生日期范围、客户/供应商名称、发生单号、产品编码、产品名称、销售经理/采购员，各一输入框，HTMX `change/keyup delay:300ms` 触发；「只看未清」toggle 与导出按钮均用 `hx-include="#xx-filter-form input:not([type=hidden])"` 携带全部当前筛选。

**`LedgerSummary`**（按 filter 聚合，逾期基准 = `due_date`）：
- `total_amount`（应收/应付总额）
- `total_outstanding`（未清余额）
- `total_overdue`（逾期：`due_date < today` 且未清）
- `due_within_7d`（7 天内到期）

### 核销
- `settle(req) -> SettleResult`（付款核销发票，支持部分核销/多对多）
- `unsettle(settlement_id)`（反核销）
- `list_settlements(filter, page)`

### 账龄分析
- `ar_aging(req) -> Vec<AgingRow>`（按客户，`AgeingBasis::DueDate`，分桶 [30,60,90,120]）
- `ap_aging(req)`（按供应商）

### 核销选择器
- `list_open_invoices(party_type, party_id)` / `list_unapplied_payments(party_type, party_id)`

## 前端台账页（fms_ar_ledger / fms_ap_ledger）

- **4 个汇总卡片**（应收/应付总额、未清余额、逾期金额、7 天内到期；复用 `stat_card` 模式）
- **往来方搜索**（`keyword`，HTMX keyup 触发）+ **只看未清/全部 toggle**（原子 class）
- **表格 10 列**：日期 / 客户(供应商) / 单据号 / 采购单号(销售单号) / 产品(聚合) / 到期日 / 金额 / 已核销 / 未清余额 / 状态
  - `ArApLedgerRow` 新增 `upstream_doc_no`（采购单号/销售单号，委外为 None）、`product_summary`（产品名称 `string_agg` 聚合，前端 `truncate`）
- **导出明细表**：标题行右侧 `export_button` → `/excel/export/{ap|ar}-ledger-detail`，`hx-include="#{ap|ar}-filter-form"` 携带当前 keyword/只看未清筛选
- **逾期高亮**（due_date 基准）：逾期红「逾期」、7 天内黄「即将到期」、已结清灰
- **样式**：UnoCSS 原子 class（无内联 `style="color"`、无失效 `btn` class）

## 数据流

销售/采购发票过账（`Posted`）→ 插入 `ar_ap_ledger`（Debit 应收 / Credit 应付）→ 台账/账龄展示 → 收款/付款核销（`settle`）→ 累加 `amount_applied` → 未清余额归零即「已结清」。

## 业财一体（Phase 1 + Phase 2 完成，2026-06）

业务单据**直接**驱动往来台账，不经发票实体、不经 GL 凭证：

- **销售发货** `ShippingRequest::ship()` → 直接 insert AR 台账（`source_type=ShippingRequest`，Debit，金额=Σ发货量×订单售价）
- **销售退货** `SalesReturnReceivedHandler`（销售退货 `complete()` 时发布的 `SalesReturnReceived` 事件，Issue #86）→ insert 反向 AR 台账（`source_type=SalesReturn`，Credit，冲减发货 Debit，金额=Σ退货明细 `amount`）。净应收 = 发货 Debit − 退货 Credit
- **采购入库** `ArrivalAcceptedHandler`（来料检验通过事件）→ 直接 insert AP 台账（`source_type=ArrivalNotice`，Credit）
- **采购退货** `PurchaseReturnSettledHandler`（采购对账单 `confirm()` 结算退货时发布的 `PurchaseReturnSettled` 事件，Issue #85）→ insert 反向 AP 台账（`source_type=PurchaseReturn`，Debit，冲减入库 Credit，金额=Σ退货明细 `amount`）。净应付 = 入库 Credit − 退货 Debit = 对账单 `confirmed_amount`
- **委外收货** `OutsourcingOrder::receive()` → 直接 insert AP 台账（`source_type=OutsourcingOrder`，加工费=`iqc_qty × unit_price`）
- **收付款核销** `CashJournal::confirm()` → 台账冲销方向（收款 Credit / 付款 Debit）+ `settle` 自动核销（业务单据 ↔ 收付款）

> **事务边界（强制）**：上述"业务单据驱动台账"的 web handler（如 `ship_shipping`）是多步写（改 `shipped_qty` → 释放预留 → 出库 `SalesShipment` → COGS → AR 立账 → 状态机），**必须用 `pool.begin()` + `commit()` 事务包裹**（参考 `mes_receipt_detail::confirm_receipt` 范式），禁止直接用 `RequestContext.conn`（`acquire` 即 autocommit）。否则 `ship()` 中途失败时已提交步骤无法回滚，产生脏数据残留。事故案例：SO-2026-06-000170 / SR-2026-06-000043，发货明细 `product_id=0` 触发出库可用量预检报错，`shipped_qty` 已提交但库存/COGS/AR/状态全未动，发货单卡在 Picking 却显示已发数量，应收入账缺失。

`ar_ap_ledger` 自包含：`amount_applied` 自记核销，`outstanding()` = `amount − amount_applied`；`settle()` 基于 `source_type` 匹配核销，不依赖任何外部单据表。

**已物理删除**（migration `067` 删 ar_ap 的 GL 列；`068` 删表/枚举/权限）：
- 发票模块 `gl/{sales_invoice,purchase_invoice,invoice}` + 表 `sales_invoices` / `purchase_invoices` 及其 items
- 纯 GL `gl/{account,entry,mapping,period}` + 表 `gl_accounts` / `gl_entries` / `gl_entry_lines` / `gl_account_mappings` / `accounting_periods`
- expense 费用报销 `fms/expense` + 表 `expense_reimbursements` 及其 items / attachments
- 枚举 `DocumentType::{GlEntry, SalesInvoice, PurchaseInvoice}`、`DomainEventType::ExpensePaymentGenerated`
- 前端 14 个页面（GL / 发票 / 费用报销）+ `routes/gl.rs` + 侧边栏「总账管理」模块

**已知留口**：① 销售立账 `tax_rate_id=None`（不含税 AR，待 `SalesOrderItem` 加税率字段）；② 发票删除后 `cancel` 红冲随之消失（台账无反向冲销，核销侧 `unsettle` 可补救）——**采购退货（#85 `PurchaseReturnSettledHandler`）/ 销售退货（#86 `SalesReturnReceivedHandler`）的反向冲减已补齐**；委外按 `iqc_passed_qty` 合格量立账，不良品在源头排除，**无需对称退货冲减**（事后索赔扣款走 `ar_ap_adjustment`）；③ 幂等已根治：`ar_ap_ledger` 加 partial unique index `(source_type, source_id) WHERE source_type <> 11`（migration 070，排除委外 `OutsourcingOrder` 分次收货多行），`ArApLedgerRepo::insert` 改 `ON CONFLICT DO NOTHING` 原子幂等（#89）；委外保留按 `transaction_date` 的应用层防重；④ 收/付款单创建页选业务单据的 `source_type` 交互待完善。

## 台账明细与导出（2026-06）

业务单据驱动的台账是**单据级**（一行 = 一张入库单/发货单/委外单 + 一个总额），但财务对账需要看到行项目级的产品维度。采用「列表单据级 + 导出产品级」分层：

- **列表页**（`list_ledger` → `ArApLedgerRow`）：保持单据级，不破坏核销 `amount_applied`/汇总卡片。新增 `upstream_doc_no`（上游单号）、`product_summary`（产品名 `string_agg DISTINCT` 聚合，前端 `truncate`）两列。
- **导出明细表**（`list_ledger_details` → `ArApLedgerDetailRow`）：展开到产品行项目级，每产品一行含 `quantity`/`unit_price`/`line_amount`。

`ArApLedgerRepo::query_details` 用 CTE 先按 filter 筛 `ar_ap_ledger`，再 `UNION ALL` 三种来源的行项目明细（均带 `source_type` 条件防 id 串台）：

| 来源 source_type | 上游单号 | 产品 | 数量 | 单价 |
|---|---|---|---|---|
| 采购入库 ArrivalNotice(16) | `arrival_notices.purchase_order_id`→`purchase_orders.doc_number` | `arrival_notice_items`（多）| `accepted_qty` | `purchase_order_items.unit_price`（经 `order_item_id`）|
| 委外 OutsourcingOrder(11) | 无 | `outsourcing_orders.product_id`（单）| `completed_qty` | `outsourcing_orders.unit_price` |
| 销售发货 ShippingRequest(3) | `shipping_requests.order_id`→`sales_orders.doc_number` | `shipping_request_items`（多）| `shipped_qty` | `sales_order_items.unit_price`（经 `order_item_id`）|

导出经 `shared/excel/ledger_detail_export.rs::LedgerDetailExporter`（`rust_xlsxwriter`），注册于 `abt-web/routes/excel.rs` 的 `execute_export`（`ap-ledger-detail`/`ar-ledger-detail`，权限 `FMS/read`）。导出按钮 `hx-include="#{ap|ar}-filter-form"` 携带当前 keyword/只看未清筛选。**无 migration**：所有新字段由 JOIN/聚合查询动态得出，不改 `ar_ap_ledger` 表结构。

## 应收应付调整单（2026-06 新增）

> 实现：`abt-core/src/fms/adjustment/`；前端 `abt-web/src/pages/fms_adjustment_create.rs` / `fms_adjustment_list.rs`

**定位**：手工调整 AR/AP 余额（坏账/折扣/抹零/错误更正/汇兑差），让台账与实际对齐。与业务单据驱动的台账互补——业务单据处理正常交易，调整单处理「账面与实际偏差」。**创建即过账**，无草稿/审批态。

**数据模型**（`ar_ap_adjustments`，migration `069`）：
- `doc_number`（自动生成，`DocumentType::ArApAdjustment` 前缀 `ADJ`）
- `party_type`(Customer=应收 / Supplier=应付)、`party_id`
- `direction`(1=Increase 增加 / 2=Decrease 减少) —— 业务方向（`AdjustmentDirection`）
- `amount`、`currency`、`exchange_rate`、`adjustment_date`、`period`
- `int_order_no`（内部订单号，文本参考）、`ext_order_no`（客户/供应商订单号）
- `description`、`ledger_id`（过账生成的 `ar_ap_ledger.id` 回填）

**过账流程** — `AdjustmentService::create_adjustment()` 同事务（参考 `cash_journal::confirm` 业财一体模式）：
1. `DocumentSequenceService.next_number(ArApAdjustment)` 生成单号
2. 插入 `ar_ap_adjustments`
3. 查往来方币种（`customers`/`suppliers`.`currency`，缺省 CNY）
4. **方向映射**到台账 `LedgerDirection`（与 cash_journal 一致）：

   | party_type | AdjustmentDirection | LedgerDirection |
   |---|---|---|
   | Customer | Increase | Debit（应收增）|
   | Customer | Decrease | Credit（应收减）|
   | Supplier | Increase | Credit（应付增）|
   | Supplier | Decrease | Debit（应付减）|

5. `ArApLedgerRepo::insert` 写台账（`source_type=ArApAdjustment`）
6. 回填 `ledger_id`
7. 审计 `AuditAction::Create` + 事件 `DomainEventType::ArApAdjustmentPosted`

**AdjustmentService 接口**（`abt-core/src/fms/adjustment/service.rs`）：
- `create_adjustment(req) -> i64`（创建即过账）
- `get_adjustment(id)` / `list_adjustments(filter, page) -> PaginatedResult<AdjustmentRow>`

## 采购作业中心消费方（2026-06 新增）

> 消费方：`abt-core/src/purchase/work_center/implt.rs`（`PurchaseWorkCenterServiceImpl`）；详见 `purchase-work-center.md` §3.6 跨域依赖。

采购作业中心行展开聚合经 `ArApService` **只读**读取 AP 数据（不写台账）：

- **PO 立账摘要**（`get_po_hub_summary`）：`list_ledger(ArApLedgerFilter{ party_type=Supplier, party_id=po.supplier_id, doc_no=po.doc_number })` → post-filter `source_type=PurchaseOrder && source_id=po.id` → `SUM(amount)`=已立应付 / `SUM(amount_applied)`=已付。因 `ArApLedgerFilter` 无 `source_id` 字段，靠 `doc_no` 缩范围 + post-filter 精确匹配（R1 契约）。
- **供应商 AP 余额**（对账付款 / 退货行展开）：`get_party_balance(CounterpartyType::Supplier, supplier_id).total_ap`。

台账写入（PO 收货立账、付款核销）仍由既有 EventHandler（`ArrivalAcceptedHandler` / FMS 付款回调）驱动，作业中心聚合层不参与写。

**前端**（应收/应付各一套入口，侧边栏「应收调整」「应付调整」）：
- 创建页：往来方选择（entity_picker，复用 journal 的 `search-counterparty`）+ 当前余额只读显示（选往来方后 htmx 查 `ArApService::get_party_balance`）+ 方向/金额/日期/内部订单号/外部订单号/说明；提交后 HX-Redirect 列表
- 列表页：单端点 + keyword 搜索 + 分页；方向标签（增加绿 / 减少红）

**枚举扩展**：`DocumentType::ArApAdjustment = 45`（prefix `ADJ`）、`DomainEventType::ArApAdjustmentPosted = 71`。
