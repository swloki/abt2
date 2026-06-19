# 业财衔接 Implementation Plan（Plan C · 财务 roadmap 第三期）

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 收付款（cash_journal.confirm）+ 报销付款（expense.generate_payment_journal）posted 时业财一体过账生成 GL 凭证，打通"资金出纳→总账"最后一公里。

**Architecture:** 复用 Plan B 的 `GlEntryService::post_from_source`（通用过账入口）+ `GlMappingService::resolve`（科目映射）。在现有 confirm / generate_payment_journal 方法**末尾追加**过账调用（同事务），不破坏既有状态流转/事件。按 `JournalType` + `direction` 推导借贷分录：银行（default_bank）一边 + 对方科目（应收/应付/费用）另一边。

**Tech Stack:** Rust 2024 / sqlx / async-trait / PostgreSQL / rust_decimal

**Spec:** `docs/superpowers/specs/2026-06-20-gl-invoice-design.md`（第 4 节过账规则表、第 6 节衔接）

## Global Constraints

（同 Plan A/B）中文沟通；conventional commit + Co-Authored-By；`cargo clippy` 验证（禁 cargo run）；跨模块只走 Service trait；禁 `let _ =` 吞错；`#[async_trait]` + `PgExecutor` + `ServiceContext`；改代码同步 docs/uml-design/；e2e 串行（共享 dev DB）。

**Plan A+B 已就绪**（commits a03799c3..3956988f）：GL 内核 + 发票闭环 + `post_from_source` + `GlMappingService::resolve` + 映射种子（default_bank/ar/ap/expense/revenue/inventory/tax_output/tax_input）。13 e2e 串行全绿。

---

## 设计决策（衔接不重复过账）

| 触发点 | JournalType | 借方 | 贷方 |
|---|---|---|---|
| `cash_journal.confirm` | SalesReceipt (Inflow) | default_bank（全额） | default_ar（全额） |
| `cash_journal.confirm` | PurchasePayment (Outflow) | default_ap（全额） | default_bank（全额） |
| `expense.generate_payment_journal` | Expense (Outflow) | default_expense（全额） | default_bank（全额） |

**为什么 Expense 只在 generate_payment_journal 过账，不在 confirm 过账**：`generate_payment_journal` 建 CJ 后直接置 `Confirmed`（跳过 `confirm` 调用），故 expense 路径必须自己过账。为避免"手动建 Expense CJ + confirm"与"generate 自动建 CJ"双路径对同一业务语义产生混淆，`confirm` 对 `JournalType::Expense` **跳过过账**（warn 日志），Expense 类型统一由 `generate_payment_journal` 过账。两条路径作用于不同 CJ 实例（不同 id），各自一张 GL 凭证，**不存在同一 CJ 双重过账**。

**Payroll / Other 类型本期不过账**：缺 `default_payroll` 等对方科目映射（YAGNI）。`confirm` 对这两类跳过过账（warn 日志），留后续里程碑。

