# P9: 域专属 CSS 迁移 + 删除 base.css 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 迁移所有剩余 ~785 个域专属 class 为原子 UnoCSS，最终删除 `static/base.css` 文件并从 UnoCSS CLI 扫描中移除。

**Architecture:** 按业务域拆分为 8 个子任务（MES / FMS / Sales+Outsourcing / Toast+Login / BOM+Cost / Permission+Dept / WMS+Demand+CardQuery / Import-Export），每个子任务独立迁移并验证。最后一个子任务删除 base.css 并清理 CLI 配置。

**Tech Stack:** UnoCSS v66.7.0, presetWind4, Maud, Rust

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

**前置条件:** P0-P8 全部完成。base.css 中 P1-P8 的 class 已删除，剩余 ~785 个域专属 class。

---

### Task 1: MES 生产管理域

**Files:**
- Modify: `abt-web/src/pages/mes_*.rs`（所有 MES 页面）
- Modify: `static/base.css`（删除 MES 相关 class）

涉及的 class 族（在 base.css 中搜索精确行号）：

**MES Batch Detail:**
- batch-detail-header, batch-detail-title-row, detail-doc-no (MES版), detail-info-grid-5, detail-info-item, detail-info-label, detail-info-value
- sub-section, sub-section-title
- progress-track, progress-step, progress-step-dot, progress-step-label, progress-step-line
- shift-toggle, shift-btn, wage-display, wage-amount

**MES Work Order:**
- wo-progress, wo-progress-fill, wo-progress-track, wo-progress-bar, wo-progress-text
- wo-status-bar, wo-row-content, wo-order-num, wo-product-info, wo-product-meta
- wo-step, wo-step-bar, wo-step-fill, wo-step-text, wo-action-btn, wo-type
- work-order-list, work-order-item, wo-header

**MES Dashboard / Config:**
- bento-grid, bento-half, bento-sub-grid
- routing-flow, flow-node, flow-icon, flow-title, flow-meta, flow-stats, flow-stat, flow-arrow, flow-flow-wrap
- audit-timeline, audit-item, audit-dot, audit-content, audit-title, audit-meta, audit-desc
- completeness-dots, comp-dot
- release-result-item, release-preview
- receipt-status-bar, calc-detail
- view-toggle, toggle-display, toggle-option
- fqc-badge, fqc-badge--failed/passed/pending/na
- dq-grid, dq-card, dq-card-icon, dq-card-value, dq-card-label
- detail-tabs, detail-tab, tab-btn
- source-trace, info-section, info-section-title
- config-section, config-grid, config-item, config-label, config-value, config-warning

- [ ] **Step 1: 搜索 MES class 在 base.css 中的定义**

对每个 class 在 base.css 中搜索定义，记录精确行号。

- [ ] **Step 2: 翻译为原子 class**

关键映射：
```
// wo-progress-track → h-2 bg-border-soft rounded-full overflow-hidden
// wo-progress-fill → h-full bg-accent rounded-full transition-all duration-300
// wo-action-btn → inline-flex items-center gap-1 text-accent text-sm hover:text-accent-hover cursor-pointer
// wo-type → inline-flex items-center gap-1.5 text-xs font-medium before:content-[''] before:w-2 before:h-2 before:rounded-full
// bento-grid → grid grid-cols-2 gap-5
// routing-flow → flex items-start gap-2
// flow-arrow → text-muted text-lg self-center
// audit-timeline → relative pl-6 before:content-[''] before:absolute before:left-[7px] before:top-1 before:bottom-1 before:w-0.5 before:bg-border-soft
```

- [ ] **Step 3: 在 Maud 中替换**

搜索所有 `mes_*.rs` 文件中的 class 引用，逐一替换。

- [ ] **Step 4: 从 base.css 删除 MES class 定义**

- [ ] **Step 5: 构建验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

- [ ] **Step 6: 页面验证**

用 agent-browser 打开 MES 相关页面验证渲染。

