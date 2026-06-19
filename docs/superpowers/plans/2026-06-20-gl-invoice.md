# 发票闭环 Implementation Plan（Plan B · 财务 roadmap 第二期）

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 销售发票（AR）+ 采购发票（AP），posted 时业财一体过账生成 GL 凭证，接上"发货→发票""到货→发票"的财务断点。

**Architecture:** 复用 Plan A 的 GL 内核。新增 `gl/mapping`（科目映射解析）+ `GlEntryService::post_from_source`（通用过账入口，一步建 posted 凭证）。`sales_invoice`/`purchase_invoice` 各自 post 时按科目映射推导借贷分录，调 post_from_source 同事务过账。发票 cancel 同步 cancel 对应 GL 凭证。

**Tech Stack:** Rust 2024 / sqlx / async-trait / PostgreSQL / rust_decimal

**Spec:** `docs/superpowers/specs/2026-06-20-gl-invoice-design.md`（第 3.5/3.6 发票模型、第 4 过账规则、第 6 衔接）

## Global Constraints

（同 Plan A）中文沟通；conventional commit + Co-Authored-By；`cargo clippy` 验证（禁 cargo run）；跨模块只走 Service trait；禁 `let _ =` 吞错；`#[async_trait]` + `PgExecutor` + `ServiceContext`；五件套模块；AssertSqlSafe；乐观锁；audit_log；decimal(18,6)；改代码同步 docs/uml-design/。

**Plan A 已就绪**（commits a03799c3..61aba926）：`gl_accounts`/`gl_entries`/`gl_entry_lines`/`accounting_periods`/`gl_account_mappings` 表 + `GlAccountService`/`GlPeriodService`/`GlEntryService`（create_manual/post/cancel/trial_balance/general_ledger/get_account_balance）+ DocumentType::GlEntry=45 + gl_account_mappings seed（default_ar/ap/revenue/inventory/tax_output/tax_input/bank/expense）。

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/migrations/057_create_invoices.sql` | sales/purchase 发票表 + 状态机 seed | 新建 |
| `abt-core/src/shared/enums/document_type.rs` | SalesInvoice=46/PurchaseInvoice=47 | 改 |
| `abt-core/src/gl/mapping/{mod,service,implt,model,repo}.rs` | 科目映射解析 | 新建 |
| `abt-core/src/gl/entry/service.rs` + `implt.rs` | 加 `post_from_source` | 改 |
| `abt-core/src/gl/invoice/{mod,enums,model}.rs` | 发票共享 model + InvoiceStatus 枚举 | 新建 |
| `abt-core/src/gl/sales_invoice/{mod,service,implt,model,repo}.rs` | 销售发票 | 新建 |
| `abt-core/src/gl/purchase_invoice/{mod,service,implt,model,repo}.rs` | 采购发票 | 新建 |
| `abt-core/src/gl/mod.rs` | 加 mapping/invoice/sales_invoice/purchase_invoice | 改 |
| `abt-web/src/state.rs` | 加 gl_mapping/sales_invoice/purchase_invoice 访问器 | 改 |
| `abt-web/tests/gl_invoice_flow_e2e.rs` | 发票过账 e2e | 新建 |

---

## Task B1: migration 057 + 科目映射 + post_from_source

**Files:** Create `057_create_invoices.sql`, `gl/mapping/*`; Modify `document_type.rs`(SalesInvoice=46/PurchaseInvoice=47 三处), `gl/entry/{service,implt}.rs`(加 post_from_source), `gl/mod.rs`, `state.rs`

**Interfaces:**
- Consumes: Plan A `GlEntryService`、`gl_account_mappings` 表
- Produces: `GlMappingService::resolve(key, product_id?) -> account_id`；`GlEntryService::post_from_source(ctx, db, source_type, source_id, entry_date, description, lines) -> Result<i64>`（一步建 posted 凭证）；发票表；DocumentType::SalesInvoice/PurchaseInvoice；状态机 seed

- [ ] **Step 1: DocumentType 加 SalesInvoice=46 / PurchaseInvoice=47**（枚举体/from_i16/prefix 三处，prefix "SI"/"PI"）

- [ ] **Step 2: migration 057** —— 发票表 + 状态机 seed（参照 055 格式）：

```sql
-- sales_invoices + items
CREATE TABLE sales_invoices (
    id BIGSERIAL PRIMARY KEY, doc_number VARCHAR(40) NOT NULL,
    customer_id BIGINT NOT NULL, issue_date DATE NOT NULL, period VARCHAR(20) NOT NULL,
    subtotal DECIMAL(18,6) NOT NULL DEFAULT 0, tax_amount DECIMAL(18,6) NOT NULL DEFAULT 0,
    total DECIMAL(18,6) NOT NULL DEFAULT 0, status SMALLINT NOT NULL DEFAULT 1, -- 1draft/2posted/3cancelled
    source_shipping_id BIGINT, operator_id BIGINT NOT NULL, version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), deleted_at TIMESTAMPTZ
);
CREATE INDEX idx_sales_invoices_customer ON sales_invoices(customer_id);
CREATE TABLE sales_invoice_items (
    id BIGSERIAL PRIMARY KEY, invoice_id BIGINT NOT NULL REFERENCES sales_invoices(id),
    product_id BIGINT NOT NULL, qty DECIMAL(18,6) NOT NULL, unit_price DECIMAL(18,6) NOT NULL,
    tax_rate_id BIGINT, line_subtotal DECIMAL(18,6) NOT NULL, line_tax DECIMAL(18,6) NOT NULL, line_total DECIMAL(18,6) NOT NULL
);
-- purchase_invoices + items（同构，supplier_id + source_arrival_id）
CREATE TABLE purchase_invoices ( /* customer_id→supplier_id, source_shipping_id→source_arrival_id，其余同 */ );
CREATE TABLE purchase_invoice_items ( /* 同 sales_invoice_items */ );