**期间约束**：过账调 `post_from_source` 会校验 `entry_date` 落在 open 的 `accounting_periods`。fms 现有 e2e（k1 报销 / k2 收款）用 `chrono::Utc::now()`（=2026-06）建单，**必须确保 2026-06 期间在 `accounting_periods` 且 status=open**（056 种子已种 2026-01..06，见 C3 验证步骤），否则追加过账会让 fms 回归报 `PeriodClosed`。

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-core/src/fms/cash_journal/implt.rs` | `confirm` 末尾追加 GL 过账（SalesReceipt/PurchasePayment） | 改 |
| `abt-core/src/fms/expense/implt.rs` | `generate_payment_journal` 末尾追加 GL 过账（Expense） | 改 |
| `abt-core/src/gl/mapping/implt.rs` 或 `service.rs` | （若需）补 helper：按 journal 推导 lines | 视情况改 |
| `abt-web/tests/gl_integration_flow_e2e.rs` | 衔接 e2e（收付款过账 + 报销付款过账） | 新建 |
| `docs/uml-design/08-gl.html` | 衔接段落补 cash_journal/expense 过账 | 改 |

---

## Task C1: cash_journal.confirm 追加 GL 过账

**Files:** Modify `abt-core/src/fms/cash_journal/implt.rs`（confirm 方法，约 79-172 行）

**Interfaces:**
- Consumes: Plan B `GlEntryService::post_from_source(ctx, db, source_type: DocumentType, source_id, entry_date, description, lines) -> Result<i64>`；`GlMappingService::resolve(ctx, db, key, product_id?) -> Result<i64>`；`new_gl_entry_service(pool)` / `new_gl_mapping_service(pool)` 工厂；`abt_core::gl::entry::model::GlEntryLineInput { account_id, debit, credit, cost_center, profit_center, project_id, memo }`
- Produces: confirm 现在会为 SalesReceipt/PurchasePayment 类型 CJ 同事务生成一张 posted GL 凭证（source_type=DocumentType::CashJournal, source_id=CJ.id）

**注意**：confirm 内现有变量可用——`journal: CashJournal`（含 `journal_type: JournalType`、`direction: CashDirection`、`amount: Decimal`、`transaction_date: NaiveDate`、`doc_number: String`、`period: String`、`id`）。明细 `CashJournalLine` 的 cost_center/profit_center 可取**第一行**（收付款单边凭证，辅助核算通常一致）作 GL 行的维度；若无明细则 None。

- [ ] **Step 1: 在 confirm 末尾（事件发布之前或之后均可，但必须在事务内、status 已更新之后）追加过账 helper 调用**

在 `cash_journal/implt.rs` 顶部确认 use：`use abt_core::gl::entry::{new_gl_entry_service, model::GlEntryLineInput}; use abt_core::gl::mapping::new_gl_mapping_service; use abt_core::shared::enums::DocumentType; use rust_decimal::Decimal;`（已 import 的跳过）。

在 confirm 方法体、`update_status` 成功之后、return 之前，追加：

```rust
// 业财一体：收付款确认后同事务过账到 GL（SalesReceipt/PurchasePayment）
// Expense 类型由 expense.generate_payment_journal 统一过账，此处跳过
// Payroll/Other 暂无对方科目映射，跳过（留后续里程碑）
let post_result = self.post_cash_journal_gl(ctx, db, &journal).await;
if let Err(e) = post_result {
    tracing::warn!(journal_id = journal.id, error = %e, "GL 过账失败，CJ 仍确认");
    // 注：过账失败是否回滚？见 Step 2 决策——本期选「过账失败视为硬错误，整事务回滚」
    // 故上面 warn 仅作示例，实际应 `?` 传播。见 Step 2。
}
```

- [ ] **Step 2: 过账失败处理决策——硬错误传播（?）**

业财一体核心原则（spec 第 4 节）：过账与单据**同事务同生共死**，过账失败则单据不算确认成功。故**不**用 warn 吞错，改为 `?` 传播。Step 1 的占位替换为：

```rust
self.post_cash_journal_gl(ctx, db, &journal).await?;
```

若调用方（fms_flow_e2e）在事务内，过账失败整个 confirm 回滚。若 fms 现有 confirm 不在外层事务（InCallerTx，db 即传入的连接），`?` 把错误抛给调用方——期望行为。

- [ ] **Step 3: 实现 post_cash_journal_gl helper（同 implt.rs 私有方法）**

在 `impl CashJournalServiceImpl` 内加私有 async 方法：

```rust
/// 收付款确认后按 JournalType 推导借贷分录，过账到 GL。
/// - SalesReceipt (Inflow):  借 default_bank / 贷 default_ar
/// - PurchasePayment (Outflow): 借 default_ap / 贷 default_bank
/// - Expense: 跳过（由 expense.generate_payment_journal 过账）
/// - Payroll/Other: 跳过（无对方科目映射）
async fn post_cash_journal_gl(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    journal: &CashJournal,
) -> Result<()> {
    use crate::fms::enums::JournalType;
    let map = new_gl_mapping_service(self.pool.clone());
    let bank_acct = map.resolve(ctx, db, "default_bank", None).await?;

    // 取第一条明细的辅助核算维度（收付款单边凭证通常一致）
    let lines_db = CashJournalLineRepo::list_by_journal(db, journal.id).await?;
    let (cc, pc) = lines_db.first()
        .map(|l| (l.cost_center, l.profit_center))
        .unwrap_or((None, None));

    let (debit_acct, credit_acct) = match journal.journal_type {
        JournalType::SalesReceipt => {
            let ar = map.resolve(ctx, db, "default_ar", None).await?;
            (bank_acct, ar) // 借银行 / 贷应收
        }
        JournalType::PurchasePayment => {
            let ap = map.resolve(ctx, db, "default_ap", None).await?;
            (ap, bank_acct) // 借应付 / 贷银行
        }
        JournalType::Expense | JournalType::Payroll | JournalType::Other => {
            // Expense 由 generate_payment_journal 过账；Payroll/Other 无对方科目映射
            tracing::info!(journal_id = journal.id, jt = ?journal.journal_type, "跳过 GL 过账");
            return Ok(());
        }
    };

    let amt = journal.amount;
    let gl_lines = vec![
        GlEntryLineInput {
            account_id: debit_acct,
            debit: amt,
            credit: Decimal::ZERO,
            cost_center: cc,
            profit_center: pc,
            project_id: None,
            memo: format!("收付款 {}", journal.doc_number),
        },
        GlEntryLineInput {
            account_id: credit_acct,
            debit: Decimal::ZERO,
            credit: amt,
            cost_center: cc,
            profit_center: pc,
            project_id: None,
            memo: format!("收付款 {}", journal.doc_number),
        },
    ];

    new_gl_entry_service(self.pool.clone())
        .post_from_source(
            ctx,
            db,
            DocumentType::CashJournal,
            journal.id,
            journal.transaction_date,
            format!("收付款确认 {}", journal.doc_number),
            gl_lines,
        )
        .await?;
    Ok(())
}
```

> 实现者注意：`CashJournalLineRepo::list_by_journal` 的确切方法名以 repo.rs 现有签名为准（可能叫 `list_by_journal_id` 或 `find_by_journal`）——**用 LSP 查 CashJournalLineRepo 的方法定义**，禁止猜名。`CashJournal` 字段名以 model.rs 为准（`transaction_date` / `amount` / `doc_number` / `journal_type`）。`JournalType` 变体名以 `fms/enums.rs` 为准。

- [ ] **Step 4: clippy + 单测（若有）+ commit**

`cargo clippy -p abt-core`。commit: `feat(fms): cash_journal.confirm 追加业财一体 GL 过账（收付款）`

---

## Task C2: expense.generate_payment_journal 追加 GL 过账

**Files:** Modify `abt-core/src/fms/expense/implt.rs`（generate_payment_journal，约 227-371 行）

**Interfaces:**
- Consumes: C1 同款 `post_from_source` + `resolve` + 工厂；`ExpenseReimbursementRepo`、`ExpenseReimbursementItemRepo`（取明细 cost_center）
- Produces: generate_payment_journal 现在会为报销付款同事务生成一张 posted GL 凭证（借 default_expense / 贷 default_bank，source_type=DocumentType::ExpenseReimbursement, source_id=expense.id）

**注意**：generate_payment_journal 现有逻辑——开 `self.pool.begin()` 独立事务 → 建 CJ → 直接置 Confirmed → expense 转 Paid → `tx.commit()`。过账必须**在 commit 之前、CJ 建好之后**调用，用同一 `&mut tx` 作 PgExecutor，保证同事务。

- [ ] **Step 1: 在 tx.commit() 之前追加过账**

在 generate_payment_journal 内、`ExpenseReimbursementRepo::update_status(..., ExpenseStatus::Paid, ...)` 之后、`tx.commit()` 之前，追加：

```rust
// 业财一体：报销付款过账（借费用 / 贷银行）
self.post_expense_gl(ctx, &mut *tx, &expense).await?;
```

> 注意 `ctx` 在原签名是 `_ctx`（未使用）。追加过账后 ctx 被使用，去掉下划线前缀改为 `ctx`。

- [ ] **Step 2: 实现 post_expense_gl helper**

```rust
/// 报销付款过账：借 default_expense / 贷 default_bank，金额 = expense.total_amount。
/// 辅助核算取第一条报销明细的 cost_center/profit_center。
async fn post_expense_gl(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    expense: &ExpenseReimbursement,
) -> Result<()> {
    let map = new_gl_mapping_service(self.pool.clone());
    let expense_acct = map.resolve(ctx, db, "default_expense", None).await?;
    let bank_acct = map.resolve(ctx, db, "default_bank", None).await?;

    let items = ExpenseReimbursementItemRepo::find_by_reimbursement(db, expense.id).await?;
    let (cc, pc) = items.first()
        .map(|i| (i.cost_center, i.profit_center))
        .unwrap_or((None, None));

    let amt = expense.total_amount;
    let gl_lines = vec![
        GlEntryLineInput {
            account_id: expense_acct, debit: amt, credit: Decimal::ZERO,
            cost_center: cc, profit_center: pc, project_id: None,
            memo: format!("报销付款 {}", expense.doc_number),
        },
        GlEntryLineInput {
            account_id: bank_acct, debit: Decimal::ZERO, credit: amt,
            cost_center: cc, profit_center: pc, project_id: None,
            memo: format!("报销付款 {}", expense.doc_number),
        },
    ];

    new_gl_entry_service(self.pool.clone())
        .post_from_source(
            ctx, db,
            DocumentType::ExpenseReimbursement, expense.id,
            expense.expense_date,
            format!("报销付款 {}", expense.doc_number),
            gl_lines,
        )
        .await?;
    Ok(())
}
```

> 实现者：`ExpenseReimbursementItemRepo` 的查询方法名 + `ExpenseReimbursement` 字段（`total_amount`/`expense_date`/`doc_number`）以 LSP/model.rs 为准。`post_from_source` 接受 `&mut *tx`（解引用 PgConnection 满足 PgExecutor）。

- [ ] **Step 3: clippy + commit**

`cargo clippy -p abt-core`。commit: `feat(fms): 报销付款追加业财一体 GL 过账（费用/银行）`

---

## Task C3: 衔接 e2e + fms/gl 回归

**Files:** Create `abt-web/tests/gl_integration_flow_e2e.rs`；Modify（若需）`docs/uml-design/08-gl.html`

**Interfaces:** Consumes C1/C2 + Plan A/B GL

- [ ] **Step 1: 前置——确认 2026-06 期间 open**

`psql` 查 `accounting_periods WHERE name='2026-06'` 存在且 status=open（1）。若不存在，fms k1/k2 回归会因 `PeriodClosed` 失败（这是 C1/C2 引入的副作用）。056 种子应已覆盖，本步骤是断言而非修复。

- [ ] **Step 2: 写 e2e（参照 gl_flow_e2e / fms_flow_e2e 模式）**

```rust
//! 业财衔接 e2e：收付款过账 + 报销付款过账 → 验证 GL 凭证 + 科目余额
mod common; use common::TestApp;
use serial_test::serial;
use rust_decimal::Decimal;
use abt_core::shared::types::ServiceContext;
use abt_core::fms::cash_journal::{model::*, CashJournalService};
use abt_core::fms::expense::{model::*, ExpenseService};
use abt_core::fms::enums::{JournalType, CashDirection};
use abt_core::gl::entry::GlEntryService;
use abt_core::gl::mapping::GlMappingService;