---

### Task 2: MES Schedule Board (Gantt/Load)

**Files:**
- Modify: `abt-web/src/pages/mes_schedule*.rs`
- Modify: `static/base.css`

涉及的 class：
- board-stats, board-stat-card, board-stat-value, board-stat-label
- schedule-toolbar, schedule-tabs, schedule-date-nav, date-nav-btn, date-range-label
- gantt-wrap, gantt-table, gantt-corner, gantt-date-header, gantt-date-today, gantt-date-day, weekday, weekend
- gantt-wc-cell, gantt-wc-dot, gantt-wc-name, gantt-wc-type
- gantt-cell, gantt-cell-today, gantt-block, gantt-block-title
- gantt-color-0 ~ gantt-color-7（8 色调色板）
- load-wrap, load-table, load-cell, load-cell-block, load-cell-pct, load-level-empty/low/mid/high, load-legend
- schedule-empty

关键映射：
```
// gantt-table → w-full border-collapse table-fixed
// gantt-block → absolute rounded h-full text-xs text-white px-1 py-0.5 overflow-hidden before:content-[''] before:absolute before:left-0 before:top-0 before:bottom-0 before:w-1 before:bg-current
// gantt-color-0 → bg-[#4f7df9], gantt-color-1 → bg-[#fa8c16], ...
// load-cell-block → absolute inset-0 rounded transition-all
```

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 3: FMS 财务管理域

**Files:**
- Modify: `abt-web/src/pages/fms_*.rs`
- Modify: `static/base.css`

涉及的 class：
- fms-dashboard 及其 scoped 子组件（mes-stat-card/mes-stat-icon/mes-stat-body/mes-stat-label/mes-stat-value/mes-stat-sub）
- quick-card, quick-card-icon, section-card, section-card-head
- flow-row, flow-dot.inflow, flow-dot.outflow
- mini-avatar, balance-positive, balance-negative
- progress-bar, progress-bar-fill（FMS 版本，含 ::after 光泽伪元素）
- chart-bar-wrap, section-block-title
- fms-form-page 及其 scoped 覆盖
- fms-list-page 及其 scoped 覆盖
- page-content scoped（FMS Expense Detail）

关键策略：
FMS 的 scoped 覆盖（如 `.fms-list-page .data-card`）无法用原子 class 的级联表达。**策略**：在 FMS 页面的 Maud 模板中，直接写 FMS 专属的原子 class 组合（含毛玻璃 `backdrop-blur-md bg-white/88`），不依赖父选择器 scoped 覆盖。

关键映射：
```
// mes-stat-card (FMS) → bg-white/88 backdrop-blur-md rounded-lg border border-white/40 shadow-md p-6 relative overflow-hidden transition-all
// mes-stat-card::before → before:content-[''] before:absolute before:left-0 before:top-0 before:bottom-0 before:w-1 before:bg-accent
// mes-stat-card.accent-green::before → before:bg-[linear-gradient(90deg,#16a34a,#4ade80)]
// flow-dot.inflow → w-2.5 h-2.5 rounded-full bg-success
// flow-dot.outflow → w-2.5 h-2.5 rounded-full bg-danger
```

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 4: Sales 履约 + Outsourcing 委外

**Files:**
- Modify: `abt-web/src/pages/sales_*.rs`, `abt-web/src/pages/om_*.rs`（或 outsourced 相关页面）
- Modify: `static/base.css`

**Sales Fulfillment:**
- fulfill-progress, fulfill-progress-header, fulfill-progress-title
- progress-bar-track, progress-bar-shipped/allocated/producing/purchasing/pending（5 色堆叠进度条）
- progress-legend, progress-stats, progress-stat
- line-status-* 变体
- fulfill-section, fulfill-header, fulfill-title, fulfill-badge, fulfill-btn
- fulfill-table, qty-bar, qty-bar-track, qty-bar-fill.*
- acquire-tag.*
- fulfill-ref-link, stock-bar-wrap, stock-bar-fill

