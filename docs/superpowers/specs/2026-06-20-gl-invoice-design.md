# 总账内核 + 发票闭环 设计（M1+M2）

**日期**：2026-06-20
**里程碑**：财务补全 roadmap 第一期（M1 总账内核 + M2 发票闭环合并）
**上游诊断**：`docs/superpowers/specs/2026-06-19-fms-flow-design.md`（fms 出纳/核销/报销/成本已打通；本期补"会计核算内核 + 发票"）

---

## 1. 背景与目标

ABT 的 fms 当前是"资金出纳 + 报销 + 成本归集"，缺会计核算内核。对比 ERPNext / Odoo / OFBiz 三大成熟 ERP，ABT 财务缺：总账科目表、复式记账凭证、销售/采购发票、AR/AP 载体、三大报表、税务、资产、预算（详见 roadmap 差距矩阵）。

本期（M1+M2）补**最内核**的两块：

1. **总账内核（M1）**：科目表 + 复式记账凭证 + 借贷过账 + 试算平衡 + 期间锁定。让所有业务单据有地方落账。
2. **发票闭环（M2）**：销售发票（AR）+ 采购发票（AP），接上"发货→发票""到货→发票"的财务断点，发票过账自动生成 GL 凭证。

**架构方向（已确认）**：业财一体自动过账 —— 业务单据（发票/收付款/报销付款）posted 时在同一事务内自动生成 GL 凭证；另支持手工凭证兜底（调整/结账/非业务事项）。

**凭证承载（已确认）**：独立 `gl_entries`（头）+ `gl_entry_lines`（行）表。出纳/业务单据保持自身语义，总账是独立一层，`source_type/source_id` 反查来源。

## 2. 范围边界

### 本期包含
- 科目表主数据（`gl_accounts`，中国准则 6 大类 + 树形层级）
- 会计凭证（`gl_entries` + `gl_entry_lines`），含手工凭证
- 会计期间（`accounting_periods`）+ 期间锁定
- 销售发票（`sales_invoices`）+ 采购发票（`purchase_invoices`），手工建立 + 可引用发货/到货
- 科目映射配置（`gl_account_mappings`）+ 产品级科目覆盖
- 业财一体自动过账：发票 / 收付款 / 报销付款 posted → 生成 GL 凭证
- 试算平衡表查询
- 期末**期间关闭**（不含损益结转）

### 本期不含（后续里程碑）
- AR/AP 账龄台账与催收（M3）
- 三大财务报表：资产负债表/利润表/现金流量表（M4）；**损益结转**随 M4 做
- 税务申报/增值税申报表（M5，依赖发票进销项数据）
- 银行对账（M6）
- 固定资产（M7）
- 预算 / 标准成本差异 / 多币种汇兑（M8）

### 默认设定（已确认）
- 发票**手工建立 + 可引用发货/到货**（不强制自动生成；自动生成留后续）
- **单币种（人民币）**；多币种放 M8
- 辅助核算本期只 `cost_center` / `profit_center` 两个维度

## 3. 数据模型

### 3.1 `gl_accounts`（科目表）
| 字段 | 类型 | 说明 |
|---|---|---|
| id | i64 | PK |
| code | varchar | 科目编码（如 1001、2202.01）唯一 |
| name | varchar | 科目名称 |
| account_type | i16 | 枚举：1资产/2负债/3权益/4收入/5成本/6费用 |
| parent_id | i64? | 父科目（树形层级） |
| is_detail | bool | 末级科目（true 才允许凭证引用） |
| balance_direction | i16 | 余额方向：1借/2贷 |
| company_id | i64 | 公司（本期单公司，字段预留） |
| status | i16 | 1活跃/2停用 |
| version | i32 | 乐观锁 |

**约束**：只有 `is_detail=true` 科目可被 `gl_entry_lines.account_id` 引用；非末级科目余额由子科目汇总得出（查询时计算，不物化）。

### 3.2 `gl_entries`（凭证头）
| 字段 | 类型 | 说明 |
|---|---|---|
| id | i64 | PK |
| doc_number | varchar | 凭证号（DocumentSequence 生成，DocumentType::GlEntry） |
| period | varchar | 期间（2026-06） |
| entry_date | date | 凭证日期 |
| source_type | i16 | DocumentType（SalesInvoice/PurchaseInvoice/CashJournal/ExpenseReimbursement/Manual） |
| source_id | i64 | 来源单据 id（Manual 时为 0） |
| description | varchar | 摘要 |
| status | i16 | 1draft/2posted |
| total_debit | decimal | 借方合计（= total_credit） |
| total_credit | decimal | 贷方合计 |
| operator_id | i64 | 制单人 |
| version | i32 | 乐观锁 |
| created_at / updated_at | timestamptz | |

