# P3: Data Card + Table + Filter 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 base.css 中 14 个数据展示 class（data-card / data-card-scroll / data-card-head / data-card-body / data-card-header / data-card-title / data-table / create-action-bar / filter-bar / filter-select / filter-date / filter-check / search-wrap / search-input）迁移为 UnoCSS 原子 class，并从 base.css 中删除对应定义。

**Architecture:** 这批 class 是列表页的核心骨架，分布在 140+ 个 Maud 文件中。策略：按功能分 4 个 Task——(1) data-card 族（容器 + 头部）、(2) data-table 族（表格 + 行 hover）、(3) filter-bar 族（搜索/筛选/日期/复选框）、(4) create-action-bar（底部固定操作栏）。每个 Task 先迁移 class、再删 CSS、再验证。最后处理 FMS scoped 覆盖。

**Tech Stack:** UnoCSS v66.7.0, presetWind4, Maud, Rust

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

**前置条件:** P0 已完成。P1 建议先完成（form-section-title / form-grid 在 data-card 内使用，P1 迁移后此处不再有 `.data-card:has(> .form-section-title)` 的依赖）。

---

## CSS → 原子 class 映射表

### Data Card 族

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `data-card` | 2613 | `background:var(--bg); border:1px solid var(--border-soft); border-radius:var(--radius-lg); box-shadow:var(--shadow-card); margin-bottom:var(--space-4); transition:box-shadow 240ms` | `bg-white border border-border-soft rounded-lg shadow-[0_1px_3px_rgba(15,23,42,0.04),0_0_0_1px_rgba(15,23,42,0.03)] mb-4 transition-shadow duration-240` |
| `data-card` 含表单时自动 padding | 510-512 | `.data-card:has(> .form-section-title)` / `:has(> form)` → `padding:var(--space-5)` | P1 迁移 form-section-title 后 `:has()` 选择器失效。策略：在 Maud 中含表单的 data-card 直接追加 `p-5`。原子 class 追加: `p-5` |
| `data-card > form > .create-action-bar:last-child` | 514-518 | 抵消父级 padding（margin 负值铺满底部） | 在 data-card 含 form 时，create-action-bar 追加 `-mx-5 -mb-5`（抵消父级 p-5） |
| `data-card-scroll` | 500-502 | `overflow-x:auto; -webkit-overflow-scrolling:touch; scrollbar-width:thin; scrollbar-color:var(--border) transparent` + webkit scrollbar 伪元素 | `overflow-x-auto overscroll-x-contain` + scrollbar 自定义需要 `scrollbar-thin scrollbar-track-transparent scrollbar-thumb-border`（UnoCSS scrollbar 插件，若不可用则 `style="scrollbar-width:thin;scrollbar-color:var(--border) transparent"`） |
| `data-card-head` | 503-506 | `padding:var(--space-4) var(--space-5); border-bottom:1px solid var(--border-soft); display:flex; justify-content:space-between; align-items:center` | `flex justify-between items-center px-5 py-4 border-b border-border-soft` |
| `data-card-head h3` | 507 | `font-size:var(--text-base); font-weight:600; color:var(--fg); margin:0` | h3 上: `text-base font-semibold text-fg m-0` |
| `data-card-body` | 508 | `padding:var(--space-4)` | `p-4` |
| `data-card-header` | 2727 | `display:flex; align-items:center; justify-content:space-between; margin-bottom:var(--space-4)` | `flex items-center justify-between mb-4` |
| `data-card-title` | 2728 | `font-size:var(--text-base); font-weight:600; color:var(--fg)` | `text-base font-semibold text-fg` |