关键映射：
```
// progress-bar-track → flex h-3 rounded-full overflow-hidden bg-border-soft
// progress-bar-shipped → bg-success（宽度用内联 style="width:N%"）
// qty-bar-fill → h-full bg-accent rounded-sm
```

**Outsourcing Detail (Hero/Timeline):**
- detail-hero, detail-hero-accent, detail-hero-body
- detail-title-row, detail-doc-no, doc-icon, detail-meta, detail-actions
- detail-info-split, info-key-grid, info-key-item, info-key-label, info-key-value
- info-progress, progress-ring-wrap, progress-ring, progress-ring-bg, progress-ring-fill, progress-ring-text
- tracking-section, tracking-head, tracking-title, tracking-icon-wrap, tracking-hint, hint-dot
- tracking-timeline（含 ::before 渐变竖线伪元素）
- track-node, track-dot, track-dot.completed/active/pending, track-content
- track-info, track-label, track-time, track-remark, track-status
- amount-bar, amount-item, amount-label, amount-value
- type-tag, type-tag.*

关键映射：
```
// detail-hero-accent → h-1 bg-gradient-to-r from-accent via-[#60a5fa] to-accent bg-[length:200%_100%] animate-shimmer-bar
// tracking-timeline → relative pl-11 before:content-[''] before:absolute before:left-[17px] before:top-[18px] before:bottom-[18px] before:w-0.5 before:rounded-sm before:bg-gradient-to-b before:from-success before:via-accent before:to-border-soft
// track-dot.completed → bg-gradient-to-br from-success to-[#22c55e] border-transparent shadow-[0_2px_10px_rgba(22,163,74,0.3)] after:content-['✓'] after:text-white after:text-sm after:font-bold
// track-dot.active → bg-gradient-to-br from-accent to-[#60a5fa] border-transparent animate-pulse-active after:content-[''] after:w-2.5 after:h-2.5 after:rounded-full after:bg-white
```

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 5: Toast + Login

**Files:**
- Modify: `abt-web/src/toast.rs`, `abt-web/src/pages/auth_*.rs` 或 `abt-web/src/layout/page.rs`（toast-container）
- Modify: `static/base.css`

**Toast:**
- toast-container, toast, toast-icon, toast-message, toast-close
- toast-error/success/warning/info
- toast-dismiss

关键映射：
```
// toast → relative flex items-start gap-3 p-4 rounded-lg shadow-lg max-w-sm bg-white overflow-hidden animate-toast-in
// toast::after → after:content-[''] after:absolute after:bottom-0 after:left-0 after:h-0.5 after:animate-toast-progress
// toast.toast-error → border-l-4 border-danger after:bg-danger
// toast.toast-success → border-l-4 border-success after:bg-success
// toast-dismiss → animate-toast-out
```

**Login:**
- login-shell（已在 uno.config.ts shortcut 中定义，P0 迁移到 preflight 后此处删除 shortcut）
- brand-panel（含 ::before 网格背景 + ::after 径向光晕）
- brand-headline, brand-desc, login-panel
- field-input（已在 shortcut 中定义）
- field-icon, pw-toggle, custom-checkbox, login-divider
- spinner（含 @keyframes spin）
- btn-sso, btn-login（已在 shortcut 中定义）

关键映射：
```
// brand-panel → relative overflow-hidden bg-gradient-to-br from-accent to-[#1e1b4b] text-white
//   before:content-[''] before:absolute before:inset-0 before:bg-[linear-gradient(rgba(255,255,255,0.05)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.05)_1px,transparent_1px)] before:bg-[size:40px_40px]
//   after:content-[''] after:absolute after:inset-0 after:bg-[radial-gradient(circle_at_50%_50%,rgba(37,99,235,0.15),transparent_70%)]
// spinner → w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin
// login-divider → flex items-center gap-3 before:content-[''] after:content-[''] 
//   （before/after 各 flex-1 h-px bg-border）
// custom-checkbox:checked::after → 无法用原子 class 表达 checkbox 勾选伪元素
//   → 保留为组件 CSS 或改用原生 checkbox 样式
```