### 3.3 `gl_entry_lines`（分录行）
| 字段 | 类型 | 说明 |
|---|---|---|
| id | i64 | PK |
| entry_id | i64 | FK→gl_entries |
| account_id | i64 | FK→gl_accounts（必须 is_detail） |
| debit | decimal(18,6) | 借方金额 |
| credit | decimal(18,6) | 贷方金额 |
| cost_center | i64? | 辅助核算-成本中心 |
| profit_center | i64? | 辅助核算-利润中心 |
| memo | varchar | 行摘要 |

**约束**：每行 debit/credit 互斥（一行只能借或贷，非负）；凭证过账前 `Σdebit == Σcredit` 且 `> 0`。

### 3.4 `accounting_periods`（会计期间）
| 字段 | 类型 | 说明 |
|---|---|---|
| id | i64 | PK |
| name | varchar | 期间名（2026-06）唯一 |
| start_date / end_date | date | |
| status | i16 | 1open/2closed |
| fiscal_year | varchar | 所属会计年度（2026） |

### 3.5 `sales_invoices`（销售发票）+ `sales_invoice_items`
- 头：`id, doc_number, customer_id, issue_date, period, subtotal, tax_amount, total, status(1draft/2posted), source_shipping_id?, operator_id, version`
- 行：`id, invoice_id, product_id, qty, unit_price, tax_rate_id, line_subtotal, line_tax, line_total`

### 3.6 `purchase_invoices`（采购发票）+ `purchase_invoice_items`
- 头：`id, doc_number, supplier_id, issue_date, period, subtotal, tax_amount, total, status, source_arrival_id?, operator_id, version`
- 行：`id, invoice_id, product_id, qty, unit_price, tax_rate_id, line_subtotal, line_tax, line_total`

### 3.7 `gl_account_mappings`（科目映射配置）
| 字段 | 说明 |
|---|---|
| id | PK |
| mapping_key | varchar | 逻辑键：`default_ar`(应收) / `default_ap`(应付) / `default_revenue`(收入) / `default_inventory`(库存) / `default_tax_output`(销项税) / `default_tax_input`(进项税) / `default_bank`(银行) / `default_expense`(费用) |
| account_id | FK→gl_accounts |
| product_id | i64? | null=全局默认；非 null=产品级覆盖 |

产品级收入/成本科目统一放 `gl_account_mappings`（`product_id` 非空的行覆盖全局默认）——**本期不在 `products` 表加 `revenue_account_id`/`cogs_account_id` 字段**，保持映射单一来源。

## 4. 过账规则（业财一体自动）

**事务一致性**：过账与单据 posted 在**同一数据库事务内同步完成**（不开异步 event）——凭证与单据同生共死，避免凭证丢失或单据/账不一致。沿用 ABT 既有 InCallerTx 模式（service 方法接受 `PgExecutor`，调用方控制事务）。

**科目推导**（过账时从 `gl_account_mappings` 解析）：

| 单据 | 借方 | 贷方 |
|---|---|---|
| 销售发票 posted | 应收账款（按客户，default_ar） | 主营业务收入（按产品 revenue 或 default_revenue）+ 销项税（default_tax_output） |
| 采购发票 posted | 库存商品（default_inventory）/费用（default_expense）+ 进项税（default_tax_input） | 应付账款（按供应商，default_ap） |
| 收付款 `cash_journal.confirm` | 银行存款（default_bank）或对方科目 | 应收/应付（由 journal_type + source 推导对方） |
| 报销付款 `generate_payment_journal` | 费用（default_expense，按报销明细 expense_type） | 银行存款（default_bank） |

**实现位置**：
- 发票：新增 `SalesInvoiceService::post` / `PurchaseInvoiceService::post`，内部调 `GlEntryService::post_from_source` 生成凭证。
- 收付款/报销：现有 `cash_journal.confirm` / `expense.generate_payment_journal` 末尾追加过账调用（不破坏现有流程）。
- 新增 `GlEntryService`：`post_from_source(ctx, db, source_type, source_id, lines)` + `create_manual(ctx, db, req)`（手工凭证）+ `trial_balance(ctx, db, period)`（试算）+ `get_account_balance`。

**状态机**：发票 `Draft→Posted` 经 `StateMachineService.transition`（seed `SalesInvoiceStatus` / `PurchaseInvoiceStatus`，复用 055 模式）。GL 凭证 `Draft→Posted` 内部直接置位（不单独走状态机，posted 由来源单据触发，同事务）。

## 5. 试算平衡 + 期间锁定 + 期末结账

- **试算平衡**：`GlEntryService::trial_balance(period)` 汇总 `gl_entry_lines`（仅 posted 凭证）按科目给出期初/本期借/贷/余额，附 `Σ借==Σ贷` 自检。
- **期间锁定**：所有过账入口校验 `entry_date` 落在 `status=open` 的 `accounting_periods`，否则 `DomainError::business_rule("PeriodClosed")`。
- **期末结账**：本期只做**期间关闭**（`accounting_periods.status: open→closed`，需校验该期所有凭证已 posted、试算平衡）。**损益结转留 M4**。

