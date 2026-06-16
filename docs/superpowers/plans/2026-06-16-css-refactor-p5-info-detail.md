# P5: Info Card/Grid + Detail Layout 原子化迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 info-card、info-grid、info-item、detail-grid、detail-card、detail-tabs 等 43 个详情页布局 class 从 base.css 迁移到 100% 原子 UnoCSS class。

**Architecture:** 详情页布局由两组组件构成：(1) info-* 族（信息卡片 + 网格 + 标签值对），(2) detail-* 族（详情页头部 + 多列网格 + 选项卡 + 标签值行）。base.css 中存在多处重复定义（info-card 有 4 处、detail-grid 有 4 处），部分页面通过 scoped 父选择器（`.fms-form-page .info-card`）覆盖全局 class。原子化后统一使用原子 class 内联到 Maud，FMS 页面直接写 FMS 专属原子组合，不依赖 scoped 级联。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Rust + Maud HTML 宏

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

## 原子 class 映射总表

### info-* 族

| 原 class | 原子 class 替换 |
|---|---|
| `info-card` | `bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm` + `hover:shadow-md transition-shadow` |
| `info-card-title` | `flex items-center gap-2 text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft` |
| `info-card-flat` | `bg-white border border-border-soft rounded-md p-5 shadow-xs transition-shadow` |
| `info-card-rows` | `border border-border-soft rounded-lg overflow-hidden` |
| `info-grid` | `grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-5` |
| `info-grid-3` | `grid grid-cols-3 gap-5` |
| `info-grid-4` | `grid grid-cols-4 gap-5` |
| `info-item` | `flex flex-col gap-1` |
| `info-label` | `text-xs font-medium text-muted tracking-wide` |
| `info-value` | `text-sm text-fg font-medium` |
| `info-value.mono` | `text-sm text-fg font-medium font-mono tabular-nums` |
| `info-section` | `bg-bg border border-border-soft rounded-lg p-6 mb-5 shadow-card` |
| `info-section-title` | `text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft` |
| `info-row` | `flex items-center px-4 py-2.5 text-[13px] border-b border-border-soft last:border-b-0` |
| `info-muted` | `text-muted` |
| `info-mono` | `font-mono text-xs text-accent` |
| `info-success` | `text-success` |
| `info-progress` | `flex flex-col items-center justify-center pl-10 border-l border-border-soft min-w-[130px]` |

### detail-* 族

| 原 class | 原子 class 替换 |
|---|---|
| `detail-grid` | `grid grid-cols-[1fr_1fr_340px] gap-5 lg:grid-cols-2 md:grid-cols-1` |
| `detail-card` | `bg-white border border-border-soft rounded-md px-6 py-5` |
| `detail-card-title` | `text-sm font-semibold mb-4 pb-2 border-b border-border-soft flex items-center justify-between` |
| `detail-row` | `flex py-2 text-sm` |
| `detail-label` | `w-[90px] shrink-0 text-muted` |
| `detail-value` | `text-fg` |
| `detail-header` | `block bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-card` |
| `detail-title-row` | `flex items-center justify-between mb-4` |
| `detail-doc-no` | `text-2xl font-extrabold text-fg tracking-tight` (或 `text-xl font-bold` for batch 版) |
| `detail-no` | `text-2xl font-extrabold tracking-tight bg-gradient-to-br from-fg to-fg-2 bg-clip-text text-transparent` |
| `detail-top` | `flex justify-between items-start mb-6 md:flex-col md:gap-4` |
| `detail-meta` | `flex items-center gap-2.5 mt-2.5 ml-[58px]` |
| `detail-actions` | `flex gap-2 shrink-0` |
| `detail-section` | `mb-5` |
| `detail-section-title` | `text-[13px] font-semibold text-fg mb-3 flex items-center gap-1.5` |
| `detail-tabs` | `flex border-b border-border-soft mb-6` |
| `detail-tab` | `px-5 py-3 text-sm text-muted border-none bg-transparent border-b-2 border-transparent cursor-pointer transition-all whitespace-nowrap font-medium` |
| `detail-tab:hover` | `hover:text-fg hover:bg-accent-bg` |
| `detail-tab.active` | `text-accent border-accent font-semibold` (添加到 detail-tab class 串) |

---

### Task 1: 创建 info_detail.rs 组件辅助文件

**Files:**
- Create: `abt-web/src/components/info_detail.rs`
- Modify: `abt-web/src/components/mod.rs`

- [ ] **Step 1: 创建 info_detail.rs**

创建 `abt-web/src/components/info_detail.rs`：

```rust
use maud::{html, Markup};

/// 信息卡片容器（详情页标准信息卡）。
///
/// ```rust
/// info_card("基本信息", html! { ... })
/// ```
pub fn info_card(title: &str, body: Markup) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm" {
            div class="flex items-center gap-2 text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                (title)
            }
            (body)
        }
    }
}

/// 信息卡片容器（无标题，直接放内容）。
pub fn info_card_bare(body: Markup) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm" {
            (body)
        }
    }
}

/// 信息网格（auto-fill, minmax 200px）。
pub fn info_grid(items: Markup) -> Markup {
    html! {
        div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-5" {
            (items)
        }
    }
}

