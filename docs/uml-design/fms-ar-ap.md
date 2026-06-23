# 应收应付（AR/AP）台账设计

> 域：fms。实现：`abt-core/src/fms/ar_ap/`；前端 `abt-web/src/pages/fms_ar_ledger.rs` / `fms_ap_ledger.rs`

## 数据模型

- **`ar_ap_ledger`**：应收应付台账明细（每笔发票/收款一行）。关键字段：`party_type`(Customer/Supplier)、`party_id`、`direction`(Debit/Credit)、`amount`、`amount_applied`(已核销)、`due_date`、`source_doc_no`、`period`。
  - 未清余额 = `amount - amount_applied`（实体方法 `outstanding()`）
- **`ar_ap_settlements`**：核销记录（付款 ↔ 发票关联，含 `exchange_gain_loss` 汇兑损益）

## ArApService 接口（`abt-core/src/fms/ar_ap/service.rs`）

### 台账查询
- `list_ledger(filter, page) -> PaginatedResult<ArApLedgerRow>` — 分页台账（JOIN 往来方名称 + 科目）
- `ledger_summary(filter) -> LedgerSummary` — 台账汇总（顶部统计卡片用，按 due_date 聚合）

**`ArApLedgerFilter`**：`party_type`、`party_id`、`outstanding_only`、`period`、`start_date`/`end_date`、`keyword`（往来方名称模糊搜，`ILIKE` + `EXISTS` 子查询）

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
- **表格 8 列**：日期 / 客户(供应商) / 单据号 / 到期日 / 金额 / 已核销 / 未清余额 / 状态
- **逾期高亮**（due_date 基准）：逾期红「逾期」、7 天内黄「即将到期」、已结清灰
- **样式**：UnoCSS 原子 class（无内联 `style="color"`、无失效 `btn` class）

## 数据流

销售/采购发票过账（`Posted`）→ 插入 `ar_ap_ledger`（Debit 应收 / Credit 应付）→ 台账/账龄展示 → 收款/付款核销（`settle`）→ 累加 `amount_applied` → 未清余额归零即「已结清」。

## 业财一体（Phase 1 + Phase 2 完成，2026-06）

业务单据**直接**驱动往来台账，不经发票实体、不经 GL 凭证：

- **销售发货** `ShippingRequest::ship()` → 直接 insert AR 台账（`source_type=ShippingRequest`，Debit，金额=Σ发货量×订单售价）
- **采购入库** `ArrivalAcceptedHandler`（来料检验通过事件）→ 直接 insert AP 台账（`source_type=ArrivalNotice`，Credit）
- **委外收货** `OutsourcingOrder::receive()` → 直接 insert AP 台账（`source_type=OutsourcingOrder`，加工费=`iqc_qty × unit_price`）
- **收付款核销** `CashJournal::confirm()` → 台账冲销方向（收款 Credit / 付款 Debit）+ `settle` 自动核销（业务单据 ↔ 收付款）

`ar_ap_ledger` 自包含：`amount_applied` 自记核销，`outstanding()` = `amount − amount_applied`；`settle()` 基于 `source_type` 匹配核销，不依赖任何外部单据表。

**已物理删除**（migration `067` 删 ar_ap 的 GL 列；`068` 删表/枚举/权限）：
- 发票模块 `gl/{sales_invoice,purchase_invoice,invoice}` + 表 `sales_invoices` / `purchase_invoices` 及其 items
- 纯 GL `gl/{account,entry,mapping,period}` + 表 `gl_accounts` / `gl_entries` / `gl_entry_lines` / `gl_account_mappings` / `accounting_periods`
- expense 费用报销 `fms/expense` + 表 `expense_reimbursements` 及其 items / attachments
- 枚举 `DocumentType::{GlEntry, SalesInvoice, PurchaseInvoice}`、`DomainEventType::ExpensePaymentGenerated`
- 前端 14 个页面（GL / 发票 / 费用报销）+ `routes/gl.rs` + 侧边栏「总账管理」模块

**已知留口**：① 销售立账 `tax_rate_id=None`（不含税 AR，待 `SalesOrderItem` 加税率字段）；② 发票删除后 `cancel` 红冲随之消失（台账无反向冲销，核销侧 `unsettle` 可补救）；③ 幂等为 SELECT 查重（`UNIQUE` 约束未加）；④ 收/付款单创建页选业务单据的 `source_type` 交互待完善。
