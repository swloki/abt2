# P2: Layout Shell + Page Header 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 base.css 中 8 个布局/页面头 class（app-shell / main-content / page-content / page-header / page-header-left / page-title / page-actions / back-link）迁移为 UnoCSS 原子 class，并从 base.css 中删除对应定义。

**Architecture:** 布局 shell（app-shell / main-content / page-content）定义在 `layout/page.rs` 的 `admin_shell` 函数中，是全局唯一入口，修改量小但影响所有页面。页面头（page-header / page-title / page-actions / back-link）散布在 130+ 个页面文件中。策略：先迁移 layout shell（3 个 class、1 个文件），再批量迁移 page-header 族（5 个 class、130+ 文件），最后处理 @media 响应式覆盖。

**Tech Stack:** UnoCSS v66.7.0, presetWind4, Maud, Rust

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

**前置条件:** P0 已完成

---

## CSS → 原子 class 映射表

### 布局 Shell 类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `app-shell` | 109-114 | `display:grid; grid-template-columns:var(--sidebar-w) 1fr; min-height:100vh; transition:grid-template-columns 240ms` | `grid grid-cols-[var(--sidebar-w)_1fr] min-h-screen transition-[grid-template-columns] duration-240 ease-standard` |
| `app-shell.sidebar-collapsed` | 115-117 | `grid-template-columns: 56px 1fr` | JS toggle 添加 class `sidebar-collapsed`，原子化处理见 Task 1 Step 2 |
| `main-content` | 240 | `display:flex; flex-direction:column; min-height:100vh; background:var(--surface); min-width:0; overflow-x:hidden` | `flex flex-col min-h-screen bg-surface min-w-0 overflow-x-hidden` |
| `page-content` | 315 | `flex:1; padding:var(--space-8); min-width:0; overflow-x:hidden` | `flex-1 p-8 min-w-0 overflow-x-hidden` |

### 页面头类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `page-header` | 2720 | `display:flex; align-items:center; justify-content:space-between; margin-bottom:var(--space-6)` | `flex items-center justify-between mb-6` |
| `page-header-left` | — | 无自身 CSS 定义（仅语义容器） | `flex flex-col gap-1`（或根据上下文调整为 `flex items-center gap-4`，见 Task 3） |
| `page-title` | 2719 | `font-size:var(--text-xl); font-weight:700; color:var(--fg); letter-spacing:-0.01em` | `text-xl font-bold text-fg tracking-[-0.01em]` |
| `page-actions` | 392 | `display:flex; gap:var(--space-3)` | `flex gap-3` |
| `back-link` | 918-924 | `display:inline-flex; align-items:center; gap:6px; color:var(--muted); font-size:var(--text-sm); margin-bottom:var(--space-3); transition:color 150ms` + hover `color:var(--accent)` + svg `width:16px; height:16px` | `inline-flex items-center gap-1.5 text-muted text-sm mb-3 transition-colors duration-150 hover:text-accent`（svg 上追加 `w-4 h-4`） |

### @media 响应式覆盖（行 374-388）

| class | 当前 CSS（@media 768px） | 原子化策略 |
|---|---|---|
| `app-shell` | `grid-template-columns: 1fr !important` | 响应式前缀: `md:grid-cols-1` |
| `main-content` | `padding-bottom: 68px` | 响应式前缀: `md:pb-[68px]` |
| `page-content` | `padding: var(--space-4)` | 响应式前缀: `md:p-4` |
| `page-header` | `flex-direction: column; align-items: stretch; gap: var(--space-3)` | 响应式前缀: `md:flex-col md:items-stretch md:gap-3` |

### FMS scoped 覆盖

| class | base.css 行 | 当前 CSS | 处理策略 |
|---|---|---|---|
| `.fms-dashboard .page-title` | 3167-3175 | `font-size:24px; font-weight:800; letter-spacing:-0.03em` + 渐变文字 | FMS dashboard 页面的 page-title 使用专属原子串，见 Task 4 |
| `.fms-list-page .page-title` | 3446-3450 | `font-size:24px; font-weight:800; letter-spacing:-0.03em` | FMS list 页面的 page-title 使用专属原子串 |
| `.fms-form-page .back-link` | 3377-3386 | `transition:all .2s; border-radius:var(--radius-md); padding:6px 12px; margin-left:-12px` + hover bg | FMS 表单页面的 back-link 使用专属原子串 |
| `.fms-expense-detail .page-content` | 3897-3922 | `background:linear-gradient(...); position:relative` + `::before` / `::after` 伪元素 | FMS expense detail 页面的 page-content 使用专属原子串 + before:/after: |

