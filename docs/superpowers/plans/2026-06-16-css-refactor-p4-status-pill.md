# P4: Status Pill 原子化迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 status-pill 及 40+ 颜色变体从 base.css 迁移到 100% 原子 UnoCSS class，统一使用 `before:` 伪元素原子模式 + 颜色映射表。

**Architecture:** Status pill 是最复杂的批次——55 个 class 散落在 base.css 的 6 处定义中，且存在多组重复定义（base.css 行 606-625、1155-1158、1295-1298、2619-2628、2734-2737、3642-3657）。原子化后统一为 `before:content-[''] before:w-1.5 before:h-1.5 before:rounded-full before:bg-*` 模式。颜色语义归并为 6 组：muted、accent、warn、success、danger、suspended。同时处理 status-tabs（状态切换标签栏）和 status-flow（状态流程条）两个相关组件。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Rust + Maud HTML 宏

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

## 颜色映射表（所有 Status Pill 变体 → 原子 class 后缀）

所有变体共享基础 class：`inline-flex items-center gap-1.5 px-3 py-0.5 rounded-full text-xs font-medium whitespace-nowrap before:content-[''] before:w-1.5 before:h-1.5 before:rounded-full before:shrink-0`

颜色后缀（根据语义分组，文字色 + 伪元素背景色 + 容器背景色）：

| 语义组 | 包含的变体 | 原子后缀 |
|---|---|---|
| **muted** | draft, neutral, inactive(class→replaced) | `bg-surface text-muted before:bg-muted` |
| **accent** | info, confirmed, sent, submitted, partial, picking, received, settled, planned | `bg-accent/8 text-accent-active before:bg-accent-active` |
| **warn** | accepted, progress, inspecting | `bg-warn/8 text-warn before:bg-warn` |
| **success** | completed, shipped, active, success, full, bom-published | `bg-success/8 text-success before:bg-success` |
| **danger** | rejected, cancelled, expired, danger, disputed, inactive, bom-draft | `bg-danger/8 text-danger before:bg-danger` |
| **suspended/defect** | suspended, defect | `bg-danger/8 text-danger before:bg-danger` (suspended) / `bg-[#fff2e8] text-[#fa8c16] before:bg-[#fa8c16]` (defect) |

> **注意 base.css 中的重复定义**：同一个 class（如 `.status-draft`）在 base.css 中出现 2-3 次，不同位置定义不同颜色值。本计划以最后一次出现（行 3642-3657）为准——这是最新版本，使用 `rgba()` 格式和 `var(--warn)`/`var(--success)` 等标准 token。

---

### Task 1: Maud 中创建 status_pill 辅助函数

**Files:**
- Modify: `abt-web/src/components/mod.rs`（如果该文件存在并 pub mod 各组件）或新建 `abt-web/src/components/status_pill.rs`

- [ ] **Step 1: 创建 status_pill.rs 组件文件**

创建 `abt-web/src/components/status_pill.rs`：

```rust
use maud::{html, Markup, PreEscaped};

/// Status pill 颜色语义组。
///
/// 所有 status pill 变体归并为 6 个语义色：
/// - Muted: 草稿/中性/停用
/// - Accent: 信息/确认/已发送/已提交/部分/拣货/收货/结算/已计划
/// - Warn: 已接受/进行中/质检中
/// - Success: 已完成/已发货/活跃/成功/已发布BOM
/// - Danger: 已拒绝/已取消/已过期/危险/争议/停用
pub enum StatusColor {
    Muted,
    Accent,
    Warn,
    Success,
    Danger,
}

impl StatusColor {
    fn classes(&self) -> &'static str {
        match self {
            StatusColor::Muted => "bg-surface text-muted before:bg-muted",
            StatusColor::Accent => "bg-accent/8 text-accent-active before:bg-accent-active",
            StatusColor::Warn => "bg-warn/8 text-warn before:bg-warn",
            StatusColor::Success => "bg-success/8 text-success before:bg-success",
            StatusColor::Danger => "bg-danger/8 text-danger before:bg-danger",
        }
    }
}

/// 渲染标准 status pill（带圆点指示器）。
///
/// ```rust
/// status_pill(StatusColor::Success, "已完成")
/// ```
pub fn status_pill(color: StatusColor, label: &str) -> Markup {
    html! {
        span class={
            "inline-flex items-center gap-1.5 px-3 py-0.5 rounded-full text-xs font-medium "
            "whitespace-nowrap "
            "before:content-[''] before:w-1.5 before:h-1.5 before:rounded-full before:shrink-0 "
            (color.classes())
        } { (label) }
    }
}

