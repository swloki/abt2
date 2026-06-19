# GL 前端 Implementation Plan（Plan D · 财务 roadmap 第四期）

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** GL 总账最小前端——科目表/凭证/销售发票/采购发票/试算平衡表/期间管理 6 类页面，可点击走通 Draft→Posted 并看到 GL 凭证。

**Architecture:** 纯前端。复用 Plan A/B/C 的 GL service（全部已就绪：`GlAccountService`/`GlEntryService`/`GlPeriodService`/`GlMappingService`/`SalesInvoiceService`/`PurchaseInvoiceService`，state.rs 访问器已加）。遵循 abt-web 既有 fms 页面模式——**抄 `fms_journal_list/detail/create.rs` + `fms_expense_*` 改**。新增权限域 `GL`、导航模块、`routes/gl.rs` 路由文件。

**Tech Stack:** Axum + Maud + HTMX + Hyperscript + UnoCSS（100% 原子化）；TypedPath；`#[require_permission("GL","...")]`

**Spec:** `docs/superpowers/specs/2026-06-20-gl-invoice-design.md`（第 10 节前端范围）

## Global Constraints

（同 Plan A/B/C）中文沟通；conventional commit + Co-Authored-By；`cargo clippy` 验证；**改 abt-web 前必须读 `abt-web/CLAUDE.md`**（组件化三原则、抗碎片化、Maud 2024 陷阱、UnoCSS 原子化、禁 sqlx 直访 DB、TypedPath、单端点列表、`hx-target="this"`、禁 `style` 内联）。

**Plan A/B/C 已就绪**（commits a03799c3..3a19d23a）：GL service 全部就绪 + state.rs 访问器（`gl_account_service`/`gl_entry_service`/`gl_period_service`/`gl_mapping_service`/`sales_invoice_service`/`purchase_invoice_service`）。16 e2e 串行全绿。

---

## 设计决策（最小前端）

- **抄既有 fms 模式**：每个 GL 页面 = 复制对应 fms 页面 + 改 service/path/字段。fms 模式已被验证（列表单端点 + 三控件、详情状态流转按钮、创建表单 + HX-Redirect）。
- **不做删除**：科目/发票/凭证本期不做删除页（软删除留后续）。科目表做「停用」（`disabled` 字段）替代删除。
- **科目表不做树形拖拽**：本期平铺列表 + parent_id 下拉选父科目（树形可视化/拖拽排序留后续，YAGNI）。
- **试算平衡表 + 期间管理**：只读 + 期间开关按钮，不做复杂报表钻取。
- **权限域 GL**：read/create/update。过账用 update（post 归入 update 权限）。
- **导航**：新增 `gl` NavModule「总账管理」。

---

## File Structure

| 文件 | 责任 | 动作 |
|---|---|---|
| `abt-web/src/routes/gl.rs` | GL 路由 TypedPath + router() | 新建 |
| `abt-web/src/routes/mod.rs` | `.merge(gl::router())` | 改 |
| `abt-web/src/layout/sidebar.rs` | 加 `gl` NavModule | 改 |
| `abt-web/src/pages/gl_account_list.rs` / `_create.rs` | 科目表列表/创建（含停用） | 新建 |
| `abt-web/src/pages/gl_entry_list.rs` / `_detail.rs` | 凭证列表/详情（含分录行） | 新建 |
| `abt-web/src/pages/sales_invoice_list.rs` / `_create.rs` / `_detail.rs` | 销售发票（含 post/cancel） | 新建 |
| `abt-web/src/pages/purchase_invoice_list.rs` / `_create.rs` / `_detail.rs` | 采购发票 | 新建 |
| `abt-web/src/pages/gl_trial_balance.rs` | 试算平衡表（按期间） | 新建 |
| `abt-web/src/pages/gl_period_list.rs` | 期间管理（开/关） | 新建 |
| `abt-core/migrations/058_gl_permissions_seed.sql` | （若缺）GL 权限 seed 给 admin | 视情况新建 |
| `docs/uml-design/08-gl.html` | 补前端页面段落 | 改 |