### Data Table 族

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `data-table` | 519 | `width:100%; min-width:860px; font-size:var(--text-sm)` | `w-full min-w-[860px] text-sm` |
| `data-table th` | 520-524 | `padding:11px var(--space-4); text-align:left; font-weight:600; color:var(--muted); background:var(--surface-raised); font-size:12px; white-space:nowrap; border-bottom:1px solid var(--border-soft); text-transform:uppercase; letter-spacing:0.04em` | th 上: `py-[11px] px-4 text-left font-semibold text-muted bg-surface-raised text-xs whitespace-nowrap border-b border-border-soft uppercase tracking-[0.04em]` |
| `data-table td` | 525 | `padding:13px var(--space-4); border-bottom:1px solid var(--border-soft); vertical-align:middle; white-space:nowrap` | td 上: `py-[13px] px-4 border-b border-border-soft align-middle whitespace-nowrap` |
| `data-table tbody tr` | 527 | `counter-increment:line-item; transition:all 150ms; cursor:pointer` | tbody tr: 追加 `cursor-pointer transition-all duration-150`（counter-reset 在 tbody 上） |
| `data-table tbody tr:hover` | 528 | `background:var(--accent-bg)` | 追加: `hover:bg-accent-bg` |
| `data-table tbody tr:last-child td` | 529 | `border-bottom:none` | 追加: `last:border-b-0`（在 td 上） |
| `data-table .empty-cell` | 530 | `text-align:center; padding:var(--space-6); color:var(--muted)` | `text-center p-6 text-muted` |
| `data-table .mono` | 531 | `font-family:var(--font-mono); font-variant-numeric:tabular-nums` | `font-mono tabular-nums` |
| `data-table .link-cell` | 532-536 | `color:var(--accent); font-weight:600; font-family:var(--font-mono); font-variant-numeric:tabular-nums; transition:color 150ms` + hover | `text-accent font-semibold font-mono tabular-nums transition-colors duration-150 hover:text-accent-hover` |
| `data-table .row-actions` | 537 | `opacity:0; transition:opacity 150ms; display:flex; gap:var(--space-1)` | `opacity-0 transition-opacity duration-150 flex gap-1 group-hover:opacity-100`（需父级 tr 加 `group`） |
| `data-table tbody tr:hover .row-actions` | 538 | `opacity:1` | 配合 group 模式 |
| `@media (max-width:768px)` data-table | 1522-1524 | `font-size:13px; th padding:9px var(--space-3); td padding:10px var(--space-3)` | 响应式前缀: `md:text-[13px]`（th: `md:py-2 md:px-3`，td: `md:py-2.5 md:px-3`） |

### Filter Bar 族

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `filter-bar` | 398 | `display:flex; align-items:center; gap:var(--space-3); margin-bottom:var(--space-6); flex-wrap:wrap` | `flex items-center gap-3 mb-6 flex-wrap md:flex-wrap` |
| `search-wrap` | 406 | `position:relative` | `relative` |
| `search-wrap svg` | 407 | `position:absolute; left:12px; top:50%; transform:translateY(-50%); width:16px; height:16px; color:var(--muted)` | svg 上: `absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted` |
| `search-input` | 399-405 | `width:260px; padding:8px 14px 8px 38px; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--bg); font-size:var(--text-sm); color:var(--fg); outline:none; transition:all 150ms` + focus + placeholder | `w-[260px] py-2 pl-[38px] pr-3.5 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)] placeholder:text-muted placeholder:opacity-70 md:w-full` |
| `filter-select` | 408-417 | `padding:8px 32px 8px 14px; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--bg) + 下拉箭头 SVG; font-size:var(--text-sm); color:var(--fg); outline:none; transition:all 150ms; appearance:none` + hover + focus | `py-2 pl-3.5 pr-8 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 cursor-pointer appearance-none hover:border-accent focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]` + 下拉箭头用 inline style background-image |
| `filter-date` | 418-425 | `padding:8px 10px; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--bg); font-size:var(--text-sm); color:var(--fg); outline:none; transition:all 150ms; cursor:pointer` + hover + focus | `py-2 px-2.5 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 cursor-pointer hover:border-accent focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]` |
| `filter-check` | 426-431 | `display:inline-flex; align-items:center; gap:var(--space-1); font-size:var(--text-sm); color:var(--fg); cursor:pointer; white-space:nowrap` + input checkbox | `inline-flex items-center gap-1 text-sm text-fg cursor-pointer whitespace-nowrap`（input: `w-[15px] h-[15px] cursor-pointer accent-accent`） |
| `@media (max-width:768px)` filter-bar | 1520-1521 | `filter-bar: flex-wrap; search-input: width:100%` | 已在 filter-bar 上加 `flex-wrap`，search-input 上加 `md:w-full` |

### Create Action Bar

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `create-action-bar` | 973-978 | `display:flex; align-items:center; justify-content:flex-end; gap:var(--space-3); padding:var(--space-4) var(--space-6); background:var(--bg); border-top:1px solid var(--border); position:sticky; bottom:0; box-shadow:0 -2px 8px rgba(0,0,0,0.04)` | `flex items-center justify-end gap-3 py-4 px-6 bg-white border-t border-border sticky bottom-0 shadow-[0_-2px_8px_rgba(0,0,0,0.04)]` |

### FMS scoped 覆盖