/// 渲染紧凑型 status pill（更小字号，无圆点指示器）。
/// 用于表格内、卡片列表等空间受限场景。
///
/// ```rust
/// status_pill_compact(StatusColor::Warn, "待审批")
/// ```
pub fn status_pill_compact(color: StatusColor, label: &str) -> Markup {
    let bg_text = match color {
        StatusColor::Muted => "bg-surface text-muted",
        StatusColor::Accent => "bg-accent/8 text-accent-active",
        StatusColor::Warn => "bg-warn/8 text-warn",
        StatusColor::Success => "bg-success/8 text-success",
        StatusColor::Danger => "bg-danger/8 text-danger",
    };
    html! {
        span class={
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-semibold "
            (bg_text)
        } { (label) }
    }
}
```

- [ ] **Step 2: 在 mod.rs 中注册模块**

在 `abt-web/src/components/mod.rs` 中添加：

```rust
pub mod status_pill;
pub use status_pill::{status_pill, status_pill_compact, StatusColor};
```

- [ ] **Step 3: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error（新文件尚无调用者，编译通过）

---

### Task 2: 迁移 status-pill 基础 + 核心变体（base.css 行 605-625）

**Files:**
- Modify: `static/base.css:605-625`（删除 `.status-pill`、`.status-pill::before`、`.status-draft`、`.status-info`、`.status-accepted`、`.status-rejected`、`.status-progress`、`.status-completed`、`.status-shipped` 及其 `::before`）
- Modify: 所有引用这些 class 的 Maud 文件

base.css 原始定义（行 605-625）：

```css
/* ─── Status Pills ─── */
.status-pill {
  display: inline-flex; align-items: center; gap: 5px; padding: 3px 12px;
  border-radius: var(--radius-pill); font-size: 12px; font-weight: 500;
  line-height: 20px; white-space: nowrap; letter-spacing: 0.01em;
}
.status-pill::before { content: ''; width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.status-draft { background: var(--surface); color: var(--muted); }
.status-draft::before { background: var(--muted); }
.status-info { background: #e8f4ff; color: var(--accent-active); }
.status-info::before { background: var(--accent-active); }
.status-accepted { background: #fff8eb; color: #d46b08; }
.status-accepted::before { background: #d46b08; }
.status-rejected { background: #fff2f0; color: #cf1322; }
.status-rejected::before { background: #cf1322; }
.status-progress { background: #fff8eb; color: #d46b08; }
.status-progress::before { background: #d46b08; }
.status-completed { background: #f0fff0; color: #389e0d; }
.status-completed::before { background: #389e0d; }
.status-shipped { background: #e8f4ff; color: var(--accent-active); }
.status-shipped::before { background: var(--accent-active); }
```

- [ ] **Step 1: 从 base.css 删除行 605-625**

删除整个 `/* ─── Status Pills ─── */` 块（行 605 到 625，含注释行）。

- [ ] **Step 2: 迁移 bom_detail.rs 中的 status-pill**

文件 `abt-web/src/pages/bom_detail.rs`。

找到所有 `format!("status-pill {status_class}")` 调用（行 217 等），替换为使用辅助函数。

在文件顶部添加导入：
```rust
use crate::components::status_pill::{status_pill, StatusColor};
```

找到 `status_class` 的赋值处（通常在 match 语句中），改为返回 `StatusColor`：

```rust
// 原代码（示例，需根据实际变量名调整）：
// let status_class = match bom.status { ... "status-draft" ... };

// 替换为：
let status_color = match bom.status {
    BomStatus::Draft => StatusColor::Danger,      // bom-draft → danger 色
    BomStatus::Published => StatusColor::Success,   // bom-published → success 色
};
let status_label = match bom.status { ... };  // label 不变
```

在模板中将：
```rust
span class=(format!("status-pill {status_class}")) { (status_label) }
```
替换为：
```rust
(status_pill(status_color, &status_label))
```

- [ ] **Step 3: 迁移 bom_edit.rs 中的 status-pill**

文件 `abt-web/src/pages/bom_edit.rs`。

找到行 595 的 `span class=(format!("status-pill {status_class}"))`。

同样的模式：将 `status_class` 的 match 改为返回 `StatusColor`，然后替换模板渲染为 `(status_pill(status_color, &status_label))`。

- [ ] **Step 4: 迁移 bom_list.rs 中的 status-pill**

文件 `abt-web/src/pages/bom_list.rs`。

找到行 392 的 `span class=(format!("status-pill {status_class}"))`。同样替换。

- [ ] **Step 5: 迁移 category_list.rs 中的 status-pill**

文件 `abt-web/src/pages/category_list.rs`。

找到行 916-922 的三处 `status-pill status-*`，替换为辅助函数调用：

```rust
// 原：
// span class="status-pill status-success" { "在用" }
// span class="status-pill status-draft" { "停用" }
// span class="status-pill status-danger" { "淘汰" }

// 替换为：
ProductStatus::Active => { (status_pill(StatusColor::Success, "在用")) }
ProductStatus::Inactive => { (status_pill(StatusColor::Muted, "停用")) }
ProductStatus::Obsolete => { (status_pill(StatusColor::Danger, "淘汰")) }
```

- [ ] **Step 6: 迁移 dashboard.rs 中的 status-pill**

文件 `abt-web/src/pages/dashboard.rs`。

该文件有两个辅助函数 `todo_item` / `activity_item`（行 154、164、211、232），接收 `status_class` 字符串参数。修改这些函数签名接收 `StatusColor`：

```rust
// 原：
// fn todo_item(status_class: &str, status_text: &str, desc: &str, time: &str) -> Markup {
//     ...
//     span class={"status-pill " (status_class)} style="font-size:11px" { (status_text) }

// 替换为：
fn todo_item(color: StatusColor, status_text: &str, desc: &str, time: &str) -> Markup {
    html! {
        // ... 内部不变 ...
        (status_pill_compact(color, status_text))
        // ...
    }
}
```

同样修改 `activity_item`、`activity_item_last`、`todo_item_last`。

调用处也需修改（行 56-59、96-100），将字符串改为枚举：
```rust
// 原：todo_item("status-progress", "拣货中", ...)
// 新：(todo_item(StatusColor::Warn, "拣货中", ...))
```

- [ ] **Step 7: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 3: 迁移 customer_detail.rs 和 customer_list.rs 的 status-pill

**Files:**
- Modify: `abt-web/src/pages/customer_detail.rs`
- Modify: `abt-web/src/pages/customer_list.rs`

这两个文件使用 `status_class` 字符串变量映射状态。

- [ ] **Step 1: 迁移 customer_detail.rs**

文件 `abt-web/src/pages/customer_detail.rs`。

行 64 的 `status_class` match（报价单状态）改为返回 `StatusColor`：
```rust
// 原：QuotationStatus::Draft => "status-draft"
// 新：QuotationStatus::Draft => StatusColor::Muted
```

行 79 的 `status_class` match（销售订单状态）：
```rust
// 原：SalesOrderStatus::Draft => "status-draft"
// 新：SalesOrderStatus::Draft => StatusColor::Muted
// SalesOrderStatus::Confirmed => StatusColor::Accent
// SalesOrderStatus::PartiallyShipped => StatusColor::Accent  // partial
// SalesOrderStatus::Shipped => StatusColor::Success
// SalesOrderStatus::Completed => StatusColor::Success
// SalesOrderStatus::Cancelled => StatusColor::Danger
```

行 94 的 `status_class` match（发货状态）：
```rust
// ShippingStatus::Draft => StatusColor::Muted
// ShippingStatus::Confirmed => StatusColor::Accent
// ShippingStatus::Picking => StatusColor::Warn
// ShippingStatus::Shipped => StatusColor::Success
// ShippingStatus::Cancelled => StatusColor::Danger
```

行 109 的 `status_class` match（退货状态）：
```rust
// ReturnStatus::Draft => StatusColor::Muted
// ReturnStatus::Confirmed => StatusColor::Accent
// ReturnStatus::Received => StatusColor::Warn
// ReturnStatus::Inspecting => StatusColor::Warn
// ReturnStatus::Completed => StatusColor::Success
// ReturnStatus::Cancelled => StatusColor::Danger
// ReturnStatus::Rejected => StatusColor::Danger
```

行 162 和 437 的 `span class=(format!("status-pill {}", tx.status_class))` 替换为 `(status_pill(tx.status_color, &tx.status_label))`。

行 378-381 的 `status_class` match（客户状态）：
```rust
// CustomerStatus::Prospective => StatusColor::Muted
// CustomerStatus::Active => StatusColor::Success  // "活跃"
// CustomerStatus::Inactive => StatusColor::Danger
// CustomerStatus::Blacklisted => StatusColor::Danger
```

- [ ] **Step 2: 迁移 customer_list.rs**

文件 `abt-web/src/pages/customer_list.rs`。

行 268-271 的 `status_class` match 改为 `StatusColor`：
```rust
// CustomerStatus::Prospective => StatusColor::Muted
// CustomerStatus::Active => StatusColor::Success
// CustomerStatus::Inactive => StatusColor::Danger
// CustomerStatus::Blacklisted => StatusColor::Danger
```

行 291 的 `span class=(format!("status-pill {status_class}"))` 替换为 `(status_pill(status_color, status_label))`。

- [ ] **Step 3: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 4: 迁移 MES 相关页面的 status-pill（batch_detail, card_query, demand_pool 等）

**Files:**
- Modify: `abt-web/src/pages/mes_batch_detail.rs`
- Modify: `abt-web/src/pages/mes_card_query.rs`
- Modify: `abt-web/src/pages/mes_demand_pool.rs`
- Modify: `abt-web/src/pages/mes_demand_pool_create.rs`
- Modify: `abt-web/src/pages/fms_cost_analysis.rs`

- [ ] **Step 1: 迁移 mes_batch_detail.rs**

文件 `abt-web/src/pages/mes_batch_detail.rs`。

行 18-25 的 `status_class` match 改为 `StatusColor`：
```rust
// Pending => StatusColor::Muted
// InProgress => StatusColor::Warn
// Suspended => StatusColor::Danger
// PendingReceipt => StatusColor::Warn      // inspecting 色
// Completed => StatusColor::Success
// Cancelled => StatusColor::Muted          // neutral 色
```

行 185、282、314、323 的 `span class=(format!("status-pill {sc}"))` 或硬编码 `span class="status-pill status-neutral"` 替换为 `(status_pill(sc, &sl))`。

其中行 282 的 `status-pill status-neutral` 替换为 `(status_pill(StatusColor::Muted, &shift_label(&r.shift)))`。

- [ ] **Step 2: 迁移 mes_card_query.rs**

文件 `abt-web/src/pages/mes_card_query.rs`。

行 108-115 的 match 改为 `StatusColor`（与 batch_detail 相同的映射）。

行 176、242、286 替换。其中行 286 比较特殊——status-pill 和 card-info-value 同时使用：
```rust
// 原：span class=(format!("card-info-value status-pill {status_cls}")) { (status_label) }
// 新：span class="card-info-value" { (status_pill(status_color, &status_label)) }
```

- [ ] **Step 3: 迁移 mes_demand_pool.rs**

文件 `abt-web/src/pages/mes_demand_pool.rs`。

行 919-928 使用 `status-pill-muted`/`status-pill-info`/`status-pill-warn`/`status-pill-success`/`status-pill-danger` 这些类——这些 class 在 base.css 中从未定义，只有基础 `.status-pill` 生效（无颜色）。迁移后直接修正为有颜色的版本：

```rust
// 原：
// let (label, cls) = match status {
//     1 => ("待处理", "status-pill-muted"),
//     2 => ("已确认", "status-pill-info"),
//     3 => ("已创建生产计划", "status-pill-warn"),
//     4 => ("已完成", "status-pill-success"),
//     5 => ("已拒绝", "status-pill-danger"),
//     _ => ("未知", "status-pill-muted"),
// };
// html! { span class=(format!("status-pill {cls}")) { (label) } }

// 替换为：
let color = match status {
    1 => StatusColor::Muted,
    2 => StatusColor::Accent,
    3 => StatusColor::Warn,
    4 => StatusColor::Success,
    5 => StatusColor::Danger,
    _ => StatusColor::Muted,
};
html! { (status_pill(color, label)) }
```

- [ ] **Step 4: 迁移 mes_demand_pool_create.rs**

文件 `abt-web/src/pages/mes_demand_pool_create.rs`。

行 280 的 `span class="status-pill status-draft" style="font-size:11px;padding:2px 8px;margin-right:6px;background:#fef3c7;color:#d97706;"` 这是一个带有内联 style 覆盖的 status pill。替换为：
```rust
span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold bg-[#fef3c7] text-[#d97706] mr-1.5" {
    "生产需求池 · 按物料聚合"
}
```

- [ ] **Step 5: 迁移 fms_cost_analysis.rs**

文件 `abt-web/src/pages/fms_cost_analysis.rs`。

行 286-291 的 `wo_status_label` 函数返回 `(label, cls)` 字符串对。改为返回 `(String, StatusColor)`：
```rust
fn wo_status_label(s: i32) -> (String, StatusColor) {
    match s {
        1 => ("草稿".into(), StatusColor::Muted),
        2 => ("已计划".into(), StatusColor::Accent),     // planned → accent
        3 => ("已下达".into(), StatusColor::Warn),        // progress → warn
        4 => ("已完工".into(), StatusColor::Success),
        5 => ("已取消".into(), StatusColor::Danger),
        _ => ("未知".into(), StatusColor::Muted),
    }
}
```

行 435 的 `span class=(format!("status-pill {cls}"))` 替换为 `(status_pill(color, &label))`。

- [ ] **Step 6: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 5: 迁移 FMS/Expense/Journal 和 MD 页面的 status-pill

**Files:**
- Modify: `abt-web/src/pages/fms_dashboard.rs`
- Modify: `abt-web/src/pages/fms_expense_detail.rs`
- Modify: `abt-web/src/pages/fms_journal_detail.rs`
- Modify: `abt-web/src/pages/fms_writeoff_list.rs`
- Modify: `abt-web/src/pages/md_work_calendar_detail.rs`
- Modify: `abt-web/src/pages/md_work_center_detail.rs`
- Modify: `abt-web/src/pages/md_work_center_list.rs`

- [ ] **Step 1: 迁移 fms_dashboard.rs**

文件 `abt-web/src/pages/fms_dashboard.rs`。

行 341 的 `span class="status-pill status-progress" style="font-size:11px"` 替换为：
```rust
(status_pill_compact(StatusColor::Warn, "待审批"))
```

- [ ] **Step 2: 迁移 fms_expense_detail.rs**

文件 `abt-web/src/pages/fms_expense_detail.rs`。

行 29-34 的 match 改为 `StatusColor`：
```rust
// ExpenseStatus::Draft => ("草稿", StatusColor::Muted)
// ExpenseStatus::Submitted => ("已提交", StatusColor::Accent)
// ExpenseStatus::Approved => ("已审批", StatusColor::Success)  // active → success
// ExpenseStatus::Paid => ("已付款", StatusColor::Success)
// ExpenseStatus::Cancelled => ("已取消", StatusColor::Danger)  // inactive → danger
```

行 95、128 的 `span class=(format!("status-pill {s_class}"))` 替换为 `(status_pill(s_color, &s_text))`。

- [ ] **Step 3: 迁移 fms_journal_detail.rs**

文件 `abt-web/src/pages/fms_journal_detail.rs`。

行 34-37 的 match 改为 `StatusColor`：
```rust
// JournalStatus::Draft => ("草稿", StatusColor::Muted)
// JournalStatus::Confirmed => ("已确认", StatusColor::Success)  // active → success
// JournalStatus::Cancelled => ("已取消", StatusColor::Danger)
```

行 75 的 `span class=(format!("status-pill {s_class}"))` 替换为 `(status_pill(s_color, &s_text))`。

- [ ] **Step 4: 迁移 fms_writeoff_list.rs**

文件 `abt-web/src/pages/fms_writeoff_list.rs`。

行 255 的 `span class="status-pill full"` 替换为：
```rust
(status_pill(StatusColor::Success, "已核销完毕"))
```

- [ ] **Step 5: 迁移 md_work_calendar_detail.rs**

文件 `abt-web/src/pages/md_work_calendar_detail.rs`。

行 134 的 `span class="status-pill status-active"` → `(status_pill(StatusColor::Success, "特殊工作日"))`

行 136 的 `span class="status-pill status-inactive"` → `(status_pill(StatusColor::Danger, "休息日"))`

- [ ] **Step 6: 迁移 md_work_center_detail.rs 和 md_work_center_list.rs**

文件 `abt-web/src/pages/md_work_center_detail.rs`。

行 81 的 `span class="status-pill status-active"` → `(status_pill(StatusColor::Success, "启用"))`

行 83 的 `span class="status-pill status-inactive"` → `(status_pill(StatusColor::Danger, "停用"))`

文件 `abt-web/src/pages/md_work_center_list.rs`。

行 174 的 `status-active` → `(status_pill(StatusColor::Success, "启用"))`

行 176 的 `status-inactive` → `(status_pill(StatusColor::Danger, "停用"))`

- [ ] **Step 7: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 6: 删除 base.css 中其余 status- 定义（5 处散落）

**Files:**
- Modify: `static/base.css`（删除 5 处散落的 status- 定义）

- [ ] **Step 1: 删除 base.css 行 1154-1158（System Management status）**

删除：
```css
/* ─── System Management ─── */
.status-active { background: #f0fff0; color: #389e0d; }
.status-active::before { background: #389e0d; }
.status-inactive { background: #fff2f0; color: #cf1322; }
.status-inactive::before { background: #cf1322; }
```

注意：`.stat-chip`（行 1159）不在本批次范围内，保留。

- [ ] **Step 2: 删除 base.css 行 1294-1298（BOM Status Colors）**

删除：
```css
/* ─── BOM Status Colors ─── */
.status-bom-draft { background: #fffbeb; color: #b45309; }
.status-bom-draft::before { background: #f59e0b; }
.status-bom-published { background: #ecfdf5; color: #047857; }
.status-bom-published::before { background: #10b981; }
```

- [ ] **Step 3: 删除 base.css 行 2618-2628（MES additional status pills）**

删除：
```css
/* MES / additional status pills */
.status-confirmed { background: #e8f4ff; color: var(--accent-active); }
.status-confirmed::before { background: var(--accent-active); }
.status-inspecting { background: #fff8eb; color: #d46b08; }
.status-inspecting::before { background: #d46b08; }
.status-neutral { background: var(--surface); color: var(--fg-2); }
.status-neutral::before { background: var(--fg-2); }
.status-suspended { background: #fff1f0; color: #cf1322; }
.status-suspended::before { background: #cf1322; }
.status-defect { background: #fff2e8; color: #fa8c16; }
.status-defect::before { background: #fa8c16; }
```

- [ ] **Step 4: 删除 base.css 行 2733-2737（Status Pill Variants additional）**

删除：
```css
/* ─── Status Pill Variants (additional) ─── */
.status-success { background: #f0fff0; color: #389e0d; }
.status-success::before { background: #389e0d; }
.status-danger { background: #fff2f0; color: #cf1322; }
.status-danger::before { background: #cf1322; }
```

- [ ] **Step 5: 删除 base.css 行 3415-3420（FMS status-pill full/partial + ::before 重复）**

删除：
```css
/* Status pill with dot indicator — inherits from global .status-pill */
.status-pill::before { content: ''; display: block; width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.status-pill.full { background: rgba(22,163,74,0.08); color: var(--success); border-color: rgba(22,163,74,0.15); }
.status-pill.full::before { background: var(--success); box-shadow: 0 0 4px rgba(22,163,74,0.4); }
.status-pill.partial { background: rgba(217,119,6,0.08); color: var(--warn); border-color: rgba(217,119,6,0.15); }
.status-pill.partial::before { background: var(--warn); box-shadow: 0 0 4px rgba(217,119,6,0.4); }
```

- [ ] **Step 6: 删除 base.css 行 3642-3657（Import/Export status 最后一组）**

删除：
```css
.status-draft { background: rgba(100, 116, 139, 0.08); color: var(--muted); }
.status-draft::before { background: var(--muted); opacity: 1; box-shadow: none; }
.status-sent, .status-confirmed, .status-info { background: rgba(37, 99, 235, 0.08); color: var(--accent); }
.status-sent::before, .status-confirmed::before, .status-info::before { background: var(--accent); opacity: 1; box-shadow: none; }
.status-accepted, .status-picking, .status-received, .status-progress { background: rgba(217, 119, 6, 0.08); color: var(--warn); }
.status-accepted::before, .status-picking::before, .status-received::before, .status-progress::before { background: var(--warn); opacity: 1; box-shadow: none; }
.status-completed, .status-shipped, .status-settled { background: rgba(22, 163, 74, 0.08); color: var(--success); }
.status-completed::before, .status-shipped::before, .status-settled::before { background: var(--success); opacity: 1; box-shadow: none; }
.status-rejected, .status-expired, .status-cancelled, .status-danger { background: rgba(220, 38, 38, 0.08); color: var(--danger); }
.status-rejected::before, .status-expired::before, .status-cancelled::before, .status-danger::before { background: var(--danger); opacity: 1; box-shadow: none; }
.status-disputed { background: rgba(220, 38, 38, 0.08); color: var(--danger); }
.status-disputed::before { background: var(--danger); opacity: 1; box-shadow: none; }
.status-inspecting { background: rgba(217, 119, 6, 0.08); color: var(--warn); }
.status-inspecting::before { background: var(--warn); opacity: 1; box-shadow: none; }
.status-partial { background: rgba(37, 99, 235, 0.08); color: var(--accent); }
.status-partial::before { background: var(--accent); opacity: 1; box-shadow: none; }
```

- [ ] **Step 7: 删除 base.css 行 3965-3970（status-submitted）**

删除：
```css
.status-submitted {
  background: rgba(37,99,235,0.08);
  color: var(--accent);
  border: 1px solid rgba(37,99,235,0.15);
}
.status-submitted::before { background: var(--accent); opacity: 1; box-shadow: none; }
```

- [ ] **Step 8: 删除 base.css 行 2909-2911（status-pill-compact + status-under-review）**

删除：
```css
/* Status pills — inherit from global .status-pill definition */
.status-pill-compact { display: inline-flex; align-items: center; padding: 2px 10px; border-radius: var(--radius-pill); font-size: 11px; font-weight: 600; border: 1px solid transparent; }
.status-under-review { background: rgba(217,119,6,0.08); color: #b45309; border-color: rgba(217,119,6,0.12); }
```

- [ ] **Step 9: 搜索确认没有遗漏的 status-pill/status-* class 引用**

使用 search 工具搜索 `abt-web/src/**/*.rs` 中的 `status-pill`、`status-draft`、`status-info` 等 class 字符串引用。如果有遗漏的文件，按照同样的模式迁移。

Expected: 所有 status-pill class 引用已被替换为辅助函数调用

---

### Task 7: 迁移 status-tabs 组件

**Files:**
- Modify: `static/base.css:481-497`（删除 `.status-tabs`、`.status-tab`、`.status-tab:hover`、`.status-tab.active`、`.status-tab .count`、`.status-tab.active .count`）
- Modify: 所有引用 status-tabs 的 Maud 文件

base.css 原始定义（行 481-497）：

```css
/* ─── Status Tabs ─── */
.status-tabs { display: flex; gap: var(--space-1); margin-bottom: var(--space-6); border-bottom: 1px solid var(--border-soft); }
.status-tab {
  padding: var(--space-3) var(--space-4); font-size: var(--text-sm); color: var(--muted);
  border-bottom: 2px solid transparent; cursor: pointer;
  transition: all var(--motion-fast); white-space: nowrap;
  border-top: none; border-left: none; border-right: none;
  background: none; text-decoration: none; display: inline-flex;
  align-items: center; gap: 6px;
}
.status-tab:hover { color: var(--fg); }
.status-tab.active { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }
.status-tab .count {
  font-size: 11px; background: var(--surface); padding: 1px 7px;
  border-radius: var(--radius-pill); margin-left: var(--space-1); color: var(--muted); font-weight: 500;
}
.status-tab.active .count { background: var(--accent-bg); color: var(--accent); }
```

- [ ] **Step 1: 在 status_pill.rs 中添加 status_tabs 组件**

在 `abt-web/src/components/status_pill.rs` 末尾添加：

```rust
/// 渲染状态标签栏（列表页 tab 切换）。
///
/// `tabs`: `&[(id, label, count)]`，`active` 为当前激活的 tab id。
/// count 为可选的计数徽章（传 `None` 则不显示）。
pub fn status_tabs(active: &str, tabs: &[(&str, &str, Option<u32>)]) -> Markup {
    html! {
        div class="flex gap-1 mb-6 border-b border-border-soft" {
            @for (id, label, count) in tabs {
                @let is_active = *id == active;
                button
                    type="button"
                    class={
                        "px-4 py-3 text-sm cursor-pointer transition-all whitespace-nowrap "
                        "border-b-2 border-transparent inline-flex items-center gap-1.5 "
                        "bg-transparent border-x-0 border-t-0 "
                        (if is_active { "text-accent border-accent font-semibold" } else { "text-muted hover:text-fg" })
                    }
                    onclick=(format!("switchTab('{id}', this)")) {
                    (label)
                    @if let Some(c) = count {
                        span class={
                            "text-[11px] px-1.5 py-px rounded-full ml-1 font-medium "
                            (if is_active { "bg-accent-bg text-accent" } else { "bg-surface text-muted" })
                        } { (c) }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: 搜索并迁移所有 status-tabs 使用处**

使用 search 工具搜索 `abt-web/src` 中 `status-tabs` 的使用。对每个文件：
- 将 `div class="status-tabs"` 替换为 `(status_tabs(active_tab, &tabs))`
- 或如果页面使用 hyperscript 切换，保持现有的切换逻辑，只替换 class 字符串

由于 status-tabs 的切换逻辑各页面不同（有的用 hyperscript，有的用 inline JS），建议保留切换逻辑，只替换 class：

```rust
// 原：div class="status-tabs" { ... }
// 新（保持结构，仅替换 class）：
div class="flex gap-1 mb-6 border-b border-border-soft" { ... }

// 原：button class="status-tab active" 或 button class="status-tab"
// 新：
button class="px-4 py-3 text-sm cursor-pointer transition-all whitespace-nowrap border-b-2 border-transparent inline-flex items-center gap-1.5 bg-transparent border-x-0 border-t-0 text-accent border-accent font-semibold"
// active=false 时末尾改为：text-muted hover:text-fg

// 原：span class 内含 count
// 新：span class="text-[11px] px-1.5 py-px rounded-full ml-1 font-medium bg-surface text-muted"
// active 时改为：bg-accent-bg text-accent
```

- [ ] **Step 3: 删除 base.css 行 481-497**

删除整个 `/* ─── Status Tabs ─── */` 块。

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 8: 迁移 status-flow 组件

**Files:**
- Modify: `static/base.css:2372-2391`（删除 `.status-flow`、`.status-flow-step`、`.status-flow-step.done`、`.status-flow-step.current`、`.status-flow-arrow`）
- Modify: 引用 status-flow 的 Maud 文件

base.css 原始定义（行 2372-2391）：

```css
.status-flow {
  display: flex; align-items: center; gap: var(--space-2);
  margin-bottom: var(--space-5); padding: var(--space-3) var(--space-4);
  background: var(--bg); border: 1px solid var(--border-soft);
  border-radius: var(--radius-md);
}
.status-flow-step {
  font-size: var(--text-xs); padding: 3px 10px;
  border-radius: var(--radius-pill); background: var(--surface);
  color: var(--muted); border: 1px solid var(--border);
}
.status-flow-step.done {
  background: rgba(82,196,26,0.08); color: var(--success);
  border-color: rgba(82,196,26,0.3);
}
.status-flow-step.current {
  background: rgba(22,119,255,0.08); color: var(--accent);
  border-color: rgba(22,119,255,0.3); font-weight: 600;
}
.status-flow-arrow { color: var(--border); font-size: 10px; }
```

- [ ] **Step 1: 定义原子 class 映射**

```text
.status-flow →
  flex items-center gap-2 mb-5 px-4 py-3 bg-bg border border-border-soft rounded-md

.status-flow-step →
  text-xs px-2.5 py-0.5 rounded-full bg-surface text-muted border border-border

.status-flow-step.done →
  bg-success/8 text-success border-success/30

.status-flow-step.current →
  bg-accent/8 text-accent border-accent/30 font-semibold

.status-flow-arrow →
  text-border text-[10px]
```

- [ ] **Step 2: 搜索并迁移 status-flow 使用处**

使用 search 工具搜索 `abt-web/src` 中的 `status-flow`。对每个引用：
```rust
// 原：div class="status-flow" { ... }
// 新：div class="flex items-center gap-2 mb-5 px-4 py-3 bg-bg border border-border-soft rounded-md" { ... }

// 原：span class="status-flow-step done"
// 新：span class="text-xs px-2.5 py-0.5 rounded-full bg-success/8 text-success border border-success/30"

// 原：span class="status-flow-step current"
// 新：span class="text-xs px-2.5 py-0.5 rounded-full bg-accent/8 text-accent border border-accent/30 font-semibold"
```

- [ ] **Step 3: 删除 base.css 行 2372-2391**

删除整个 `.status-flow` 到 `.status-flow-arrow` 块。注意保留行 2392 的 `@media` 块（它包含 `.summary-bar` 响应式规则，不属于本批次）。

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 9: 构建 CSS 并验证

**Files:**
- 无文件修改（验证步骤）

- [ ] **Step 1: 重新构建 CSS**

Run: `cd E:/work/abt && npm run build:css`

Expected: 成功生成 app.css，无错误

- [ ] **Step 2: 验证 app.css 不再包含 status-pill 定义**

使用 search 工具搜索 `static/app.css` 中的 `status-pill`。由于 base.css 中的定义已删除，且 Maud 中不再使用 `status-pill` class 名（改为原子 class），app.css 中应不再有 `.status-pill` 规则。

Expected: 无匹配

- [ ] **Step 3: 验证 app.css 包含 before: 伪元素原子 class**

使用 search 工具搜索 `static/app.css` 中的 `before\:content-\[''\]` 或等效的 UnoCSS 生成规则。

Expected: 存在（UnoCSS 生成了 before: 伪元素相关的 utility）

- [ ] **Step 4: 用 agent-browser 验证页面渲染**

Run:
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/mes/batches/1"
agent-browser --cdp 9222 eval "JSON.stringify({
  pillBg: getComputedStyle(document.querySelector('.before\\:content-\\[\\'\\'\\]')?.parentElement || document.body).backgroundColor,
  pillCount: document.querySelectorAll('span[class*=before\\:content]').length
})"
```

Expected: pillCount > 0（页面中有 status pill 元素）

验证代表性页面：
- `/admin/sales/customers/1` — 客户详情页 status pill
- `/admin/mes/batches/1` — 批次详情 status pill
- `/admin/fms/expenses/1` — 费用详情 status pill
- `/admin/master-data/work-centers/1` — 工作中心 status pill

- [ ] **Step 5: cargo clippy 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error 输出

---

### Task 10: 提交

- [ ] **Step 1: Git 提交**

```bash
cd E:/work/abt && git add abt-web/src/components/status_pill.rs abt-web/src/components/mod.rs abt-web/src/pages/*.rs static/base.css static/app.css && git commit -m "refactor(css): P4 — migrate status-pill + status-tabs + status-flow to atomic UnoCSS

- Add status_pill() helper with StatusColor enum (6 semantic groups)
- Add status_pill_compact() for space-constrained contexts
- Replace 55+ status-* class variants across 20+ Maud files
- Remove 8 scattered status-pill definition blocks from base.css (6 locations)
- Remove status-tabs and status-flow component definitions
- All status pills now use before: pseudo-element atomic pattern"
```