/// 信息项（label + value 竖排）。
pub fn info_item(label: &str, value: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-1" {
            span class="text-xs font-medium text-muted tracking-wide" { (label) }
            span class="text-sm text-fg font-medium" { (value) }
        }
    }
}

/// 信息行（label + value 横排，带分隔线）。
pub fn info_row(label: &str, value: Markup) -> Markup {
    html! {
        div class="flex items-center px-4 py-2.5 text-[13px] border-b border-border-soft last:border-b-0" {
            span class="w-20 shrink-0 text-muted text-xs" { (label) }
            span class="text-fg font-medium" { (value) }
        }
    }
}

/// 详情页标签值行（detail-row + detail-label + detail-value）。
pub fn detail_row(label: &str, value: Markup) -> Markup {
    html! {
        div class="flex py-2 text-sm" {
            span class="w-[90px] shrink-0 text-muted" { (label) }
            span class="text-fg" { (value) }
        }
    }
}

/// 详情页 Tab 栏。
pub fn detail_tabs(active: &str, tabs: &[(&str, &str)]) -> Markup {
    html! {
        div class="flex border-b border-border-soft mb-6" {
            @for (id, label) in tabs {
                @let is_active = *id == active;
                button
                    type="button"
                    class={
                        "px-5 py-3 text-sm cursor-pointer transition-all whitespace-nowrap font-medium "
                        "border-none bg-transparent border-b-2 "
                        (if is_active {
                            "border-accent text-accent font-semibold"
                        } else {
                            "border-transparent text-muted hover:text-fg hover:bg-accent-bg"
                        })
                    }
                    onclick=(format!("switchDetailTab('{id}', this)")) {
                    (label)
                }
            }
        }
        (maud::PreEscaped(r#"<script>function switchDetailTab(t,b){document.querySelectorAll('.tab-panel').forEach(function(p){p.style.display='none'});document.querySelectorAll('[data-detail-tab]').forEach(function(x){x.classList.remove('is-active')});var e=document.getElementById('tab-'+t);if(e)e.style.display='';if(b)b.classList.add('is-active')};setTimeout(function(){var p=new URLSearchParams(location.search);var t=p.get('tab');if(t){var b=document.querySelector('[data-detail-tab][onclick*="'+t+'"]');if(b)switchDetailTab(t,b)}},0);</script>"#))
    }
}

/// 详情页 Tab 内容面板。
pub fn tab_panel(id: &str, active: bool, content: Markup) -> Markup {
    let style = if active { "" } else { "display:none" };
    html! {
        div class="tab-panel" id=(format!("tab-{id}")) style=(style) {
            (content)
        }
    }
}
```

> **注意**：`detail_tabs` 的 JS 中选择器从 `.detail-tab` 改为 `[data-detail-tab]`，因为原子化后不再有 `.detail-tab` class。Tab 按钮上需要添加 `data-detail-tab` 属性。如果 JS 改动风险过高，可以保持 `.detail-tab` 作为纯 JS 标记 class（无 CSS 定义），但这违反"100% 纯原子"原则。推荐使用 `data-detail-tab` 属性。

- [ ] **Step 2: 在 mod.rs 中注册**

在 `abt-web/src/components/mod.rs` 中添加：

```rust
pub mod info_detail;
pub use info_detail::{
    info_card, info_card_bare, info_grid, info_item, info_row,
    detail_row, detail_tabs, tab_panel,
};
```

- [ ] **Step 3: 更新 detail.rs 中的 detail_row 和 detail_tabs**

文件 `abt-web/src/components/detail.rs`。

将 `detail_row` 和 `detail_tabs` 函数体替换为使用原子 class（与 info_detail.rs 中相同的实现）。或更简单地：删除 detail.rs 中的 `detail_row` 和 `detail_tabs`，改为从 info_detail.rs re-export。

最简洁方案——将 detail.rs 的内容替换为：

```rust
pub use crate::components::info_detail::{detail_row, detail_tabs, tab_panel};
```

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 2: 迁移 info-card / info-card-title（base.css 行 999-1011）

**Files:**
- Modify: `static/base.css:999-1011`（删除第一组 info-card 定义）
- Modify: `static/base.css:1776`（删除第二组 info-card 定义）
- Modify: `static/base.css:2086-2093`（删除第三组 info-card-rows + info-muted 等）
- Modify: `static/base.css:3972-3993`（删除第四组 info-card glassmorphism 定义）
- Modify: 所有引用 info-card 的 Maud 文件

- [ ] **Step 1: 删除 base.css 行 999-1011（Info Card 第一组）**

删除：
```css
/* ─── Info Card (Detail Page) ─── */
.info-card {
  background: var(--bg); border: 1px solid var(--border-soft); border-radius: var(--radius-md);
  padding: var(--space-6); margin-bottom: var(--space-6);
  box-shadow: 0 1px 2px rgba(0,0,0,0.03); transition: box-shadow var(--motion-base);
}
.info-card:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.06); }
.info-card-title {
  display: flex; align-items: center; gap: 8px;
  font-size: var(--text-base); font-weight: 600; color: var(--fg);
  margin-bottom: var(--space-4); padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-soft);
}
```

- [ ] **Step 2: 删除 base.css 行 1776-1781（Info Card 第二组 — info-row 版）**

删除：
```css
.info-card{border:1px solid var(--border-soft);border-radius:var(--radius-lg);overflow:hidden}
.info-row{display:flex;align-items:center;padding:9px var(--space-4);font-size:13px;border-bottom:1px solid var(--border-soft)}
.info-row:last-child{border-bottom:none}
.info-label{width:80px;flex-shrink:0;color:var(--muted);font-size:12px}
.info-val{color:var(--fg);font-weight:500}
.info-val.mono{font-family:var(--font-mono);font-size:12px;color:var(--accent)}
```

- [ ] **Step 3: 删除 base.css 行 2086-2093（info-card-rows + info-mono + info-muted + info-success）**

删除：
```css
.info-card-rows{border:1px solid var(--border-soft);border-radius:var(--radius-lg);overflow:hidden}
.info-row{display:flex;align-items:center;padding:9px var(--space-4);font-size:13px;border-bottom:1px solid var(--border-soft)}
.info-row:last-child{border-bottom:none}
.info-label{width:80px;flex-shrink:0;color:var(--muted);font-size:12px}
.info-val{color:var(--fg);font-weight:500}
.info-mono{font-family:var(--font-mono);font-size:12px;color:var(--accent)}
.info-muted{color:var(--muted) !important}
.info-success{color:#389e0d !important}
```

- [ ] **Step 4: 删除 base.css 行 3972-3993（Info Card glassmorphism + title 装饰）**

删除：
```css
.info-card {
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  background: rgba(255,255,255,0.92);
  border: 1px solid rgba(255,255,255,0.6);
  box-shadow: 0 4px 24px rgba(15,23,42,0.05), 0 1px 3px rgba(15,23,42,0.03);
}
.info-card-title {
  position: relative;
  padding-left: 14px;
}
.info-card-title::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 4px;
  height: 18px;
  border-radius: 2px;
  background: linear-gradient(180deg, var(--accent), #3b82f6);
}
```

- [ ] **Step 5: 迁移 fms_cost_analysis.rs 中的 info-card**

文件 `abt-web/src/pages/fms_cost_analysis.rs`。

行 344、401、447、490 的 `div class="info-card"` 替换为：
```rust
div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm" { ... }
```

行 345、402、448、491 的 `div class="info-card-title"` 替换为：
```rust
div class="flex items-center gap-2 text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { ... }
```

- [ ] **Step 6: 迁移 fms_expense_create.rs 和 fms_expense_detail.rs 中的 info-card**

文件 `abt-web/src/pages/fms_expense_create.rs`。

行 110、140 的 `div class="info-card"` 替换为：
```rust
div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm" { ... }
```

行 111、141 的 `div class="info-card-title"` 替换为：
```rust
div class="flex items-center gap-2 text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { ... }
```

文件 `abt-web/src/pages/fms_expense_detail.rs`。

行 100 的 `div.info-card` 替换为 `div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm"`。

行 101 的 `div.info-card-title` 替换为 `div class="flex items-center gap-2 text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft"`。

- [ ] **Step 7: 迁移 fms_journal_detail.rs 中的 info-card**

文件 `abt-web/src/pages/fms_journal_detail.rs`。

行 81 的 `div class="info-card"` → `div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm"`。

- [ ] **Step 8: 迁移 md_work_calendar_detail.rs 和 md_work_center_detail.rs 中的 info-card**

文件 `abt-web/src/pages/md_work_calendar_detail.rs`。

行 72、88、116 的 `div class="info-card"` → `div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm"`。

行 73、89、117 的 `div class="info-section-title"` → `div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"`。

文件 `abt-web/src/pages/md_work_center_detail.rs`。

行 69、93 的 `div class="info-card"` → 同上替换。

行 70、94 的 `div class="info-section-title"` → 同上替换。

- [ ] **Step 9: 迁移 mes_exception_detail.rs 中的 info-card**

文件 `abt-web/src/pages/mes_exception_detail.rs`。

行 81、113、121 的 `div class="info-card"` → `div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm"`。

行 87 的 `div class="info-grid"` → `div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-5"`。

行 88-107 的 `div class="info-item"` → `div class="flex flex-col gap-1"`。

- [ ] **Step 10: 迁移 mes_inspection_detail.rs 中的 info-card**

文件 `abt-web/src/pages/mes_inspection_detail.rs`。

行 51 的 `div class="info-card"` → `div class="bg-bg border border-border-soft rounded-md p-6 mb-6 shadow-sm"`。

行 52 的 `div class="info-grid"` → `div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-5"`。

行 53-62 的 `div class="info-item"` → `div class="flex flex-col gap-1"`。

- [ ] **Step 11: 迁移 department_list.rs 中的 info-card 和 info-row**

文件 `abt-web/src/pages/department_list.rs`。

行 482 的 `div class="info-card"` → `div class="border border-border-soft rounded-lg overflow-hidden"`（该页面使用 info-row 行布局，不是标准 info-card 布局）。

行 483、487、491、496 的 `div class="info-row"` → `div class="flex items-center px-4 py-2.5 text-[13px] border-b border-border-soft last:border-b-0"`。

行 484、488、492、497 的 `span class="info-label"` → `span class="w-20 shrink-0 text-muted text-xs"`。

行 485、489、493、498 的 `span class="info-val mono"` → `span class="font-mono text-xs text-accent"` 或 `span class="text-fg font-medium"`（非 mono 的行）。

- [ ] **Step 12: 迁移剩余 info-card 引用文件**

搜索所有剩余使用 `info-card` class 的 Maud 文件（使用 search 工具搜索 `abt-web/src` 中的 `"info-card"` 字符串），对每个文件按照同样的映射替换。

涉及文件还包括：`mes_order_detail.rs`、`mes_material_usage.rs` 等。

- [ ] **Step 13: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 3: 迁移 info-grid / info-item / info-label / info-value（base.css 行 1013-1018）

**Files:**
- Modify: `static/base.css:1013-1018`（删除 info-grid 定义）
- Modify: `static/base.css:4241-4242`（删除 info-grid-3/4）
- Modify: 所有引用这些 class 的 Maud 文件

- [ ] **Step 1: 删除 base.css 行 1013-1018（Info Grid 定义）**

删除：
```css
/* ─── Info Grid (Detail Page) ─── */
.info-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: var(--space-5); }
.info-item { display: flex; flex-direction: column; gap: 4px; }
.info-label { font-size: 12px; font-weight: 500; color: var(--muted); letter-spacing: 0.02em; }
.info-value { font-size: var(--text-sm); color: var(--fg); font-weight: 500; }
.info-value.mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
```

- [ ] **Step 2: 删除 base.css 行 4241-4242（info-grid-3/4）**

删除：
```css
.info-grid-3{display:grid;grid-template-columns:repeat(3,1fr);gap:var(--space-5)}
.info-grid-4{display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-5)}
```

- [ ] **Step 3: 搜索并迁移所有 info-grid/info-item/info-label/info-value 引用**

使用 search 工具搜索 `abt-web/src` 中的这些 class 字符串。对每处引用：

```rust
// info-grid →
div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-5"

// info-grid-3 →
div class="grid grid-cols-3 gap-5"

// info-grid-4 →
div class="grid grid-cols-4 gap-5"

// info-item →
div class="flex flex-col gap-1"

// info-label (span) →
span class="text-xs font-medium text-muted tracking-wide"

// info-value (span) →
span class="text-sm text-fg font-medium"

// info-value.mono →
span class="text-sm text-fg font-medium font-mono tabular-nums"
```

涉及文件（非穷尽，需 search 确认）：`fms_expense_detail.rs`、`fms_journal_detail.rs`、`md_work_calendar_detail.rs`、`md_work_center_detail.rs`、`mes_exception_detail.rs`、`mes_inspection_detail.rs`、`mes_order_detail.rs`、`category_list.rs`、`dashboard.rs`、`md_dashboard.rs`。

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 4: 迁移 info-section / info-section-title（base.css 行 4218-4219）

**Files:**
- Modify: `static/base.css:4218-4219`

- [ ] **Step 1: 删除 base.css 行 4218-4219**

删除：
```css
.info-section{background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-lg);padding:var(--space-6);margin-bottom:var(--space-5);box-shadow:var(--shadow-card)}
.info-section-title{font-size:var(--text-sm);font-weight:600;color:var(--fg);margin-bottom:var(--space-3);padding-bottom:var(--space-2);border-bottom:1px solid var(--border-soft)}
```

- [ ] **Step 2: 搜索并迁移 info-section/info-section-title 引用**

```rust
// info-section →
div class="bg-bg border border-border-soft rounded-lg p-6 mb-5 shadow-card"

// info-section-title →
div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"
```

注意：部分文件（如 md_work_calendar_detail.rs 行 73）已经在前面的 Task 中将 `info-section-title` 误标为需要在 info-card 内替换——确认这里是同一个 class，统一替换。

- [ ] **Step 3: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 5: 迁移 info-card-flat（base.css 行 2723）

**Files:**
- Modify: `static/base.css:2722-2723`
- Modify: `abt-web/src/pages/dashboard.rs`
- Modify: `abt-web/src/pages/md_dashboard.rs`

- [ ] **Step 1: 删除 base.css 行 2722-2723**

删除：
```css
/* ─── Info Card Flat ─── */
.info-card-flat { background: #fff; border: 1px solid var(--border-soft); border-radius: var(--radius-md); padding: var(--space-5); box-shadow: var(--shadow-xs); transition: box-shadow 240ms; }
```

注意：`.dash-stat`（行 2724）保留，不在本批次。

- [ ] **Step 2: 迁移 dashboard.rs 中的 info-card-flat**

文件 `abt-web/src/pages/dashboard.rs`。

行 110、129、142 的 `div class="info-card-flat"` 替换为：
```rust
div class="bg-white border border-border-soft rounded-md p-5 shadow-xs transition-shadow" { ... }
```

行 111、130、143 的 `span class="info-label"` 替换为：
```rust
span class="text-xs font-medium text-muted tracking-wide" { ... }
```

- [ ] **Step 3: 迁移 md_dashboard.rs 中的 info-card-flat**

文件 `abt-web/src/pages/md_dashboard.rs`。

行 127 的 `div class="info-card-flat"` → 同上替换。

行 128 的 `span class="info-label"` → 同上替换。

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 6: 迁移 detail-grid / detail-card / detail-row / detail-label / detail-value

**Files:**
- Modify: `static/base.css:1180-1189`（删除第一组）
- Modify: `static/base.css:2076`（删除第二组 detail-grid）
- Modify: `static/base.css:3659-3670`（删除第三组）
- Modify: `static/base.css:1515-1529`（删除响应式 @media 中的 detail-grid）
- Modify: `static/base.css:2128-2129`（删除响应式 @media 中的 detail-grid）
- Modify: 所有引用这些 class 的 Maud 文件

- [ ] **Step 1: 删除 base.css 行 1180-1189（detail-grid 第一组）**

删除：
```css
.detail-grid { display: grid; grid-template-columns: 1fr 1fr 340px; gap: var(--space-5); }
.detail-card { background: #fff; border: 1px solid var(--border-soft); border-radius: var(--radius-md); padding: var(--space-5) var(--space-6); }
.detail-card-title {
  font-size: var(--text-sm); font-weight: 600; margin-bottom: var(--space-4);
  padding-bottom: var(--space-2); border-bottom: 1px solid var(--border-soft);
  display: flex; align-items: center; justify-content: space-between;
}
.detail-row { display: flex; padding: var(--space-2) 0; font-size: var(--text-sm); }
.detail-label { width: 90px; flex-shrink: 0; color: var(--muted); }
.detail-value { color: var(--fg); }
```

- [ ] **Step 2: 删除 base.css 行 2076（detail-grid 第二组 — User Detail）**

删除：
```css
.detail-grid{display:grid;grid-template-columns:1fr 1fr;gap:var(--space-5)}
```

- [ ] **Step 3: 删除 base.css 行 3659-3670（detail-grid 第三组 — Customer Detail）**

删除：
```css
.detail-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: var(--space-6); }
.customer-identity { display: flex; align-items: center; gap: var(--space-5); }
.customer-avatar { ... }
.customer-name { ... }
.customer-meta { ... }
.detail-grid { display: grid; grid-template-columns: 1fr 1fr 340px; gap: var(--space-5); }
.detail-card { background: #fff; border: 1px solid var(--border-soft); border-radius: var(--radius-md); padding: var(--space-5) var(--space-6); }
.detail-card-title { ... }
.detail-row { display: flex; padding: var(--space-2) 0; font-size: var(--text-sm); }
.detail-label { width: 90px; flex-shrink: 0; color: var(--muted); }
.detail-value { color: var(--fg); }
```

注意：`.customer-identity`、`.customer-avatar`、`.customer-name`、`.customer-meta` 也在此处定义——也需一并迁移（属于 detail 页面组件）。

- [ ] **Step 4: 删除 base.css 行 1169（detail-top 第一组）**

删除行 1169 的 `.detail-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: var(--space-6); }`。

- [ ] **Step 5: 删除响应式 @media 中的 detail-grid / detail-top**

在行 1515-1529 的 `@media` 块中，删除 `.detail-grid` 和 `.detail-top` 的响应式规则（原子 class 的 `lg:`/`md:` 前缀已替代）。

在行 2128-2129 的 `@media` 块中，删除 `.detail-grid` 的响应式规则。

- [ ] **Step 6: 迁移 customer_detail.rs 中的 detail-grid/detail-card**

文件 `abt-web/src/pages/customer_detail.rs`。

行 393 的 `div class="detail-top"` → `div class="flex justify-between items-start mb-6 md:flex-col md:gap-4"`。

行 394 的 `div class="customer-identity"` → `div class="flex items-center gap-5"`。

行 414 的 `div class="detail-grid"` → `div class="grid grid-cols-[1fr_1fr_340px] gap-5 lg:grid-cols-2 md:grid-cols-1"`。

行 416、430、434 等的 `div class="detail-card"` → `div class="bg-white border border-border-soft rounded-md px-6 py-5"`。

行 417 的 `div class="detail-card-title"` → `div class="text-sm font-semibold mb-4 pb-2 border-b border-border-soft flex items-center justify-between"`。

行 418、422、426 等的 `div class="detail-row"` → `div class="flex py-2 text-sm"`。

行 419、423、427 等的 `span class="detail-label"` → `span class="w-[90px] shrink-0 text-muted"`。

行 420、424、428 等的 `span class="detail-value"` → `span class="text-fg"`。

- [ ] **Step 7: 迁移 bom_detail.rs 中的 detail-card**

文件 `abt-web/src/pages/bom_detail.rs`。

行 206 的 `div class="detail-top"` → `div class="flex justify-between items-start mb-6 md:flex-col md:gap-4"`。

行 288 的 `div class="detail-card"` → `div class="bg-white border border-border-soft rounded-md px-6 py-5"`。

行 289 的 `div class="detail-card-title"` → `div class="text-sm font-semibold mb-4 pb-2 border-b border-border-soft flex items-center justify-between"`。

- [ ] **Step 8: 迁移 detail.rs 组件中的 detail_row**

文件 `abt-web/src/components/detail.rs` 已在 Task 1 中迁移（使用 info_detail.rs 的 re-export）。如果仍有直接使用 `detail_row` 的页面引用 `.detail-row`/`.detail-label`/`.detail-value` class，搜索并替换为原子 class 或使用辅助函数。

- [ ] **Step 9: 搜索并迁移所有剩余 detail-grid/detail-card/detail-row 引用**

使用 search 工具搜索 `abt-web/src` 中的这些 class 字符串，对每处按映射表替换。

- [ ] **Step 10: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 7: 迁移 detail-header / detail-title-row / detail-doc-no / detail-no / detail-meta / detail-actions

**Files:**
- Modify: `static/base.css:3940-3963`（删除 detail-header + detail-no）
- Modify: `static/base.css:2808-2816`（删除 Outsourcing detail title row + meta + actions）
- Modify: `static/base.css:2632-2633`（删除 MES batch detail title row + doc-no）
- Modify: `static/base.css:891-896`（删除 detail-section + detail-section-title）
- Modify: 所有引用这些 class 的 Maud 文件

- [ ] **Step 1: 删除 base.css 行 3940-3963（detail-header + detail-no gradient）**

删除：
```css
.detail-header {
  display: block;
  background: var(--bg);
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-lg);
  padding: var(--space-6);
  margin-bottom: var(--space-6);
  box-shadow: var(--shadow-card);
}
.detail-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}
.detail-no {
  font-size: var(--text-2xl);
  font-weight: 800;
  letter-spacing: -0.5px;
  background: linear-gradient(135deg, var(--fg) 0%, var(--fg-2) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}
```

- [ ] **Step 2: 删除 base.css 行 2808-2816（Outsourcing detail hero）**

删除：
```css
.detail-hero-body { padding: 32px 40px; }
.detail-title-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 20px; }
.detail-doc-no { font-size: 24px; font-weight: 700; color: var(--fg); display: flex; align-items: center; gap: 14px; letter-spacing: -0.02em; }
.detail-doc-no .doc-icon { width: 44px; height: 44px; border-radius: 12px; background: linear-gradient(135deg, var(--accent-bg), rgba(37,99,235,0.12)); display: grid; place-items: center; flex-shrink: 0; }
.detail-doc-no .doc-icon svg { width: 22px; height: 22px; color: var(--accent); }
.detail-meta { display: flex; align-items: center; gap: 10px; margin-top: 10px; margin-left: 58px; }
.detail-actions { display: flex; gap: 8px; flex-shrink: 0; }
.detail-actions .btn { border-radius: var(--radius-md); padding: 8px 18px; font-size: 13px; font-weight: 500; }
.detail-actions .btn svg { width: 15px; height: 15px; }
```

- [ ] **Step 3: 删除 base.css 行 2632-2633（MES batch detail-doc-no）**

删除：
```css
.batch-detail-title-row { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-5); }
.detail-doc-no { font-size: var(--text-xl, 18px); font-weight: 700; color: var(--fg); display: flex; align-items: center; gap: var(--space-3); }
```

- [ ] **Step 4: 删除 base.css 行 891-896（detail-section + detail-section-title）**

删除：
```css
.detail-section { margin-bottom: 20px; }
.detail-section-title {
  font-size: 13px; font-weight: 600; color: var(--fg); margin-bottom: 12px;
  display: flex; align-items: center; gap: 6px;
}
.detail-section-title svg { color: var(--muted); }
```

- [ ] **Step 5: 迁移 mes_order_detail.rs 中的 detail-header**

文件 `abt-web/src/pages/mes_order_detail.rs`。

行 334 的 `div class="detail-header"` → `div class="block bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-card"`。

行 335 的 `div class="detail-title-row"` → `div class="flex items-center justify-between mb-4"`。

行 336 的 `div class="detail-doc-no mono"` → `div class="text-2xl font-extrabold tracking-tight flex items-center gap-3.5 font-mono"`。

- [ ] **Step 6: 迁移 fms_expense_create.rs 和 fms_expense_detail.rs 中的 detail-header**

文件 `abt-web/src/pages/fms_expense_create.rs`。

行 104 的 `div class="detail-header"` → `div class="block bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-card"`。

行 105 的 `h1 class="detail-no"` → `h1 class="text-2xl font-extrabold tracking-tight bg-gradient-to-br from-fg to-fg-2 bg-clip-text text-transparent"`。

文件 `abt-web/src/pages/fms_expense_detail.rs`。

行 92 的 `div.detail-header` → 同上替换。

行 93 的 `div.detail-title-row` → `div class="flex items-center justify-between mb-4"`。

行 94 的 `h1.detail-no` → 同上行 105 替换。

- [ ] **Step 7: 迁移 mes_batch_detail.rs 中的 detail-doc-no**

文件 `abt-web/src/pages/mes_batch_detail.rs`。

行 182 的 `div class="batch-detail-title-row"` → `div class="flex items-center justify-between mb-5"`。

行 183 的 `div class="detail-doc-no"` → `div class="text-xl font-bold text-fg flex items-center gap-3"`。

- [ ] **Step 8: 迁移 om_outsourcing_detail.rs 中的 detail-doc-no + detail-meta + detail-actions**

文件 `abt-web/src/pages/om_outsourcing_detail.rs`。

搜索该文件中的 `detail-doc-no`、`detail-meta`、`detail-actions`、`detail-title-row` class 引用：

```rust
// detail-title-row →
div class="flex items-start justify-between gap-5"

// detail-doc-no →
div class="text-2xl font-bold text-fg flex items-center gap-3.5 tracking-tight"

// detail-meta →
div class="flex items-center gap-2.5 mt-2.5 ml-[58px]"

// detail-actions →
div class="flex gap-2 shrink-0"
```

- [ ] **Step 9: 迁移 category_list.rs 中的 detail-section**

文件 `abt-web/src/pages/category_list.rs`。

该文件有内联 `<style>` 块（行 469-571）中定义了 `.info-card` 和 `.detail-section` 等——这些是页面级 scoped CSS，需一并删除并替换为原子 class。

行 553-571 的 `.detail-section`、`.detail-section-header`、`.detail-section-title`、`.detail-section-count` 从内联 style 删除。

模板中的引用替换为原子 class：
```rust
// detail-section →
div class="mb-6"

// detail-section-header →
div class="flex items-center justify-between mb-3"

// detail-section-title →
span class="inline text-base font-semibold text-fg"

// detail-section-count →
span class="inline text-xs text-muted"
```

同时删除该文件内联 style 中的 `.info-card` 定义（行 469-472）。

- [ ] **Step 10: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 8: 迁移 detail-tabs / detail-tab（base.css 行 4187-4191）

**Files:**
- Modify: `static/base.css:4187-4191`
- Modify: `abt-web/src/components/detail.rs`（已在 Task 1 处理）
- Modify: 使用 detail-tabs 的页面文件

- [ ] **Step 1: 删除 base.css 行 4187-4191**

删除：
```css
/* detail_tabs component (S3) — tab-panel display controlled by inline style, NOT css */
.detail-tabs{display:flex;border-bottom:1px solid var(--border-soft);margin-bottom:var(--space-6)}
.detail-tab{padding:var(--space-3) var(--space-5);font-size:var(--text-sm);color:var(--muted);border:none;background:none;border-bottom:2px solid transparent;cursor:pointer;transition:all .15s;white-space:nowrap;font-weight:500}
.detail-tab:hover{color:var(--fg);background:var(--accent-bg)}
.detail-tab.active{color:var(--accent);border-bottom-color:var(--accent);font-weight:600}
```

- [ ] **Step 2: 搜索使用 detail-tabs 的页面并迁移**

使用 search 工具搜索 `abt-web/src` 中的 `detail-tabs` 和 `detail-tab`。

对直接在模板中写 `div class="detail-tabs"` 的页面（非使用组件函数），替换 class 为：
```rust
// detail-tabs →
div class="flex border-b border-border-soft mb-6"

// detail-tab (inactive) →
button class="px-5 py-3 text-sm text-muted border-none bg-transparent border-b-2 border-transparent cursor-pointer transition-all whitespace-nowrap font-medium hover:text-fg hover:bg-accent-bg"

// detail-tab active → （在 class 串末尾追加）
text-accent border-accent font-semibold
```

> 注意：`detail_tabs` 组件函数（在 detail.rs / info_detail.rs 中）已使用原子 class，调用组件函数的页面不需要修改。

- [ ] **Step 3: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 9: 迁移 FMS scoped 覆盖（fms-form-page/fms-list-page .info-card）

**Files:**
- Modify: `static/base.css:3227-3250`（删除 fms-form-page .info-card scoped 覆盖）
- Modify: `static/base.css:3528-3548`（删除 fms-list-page .info-card scoped 覆盖）
- Modify: FMS 相关 Maud 页面

- [ ] **Step 1: 删除 base.css 行 3227-3250（fms-form-page scoped）**

删除：
```css
/* Info card — glassmorphism + left accent bar */
.fms-form-page .info-card {
  background: rgba(255, 255, 255, 0.72);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  ...
}
.fms-form-page .info-card::before {
  content: '';
  position: absolute;
  ...
}
.fms-form-page .info-card-title {
  font-weight: 600;
  color: var(--fg);
  letter-spacing: 0.01em;
  ...
}
```

- [ ] **Step 2: 删除 base.css 行 3528-3548（fms-list-page scoped）**

删除：
```css
.fms-list-page .info-card {
  backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
  background: rgba(255,255,255,0.88);
  border: 1px solid rgba(255,255,255,0.6);
  ...
}
.fms-list-page .info-card-title { position: relative; padding-left: 16px; font-weight: 600; margin-bottom: var(--space-5); }
.fms-list-page .info-card-title::before {
  content: ''; position: absolute; left: 0; top: 50%;
  transform: translateY(-50%); width: 4px; height: 60%;
  border-radius: 2px; background: linear-gradient(180deg, var(--accent), #7c3aed);
}
.fms-list-page .info-card .data-table thead th { ... }
```

- [ ] **Step 3: 在 FMS 页面中使用 FMS 专属原子 class**

搜索 FMS 相关页面（`fms_*.rs`）中的 `info-card` class，替换为 FMS 毛玻璃版原子组合：

```rust
// FMS info-card →
div class="backdrop-blur-md bg-white/88 border border-white/60 rounded-md p-6 mb-6 shadow-lg" { ... }

// FMS info-card-title →
div class="relative pl-4 font-semibold text-fg mb-5
           before:content-[''] before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2
           before:w-1 before:h-[60%] before:rounded-sm
           before:bg-gradient-to-b before:from-accent before:to-[#7c3aed]" { ... }
```

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 10: 迁移 info-progress / detail-info-split / info-key-* / info-detail-row

**Files:**
- Modify: `static/base.css:2817-2833`（删除 outsourcing detail info split）
- Modify: `static/base.css:2927-2929`（删除响应式）
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`

- [ ] **Step 1: 删除 base.css 行 2817-2833**

删除：
```css
/* ─── Outsourcing Detail: Info Split ─── */
.detail-info-split { display: grid; grid-template-columns: 1fr auto; gap: 0; margin-top: 28px; padding-top: 24px; border-top: 1px solid var(--border-soft); }
.info-key-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px 48px; }
.info-key-item { display: flex; flex-direction: column; gap: 6px; }
.info-key-label { font-size: 12px; color: var(--muted); font-weight: 600; letter-spacing: 0.04em; }
.info-key-value { font-size: 15px; color: var(--fg); font-weight: 600; line-height: 1.4; }
.info-key-value.mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
.info-progress { display: flex; flex-direction: column; align-items: center; justify-content: center; padding-left: 40px; border-left: 1px solid var(--border-soft); min-width: 130px; }
```

同时删除行 2831-2833 的 `.info-detail-row` 和 `.info-detail-chip`。

- [ ] **Step 2: 删除 base.css 行 2927-2929（响应式）**

删除 outsourcing 相关响应式 @media 规则。

- [ ] **Step 3: 迁移 om_outsourcing_detail.rs 中的原子 class**

```rust
// detail-info-split →
div class="grid grid-cols-[1fr_auto] mt-7 pt-6 border-t border-border-soft md:grid-cols-1"