| 选择器 | base.css 行 | 当前 CSS | 处理策略 |
|---|---|---|---|
| `.fms-list-page .filter-bar` | 3460-3467 | 毛玻璃背景 `rgba(255,255,255,0.7) + backdrop-blur(12px) + border + radius-lg + shadow-xs` | FMS list 页的 filter-bar 追加 `backdrop-blur-md bg-white/70 border border-white/50 rounded-lg shadow-xs` |
| `.fms-list-page .data-card` | 3468-3476 | `bg:rgba(255,255,255,0.95) + backdrop-blur(12px) + border + radius-lg + shadow-md + overflow:hidden` | FMS list 页的 data-card 替换为 FMS 专属原子串 |
| `.fms-list-page .data-table thead th` | 3477-3487 | 渐变背景 + 字重/颜色/padding 覆盖 | FMS 页的 th 追加 FMS 专属原子串 |
| `.fms-list-page .data-table tbody tr:hover` | 3489 | `background:rgba(37,99,235,0.03)` | FMS 页的 tr hover 追加 `hover:bg-[rgba(37,99,235,0.03)]` |
| `.fms-list-page .data-table tbody td` | 3490 | `padding:14px 16px; font-size:13px` | FMS 页的 td 追加 FMS padding |
| `.fms-list-page .search-wrap` | 3493-3494 | 毛玻璃 + border + radius-md + focus-within | FMS 页的 search-wrap 追加 FMS 原子串 |
| `.fms-list-page .search-input` | 3495 | `background:transparent` | FMS 页的 search-input 追加 `bg-transparent` |
| `.fms-list-page .filter-select` | 3496-3497 | 毛玻璃 + border + radius-md + focus | FMS 页的 filter-select 追加 FMS 原子串 |

---

## 涉及文件清单（140+ 个）

### 组件文件（4 个）
- `abt-web/src/components/category_select.rs`
- `abt-web/src/components/customer_info.rs`
- `abt-web/src/components/entity_picker.rs`
- `abt-web/src/components/product_picker.rs`

### 页面文件

**含 data-card 的文件（列表 + 详情 + 表单页面，约 120 个）**：
所有 `*_list.rs`、`*_detail.rs`、`*_create.rs`、`*_edit.rs` 文件。

**含 data-table 的文件（列表页，约 90 个）**：
bom_list.rs, customer_list.rs, product_list.rs, supplier_list.rs, quotation_list.rs, sales_order_list.rs, purchase_order_list.rs, purchase_quotation_list.rs, purchase_return_list.rs, purchase_recon_list.rs, sales_return_list.rs, shipping_list.rs, reconciliation_list.rs, role_list.rs, user_list.rs, fms_expense_list.rs, fms_journal_list.rs, fms_writeoff_list.rs, fms_cost_analysis.rs, mes_order_list.rs, mes_plan_list.rs, mes_receipt_list.rs, mes_report_list.rs, mes_inspection_list.rs, mes_exception_list.rs, mes_wage_list.rs, misc_request_list.rs, om_outsourcing_list.rs, om_tracking_list.rs, payment_request_list.rs, qms_mrb_list.rs, qms_result_list.rs, qms_rma_list.rs, qms_spec_list.rs, routing_list.rs, wms_arrival_list.rs, wms_backflush_list.rs, wms_bin_list.rs, wms_cascade_list.rs, wms_conversion_list.rs, wms_cycle_count_list.rs, wms_lock_list.rs, wms_requisition_list.rs, wms_stock_in_list.rs, wms_stock_list.rs, wms_stock_out_list.rs, wms_strategy_list.rs, wms_transaction_log_list.rs, wms_transfer_list.rs, wms_warehouse_list.rs 等

**含 filter-bar / search-wrap / search-input / filter-select 的文件（列表页，约 60 个）**：
bom_list.rs, customer_list.rs, product_list.rs, supplier_list.rs, quotation_list.rs, sales_order_list.rs, purchase_order_list.rs, purchase_quotation_list.rs, fms_expense_list.rs, fms_journal_list.rs, mes_order_list.rs, mes_plan_list.rs, mes_exception_list.rs, mes_inspection_list.rs, misc_request_list.rs, om_outsourcing_list.rs, qms_mrb_list.rs, qms_result_list.rs, qms_rma_list.rs, qms_spec_list.rs, reconciliation_list.rs, wms_arrival_list.rs, wms_stock_list.rs 等

