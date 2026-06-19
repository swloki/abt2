# FMS 财务块全链路打通设计

**日期**：2026-06-19
**主题**：修复 fms 四子模块链路断点，使财务块端到端可走通，并补 e2e 测试与前端状态流转按钮。

---

## 1. 背景与诊断

目标：验证 fms（cash_journal / write_off / expense / cost_accounting）能否走完。经代码与 dev 库核查，发现**所有写入链路目前在代码层面跑不通**，根因是状态机未注册 + expense 缺状态推进接口。

### 1.1 现状矩阵

| 链路 | 现状 | 根因 |
|---|---|---|
| `expense.create` | 硬失败 | 调 `transition("ExpenseStatus", id, "Draft")`，状态机表无定义 → `InvalidStateTransition`（用 `?` 传播，非 `.ok()` 吞掉）|
| `cash_journal.create` | 硬失败 | 同上，无 JournalStatus 定义 |
| `cash_journal.confirm` | 硬失败 | `transition("Confirmed")` 无定义 |
| `expense` Draft→Approved | 无路径 | trait 无 submit/approve；`transition` 不更新业务表 status 列 |
| `generate_payment_journal` | 前置不满足 | 直接走 Repo（不经状态机），但要求 `expense.status == Approved` |
| `write_off` | 代码完整 | 直接走 Repo，不依赖状态机 |
| `cost_accounting` | 正常（只读）| 纯查询 |

### 1.2 设计文档 vs 现实的偏差

`docs/uml-design/07-fms.html` 写明"submit/approve 由 StateMachineService.transition() 驱动"。但现实存在两处偏差：

1. **状态机表未 seed** `ExpenseStatus` / `JournalStatus`（dev 库 `state_definitions` 仅有 Purchase/MiscellaneousRequest/PaymentRequest 系列）。
2. **`transition` 不更新业务表 status 列**：`StateMachineService::transition` 只做"校验转换规则 + 写 `entity_state_logs` + 触发 side_effects"，而 `SideEffect::UpdateField` 是空实现（`{} => {}`）。因此光靠 `transition` 永远无法让 `expense_reimbursements.status` 变成 Approved。

对比 `cash_journal.confirm` 的既有正确模式：`transition("Confirmed")` ＋ 显式 `CashJournalRepo::update_status` **双写**。expense 必须沿用同一模式。

> 本设计据此修正设计文档，使"设计 ↔ 代码"双向同步。

---

## 2. 方案总览

四块改动，按依赖顺序：

1. **migration 055**：seed `ExpenseStatus` + `JournalStatus` 状态机定义（数据补全，无逻辑）
2. **abt-core**：`ExpenseReimbursementService` 补 `submit` / `approve`（双写，仿 `cash_journal.confirm`）
3. **abt-web**：expense / journal 详情页补状态流转按钮 + POST handler
4. **e2e 测试** + **文档同步**

`pay` 不新增接口——`generate_payment_journal` 本身即是完整的 pay（校验 Approved → 建 CashJournal → expense→Paid → 发 `ExpensePaymentGenerated` 事件），由前端「付款」按钮直接调用。

---

## 3. 详细设计

### 3.1 migration `055_fms_state_transitions.sql`

参照 `024_bom_state_transitions.sql` 格式。含初始转换 `('','X')`（新实体 from_state 为空字符串）。

**state_definitions**：

| entity_type | state_name | label | is_initial | is_final |
|---|---|---|---|---|
| JournalStatus | Draft | 草稿 | TRUE | FALSE |
| JournalStatus | Confirmed | 已确认 | FALSE | FALSE |
| JournalStatus | Cancelled | 已取消 | FALSE | TRUE |
| ExpenseStatus | Draft | 草稿 | TRUE | FALSE |
| ExpenseStatus | Submitted | 已提交 | FALSE | FALSE |
| ExpenseStatus | Approved | 已审批 | FALSE | FALSE |
| ExpenseStatus | Paid | 已付款 | FALSE | TRUE |
| ExpenseStatus | Cancelled | 已取消 | FALSE | TRUE |

**state_transition_defs**（`side_effects` 默认 `'[]'`）：

| entity_type | from | to |
|---|---|---|
| JournalStatus | `` | Draft |
| JournalStatus | Draft | Confirmed |
| JournalStatus | Draft | Cancelled |
| ExpenseStatus | `` | Draft |
| ExpenseStatus | Draft | Submitted |
| ExpenseStatus | Submitted | Approved |
| ExpenseStatus | Submitted | Cancelled |
| ExpenseStatus | Approved | Cancelled |

> `Approved→Paid` **不进状态机表**：由 `generate_payment_journal` 内部直接 `ExpenseReimbursementRepo::update_status` 完成（与现有代码一致，避免与"付款即建账"的复合操作割裂）。
>
> **关于 cancel 转换**：`Submitted→Cancelled` / `Approved→Cancelled` 本次 seed 为状态机完整性一次性预置，但**前端 cancel 按钮本次不实现**（YAGNI，非链路必需）。未来加 cancel 按钮时无需再补 migration。

**存量数据**：dev 库现有 expense / cash_journal（若有）无 `entity_state_logs` 记录。本次**不做 backfill**——测试只新建数据。若后续需处理存量，另立 migration（仿 024 的 backfill 段）。

### 3.2 abt-core：`ExpenseReimbursementService` 补接口

**trait 新增**（`abt-core/src/fms/expense/service.rs`）：

```rust
/// Draft → Submitted（提交审批）
async fn submit(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

/// Submitted → Approved（审批通过）
async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
```