## 6. 与现有模块衔接

- `cash_journal.confirm`：现有状态流转 + 事件保留；末尾追加 `GlEntryService::post_from_source(CashJournal, id, 由 journal_type 推导的 lines)`。
- `expense.generate_payment_journal`：现有建 CashJournal + 转 Paid 逻辑保留；补过 gl_entry（费用/银行）。
- `write_off`：本期不展开，其产生的核销在 M3 AR/AP 台账体现；确保不破坏现有 write_off 行为。
- `cost_center/profit_center`：从 `cash_journal_line` 平移到 `gl_entry_lines`；`cash_journal_line` 保留（出纳明细），过账时透传。
- 新增 DocumentType：`GlEntry` / `SalesInvoice` / `PurchaseInvoice`（供 DocumentSequence + source_type 用）。

## 7. 测试策略

新增 `abt-web/tests/gl_flow_e2e.rs`（沿用 fms_flow_e2e 模式：TestApp + service 直调 + 真实 DB）：

1. **k1 销售发票过账**：建科目表 + 映射 → 建销售发票 → post → 验证 gl_entry 存在、借贷平衡、应收/收入/销项税科目余额正确、发票 status=Posted。
2. **k2 采购发票过账**：建采购发票 → post → 验证库存/进项税/应付科目余额。
3. **k3 收付款过账**：cash_journal.create(SalesReceipt) + confirm → 验证银行/应收 gl_entry。
4. **k4 试算平衡**：跑 trial_balance(period) → 验证 Σ借==Σ贷、各科目余额。
5. **k5 期间锁定**：关闭某期间 → 该期 post 发票应报 `PeriodClosed`。
6. **k6 手工凭证**：create_manual → 验证借贷平衡 + source_type=Manual。
7. **k7 报销付款过账**：expense submit/approve/pay → 验证费用/银行 gl_entry（复用 fms k1 链）。

数据隔离：每用例自建科目表/映射/发票，用唯一 doc_number/期间后缀，避免 dev 库 stale 污染（吸取 fms 教训）。

## 8. 文档同步

- 新增 `docs/uml-design/08-gl.html`：GL 内核 + 发票设计（科目表模型、凭证过账规则、试算/期间、业财一体衔接）。
- 更新 `07-fms.html`：注明 cash_journal.confirm / expense.generate_payment_journal 现会过账到 GL，write_off 将在 M3 体现 AR/AP。

## 9. 验证标准

- `cargo clippy --workspace --tests` 无新增错误
- `cargo test -p abt-web --test gl_flow_e2e` 七用例全绿，可连跑两次（幂等）
- 手动：发票/凭证页面（若本期含前端，见下）可走 Draft→Posted 并看到 GL 凭证

## 10. 前端范围（已确认：含最小前端）

本期**含最小前端**，覆盖到可点击走通：
- 科目表 CRUD 页（`gl_accounts` 树形 + 增删改）
- 销售发票 / 采购发票：创建 / 列表 / 详情 + post 按钮
- 凭证列表 / 详情（`gl_entries`，可按 source 反查来源单据）
- 试算平衡表页（按期间）
- 期间管理页（开/关期间）

遵循 abt-web 既有模式：TypedPath、单端点列表、`hx-post` + HX-Redirect、`#[require_permission("GL","read/create/update")]`、100% UnoCSS。新增权限域 `GL`。

## 11. 非目标（YAGNI）

- 不做发票自动从发货/到货生成（手工建立 + 引用即可）
- 不做损益结转、三大报表、AR/AP 账龄、税务申报、资产、预算、多币种
- 不做凭证反过账/红字冲销的复杂流程（本期 posted 凭证不可逆，冲错用新凭证）
- 不做多公司合并

## 12. 风险

- **科目映射缺失导致过账失败**：发票 post 时若 `gl_account_mappings` 缺必需键，应报清晰错误（`MissingAccountMapping(mapping_key)`），不允许半过账。缓解：测试 k1/k2 覆盖；初始化迁移 seed 一套默认映射指向默认科目。
- **业财一体事务边界**：过账与单据同事务，若过账失败整个单据 post 回滚——这是期望行为（单据未过账不算 posted）。缓解：service 层用 `?` 传播，调用方事务回滚。
- **现有 cash_journal.confirm 改动**：追加过账可能影响现有 fms_flow_e2e（k2 收款核销）。缓解：过账追加在现有逻辑之后，不改变 cash_journal 自身断言；fms 测试若因新增 gl_entry 受影响则相应调整。
- **范围膨胀**：M1+M2 合并体量大。缓解：spec 已明确边界（不含损益结转/报表/账龄），按 e2e 用例驱动增量实现。