#[tokio::test]
#[serial]
async fn k1_sales_receipt_confirm_posts_gl() {
    // 建 SalesReceipt CJ → confirm → 验证 default_bank 借 +1000、default_ar 贷 +1000
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let cj = app.state.cash_journal_service();
    let entry = app.state.gl_entry_service();
    let map = app.state.gl_mapping_service();

    let bank_before = entry.get_account_balance(&ctx, &mut conn,
        map.resolve(&ctx, &mut conn, "default_bank", None).await.unwrap(), None, None).await.unwrap();
    let ar_before = entry.get_account_balance(&ctx, &mut conn,
        map.resolve(&ctx, &mut conn, "default_ar", None).await.unwrap(), None, None).await.unwrap();

    // 建 SalesReceipt CJ（借银行 1000 / 贷应收 1000，明细平衡）
    let id = cj.create(&ctx, &mut conn, CreateCashJournalReq {
        /* journal_type: SalesReceipt, direction: Inflow, amount: 1000,
           counterparty: 客户, source: None, bank_account, transaction_date: today,
           lines: [借银行1000, 贷应收1000] —— 字段以 fms model.rs CreateCashJournalReq 为准 */
        ..以 LSP/model.rs 实际签名为准填
    }).await.unwrap();
    cj.confirm(&ctx, &mut conn, id, None).await.unwrap();

    let bank_after = entry.get_account_balance(&ctx, &mut conn,
        map.resolve(&ctx, &mut conn, "default_bank", None).await.unwrap(), None, None).await.unwrap();
    let ar_after = entry.get_account_balance(&ctx, &mut conn,
        map.resolve(&ctx, &mut conn, "default_ar", None).await.unwrap(), None, None).await.unwrap();
    assert_eq!(bank_after - bank_before, Decimal::from(1000));
    assert_eq!(ar_after - ar_before, Decimal::from(1000));
}