---

## 涉及文件清单

### 布局 Shell（1 个文件）
- `abt-web/src/layout/page.rs`（`admin_shell` 函数，行 31-53）

### 页面头（130+ 文件）

所有通过 `admin_page()` 渲染的页面文件都间接使用 page-content（通过 layout），但 page-header / page-title / page-actions / back-link 是在各页面 Maud 模板中直接写的。

含 `page-header` 的文件（代表性）：
`dashboard.rs`, `bom_list.rs`, `bom_create.rs`, `bom_detail.rs`, `bom_edit.rs`, `category_list.rs`, `customer_list.rs`, `customer_create.rs`, `customer_detail.rs`, `customer_edit.rs`, `fms_dashboard.rs`, `fms_cost_analysis.rs`, `fms_expense_list.rs`, `fms_journal_create.rs`, `fms_journal_detail.rs`, `fms_journal_list.rs`, `mes_dashboard.rs`, `mes_order_list.rs`, `mes_plan_list.rs`, `purchase_dashboard.rs`, `purchase_order_list.rs`, `quotation_list.rs`, `sales_order_list.rs`, `supplier_list.rs`, `user_list.rs`, `role_list.rs`, `wms_dashboard.rs` 等（全部 130+ 个 *_list.rs / *_detail.rs / *_create.rs / *_edit.rs）

含 `back-link` 的文件（代表性）：
`bom_create.rs`, `customer_create.rs`, `customer_detail.rs`, `customer_edit.rs`, `fms_journal_create.rs`, `fms_journal_detail.rs`, `fms_expense_create.rs`, `fms_expense_detail.rs` 等

含 `page-header-left` 的文件：
`fms_journal_create.rs`, `fms_journal_detail.rs`, `purchase_settings.rs` 等

---

### Task 1: 迁移布局 Shell（app-shell / main-content / page-content）

**Files:**
- Modify: `abt-web/src/layout/page.rs:42-46`
- Modify: `static/base.css:109-117`（app-shell 块）
- Modify: `static/base.css:240`（main-content）
- Modify: `static/base.css:315`（page-content）
- Modify: `static/base.css:374-388`（@media 响应式覆盖）

- [ ] **Step 1: 替换 admin_shell 中的三个布局 class**

在 `abt-web/src/layout/page.rs` 的 `admin_shell` 函数中：

行 42:
```rust
// 旧:
div class="app-shell" _="on load if localStorage.getItem('sidebar-collapsed') is 'true' add .sidebar-collapsed" {
// 新:
div class="grid grid-cols-[var(--sidebar-w)_1fr] min-h-screen transition-[grid-template-columns] duration-240 ease-standard md:grid-cols-1"
     _="on load if localStorage.getItem('sidebar-collapsed') is 'true' add .sidebar-collapsed" {
```

行 44:
```rust
// 旧:
div class="main-content" {
// 新:
div class="flex flex-col min-h-screen bg-surface min-w-0 overflow-x-hidden md:pb-[68px]" {
```

行 46:
```rust
// 旧:
div class="page-content" { (content) }
// 新:
div class="flex-1 p-8 min-w-0 overflow-x-hidden md:p-4" { (content) }
```

- [ ] **Step 2: 处理 app-shell.sidebar-collapsed 的 JS toggle**

sidebar.rs 行 545-546 的 hyperscript 通过 `toggle .sidebar-collapsed on .app-shell` 切换折叠状态。

问题：原子化后不再有 `.app-shell` class，JS 无法用 class 选择器 toggle。

**方案 A（推荐）**：保留 `app-shell` 作为语义标记 class（不依赖 CSS 定义，仅用于 JS 选择器），与原子 class 共存：
```rust
div class="app-shell grid grid-cols-[var(--sidebar-w)_1fr] min-h-screen transition-[grid-template-columns] duration-240 ease-standard md:grid-cols-1"
     _="on load if localStorage.getItem('sidebar-collapsed') is 'true' add .sidebar-collapsed" {
```