**implt 实现**（`abt-core/src/fms/expense/implt.rs`），两者结构相同，严格仿 `cash_journal.confirm`：

1. `ExpenseReimbursementRepo::get_by_id` 取实体 + version
2. 前置状态校验：`submit` 要求 `Draft`，`approve` 要求 `Submitted`（不符报 `business_rule("InvalidState")`）
3. `new_state_machine_service().transition(ctx, db, "ExpenseStatus", id, "Submitted"/"Approved", None)` — 校验转换合法性 + 写 state_logs
4. `ExpenseReimbursementRepo::update_status(id, new_status, version)` — 更新业务表（乐观锁，rows==0 报 `ConcurrentConflict`）
5. `new_audit_log_service().record(...)` — 审计（`AuditAction::Transition`，changes 记 from/to）

事务模式：`InCallerTx`（与 `create` 一致，不开独立 tx）。

> 不发领域事件（submit/approve 是中间态流转，无下游消费者；`generate_payment_journal` 已发 `ExpensePaymentGenerated`）。
> 不引入幂等键（与 create 一致；idempotency 仅在 confirm/pay 这类易重放的复合操作上）。

### 3.3 abt-web：状态流转按钮 + handler

新增 TypedPath（`abt-web/src/routes/fms.rs`）：

```rust
#[typed_path("/admin/fms/expenses/{id}/submit")]
pub struct ExpenseSubmitPath { pub id: i64 }

#[typed_path("/admin/fms/expenses/{id}/approve")]
pub struct ExpenseApprovePath { pub id: i64 }

#[typed_path("/admin/fms/expenses/{id}/pay")]
pub struct ExpensePayPath { pub id: i64 }

#[typed_path("/admin/fms/journals/{id}/confirm")]
pub struct JournalConfirmPath { pub id: i64 }
```

| 页面 | 按钮（按 status 条件渲染） | handler 行为 |
|---|---|---|
| `fms_expense_detail` | Draft→「提交审批」；Submitted→「审批通过」；Approved→「付款」 | 分别调 `expense_service().submit/approve/generate_payment_journal`，成功后重渲染详情片段 |
| `fms_journal_detail` | Draft→「确认」 | 调 `cash_journal_service().confirm(id, None)` |

遵循 abt-web 约束：`hx-post` + `hx-target="this"` + `hx-swap="outerHTML"` 自包含刷新；`TypedPath`；不直接访问 DB（全走 service）。handler 成功返回详情片段（或 `HX-Redirect` 回详情页），失败由现有 `DomainError→HTTP` 映射 + toast。

> 按钮落点：本次仅在 **detail 页**。list 页不加快捷按钮（YAGNI，详情页是状态流转的自然位置）。

### 3.4 e2e 测试 `abt-web/tests/fms_flow_e2e.rs`

参照 `sales_to_wms_e2e.rs`：`mod common; use common::TestApp;` + `app.state.xxx_service()` + `ServiceContext::new(1)` + `app.state.pool.acquire()`。

| 用例 | 链路 | 断言 |
|---|---|---|
| **k1 报销付款** | `expense.create` → `submit` → `approve` → `generate_payment_journal` | 返回 journal_id；`cash_journal.get` 为 Outflow/Expense/Confirmed、金额=expense.total；`expense.get` status=Paid；当期 `get_balance` outflow 含本次 |
| **k2 收款核销** | `cash_journal.create`(SalesReceipt/Inflow, source=SalesOrder, lines 借贷平衡) → `confirm` → `write_off.write_off` | `write_off.get_unreconciled_amount` = source_total − amount；`list_by_source` 命中本核销 |
| **k3 过度核销防护** | 在 k2 基础上再 write_off 超额 | 第二次 `write_off` 返回 `Err(OverWriteOff)` |
| **k4 成本核算只读** | `cost_accounting.get_product_cost` / `list_product_costs` / `get_margin_analysis` | 各返回 `Ok`（结果可空） |

数据隔离：source_id 用新生成的 expense/journal id；不依赖 dev 库特定存量（避免 stale 污染，吸取上次 session 教训）。

### 3.5 文档同步 `docs/uml-design/07-fms.html`

修正两处：
1. "submit/approve 由 StateMachineService 驱动" → **"transition 校验+记日志 ＋ Repo::update_status 双写"**（注明 `SideEffect::UpdateField` 未实现，业务表 status 由 service 显式更新）。
2. 补 seed 状态机定义说明（`055` migration）+ `Approved→Paid` 由 `generate_payment_journal` 显式完成、本次以 `pay` 端点暴露。

---

## 4. 验证标准

- `cargo clippy`（abt-core + abt-web）零警告
- `cargo test -p abt-web --test fms_flow_e2e` 四用例全绿
- 手动：前端 detail 页按钮按 status 正确出现、点击后状态流转、toast 反馈

## 5. 非目标（YAGNI）

- 不为存量 expense/cash_journal backfill state_logs
- 不引入 workflow 引擎触发付款（pay 由显式按钮/接口调用）
- list 页不加快捷状态按钮
- write_off / cost_analysis 不加前端按钮（已通/只读）

## 6. 风险

- **状态机 seed 漏转换**：若遗漏某条 `from→to`，运行时 `InvalidStateTransition`。缓解：测试 k1/k2 覆盖 Draft→Submitted→Approved→Paid 与 Draft→Confirmed 全路径。
- **双写一致性**：transition 成功但 update_status 因并发失败（rows==0）会留下 state_log 与业务表不一致。缓解：`?` 传播错误，事务内（InCallerTx）由调用方回滚；测试 k1 含并发路径外的正常流。