// info-key-grid →
div class="grid grid-cols-3 gap-5 gap-x-12 md:grid-cols-2 sm:grid-cols-1"

// info-key-item →
div class="flex flex-col gap-1.5"

// info-key-label →
span class="text-xs text-muted font-semibold tracking-wide"

// info-key-value →
span class="text-[15px] text-fg font-semibold leading-tight"

// info-key-value.mono →
span class="text-[15px] text-fg font-semibold leading-tight font-mono tabular-nums"

// info-progress →
div class="flex flex-col items-center justify-center pl-10 border-l border-border-soft min-w-[130px] md:border-l-0 md:border-t md:pt-5 md:pl-0"

// info-detail-row →
div class="flex flex-wrap gap-2 gap-x-6 mt-5 pt-4 border-t border-dashed border-border-soft md:flex-col md:gap-1.5"

// info-detail-chip →
span class="flex items-baseline gap-1.5 text-xs text-muted"
```

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 11: 构建 CSS 并验证

- [ ] **Step 1: 重新构建 CSS**

Run: `cd E:/work/abt && npm run build:css`

Expected: 成功生成 app.css

- [ ] **Step 2: 验证 app.css 不再包含 info-card/detail-grid 等 class 定义**

使用 search 工具搜索 `static/app.css` 中的 `info-card`、`detail-grid`、`detail-card`。

Expected: 无匹配（这些 class 已从 base.css 删除，Maud 中也不再使用）

- [ ] **Step 3: 用 agent-browser 验证详情页渲染**

Run:
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/sales/customers/1"
agent-browser --cdp 9222 eval "JSON.stringify({
  cardCount: document.querySelectorAll('[class*=bg-bg][class*=border][class*=rounded-md]').length,
  gridCount: document.querySelectorAll('[class*=grid-cols]').length
})"
```