然后在 sidebar.rs 的 toggle 按钮中，保持原 hyperscript 不变。`sidebar-collapsed` 的 grid 效果通过 hyperscript 动态切换 inline style：
```rust
// sidebar.rs 行 545-546 更新 hyperscript:
_="on click toggle .sidebar-collapsed on .app-shell
   then if .app-shell matches .sidebar-collapsed
     set .app-shell's style.gridTemplateColumns to '56px 1fr'
     and call localStorage.setItem('sidebar-collapsed','true')
   else
     set .app-shell's style.gridTemplateColumns to 'var(--sidebar-w) 1fr'
     and call localStorage.removeItem('sidebar-collapsed')"
```

并在 page.rs 行 42 的 load 事件中追加：
```rust
_="on load if localStorage.getItem('sidebar-collapsed') is 'true'
     add .sidebar-collapsed to .app-shell
     and set .app-shell's style.gridTemplateColumns to '56px 1fr'"
```

**方案 B（备选）**：在 uno.config.ts shortcuts 中保留 `app-shell` 一条（过渡期），但设计文档要求 shortcuts 全部清空。

采用方案 A。

- [ ] **Step 3: 从 base.css 删除布局 class 定义**

删除：
- 行 108（`:root { --sidebar-w: 240px; --header-h: 60px; }` — 注意这行包含 `--header-h`，P8 会用到。只删 `--sidebar-w`，保留 `--header-h`。改为 `:root { --header-h: 60px; }`）
- 行 109-117（app-shell + sidebar-collapsed）
- 行 240（main-content）
- 行 315（page-content）

- [ ] **Step 4: 从 base.css 删除 @media 响应式覆盖中的对应行**

行 374-388 的 @media 块中：
- 删除行 377（`.app-shell { grid-template-columns: 1fr !important; }`）
- 删除行 384（`.main-content { padding-bottom: 68px; }`）
- 删除行 385（`.page-content { padding: var(--space-4); }`）
- 删除行 387（`.page-header { ... }` — 在 Task 3 处理后删除）

保留行 378-383（#sidebar 移动端定位 — 属于 P8）、行 386（.top-header — 属于 P8）、行 376（.mobile-nav）。

- [ ] **Step 5: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 6: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 7: 页面验证**

用 agent-browser 打开任意页面（如 `http://localhost:8000/admin/dashboard`），检查：
- 左侧 sidebar + 右侧 main-content 的 grid 布局正常
- sidebar 宽度 240px
- page-content 内边距 32px
- 点击侧栏「收起」按钮，sidebar 折叠到 56px，grid 列宽变化
- 刷新页面后折叠状态保持（localStorage）
- 窗口缩小到 768px 以下，布局变为单列

---

### Task 2: 迁移 page-header / page-title / page-actions

**Files:**
- Modify: 全部 130+ 个含 `page-header` / `page-title` / `page-actions` 的 Maud 文件
- Modify: `static/base.css:2719-2720`（page-title + page-header）
- Modify: `static/base.css:392`（page-actions）

- [ ] **Step 1: 替换 page-header**

全局搜索 `class="page-header"`，替换为：
```
class="flex items-center justify-between mb-6 md:flex-col md:items-stretch md:gap-3"
```

注意：部分页面的 page-header 已有额外 inline style（如 `style="..."`），保留 inline style 不变，仅替换 class。

- [ ] **Step 2: 替换 page-title**

全局搜索 `class="page-title"`，替换为：
```
class="text-xl font-bold text-fg tracking-[-0.01em]"
```

注意：部分页面的 page-title 已有额外 inline style（如 bom_edit.rs 行 593 有 `style="display:flex;align-items:center;..."`），保留 inline style。

- [ ] **Step 3: 替换 page-actions**

全局搜索 `class="page-actions"`，替换为：
```
class="flex gap-3"
```

- [ ] **Step 4: 替换 page-header-left**

全局搜索 `class="page-header-left"`。

当前 base.css 中无 `.page-header-left` 的 CSS 定义（仅语义容器）。根据使用场景（通常包含 back-link + page-title），设为：
```
class="flex flex-col gap-1"
```

如果是水平排列场景（back-link + title 并排），则为 `flex items-center gap-3`。根据各文件实际结构判断。

- [ ] **Step 5: 从 base.css 删除**

删除：
- 行 392（page-actions）
- 行 2719-2720（page-title + page-header）

- [ ] **Step 6: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 7: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 8: 页面验证**