---

## Task D1: 基建（路由文件 + 导航 + 权限 + 科目表页）

**Files:** Create `abt-web/src/routes/gl.rs`, `abt-web/src/pages/gl_account_list.rs`, `abt-web/src/pages/gl_account_create.rs`; Modify `abt-web/src/routes/mod.rs`, `abt-web/src/layout/sidebar.rs`, `abt-web/src/pages/mod.rs`; 视情况 Create `058_gl_permissions_seed.sql`

**Interfaces:**
- Consumes: `state.gl_account_service()`（`GlAccountService::{list(ctx,db,filter,PageParams) -> PagedResult<GlAccount>; get; create(ctx,db,CreateGlAccountReq); update}`——签名以 LSP/service.rs 为准）
- Produces: `routes/gl.rs`（TypedPath + router）；`gl` NavModule；GL 权限 seed；科目表列表页（含停用切换）+ 创建页

- [ ] **Step 1: 权限 seed 验证** —— `psql` 查 `role_permissions WHERE resource='GL'`。若无 admin 的 GL 权限，建 `058_gl_permissions_seed.sql` 给 admin role 插入 GL read/create/update（参照既有 FMS 权限 seed 的格式——用 LSP/grep 找 FMS 权限 seed 文件）。应用 + 验证。

- [ ] **Step 2: routes/gl.rs 骨架** —— 抄 `routes/fms.rs` 结构。定义 GL TypedPath（先科目表 + 凭证 + 试算 + 期间，发票在 D3/D4 补）：

```rust
use axum::Router;
use axum_extra::routing::TypedPath;
use axum::routing::{get, post};
use serde::Deserialize;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/accounts")]
pub struct GlAccountListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/accounts/create")]
pub struct GlAccountCreatePath;

// 凭证
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/entries")]
pub struct GlEntryListPath;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/entries/{id}")]
pub struct GlEntryDetailPath { pub id: i64 }

// 试算 / 期间
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/trial-balance")]
pub struct GlTrialBalancePath;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/periods")]
pub struct GlPeriodListPath;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/periods/{id}/close")]
pub struct GlPeriodClosePath { pub id: i64 }

pub fn router() -> Router<AppState> {
    Router::new()
        .route(GlAccountListPath::PATH, get(crate::pages::gl_account_list::get_list))
        .route(GlAccountCreatePath::PATH,
            get(crate::pages::gl_account_create::get_create)
            .post(crate::pages::gl_account_create::create))
        // 其余 route 在后续 task 补
}
```
> **注意 abt-web/CLAUDE.md 陷阱**：`Serialize` 与 `TypedPath` derive 冲突——TypedPath struct **不要** derive Serialize（只 `TypedPath, Deserialize, Clone`）。

- [ ] **Step 3: mod.rs 注册** —— `routes/mod.rs` 加 `mod gl;` + `.merge(gl::router())`（在 fms::router() 后）。`pages/mod.rs` 加 `pub mod gl_account_list; pub mod gl_account_create;`（后续 task 逐个补）。

- [ ] **Step 4: sidebar 加 gl NavModule** —— 抄 `sidebar.rs` 现有 `NavModule`（finance 块），新增 `gl` 模块（科目表/凭证/销售发票/采购发票/试算平衡表/期间管理 6 项，permission `Some(("GL","read"))`）。NavIcon 用现有枚举值（File/ClipboardDoc/Check/Calendar 等，LSP 查 NavIcon 变体）。

- [ ] **Step 5: 科目表列表页** `gl_account_list.rs` —— 抄 `fms_journal_list.rs`：单端点 `get_list`（`#[require_permission("GL","read")]`）+ QueryParams（code/name/account_type/disabled 筛选 + page）+ status_tabs（按 account_type 或 disabled 分 tab）+ filter_form + pagination + data-card 表格（code/name/account_type/balance_direction/disabled 列）。`disabled` 列加切换按钮（hx-post 到停用端点——若 GlAccountService 无 toggle，用 update 改 disabled；端点可在列表页内或 detail，本期简化：列表行内 hx-post toggle）。**用 LSP 查 GlAccountService 的 list/get/update 签名**，禁猜。