验证代表性页面：
- `/admin/sales/customers/1` — 客户详情页（detail-grid 3 列）
- `/admin/mes/batches/1` — 批次详情（detail-doc-no + detail-info-grid-5）
- `/admin/fms/expenses/1` — 费用详情（detail-header + info-card + info-grid）
- `/admin/master-data/work-centers/1` — 工作中心（info-card + info-section-title）
- `/admin/bom/1` — BOM 详情（detail-card + detail-top）

- [ ] **Step 4: cargo clippy 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 12: 提交

- [ ] **Step 1: Git 提交**

```bash
cd E:/work/abt && git add abt-web/src/components/info_detail.rs abt-web/src/components/detail.rs abt-web/src/components/mod.rs abt-web/src/pages/*.rs static/base.css static/app.css && git commit -m "refactor(css): P5 — migrate info-card/detail-grid/detail-tabs to atomic UnoCSS

- Add info_detail.rs component with info_card/info_grid/info_item/detail_row/detail_tabs helpers
- Replace 43 info-*/detail-* class variants across all detail pages
- Remove 4 duplicate info-card definitions + 3 duplicate detail-grid definitions
- Remove FMS scoped overrides (fms-form-page/fms-list-page .info-card)
- Remove outsourcing detail-info-split + info-key-* + info-detail-row
- Add responsive atomic prefixes (lg:/md:) replacing @media rules"
```