**含 create-action-bar 的文件（表单页，约 40 个）**：
bom_create.rs, customer_create.rs, customer_edit.rs, fms_journal_create.rs, fms_expense_create.rs, mes_demand_pool_create.rs, mes_inspection_create.rs, mes_order_create.rs, mes_plan_create.rs, mes_receipt_create.rs, mes_report_create.rs, om_outsourcing_create.rs, payment_request_create.rs, purchase_order_create.rs, purchase_order_edit.rs, purchase_quotation_create.rs, purchase_recon_create.rs, purchase_return_create.rs, purchase_demand_pool_create.rs, purchase_settings.rs, qms_mrb_create.rs, qms_result_create.rs, qms_rma_create.rs, qms_spec_create.rs, quotation_create.rs, quotation_edit.rs, reconciliation_create.rs, sales_order_create.rs, sales_order_edit.rs, sales_return_create.rs, shipping_create.rs, supplier_create.rs, supplier_edit.rs, wms_arrival_create.rs, wms_conversion_create.rs, wms_cycle_count_create.rs, wms_requisition_create.rs, wms_stock_in_create.rs, wms_stock_out_create.rs, wms_transfer_create.rs 等

---

### Task 1: Data Card 族（data-card / data-card-scroll / data-card-head / data-card-body / data-card-header / data-card-title）

**Files:**
- Modify: 全部含这些 class 的 Maud 文件（约 120 个）
- Modify: `static/base.css:499-518`（data-card-scroll / data-card-head / data-card-body + :has 规则）
- Modify: `static/base.css:2613`（data-card 基础定义）
- Modify: `static/base.css:2726-2728`（data-card-header / data-card-title）

- [ ] **Step 1: 替换 data-card**

全局搜索 `class="data-card"`。

**标准列表页 data-card**（无表单内容，内含 data-card-scroll + data-table）：
```
class="bg-white border border-border-soft rounded-lg shadow-[0_1px_3px_rgba(15,23,42,0.04),0_0_0_1px_rgba(15,23,42,0.03)] mb-4 transition-shadow duration-240"
```

**含表单内容的 data-card**（内含 form-section-title 或 form，P1 迁移后 form-section-title 已是原子 class）：
追加 `p-5`：
```
class="bg-white border border-border-soft rounded-lg shadow-[0_1px_3px_rgba(15,23,42,0.04),0_0_0_1px_rgba(15,23,42,0.03)] mb-4 transition-shadow duration-240 p-5"
```

判断标准：如果 data-card 内直接包含 `form-section-title`（已是原子 class `text-sm font-semibold ...`）或 `<form>` 标签，则加 `p-5`。

注意：部分 data-card 有额外 inline style（如 `style="padding:0;overflow:hidden"` 或 `style="margin-bottom:var(--space-4)"`）。对于 `style="padding:0"`，原子 class 不加 p-5，保留 `overflow-hidden`（转为 `overflow-hidden`）。对于 `style="margin-bottom:var(--space-4)"`，已被 mb-4 覆盖，删除 inline style。

- [ ] **Step 2: 替换 data-card-scroll**

全局搜索 `class="data-card-scroll"`，替换为：
```
class="overflow-x-auto overscroll-x-contain"
```

注意：`-webkit-overflow-scrolling:touch` 在现代浏览器已默认启用。scrollbar 样式（thin/border color）如果 UnoCSS scrollbar 插件不可用，暂时省略（影响很小，默认 scrollbar 也能用），或保留一条 inline style：
```rust
div class="overflow-x-auto overscroll-x-contain" style="scrollbar-width:thin;scrollbar-color:var(--border) transparent" {
```

**推荐方案**：保留 inline style 处理 scrollbar，因为这是 webkit 伪元素无法用原子 class 表达。

- [ ] **Step 3: 替换 data-card-head**

全局搜索 `class="data-card-head"`，替换为：
```
class="flex justify-between items-center px-5 py-4 border-b border-border-soft"
```

data-card-head 内的 `<h3>` → 追加 `class="text-base font-semibold text-fg m-0"`

- [ ] **Step 4: 替换 data-card-body**

全局搜索 `class="data-card-body"`，替换为：
```
class="p-4"
```

- [ ] **Step 5: 替换 data-card-header**

全局搜索 `class="data-card-header"`，替换为：
```
class="flex items-center justify-between mb-4"
```

- [ ] **Step 6: 替换 data-card-title**

全局搜索 `class="data-card-title"`，替换为：
```
class="text-base font-semibold text-fg"
```

- [ ] **Step 7: 从 base.css 删除**