- [ ] **Step 6: 科目表创建页** `gl_account_create.rs` —— 抄 `fms_journal_create.rs`：`get_create`（渲染表单）+ `create`（`#[require_permission("GL","create")]`，Form<GlAccountCreateForm> → `CreateGlAccountReq` → svc.create → HX-Redirect 列表）。表单字段：code/name/account_type(select 1-6)/parent_id(select 现有科目)/balance_direction(借/贷)/is_detail(checkbox)/currency(默认 CNY)。form-section + form-field shortcut。

- [ ] **Step 7: clippy + 手动验证 + commit**
`cargo clippy -p abt-web`。手动：浏览器（用户已开 Chrome 9222，用 agent-browser --cdp 9222 + snapshot 验证科目表页可访问 + 创建一个科目——但**禁止截图**，用 snapshot -i 无障碍树 + get text @eN 验证）。commit: `feat(gl-web): GL 路由基建 + 导航 + 科目表页`

---

## Task D2: 凭证列表/详情

**Files:** Create `abt-web/src/pages/gl_entry_list.rs`, `gl_entry_detail.rs`; Modify `routes/gl.rs`(加 entry list/detail route), `pages/mod.rs`

**Interfaces:**
- Consumes: `state.gl_entry_service()`（`GlEntryService::{list(ctx,db,filter,PageParams); get(ctx,db,id) -> (GlEntry, Vec<GlEntryLine>)}`——签名以 LSP 为准）

- [ ] **Step 1: 列表页** `gl_entry_list.rs` —— 抄 fms_journal_list。QueryParams：period/source_type/status/doc_number 筛选 + page。data-card 表格：doc_number/entry_date/period/voucher_type/source_type/status(Draft/Posted/Cancelled 标签)/total_debit。status_tabs 按 status（全部/Draft/Posted/Cancelled）。

- [ ] **Step 2: 详情页** `gl_entry_detail.rs` —— 抄 fms_journal_detail。`#[require_permission("GL","read")]`。头部信息（doc_number/period/entry_date/status/total）+ 分录行表格（account.code/account.name/debit/credit/memo）+ 借贷合计行（自检平衡）。**用 LSP 查 GlEntry/GlEntryLine 字段 + account 关联怎么取**（get 返回的 line 是否含 account_code/name，或需额外查 GlAccountService）。

- [ ] **Step 3: routes 补 entry 路由 + mod.rs + clippy + commit** `feat(gl-web): 凭证列表/详情页`

---

## Task D3: 销售发票（列表/创建/详情 + post/cancel）

**Files:** Create `abt-web/src/pages/sales_invoice_list.rs`, `_create.rs`, `_detail.rs`; Modify `routes/gl.rs`(SalesInvoice TypedPath: list/create/detail/post/cancel), `pages/mod.rs`

**Interfaces:**
- Consumes: `state.sales_invoice_service()`（`SalesInvoiceService::{create/post/cancel/get/list}`）；`state.customer_service()` 或 master_data（取客户列表填充创建表单客户下拉）；`state.product_service()`（产品下拉）

- [ ] **Step 1: routes** —— 加 SalesInvoiceListPath/CreatePath/DetailPath/PostPath/CancelPath（`/admin/gl/sales-invoices[/create]`/`/{id}[/post|/cancel]`），注册到 router。

- [ ] **Step 2: 列表页** —— 抄 fms。QueryParams：customer_id/status/period/issue_date + page。表格：doc_number/issue_date/customer/total/status。status_tabs（Draft/Posted/Cancelled）。

- [ ] **Step 3: 创建页** —— 抄 `fms_expense_create.rs`（行项目 + items_json 桥接模式）。头：customer_id(select)/issue_date(date)。行项目：product_id(select)/qty/unit_price，用 `lineItemCalc('#sales-invoice-tbody')` 算 line_subtotal/total（抄报价单/销售单既有行项目计算器）。隐藏 items_json 提交。`#[require_permission("GL","create")]` → svc.create(CreateSalesInvoiceReq) → HX-Redirect 列表。**CreateSalesInvoiceReq 签名以 sales_invoice/model.rs LSP 为准**。