用 agent-browser 打开 5 个代表性页面：
- `/admin/dashboard` — page-header + page-title + page-actions 水平排列
- `/admin/customers` — list 页 page-header
- `/admin/bom` — BOM 列表页 page-header + 导出按钮
- `/admin/fms/journals/new` — page-header-left 包含 back-link + page-title
- `/admin/purchase/settings` — page-header-left 包含 page-title

检查点：
- page-title 字号 21px、粗体、颜色 fg
- page-header 左右两端对齐
- page-actions 按钮间距 12px
- 窗口缩小到 768px 以下，page-header 变为垂直排列

---

### Task 3: 迁移 back-link

**Files:**
- Modify: 全部含 `class="back-link"` 的 Maud 文件
- Modify: `static/base.css:917-924`（back-link 块）

- [ ] **Step 1: 替换 back-link（标准版）**

全局搜索 `class="back-link"`，替换为：
```
class="inline-flex items-center gap-1.5 text-muted text-sm mb-3 transition-colors duration-150 hover:text-accent"
```

back-link 内的 svg 已通过 inline class 设置尺寸（如 `w-4 h-4`），保留不变。如果没有 inline class 的 svg，追加 `w-4 h-4`。

- [ ] **Step 2: 处理简写形式 `a.back-link`**

部分文件使用 Maud 简写 `a.back-link`（如 fms_expense_detail.rs 行 86）。改为：
```rust
a class="inline-flex items-center gap-1.5 text-muted text-sm mb-3 transition-colors duration-150 hover:text-accent" href=(...)
```

- [ ] **Step 3: 处理 inline style 覆盖的 back-link**

部分文件的 back-link 使用 inline style 而非 class（如 fms_expense_create.rs 行 98）：
```rust
a href=(...) class="back-link" style="display:inline-flex;align-items:center;gap:6px;font-size:14px;color:var(--muted);margin-bottom:var(--space-6)"
```

这些替换为原子 class 后删除 inline style（注意此例 margin-bottom 为 space-6 而非标准的 space-3，需保留 `mb-6`）：
```
class="inline-flex items-center gap-1.5 text-muted text-sm mb-6 transition-colors duration-150 hover:text-accent"
```

- [ ] **Step 4: 从 base.css 删除**

删除行 917-924（back-link + hover + svg 块）。

- [ ] **Step 5: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 6: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 7: 页面验证**

打开含 back-link 的页面：
- `/admin/customers/new` — 返回链接灰色、hover 变蓝
- `/admin/bom/new` — back-link + 箭头图标
- `/admin/fms/journals/new` — page-header-left 内的 back-link

---

### Task 4: FMS scoped 覆盖处理

**Files:**
- Modify: `abt-web/src/pages/fms_dashboard.rs`（page-title FMS 版）
- Modify: `abt-web/src/pages/fms_expense_list.rs` 或其他 fms-list-page（page-title FMS 版）
- Modify: `abt-web/src/pages/fms_journal_create.rs` 或其他 fms-form-page（back-link FMS 版）
- Modify: `abt-web/src/pages/fms_expense_detail.rs`（page-content FMS 版 + 伪元素）
- Modify: `static/base.css:3167-3175`（.fms-dashboard .page-title）
- Modify: `static/base.css:3377-3386`（.fms-form-page .back-link）
- Modify: `static/base.css:3446-3450`（.fms-list-page .page-title）
- Modify: `static/base.css:3897-3922`（.fms-expense-detail page-content + ::before/::after）

- [ ] **Step 1: 读取 FMS scoped 覆盖的完整 CSS**

先读取 base.css 行 3167-3175、3377-3386、3446-3450、3897-3922 的完整定义。

- [ ] **Step 2: 替换 FMS dashboard 的 page-title**

fms_dashboard.rs 行 194 `class="page-title"` 替换为 FMS 专属：
```
class="text-2xl font-extrabold text-fg tracking-[-0.03em] bg-gradient-to-r from-[#2563eb] to-[#4f7df7] bg-clip-text text-transparent"
```

注意：需读取行 3167-3175 确认是否含渐变文字效果。如果是 `background-clip:text` + `text-fill-color:transparent`，用上述原子串。如果仅是字号/字重变化，则简化为 `text-2xl font-extrabold tracking-[-0.03em]`。

- [ ] **Step 3: 替换 FMS list 页面的 page-title**

所有 `.fms-list-page` 内的 `class="page-title"` 替换为：
```
class="text-2xl font-extrabold text-fg tracking-[-0.03em]"
```