注意：`custom-checkbox:checked::after` 的勾选符号伪元素无法用 UnoCSS 原子 class 表达。保留这一条 CSS 规则在 preflights 中或改用 Hyperscript 实现自定义 checkbox。

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 6: BOM 编辑 + Cost Drawer

**Files:**
- Modify: `abt-web/src/pages/bom_*.rs`, 相关 cost/price 页面
- Modify: `static/base.css`

**BOM Table/Edit:**
- bom-table, bom-collapse-btn, bom-collapse-icon
- bom-level-badge, bom-row-level-0 ~ bom-row-level-N（彩色行）
- bom-toolbar, bom-category-select
- bom-dragging, bom-drop-indicator

关键映射：
```
// bom-row-level-0 → bg-[#7030a0]/5（紫色行背景）
// bom-collapse-btn → w-5 h-5 grid place-items-center cursor-pointer transition-transform
// bom-collapse-btn.collapsed → -rotate-90
// bom-dragging → opacity-50 ring-2 ring-accent
// bom-drop-indicator → h-1 bg-accent rounded-full
```

**Cost Drawer:**
- cost-warning-banner, cost-summary-card, cost-drawer-table
- cost-section, cost-drawer-footer
- temp-price-notice, temp-price-input, temp-price-confirm, temp-price-revert
- labor-summary-card
- cost-warning-list（含 `grid-template-rows: 0fr → 1fr` 折叠动画）

注意：`cost-warning-list` 的折叠动画（CSS Grid `0fr→1fr` 过渡）无法用原子 class 表达。保留这一条 CSS 规则在 preflights 中。

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 7: Permission + Department + WMS + Demand + CardQuery

**Files:**
- Modify: `abt-web/src/pages/perm*.rs`, `abt-web/src/pages/dept*.rs`, `abt-web/src/pages/wms_*.rs`, `abt-web/src/pages/mes_demand*.rs`, `abt-web/src/pages/mes_card*.rs`
- Modify: `static/base.css`

**Permission Config:**
- perm-page, perm-layout, perm-panel, perm-role-head
- perm-group, perm-group-head, perm-group-icon.g1~g5
- perm-group-body, perm-row, perm-row-header
- perm-resource, perm-code, perm-cell, perm-cell-header
- perm-btn, perm-res-row, perm-res-btns, perm-res-count
- perm-legend, perm-readonly-hint, perm-empty, perm-inherit-hint

注意：`perm-cell input:checked::after` 自定义 checkbox 勾选符号伪元素，同 Task 5 的 custom-checkbox 问题。

**Department List:**
- dept-layout, dept-tree-panel, tree-top-bar, tree-search, tree-list, tree-item, tree-code.tc-*（6 个渐变变体）
- tree-name, tree-tag, tree-foot
- dept-detail, dept-empty, d-hero, d-hero-icon, d-hero-text, d-hero-code, d-hero-sub, d-hero-actions
- d-stats, d-stat, d-stat-dot, d-body, d-section, d-section-head, d-section-title, d-section-count
- member-grid, m-card, m-ava, m-text, m-name, m-role, m-more

**WMS:**
- wms-form-section, wms-form-grid, detail-table, type-switch, type-btn
- source-info, add-row-btn, remove-row-btn, stat-card (WMS版), quick-link, form-select

**Demand Pool / Material:**
- stat-mini-grid, stat-mini
- view-toggle-bar, view-toggle, view-toggle-btn
- material-row, material-row-header, material-info, material-icon, material-name, material-code
- material-stat, material-stat-value, material-stat-label, material-actions
- demand-expand, batch-bar, tag-danger, tag-warn, tag-info, tag-muted
- demand-check, demand-row-selected, batch-action-bar, scheduling-hint