- [ ] **Step 4: 详情页 + post/cancel 按钮** —— 抄 fms_expense_detail 状态流转。头部：doc_number/customer/issue_date/subtotal/tax/total/status。行项目表。状态按钮：
  - Draft → 「过账」hx-post=PostPath（`#[require_permission("GL","update")]` → svc.post → HX-Redirect detail）
  - Posted → 「取消」hx-post=CancelPath（→ svc.cancel）
  - 详情显示关联 gl_entry_id（若 posted，链接到 GlEntryDetailPath）。

- [ ] **Step 5: clippy + 手动验证（建销售发票→post→看 GL 凭证）+ commit** `feat(gl-web): 销售发票（列表/创建/详情 + 过账/取消）`

---

## Task D4: 采购发票（列表/创建/详情 + post/cancel）

**Files:** Create `purchase_invoice_list.rs`, `_create.rs`, `_detail.rs`; Modify `routes/gl.rs`(PurchaseInvoice TypedPath), `pages/mod.rs`

**Interfaces:** 对称 D3，`state.purchase_invoice_service()`，supplier_id 替代 customer_id（`state.supplier_service()`）。AP 规则（库存/进项税/应付）由 svc.post 内部处理，前端只调 post。

- [ ] **Step 1-5**：抄 D3 结构，supplier/product 下拉。CreatePurchaseInvoiceReq 以 purchase_invoice/model.rs LSP 为准。clippy + 手动验证（建采购发票→post→看 GL）+ commit `feat(gl-web): 采购发票（列表/创建/详情 + 过账/取消）`

---

## Task D5: 试算平衡表 + 期间管理

**Files:** Create `gl_trial_balance.rs`, `gl_period_list.rs`; Modify `routes/gl.rs`(trial-balance route 已在 D1 定义；period list/close route 已在 D1 定义), `pages/mod.rs`

**Interfaces:**
- Consumes: `state.gl_entry_service().trial_balance(ctx, db, period) -> Vec<TrialBalanceRow>`；`state.gl_period_service().{list(ctx,db,PageParams); close(ctx,db,id)}`（签名以 LSP 为准）

- [ ] **Step 1: 试算平衡表** `gl_trial_balance.rs` —— 顶部期间选择（select 当前 open 期间，hx-get 切换）→ 表格：account.code/name/期初余额/本期借/本期贷/期末余额 → 底部合计行（Σ借==Σ贷 自检，不平衡标红）。**TrialBalanceRow 字段以 LSP 为准**。

- [ ] **Step 2: 期间管理** `gl_period_list.rs` —— 列表：name/start_date/end_date/status(open/closed)/fiscal_year。open 期间行加「关闭」按钮（hx-post=GlPeriodClosePath → svc.close → HX-Redirect）。closed 不可再开（本期单向）。`#[require_permission("GL","update")]` 关闭。

- [ ] **Step 3: clippy + 手动验证（看试算平衡 + 关闭一个期间）+ commit** `feat(gl-web): 试算平衡表 + 期间管理`

---

## 完成验证（Plan D 收尾）

- [ ] `cargo clippy --workspace --tests` 无新错
- [ ] 浏览器手动走通：建科目 → 建销售发票 → post → 看凭证 + 试算平衡变化；建采购发票 → post；关闭期间后该期 post 报错（snapshot -i 验证，禁截图）
- [ ] GL 导航模块 6 个菜单项可访问
- [ ] 更新 docs/uml-design/08-gl.html 补前端段落

## Plan D 产出

- GL 前端 6 类页面（科目表/凭证/销售发票/采购发票/试算平衡表/期间管理）
- routes/gl.rs + gl NavModule + GL 权限域
- 业财一体从后端贯通到 UI（可点击走通 Draft→Posted + 看到 GL 凭证）