## 13. GL 内核补强（基于 ERPNext/Odoo/OFBiz 对照 · 修订前文简化）

> 本节经对照三大 ERP 的 GL 内核精细设计（凭证生命周期/科目模型/辅助核算/期初/明细账/币种），修订前文的过度简化。**覆盖**：3.1 科目字段、3.3 凭证行字段、3.2 凭证状态、第 4 节"posted 不可逆"、第 5 节试算范围、默认设定"单币种"。以下为最终设计。

### A. 凭证三态 + 作废（覆盖第 4 节"posted 不可逆"）
- `gl_entries.status`：`1draft / 2posted / 3cancelled`（三态）
- 新增 `GlEntryService::cancel(ctx, db, id)`：`posted→cancelled`，记审计；余额实时计算时 `status!=posted` 的凭证不计入（等价反向冲销效果，参照 Odoo `button_cancel` 模式）
- **不做** unpost（回 draft）、不做自动生成红字反向凭证（cancel 标记 + 余额排除已足够会计正确）

### B. 期初余额
- `gl_accounts` 加 `opening_balance DECIMAL(18,6)`（本位币期初，默认 0）
- `gl_entries` 加 `is_opening BOOLEAN` 标志；期初通过一张 `is_opening=true` 的手工凭证录入（参照 ERPNext）
- 余额公式：`科目余额 = opening_balance + Σ(本明细行所属凭证 status=posted 的 debit/credit，按 balance_direction 定向)`

### C. 辅助核算加 project 维度（覆盖 3.3）
- `gl_entry_lines` 维度字段：`cost_center i64?` / `profit_center i64?` / **`project_id i64?`**（三维，均裸 id 无主数据表，同 CashJournalLine 既有模式）
- **不做** 可配置维度系统（ERPNext AccountingDimension 太重，固定三维够用）

### D. 明细账查询（覆盖第 5 节"只有试算平衡"）
- 新增 `GlEntryService::general_ledger(ctx, db, account_id, from_date?, to_date?) -> Vec<GlDetailRow>`：按科目查明细分录流水（日期/凭证号/摘要/对方/借/贷/累计余额）
- `get_account_balance(ctx, db, account_id, as_of_date?) -> Decimal`：支持按日期切片（原仅按 period，补 as_of_date）
- 余额实时计算（参照 ERPNext 实时 gl_entry 累加），**不做**物化余额表（OFBiz GlAccountHistory 模式增加一致性成本，实时计算 + 索引够用）

### E. 科目模型补字段（覆盖 3.1）
- `gl_accounts` 补：`reconcile BOOLEAN`（是否需对账——AR/AP 类科目置 true，供 M3 AR/AP 核销用）、`disabled BOOLEAN`（停用，替代原 `status` 字段语义，更贴 ERP）、`currency VARCHAR(10) DEFAULT 'CNY'`（科目币种）
- 原 `status` 字段移除（用 `disabled` 取代）

### F. 多币种基础字段（覆盖默认设定"单币种"）
- `gl_entry_lines` 补：`amount_currency DECIMAL(18,6)`（原币金额）、`currency VARCHAR(10)`、`exchange_rate DECIMAL(18,6)`
- **本期本位币记账**：`debit/credit` 即本位币（CNY），原币字段预留录入；**完整多币种逻辑**（汇率主数据、自动汇兑损益重估）放 M8
- 默认设定修订：原"单币种"→"本位币记账 + 多币种字段预留"

### G. 凭证类型
- `gl_entries` 补 `voucher_type VARCHAR(20)`：`Journal Entry / Receipt / Payment / Contra / Opening`（统计与分类用）
- `source_type/source_id`（DocumentType + id）保留用于业务单据追溯（覆盖第 6 节，不变）

### 留后续 milestone 的边缘项（agent 对照认同，本期不做）
- 多账簿 / 多公司合并（FinanceBook）→ 后续
- 可配置会计维度（ERPNext AccountingDimension）→ 固定三维够用
- 自动损益结转（PeriodClosingVoucher，收入/费用→本年利润）→ **M4 三大报表期**做（依赖成熟科目分类体系）
- 物化余额表（GlAccountHistory）→ 实时计算够用
- 自动汇兑损益重估 → M8

### 对 Plan A 的影响
Plan A（`docs/superpowers/plans/2026-06-20-gl-core.md`）A1-A5 task 对应扩展：A1 migration 补字段（reconcile/disabled/opening_balance/currency/is_opening/voucher_type/project_id/amount_currency/currency/exchange_rate）+ EntryStatus 加 Cancelled；A2 科目 CRUD 补字段；A4 凭证补 `cancel`/`general_ledger`/`get_balance(as_of_date)` + 期初凭证；A5 e2e 补 cancel/明细账/期初用例。