- [ ] **Step 4: 替换 FMS form 页面的 back-link**

所有 `.fms-form-page` 内的 `class="back-link"` 替换为：
```
class="inline-flex items-center gap-1.5 text-muted text-sm mb-3 transition-all duration-200 hover:text-accent hover:bg-accent-bg rounded-md py-1.5 px-3 -ml-3"
```

- [ ] **Step 5: 替换 FMS expense detail 的 page-content**

fms_expense_detail.rs 的 page-content 有渐变背景 + 伪元素装饰。读取行 3897-3922 确认完整效果后，在 page-content 元素上替换为：
```
class="flex-1 p-8 min-w-0 overflow-x-hidden relative bg-[linear-gradient(180deg,#f0f4ff_0%,var(--surface)_25%)]
       before:content-[''] before:absolute before:-top-10 before:left-1/4 before:w-72 before:h-72 before:rounded-full
       before:bg-[radial-gradient(circle,rgba(37,99,235,0.06),transparent_70%)] before:pointer-events-none
       after:content-[''] after:absolute after:bottom-15 after:right-1/4 after:w-96 after:h-96 after:rounded-full
       after:bg-[radial-gradient(circle,rgba(99,102,241,0.04),transparent_70%)] after:pointer-events-none"
```

注意：需精确读取行 3897-3922 的值，上述为预估，实施时以实际 CSS 为准。

- [ ] **Step 6: 从 base.css 删除 FMS 覆盖**

删除：
- 行 3167-3175（.fms-dashboard .page-title）
- 行 3377-3386（.fms-form-page .back-link）
- 行 3446-3450（.fms-list-page .page-title）
- 行 3897-3922（.fms-expense-detail page-content + ::before/::after + back-link 重复定义 3924-3938）

- [ ] **Step 7: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 8: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 9: 页面验证**

打开 FMS 页面：
- `/admin/fms/dashboard` — page-title 渐变文字效果
- `/admin/fms/journals/new` — back-link 带 padding + hover 背景
- `/admin/fms/expenses/{id}` — page-content 渐变背景 + 装饰圆斑

---

### Task 5: 最终构建与回归验证

**Files:**
- 无新文件修改

- [ ] **Step 1: 全量构建**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 2: 全量编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 3: 确认 base.css 已删除所有 P2 class**

Run（用 search 工具）: 在 `static/base.css` 中搜索以下 class（排除注释），确认全部不存在：
`app-shell`, `main-content`, `page-content`, `page-header`, `page-header-left`, `page-title`, `page-actions`, `back-link`

Expected: 全部 0 匹配。

- [ ] **Step 4: 回归验证 6 个代表性页面**

| 页面 | URL | 检查重点 |
|---|---|---|
| Dashboard | `/admin/dashboard` | app-shell grid + sidebar + page-header + page-title |
| 客户列表 | `/admin/customers` | page-header + page-actions + page-title |
| 客户新建 | `/admin/customers/new` | back-link hover + page-header |
| BOM 列表 | `/admin/bom` | page-header + page-actions 导出按钮 |
| FMS Dashboard | `/admin/fms` | page-title 渐变文字 |
| 采购参数 | `/admin/purchase/settings` | page-header-left + page-title |

- [ ] **Step 5: sidebar 折叠功能回归**

打开任意页面，点击 sidebar 底部「收起」按钮：
- sidebar 从 240px 折叠到 56px
- grid 列宽平滑过渡（transition 240ms）
- 刷新页面后状态保持
- 再次点击展开

- [ ] **Step 6: 响应式回归**

将浏览器窗口缩小到 768px 以下：
- app-shell 变为单列
- page-header 变为垂直排列
- page-content padding 缩小到 16px

- [ ] **Step 7: 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P2 — migrate layout shell + page header to atomic UnoCSS

Migrate 8 layout/header classes from base.css to inline atomic UnoCSS:
app-shell, main-content, page-content, page-header, page-header-left,
page-title, page-actions, back-link.

Updated layout/page.rs (admin_shell) + 130+ page files.
Sidebar collapse JS updated to use inline style toggle.
FMS scoped overrides (dashboard gradient title, form back-link,
expense detail page-content) migrated to page-specific atomic classes.
Deleted ~60 lines of CSS from base.css.
Verified: build:css success, cargo clippy clean, 6 pages + sidebar/responsive checked."
```