删除：
- 行 499-518（data-card-scroll + 伪元素 + data-card-head + h3 + data-card-body + :has 规则 + create-action-bar 抵消规则）
- 行 2613（data-card 基础定义）
- 行 2726-2728（data-card-header + data-card-title）

注意：行 2614-2616（cell-stack / text-warn）不属于本批次，保留。

- [ ] **Step 8: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 9: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 10: 页面验证**

用 agent-browser 打开：
- `/admin/customers` — data-card 白色卡片 + 阴影 + 圆角
- `/admin/bom` — data-card + data-card-scroll 横向滚动
- `/admin/customers/new` — 含表单的 data-card 带 p-5 padding
- `/admin/fms/expenses/{id}` — data-card-head（如有）

检查点：卡片阴影、边框、圆角、滚动条样式、含表单时的内边距。

---

### Task 2: Data Table 族（data-table / th / td / tr hover / row-actions）

**Files:**
- Modify: 全部含 `class="data-table"` 的 Maud 文件（约 90 个）
- Modify: `static/base.css:519-538`（data-table 全块）
- Modify: `static/base.css:1522-1524`（@media data-table 响应式）

- [ ] **Step 1: 替换 data-table（table 标签）**

全局搜索 `class="data-table"`，替换为：
```
class="w-full min-w-[860px] text-sm md:text-[13px]"
```

注意：部分 data-table 有 inline `style="min-width:900px"` 等（如 fms_cost_analysis.rs），保留该 inline style 覆盖 min-width。或合并为 `min-w-[900px]`。

- [ ] **Step 2: 替换 data-table th**

data-table 内的 `<th>` 如果没有 class，添加：
```
class="py-[11px] px-4 text-left font-semibold text-muted bg-surface-raised text-xs whitespace-nowrap border-b border-border-soft uppercase tracking-[0.04em] md:py-2 md:px-3"
```

如果 th 已有 inline style（如 `style="width:30%"`），保留 inline style，仅添加 class。

注意：这是一个高频操作（每个 data-table 有 5-10 个 th），可以考虑在 Maud 中提取为变量：
```rust
let th_cls = "py-[11px] px-4 text-left font-semibold text-muted bg-surface-raised text-xs whitespace-nowrap border-b border-border-soft uppercase tracking-[0.04em] md:py-2 md:px-3";
```
然后在每个 th 上用 `class=(th_cls)`。

- [ ] **Step 3: 替换 data-table td**

data-table 内的 `<td>` 如果没有 class，添加：
```
class="py-[13px] px-4 border-b border-border-soft align-middle whitespace-nowrap md:py-2.5 md:px-3 last:border-b-0"
```

注意：由于 `tbody tr:last-child td { border-bottom:none }` 是后代选择器，原子化后需在每个 td 上加 `last:border-b-0`。但这意味着最后一个 td 的 class 需要特殊处理。

**替代方案**：在 `tbody tr` 上加 `last:border-b-0`——但这不对，因为 border 在 td 上不在 tr 上。

**正确方案**：在 `tbody` 上加 `[&>tr:last-child>td]:border-b-0`（UnoCSS arbitrary variant）：
```
table class="w-full min-w-[860px] text-sm md:text-[13px] [&>tbody>tr:last-child>td]:border-b-0"
```
这样所有 td 保持统一的 `py-[13px] px-4 border-b border-border-soft align-middle whitespace-nowrap`，最后一行通过父级选择器去掉 border。

或者简化：不在 td 上加 border class，而是在 thead th 上加 `border-b border-border-soft`，tbody 行之间用 `border-b border-border-soft` 加在每个 tr 上（tr 加 `border-b`）。这样最后一行可以直接 `last:border-b-0`。

**最终方案（推荐）**：
- thead th 上: `py-[11px] px-4 ... border-b border-border-soft ...`
- tbody tr 上: `border-b border-border-soft cursor-pointer transition-all duration-150 hover:bg-accent-bg last:border-b-0`
- td 上: `py-[13px] px-4 align-middle whitespace-nowrap md:py-2.5 md:px-3`

将 border 从 td 移到 tr，简化每个 td 的 class。

- [ ] **Step 4: 替换 data-table tbody tr**

在 `tbody tr` 上添加（如果没有 class）：
```
class="border-b border-border-soft cursor-pointer transition-all duration-150 hover:bg-accent-bg last:border-b-0"
```