#[tokio::test]
#[serial]
async fn k2_purchase_payment_confirm_posts_gl() {
    // 建 PurchasePayment CJ → confirm → 验证 default_ap 借 +X、default_bank 贷 +X
}

#[tokio::test]
#[serial]
async fn k3_expense_payment_posts_gl() {
    // expense submit → approve → generate_payment_journal → 验证 default_expense 借 +X、default_bank 贷 +X
}
```

> 实现者：`CreateCashJournalReq` / 报销建单签名以 fms model.rs + fms_flow_e2e.rs 现有写法为准（**抄 fms_flow_e2e k1/k2 的建单代码**，不要凭空写）。金额用 `Decimal::from(N)`。e2e 用例取金额要与明细平衡（CJ confirm 会校验借贷平衡）。客户/供应商/员工 id 用 fms_flow_e2e 同款 dev 库固定 id。

- [ ] **Step 3: 跑新 e2e + fms/gl 全回归（串行）**

`cargo test -p abt-web --test gl_integration_flow_e2e --test fms_flow_e2e --test gl_flow_e2e --test gl_invoice_flow_e2e -- --test-threads=1`

期望：gl_integration 3 + fms 4 + gl_flow 6 + gl_invoice 3 = **16 passed**。

**若 fms k1/k2 因 PeriodClosed 失败**：确认 2026-06 期间 open（Step 1），若种子缺失则补 migration seed（但 056 应已种）。

- [ ] **Step 4: 更新 docs/uml-design/08-gl.html** 衔接段落（注明 cash_journal.confirm / expense.generate_payment_journal 现过账到 GL）。clippy + commit `test(gl): 业财衔接 e2e（收付款/报销过账）+ 08-gl 衔接文档`

---

## 完成验证（Plan C 收尾）

- [ ] `cargo clippy --workspace --tests` 无新错
- [ ] gl_integration_flow_e2e 3 + fms 4 + gl_flow 6 + gl_invoice 3 = 16 串行全绿
- [ ] 收付款 confirm / 报销付款真正生成 GL 凭证 + 科目余额正确（业财一体闭环贯通）

## Plan C 产出

- cash_journal.confirm 业财一体过账（收付款 → GL）
- expense.generate_payment_journal 业财一体过账（报销付款 → GL）
- gl_integration_flow_e2e（衔接验证）
- fms 现金出纳与 GL 总账贯通，业财一体闭环完整（发票/收付款/报销付款均自动过账）