**Card Query:**
- card-search-box（含 ::before 顶部渐变条）
- card-search-title, card-search-desc, card-search-input-wrap, card-search-input, card-scan-btn
- card-result, card-result-header, card-result-no, card-result-meta, card-result-body
- flow-progress, flow-step, flow-step-node, flow-step-name, flow-step-info, flow-step-bar（含 calc() 连接线）
- card-info-grid, card-info-item, card-info-label, card-info-value, card-sub-table
- recent-section, recent-title, recent-grid, recent-card, recent-card-top, recent-card-no, recent-card-product, recent-card-progress, recent-card-progress-bar

- [ ] **Step 1-6:** 同 Task 1 流程

---

### Task 8: Import/Export + 其他剩余 class

**Files:**
- Modify: `abt-web/src/pages/import*.rs`, `abt-web/src/pages/export*.rs` 以及其他任何引用剩余 class 的文件
- Modify: `static/base.css`

**Import/Export:**
- import-file-zone, import-cols, import-actions
- import-progress-bar, import-progress-fill, import-result-stats, import-stat, import-stat-value, import-stat-label, import-errors
- export-dropdown, export-dropdown-menu, export-result, file-item

注意：这些 class 引用了不存在的 CSS 变量（`--primary-50`/`--slate-50`/`--green-600` 等 Tailwind 变量名），当前样式可能已失效。迁移时用正确的项目 CSS 变量。

**其他剩余 class：**
搜索 base.css 中所有尚未被 P1-P9 Task 1-8 迁移的 class 定义，逐一处理。

- [ ] **Step 1: 确认 base.css 中剩余的 class 定义**

Run: `grep -c '^\.' static/base.css`

如果返回 0，说明所有 class 已迁移。如果 > 0，搜索剩余 class 在 Maud 中的引用并迁移。

- [ ] **Step 2-6:** 迁移剩余 class，同前述流程

---

### Task 9: 删除 base.css + 清理 CLI 配置

**Files:**
- Delete: `static/base.css`
- Modify: `uno.config.ts`（cli.entry.patterns 移除 `static/base.css`）
- Modify: `uno.config.ts`（删除 shortcuts 块中所有剩余的 shortcut 定义）

- [ ] **Step 1: 确认 base.css 已无引用**

Run: `grep -c '^\.' static/base.css`

Expected: 0（所有 class 已迁移或删除）

- [ ] **Step 2: 删除 base.css 文件**

Run: `rm static/base.css`

- [ ] **Step 3: 修改 uno.config.ts CLI 配置**

将 `cli.entry.patterns` 从：
```typescript
patterns: ["abt-web/**/*.rs", "static/base.css"],
```
改为：
```typescript
patterns: ["abt-web/**/*.rs"],
```

- [ ] **Step 4: 清空 shortcuts 块**

将 `uno.config.ts` 中的 `shortcuts` 块改为空：
```typescript
shortcuts: {},
```

- [ ] **Step 5: 重新构建 CSS**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 6: 全量页面验证**

用 agent-browser 打开以下代表性页面验证：
- Dashboard: `/admin`
- 列表页: `/admin/mes/orders`, `/admin/purchase/orders`
- 详情页: `/admin/mes/orders/1`
- 表单页: `/admin/purchase/settings`
- FMS: `/admin/fms`
- 弹窗/抽屉: 任意带 modal 的页面

检查计算样式是否正确。

- [ ] **Step 7: cargo clippy 最终验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

- [ ] **Step 8: 验证 app.css 行数**

Run: `wc -l static/app.css`

Expected: ~2000 行（纯 UnoCSS 输出，无 base.css 拼接）

- [ ] **Step 9: 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P9 — delete base.css, complete atomic UnoCSS migration

Remove static/base.css (4476 lines). All 1065 component classes migrated
to inline atomic UnoCSS classes in Maud templates. CLI no longer scans
base.css. app.css is now pure UnoCSS output (~2000 lines)."
```