注意：并非所有 tr 都需要 `cursor:pointer`（只有可点击行才需要）。如果表格行不可点击（纯展示），省略 `cursor-pointer`。判断标准：如果页面中有行点击跳转逻辑（hx-get / onclick），则加 cursor-pointer。

- [ ] **Step 5: 替换 data-table 辅助 class**

`.empty-cell` → `class="text-center p-6 text-muted"`

`.mono` → `class="font-mono tabular-nums"`

`.link-cell` → `class="text-accent font-semibold font-mono tabular-nums transition-colors duration-150 hover:text-accent-hover"`

- [ ] **Step 6: 替换 row-actions（hover 显示）**

`.row-actions` 当前是 `opacity:0`，tr hover 时 `opacity:1`。

原子化策略（group 模式）：在 `tbody tr` 上追加 `group`：
```
class="group border-b border-border-soft cursor-pointer transition-all duration-150 hover:bg-accent-bg last:border-b-0"
```

row-actions 元素上：
```
class="opacity-0 transition-opacity duration-150 flex gap-1 group-hover:opacity-100"
```

- [ ] **Step 7: 从 base.css 删除**

删除：
- 行 519-538（data-table + th + td + tbody + tr + hover + last-child + empty-cell + mono + link-cell + row-actions 全块）
- 行 1522-1524（@media data-table 响应式）

- [ ] **Step 8: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 9: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 10: 页面验证**

打开列表页：
- `/admin/customers` — data-table 表头灰色背景、大写、字间距；行 hover 蓝色背景；最后一行无 border
- `/admin/bom` — 行 hover + row-actions 按钮显示
- `/admin/products` — 表格字体、padding

检查点：th 背景色、uppercase、letter-spacing；td padding；hover 背景；窗口缩小到 768px 字体变小。

---

### Task 3: Filter Bar 族（filter-bar / search-wrap / search-input / filter-select / filter-date / filter-check）

**Files:**
- Modify: 全部含这些 class 的 Maud 文件（约 60 个列表页）
- Modify: `static/base.css:397-431`（filter-bar 全块）
- Modify: `static/base.css:1520-1521`（@media filter-bar 响应式）

- [ ] **Step 1: 替换 filter-bar**

全局搜索 `class="filter-bar"`。

注意：大多数 filter-bar 同时有多个 class，如 `class="filter-bar filter-form"`。其中 `filter-form` 是 HTMX 表单的语义标记（无 CSS 定义），保留。

`class="filter-bar filter-form"` → `class="flex items-center gap-3 mb-6 flex-wrap filter-form"`

（保留 `filter-form` 作为 HTMX id 关联的语义标记。）

- [ ] **Step 2: 替换 search-wrap**

全局搜索 `class="search-wrap"`，替换为：
```
class="relative"
```

search-wrap 内的 svg → 追加 `class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted"`（如果 svg 已有 class 如 `w-4 h-4`，合并为完整串）。

- [ ] **Step 3: 替换 search-input**

全局搜索 `class="search-input"`，替换为：
```
class="w-[260px] py-2 pl-[38px] pr-3.5 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)] placeholder:text-muted placeholder:opacity-70 md:w-full"
```

注意：部分 search-input 有 inline `style="width:200px"`（如 fms_journal_list.rs），替换为 `w-[200px]` 并删除 inline style。

- [ ] **Step 4: 替换 filter-select**

全局搜索 `class="filter-select"`。

filter-select 的下拉箭头 SVG 通过 CSS background-image 实现，原子化后需要用 inline style 保留：
```
class="py-2 pl-3.5 pr-8 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 cursor-pointer appearance-none hover:border-accent focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]"
style="background-image:url(\"data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%23697386' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E\");background-repeat:no-repeat;background-position:right 12px center"
```

注意：这个 background-image inline style 在每个 filter-select 上重复出现。可以在 Maud 中提取为变量：
```rust
let filter_select_cls = "py-2 pl-3.5 pr-8 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 cursor-pointer appearance-none hover:border-accent focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]";
let filter_select_arrow = "background-image:url(\"data:image/svg+xml,...\");background-repeat:no-repeat;background-position:right 12px center";
```

由于下拉箭头是所有 select 的通用需求，也可以考虑将其定义到 uno.config.ts 的 rule 中（但设计文档要求零手写 CSS）。折中方案：保留 inline style。

- [ ] **Step 5: 替换 filter-date**

全局搜索 `class="filter-date"`，替换为：
```
class="py-2 px-2.5 border border-border rounded-sm bg-white text-sm text-fg outline-none transition-all duration-150 cursor-pointer hover:border-accent focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]"
```