-- 状态机 seed（参照 055）
INSERT INTO state_definitions (entity_type,state_name,label,is_initial,is_final) VALUES
    ('SalesInvoiceStatus','Draft','草稿',TRUE,FALSE),('SalesInvoiceStatus','Posted','已过账',FALSE,FALSE),('SalesInvoiceStatus','Cancelled','已取消',FALSE,TRUE),
    ('PurchaseInvoiceStatus','Draft','草稿',TRUE,FALSE),('PurchaseInvoiceStatus','Posted','已过账',FALSE,FALSE),('PurchaseInvoiceStatus','Cancelled','已取消',FALSE,TRUE)
ON CONFLICT DO NOTHING;
INSERT INTO state_transition_defs (entity_type,from_state,to_state,sort_order) VALUES
    ('SalesInvoiceStatus','','Draft',1),('SalesInvoiceStatus','Draft','Posted',2),('SalesInvoiceStatus','Draft','Cancelled',3),
    ('PurchaseInvoiceStatus','','Draft',1),('PurchaseInvoiceStatus','Draft','Posted',2),('PurchaseInvoiceStatus','Draft','Cancelled',3)
ON CONFLICT DO NOTHING;
```
应用 + 验证 seed（`psql` → state_definitions 各 3 行）+ clippy + commit。

- [ ] **Step 3: gl/mapping 模块** —— `GlMappingService::resolve(ctx, db, key: &str, product_id: Option<i64>) -> Result<i64>`：先查 `gl_account_mappings WHERE mapping_key=$1 AND product_id=$2`（产品级），无则 `WHERE mapping_key=$1 AND product_id IS NULL`（全局默认）；都没有 `DomainError::business_rule("MissingAccountMapping")`。model/repo/service/implt 五件套 + 工厂 `new_gl_mapping_service`。`gl/mod.rs` 加 `pub mod mapping;`，state.rs 加 `gl_mapping_service()`。

- [ ] **Step 4: GlEntryService::post_from_source** —— trait 加方法，implt 实现：

```rust
/// 业务单据过账入口：一步建 posted 凭证（status=Posted），同事务。
/// 校验借贷平衡 + 期间 open + 科目末级；source_type/source_id 反查来源。
async fn post_from_source(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>,
    source_type: DocumentType, source_id: i64, entry_date: NaiveDate,
    description: String, lines: Vec<GlEntryLineInput>,
) -> Result<i64>;
```
implt：校验 lines 借贷 XOR + Σ借=Σ贷>0 + 各科目末级 + `resolve_open(entry_date)` 取 period → 建 `gl_entries`(status=Posted, source_type/source_id, total_debit/credit, voucher_type 按 source 推导) → batch_lines → audit（Create）。返回 entry_id。（与 create_manual+post 等价但一步到位 posted，供发票/收付款过账用。）

- [ ] **Step 5: clippy + commit** `feat(gl): migration 057 发票表 + 科目映射 + post_from_source`

---

## Task B2: sales_invoice 销售发票模块

**Files:** Create `gl/sales_invoice/{mod,service,implt,model,repo}.rs`, `gl/invoice/{mod,enums,model}.rs`(共享 InvoiceStatus); Modify `gl/mod.rs`, `state.rs`

**Interfaces:**
- Consumes: B1 `post_from_source` + `GlMappingService::resolve` + Plan A `StateMachineService`
- Produces: `SalesInvoiceService::{create, post, cancel, get, list}`；`app.state.sales_invoice_service()`

- [ ] **Step 1: gl/invoice 共享** —— `InvoiceStatus` 枚举（Draft=1/Posted=2/Cancelled=3，复用模式）；`gl/mod.rs` 加 `pub mod invoice; pub mod sales_invoice;`

- [ ] **Step 2: sales_invoice/model.rs** —— `SalesInvoice`(id/doc_number/customer_id/issue_date/period/subtotal/tax_amount/total/status/source_shipping_id?/operator_id/version/...) + `SalesInvoiceItem`(id/invoice_id/product_id/qty/unit_price/tax_rate_id?/line_subtotal/line_tax/line_total) + `CreateSalesInvoiceReq`(customer_id/issue_date/items/source_shipping_id?) + `SalesInvoiceItemInput`(product_id/qty/unit_price/tax_rate_id?)

- [ ] **Step 3: sales_invoice/repo.rs** —— `create`(头) + `batch_items` + `get_by_id` + `list_items` + `update_status`(乐观锁) + `list`(filter: customer_id/status/period)。参照 fms/expense/repo。

- [ ] **Step 4: sales_invoice/service.rs + implt.rs** —— trait `create/post/cancel/get/list`。核心 **post 过账**（AR 规则，spec 第 4 节）：

```rust
async fn post(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
    let inv = SalesInvoiceRepo::get_by_id(db, id).await?.ok_or(...)?;
    if inv.status != InvoiceStatus::Draft { return Err(business_rule("Only Draft")); }
    let items = SalesInvoiceRepo::list_items(db, id).await?;
    let map = new_gl_mapping_service(self.pool.clone());
    let ar_acct = map.resolve(ctx, db, "default_ar", None).await?;       // 应收
    // 贷方 lines：每产品 收入(+销项税)
    let mut credit_lines = vec![];
    let mut total_tax = Decimal::ZERO;
    for it in &items {
        let rev_acct = map.resolve(ctx, db, "default_revenue", Some(it.product_id)).await
            .or_else(|_| map.resolve(ctx, db, "default_revenue", None)).await?;
        credit_lines.push(GlEntryLineInput { account_id: rev_acct, credit: it.line_subtotal, debit: ZERO, project_id: None, .. });
        total_tax += it.line_tax;
    }
    if total_tax > ZERO {
        let tax_acct = map.resolve(ctx, db, "default_tax_output", None).await?;
        credit_lines.push(GlEntryLineInput { account_id: tax_acct, credit: total_tax, debit: ZERO, project_id: None, .. });
    }
    let mut lines = vec![GlEntryLineInput { account_id: ar_acct, debit: inv.total, credit: ZERO, project_id: None, ..default() }];
    lines.extend(credit_lines);
    // 同事务过账
    let entry_id = new_gl_entry_service(self.pool.clone())
        .post_from_source(ctx, db, DocumentType::SalesInvoice, id, inv.issue_date,
            format!("销售发票 {}", inv.doc_number), lines).await?;
    // 状态机 + 业务表 status
    new_state_machine_service(self.pool.clone()).transition(ctx, db, "SalesInvoiceStatus", id, "Posted", None).await?;
    SalesInvoiceRepo::update_status(db, id, InvoiceStatus::Posted, inv.version).await?; // rows==0 → ConcurrentConflict
    SalesInvoiceRepo::attach_gl_entry(db, id, entry_id).await?; // 见下
    audit(...).await?;
    Ok(())
}
```
> `sales_invoices` 表加 `gl_entry_id BIGINT` 列（B1 migration 里加）记录过账生成的凭证 id，cancel 时用它同步 cancel GL。`attach_gl_entry` 更新该列。
> `cancel`：校验 Posted → cancel 对应 gl_entry（调 `GlEntryService::cancel(gl_entry_id)`）→ 状态机 SalesInvoiceStatus→Cancelled + 业务表 status → audit。
> `create`：算 subtotal/tax_amount/total（Σ items）+ DocumentSequence(SalesInvoice) + 状态机 ''→Draft + audit。

- [ ] **Step 5: state.rs 加 sales_invoice_service() + clippy + commit** `feat(gl): 销售发票模块（post 过账 AR 凭证）`

---

## Task B3: purchase_invoice 采购发票模块

**Files:** Create `gl/purchase_invoice/*`; Modify `gl/mod.rs`, `state.rs`

**Interfaces:** 对称 B2，AP 规则（spec 第 4 节）：借 库存(default_inventory，按产品) + 进项税(default_tax_input) / 贷 应付(default_ap)。`source_arrival_id` 替代 source_shipping_id。

- [ ] **Step 1-5**：同 B2 结构。**post 过账（AP）**：

```rust
// 借方：库存（按产品 line_subtotal）+ 进项税（Σ line_tax）
// 贷方：应付(default_ap, total)
let mut lines = vec![];
for it in &items {
    let inv_acct = map.resolve(ctx, db, "default_inventory", Some(it.product_id)).await.or(...)?;
    lines.push(GlEntryLineInput { account_id: inv_acct, debit: it.line_subtotal, .. });
}
if total_tax > ZERO { lines.push(借 进项税); }
lines.push(GlEntryLineInput { account_id: ap_acct, credit: inv.total, .. });
post_from_source(ctx, db, DocumentType::PurchaseInvoice, id, ...);
// 状态机 PurchaseInvoiceStatus→Posted + status + attach_gl_entry + audit
```
`purchase_invoices` 表加 `gl_entry_id`（B1 migration 里加）。cancel 同 B2。state.rs 加 `purchase_invoice_service()`。clippy + commit `feat(gl): 采购发票模块（post 过账 AP 凭证）`。

---

## Task B4: 发票 e2e（gl_invoice_flow_e2e.rs）

**Files:** Create `abt-web/tests/gl_invoice_flow_e2e.rs`

**Interfaces:** Consumes B2/B3 + Plan A GL

- [ ] **Step 1: 写 e2e**（参照 gl_flow_e2e/fms_flow_e2e 模式）：

```rust
mod common; use common::TestApp;
use rust_decimal::Decimal;
use abt_core::shared::types::ServiceContext;
use abt_core::gl::sales_invoice::{model::*, SalesInvoiceService};
use abt_core::gl::purchase_invoice::{model::*, PurchaseInvoiceService};
use abt_core::gl::entry::GlEntryService;
use abt_core::gl::enums::EntryStatus;

async fn customer_id() -> i64 { 135 }   // dev 库客户
async fn supplier_id() -> i64 { 129 }   // dev 库供应商
async fn product_id() -> i64 { 565 }    // dev 库产品

#[tokio::test]
async fn k1_sales_invoice_posts_ar_entry() {
    // 建销售发票 → post → 验证 gl_entry（source=SalesInvoice）+ 应收/收入/销项税科目余额
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.sales_invoice_service();
    let id = svc.create(&ctx, &mut conn, CreateSalesInvoiceReq {
        customer_id: customer_id().await, issue_date: chrono::Utc::now().date_naive(),
        source_shipping_id: None,
        items: vec![SalesInvoiceItemInput { product_id: product_id().await, qty: Decimal::from(10), unit_price: Decimal::from(100), tax_rate_id: None }],
    }).await.unwrap();
    svc.post(&ctx, &mut conn, id).await.unwrap();
    let inv = svc.get(&ctx, &mut conn, id).await.unwrap();
    assert_eq!(inv.status, InvoiceStatus::Posted);
    assert_eq!(inv.total, Decimal::from(1000)); // 10*100，无税
    // 应收科目余额 = 1000（default_ar=1122）；收入科目余额=1000
    // 用 gl_entry_service.get_account_balance 验证（需先 resolve mapping 拿 account_id）
}

#[tokio::test]
async fn k2_purchase_invoice_posts_ap_entry() {
    // 建采购发票 → post → 验证 库存/应付 余额（AP）
}

#[tokio::test]
async fn k3_invoice_cancel_cancels_gl() {
    // post 发票 → cancel → 验证发票 Cancelled + 对应 gl_entry Cancelled + 余额归零
}
```
> implementer 补全 k2（AP：库存/应付余额）+ k3（cancel：发票+GL 同步 cancelled，余额排除）。用 `gl_mapping_service().resolve("default_ar"/"default_ap"/..., None)` 拿 account_id 再 `gl_entry_service().get_account_balance` 断言。

- [ ] **Step 2: 跑 e2e + fms/gl 回归** → `cargo test -p abt-web --test gl_invoice_flow_e2e`（3 passed）+ gl_flow_e2e（6）+ fms_flow_e2e（4）全绿。clippy + commit `test(gl): 发票过账 e2e（AR/AP/cancel）`。

---

## 完成验证（Plan B 收尾）

- [ ] `cargo clippy --workspace --tests` 无新错
- [ ] gl_invoice_flow_e2e 3 passed + gl_flow_e2e 6 + fms_flow_e2e 4 回归全绿
- [ ] 发票 posted 真正生成 GL 凭证 + 科目余额正确（业财一体闭环）

## Plan B 产出（供 C/D 依赖）

- `SalesInvoiceService`/`PurchaseInvoiceService`（create/post/cancel/get/list）
- `GlEntryService::post_from_source`（业务单据过账通用入口，Plan C 收付款/报销衔接复用）
- `GlMappingService::resolve`（科目映射解析）
- 发票表 + DocumentType::SalesInvoice/PurchaseInvoice + 状态机