- [ ] **Step 6: 替换 filter-check**

全局搜索 `class="filter-check"`，替换为：
```
class="inline-flex items-center gap-1 text-sm text-fg cursor-pointer whitespace-nowrap"
```

filter-check 内的 `<input type="checkbox">` → 追加 `class="w-[15px] h-[15px] cursor-pointer accent-accent"`

- [ ] **Step 7: 从 base.css 删除**

删除：
- 行 397-431（filter-bar + search-input + search-wrap + filter-select + filter-date + filter-check 全块）
- 行 1520-1521（@media filter-bar 响应式）

- [ ] **Step 8: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 9: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 10: 页面验证**

打开列表页：
- `/admin/customers` — filter-bar + search-wrap（搜索图标在 input 左侧）+ search-input focus 蓝色环 + filter-select 下拉箭头
- `/admin/bom` — filter-date 日期选择器 + filter-check 复选框
- `/admin/fms/journals` — filter-bar 排列

检查点：搜索图标定位、input focus 效果、select 下拉箭头、日期选择器 hover、复选框大小。

---

### Task 4: Create Action Bar

**Files:**
- Modify: 全部含 `class="create-action-bar"` 的 Maud 文件（约 40 个表单页）
- Modify: `static/base.css:972-978`（create-action-bar 块）

- [ ] **Step 1: 替换 create-action-bar（标准版）**

全局搜索 `class="create-action-bar"`，替换为：
```
class="flex items-center justify-end gap-3 py-4 px-6 bg-white border-t border-border sticky bottom-0 shadow-[0_-2px_8px_rgba(0,0,0,0.04)]"
```

- [ ] **Step 2: 处理 data-card 内 form 的 create-action-bar 抵消 padding**

base.css 行 514-518 定义了 `.data-card > form > .create-action-bar:last-child` 的负 margin 抵消父级 padding。

P1 迁移后 form-section-title / data-card 已原子化。对于含表单的 data-card（追加了 `p-5`），其内 form 末尾的 create-action-bar 需要抵消 padding：

在 data-card 内 form 的 create-action-bar 上追加负 margin：
```
class="flex items-center justify-end gap-3 py-4 px-6 bg-white border-t border-border sticky bottom-0 shadow-[0_-2px_8px_rgba(0,0,0,0.04)] -mx-5 -mb-5"
```

判断标准：如果 create-action-bar 是 `data-card > form > .create-action-bar:last-child`（即在含 p-5 的 data-card 的 form 内的最后一个元素），则加 `-mx-5 -mb-5`。

涉及文件：customer_create.rs, customer_edit.rs, purchase_settings.rs 等含 data-card > form > create-action-bar 结构的页面。

- [ ] **Step 3: 从 base.css 删除**

删除：
- 行 972-978（create-action-bar 块）
- 行 514-518 已在 Task 1 Step 7 删除（data-card > form > .create-action-bar 抵消规则）

- [ ] **Step 4: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 5: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 6: 页面验证**

打开表单页：
- `/admin/customers/new` — 底部 create-action-bar 固定在视口底部，白色背景 + 上边框 + 阴影
- `/admin/bom/new` — create-action-bar 按钮排列
- `/admin/purchase/settings` — data-card 内的 create-action-bar 铺满底部边缘

检查点：sticky 定位、背景色、边框、阴影、抵消 padding 后铺满边缘。

---

### Task 5: FMS scoped 覆盖处理

**Files:**
- Modify: 含 `class="fms-list-page"` 或 `class="fms-form-page"` 的 FMS 页面文件
- Modify: `static/base.css:3460-3497`（FMS list 页面 filter-bar / data-card / data-table / search-wrap / filter-select 覆盖）
- Modify: `static/base.css:3545-3548`（FMS info-card data-table thead 覆盖）

- [ ] **Step 1: 搜索 FMS scoped 页面**

Run（用 search 工具）: 搜索 `abt-web/src` 中 `fms-list-page` 的引用，确认哪些文件使用 FMS scoped 覆盖。

- [ ] **Step 2: 替换 FMS list 页面的 filter-bar**

在 FMS list 页面中，`class="filter-bar filter-form"` 替换为 FMS 专属：
```
class="flex items-center gap-3 mb-6 flex-wrap filter-form backdrop-blur-md bg-white/70 border border-white/50 rounded-lg shadow-xs"
```

- [ ] **Step 3: 替换 FMS list 页面的 data-card**

在 FMS list 页面中，data-card 替换为 FMS 专属：
```
class="backdrop-blur-md bg-white/95 border border-[rgba(226,232,240,0.6)] rounded-lg shadow-md mb-4 overflow-hidden transition-shadow duration-240"
```

- [ ] **Step 4: 替换 FMS list 页面的 data-table thead th**

在 FMS list 页面的 th 上追加 FMS 专属样式：
```
class="... bg-[linear-gradient(180deg,#f8fafc_0%,#f1f5f9_100%)] text-[11px] font-bold px-4 py-3 ..."
```

（覆盖标准 th 的 background-surface-raised 和 padding。）

- [ ] **Step 5: 替换 FMS list 页面的 data-table tbody tr hover + td**

FMS 页面的 tr hover 追加：`hover:bg-[rgba(37,99,235,0.03)]`（替代标准的 hover:bg-accent-bg）

FMS 页面的 td 追加 FMS padding: `py-3.5 px-4 text-[13px]`

- [ ] **Step 6: 替换 FMS list 页面的 search-wrap + search-input**

search-wrap 追加：`backdrop-blur-md bg-white/80 border border-border rounded-md transition-all duration-150 focus-within:border-accent focus-within:shadow-[0_0_0_3px_rgba(37,99,235,0.1)] focus-within:bg-white/95`

search-input 追加：`bg-transparent`

- [ ] **Step 7: 替换 FMS list 页面的 filter-select**

filter-select 追加 FMS 毛玻璃：`backdrop-blur-md bg-white/80 border border-border rounded-md`

- [ ] **Step 8: 从 base.css 删除 FMS 覆盖**

删除：
- 行 3460-3497（全部 FMS list 页面 scoped 覆盖）
- 行 3545-3548（.fms-list-page .info-card .data-table thead th 覆盖）

- [ ] **Step 9: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 10: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 11: 页面验证**

打开 FMS list 页面：
- `/admin/fms/journals` — 毛玻璃 filter-bar + data-card + 渐变表头
- `/admin/fms/expenses` — FMS scoped 表格样式

检查点：毛玻璃效果、渐变表头、hover 颜色差异。

---

### Task 6: 最终构建与回归验证

**Files:**
- 无新文件修改

- [ ] **Step 1: 全量构建**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 2: 全量编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 3: 确认 base.css 已删除所有 P3 class**

Run（用 search 工具）: 在 `static/base.css` 中搜索以下 class（排除注释），确认全部不存在：
`data-card`, `data-card-scroll`, `data-card-head`, `data-card-body`, `data-card-header`, `data-card-title`, `data-table`, `create-action-bar`, `filter-bar`, `filter-select`, `filter-date`, `filter-check`, `search-wrap`, `search-input`

Expected: 全部 0 匹配。

- [ ] **Step 4: 回归验证 6 个代表性页面**

| 页面 | URL | 检查重点 |
|---|---|---|
| 客户列表 | `/admin/customers` | data-card + filter-bar + search-wrap + data-table + th/td + hover |
| BOM 列表 | `/admin/bom` | filter-bar + filter-select + filter-date + filter-check + data-table + row-actions |
| 客户新建 | `/admin/customers/new` | data-card(表单版 p-5) + create-action-bar(抵消 padding) |
| FMS 日记账列表 | `/admin/fms/journals` | FMS scoped 毛玻璃 + 渐变表头 |
| 产品列表 | `/admin/products` | data-card + data-table 标准版 |
| 采购参数配置 | `/admin/purchase/settings` | data-card + create-action-bar |

- [ ] **Step 5: 横向滚动回归**

在 `/admin/bom` 页面，如果表格内容超出视口宽度：
- data-card-scroll 横向滚动正常
- 滚动条样式（thin、border 颜色）

- [ ] **Step 6: 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P3 — migrate data card + table + filter to atomic UnoCSS

Migrate 14 data display classes from base.css to inline atomic UnoCSS:
data-card, data-card-scroll, data-card-head, data-card-body,
data-card-header, data-card-title, data-table, create-action-bar,
filter-bar, filter-select, filter-date, filter-check, search-wrap,
search-input.

Updated 140+ Maud files (4 components + ~120 pages).
Border moved from td to tr for simpler last-row handling.
Row-actions uses group-hover pattern.
FMS scoped overrides (glassmorphism filter-bar/data-card, gradient
table headers) migrated to page-specific atomic classes.
Deleted ~100 lines of CSS from base.css.
Verified: build:css success, cargo clippy clean, 6 pages checked."
```
