# P1: Form Controls 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 base.css 中 18 个表单控件 class（form-field / form-input / form-select / form-label / form-grid / form-section / form-section-title / form-section-card / form-actions / form-textarea / form-hint / form-group / form-row / form-check / form-check-hint / checkbox-row / section-desc / password-wrap / password-toggle）全部迁移为 UnoCSS 原子 class，并从 base.css 中删除对应定义。

**Architecture:** 先在 base.css 中定位每个 class 的精确 CSS 定义（行 1111-1145、1576、1815-1822、1884-1893、2026-2030、2146-2148、2257-2264、2659-2671），将每条属性翻译为 UnoCSS 原子 class（利用 uno.config.ts 中已定义的 theme colors/spacing/fontSize/radius），然后在 74 个 Maud 文件中将 `class="form-xxx"` 替换为原子 class 串，最后从 base.css 中删除已迁移的 class 定义并重新构建。

**Tech Stack:** UnoCSS v66.7.0, presetWind4, Maud, Rust

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

**前置条件:** P0 已完成（preflights + theme.animation 已移入 uno.config.ts，base.css 仍被 CLI 扫描）

---

## CSS → 原子 class 映射表

下表是本批次所有 class 的精确映射，实施时逐条对照。

### 表单结构类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `form-section` | 1122 | `background:var(--bg); border:1px solid var(--border); border-radius:var(--radius-md); padding:var(--space-6); margin-bottom:var(--space-6)` | `bg-white border border-border rounded-md p-6 mb-6` |
| `form-section-card` | 2026 | `background:var(--bg); border:1px solid var(--border); border-radius:var(--radius-md); padding:var(--space-6); margin-bottom:var(--space-5); box-shadow:var(--shadow-xs)` | `bg-white border border-border rounded-md p-6 mb-5 shadow-xs` |
| `form-section-title` | 1112-1117 | `font-size:var(--text-sm); font-weight:600; color:var(--fg); margin-bottom:var(--space-4); padding-bottom:var(--space-2); border-bottom:1px solid var(--border-soft); display:flex; align-items:center; gap:var(--space-2)` | `text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft flex items-center gap-2` |
| `form-section-card .form-section-title` 覆盖 | 2027-2028 | `border-bottom:none; padding-bottom:0; margin-bottom:var(--space-5); display:flex; align-items:center; gap:var(--space-2)`（svg 颜色 muted） | 覆盖写法: `flex items-center gap-2 mb-5 pb-0 border-b-0`（svg 已用 inline `w-[18px] h-[18px] text-muted`，无需额外规则） |
| `form-grid` | 1119 | `display:grid; grid-template-columns:1fr 1fr; gap:var(--space-4) var(--space-6); margin-bottom:var(--space-6)` | `grid grid-cols-2 gap-x-6 gap-y-4 mb-6` |
| `form-grid .span-2` | 1121 | `grid-column: 1 / -1` | `col-span-full` |
| `form-grid .field-full` | 1120 | `grid-column: 1 / -1` | `col-span-full` |
| `form-section .form-grid` 覆盖 | 1123 | `margin-bottom:0; grid-template-columns:repeat(4,1fr); gap:var(--space-5)` | 覆盖写法: `grid grid-cols-4 gap-5 mb-0` |
| `form-section .form-grid .span-2` 覆盖 | 1124 | `grid-column: span 2` | `col-span-2` |
| `form-section-card .form-grid` 覆盖 | 2029 | `margin-bottom:0; grid-template-columns:repeat(4,1fr); gap:var(--space-5)` | 覆盖写法: `grid grid-cols-4 gap-5 mb-0` |
| `form-section-card .form-grid .span-2` 覆盖 | 2030 | `grid-column: span 2` | `col-span-2` |
| `form-section .form-grid.cols-2` 覆盖 | 2670 | `grid-template-columns:repeat(2,1fr)` | 覆盖写法: `grid-cols-2`（与其他 class 拼接） |

### 表单字段类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `form-field` (容器) | — | 无自身样式（仅后代选择器） | 删除 class，样式直接放到 label/input 上 |
| `form-field label` | 1131 | `display:block; font-size:12px; font-weight:500; color:var(--fg-2); margin-bottom:var(--space-1); white-space:nowrap` | label 上: `block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap` |
| `form-field input/select/textarea` | 1132-1138 | `width:100%; padding:8px 12px; border:1px solid var(--border); border-radius:var(--radius-sm); font-size:var(--text-sm); font-family:var(--font-body); transition:all 150ms; background:#fff; color:var(--fg)` | input/select/textarea 上: `w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150` |
| `form-field input:focus` 等 | 1139-1141 | `outline:none; border-color:var(--accent); box-shadow:var(--shadow-focus)` | 追加: `focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]` |
| `form-field textarea` | 1142 | `resize:vertical; min-height:72px` | 追加: `resize-y min-h-[72px]` |
| `form-input` | 1819 | `width:100%; padding:8px 12px; border:1px solid var(--border); border-radius:var(--radius-sm); font-size:14px; background:#fff; color:var(--fg); transition:all .15s` | `w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150` |
| `form-input:focus` | 1820 | `outline:none; border-color:var(--accent); box-shadow:0 0 0 3px rgba(22,119,255,.1)` | `focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)]` |
| `form-input::placeholder` | 1821 | `color:var(--muted); opacity:.6` | `placeholder:text-muted placeholder:opacity-60` |
| `textarea.form-input` | 1822 | `min-height:72px; resize:vertical; font-family:var(--font-body)` | `min-h-[72px] resize-y` |
| `form-input-readonly` | 1242, 2137 | `opacity:.5; cursor:not-allowed` | `opacity-50 cursor-not-allowed` |
| `form-input-mono` | 1243, 2138 | `text-transform:uppercase; font-family:var(--font-mono)` | `uppercase font-mono` |
| `form-input-disabled` | 2139 | `background:var(--surface); color:var(--muted); cursor:not-allowed` | `bg-surface text-muted cursor-not-allowed` |
| `form-select` | 2257-2263 | `height:36px; padding:0 var(--space-3); border:1px solid var(--border); border-radius:var(--radius-sm); font-size:var(--text-sm); color:var(--fg); background:var(--bg); outline:none; transition:border-color 150ms; width:100%` | `h-9 w-full px-3 border border-border rounded-sm text-sm text-fg bg-white outline-none transition-colors duration-150` |
| `form-select:focus` | 2264 | `border-color:var(--accent); box-shadow:var(--shadow-focus)` | `focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]` |
| `form-textarea` | 2659 | `width:100%; padding:8px 12px; border:1px solid var(--border); border-radius:var(--radius-sm); font-size:14px; background:#fff; color:var(--fg); font-family:var(--font-body); min-height:80px; resize:vertical; line-height:1.5; transition:all .15s` | `w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg min-h-[80px] resize-y leading-normal transition-all duration-150` |
| `form-textarea:focus` | 2660 | `outline:none; border-color:var(--accent); box-shadow:0 0 0 3px rgba(22,119,255,.1)` | `focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)]` |
| `form-textarea::placeholder` | 2661 | `color:var(--muted); opacity:.6` | `placeholder:text-muted placeholder:opacity-60` |
| `line-items-table .form-input` | 542-543 | `width:100%; padding:6px 8px; font-size:13px` | `w-full px-2 py-1.5 text-[13px]`（在 line-items-table 内的 form-input 上额外追加） |

### 表单辅助类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `form-label` | 1885 | `font-size:13px; font-weight:500; color:var(--fg)` | `text-[13px] font-medium text-fg` |
| `form-label .required` | 1886 | `color:var(--danger); margin-left:2px` | `text-danger ml-0.5` |
| `form-hint` | 1576 | `font-size:12px; color:var(--muted); margin-top:2px` | `text-xs text-muted mt-0.5` |
| `form-group` | 1884 | `display:flex; flex-direction:column; gap:var(--space-1)` | `flex flex-col gap-1` |
| `form-actions` | 2671 | `display:flex; align-items:center; justify-content:flex-end; gap:var(--space-3); padding:var(--space-5) 0` | `flex items-center justify-end gap-3 py-5` |
| `form-row` | 1815 | `margin-bottom:var(--space-4)` | `mb-4` |
| `form-row:last-child` | 1816 | `margin-bottom:0` | 追加: `last:mb-0` |
| `form-row > label` | 1817 | `display:block; font-size:13px; font-weight:500; color:var(--fg); margin-bottom:4px` | label 上: `block text-[13px] font-medium text-fg mb-1` |
| `form-row > label .req` | 1818 | `color:var(--danger); margin-left:2px` | `text-danger ml-0.5` |

### 复选框 / 密码类

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `form-check` | 2146 | `display:flex; align-items:center; gap:var(--space-2); font-size:13px; cursor:pointer; padding:6px 0` | `flex items-center gap-2 text-[13px] cursor-pointer py-1.5` |
| `form-check input` | 2147 | `width:16px; height:16px; accent-color:var(--accent)` | input 上: `w-4 h-4 accent-accent` |
| `form-check-hint` | 2148 | `font-size:11px; color:var(--muted); margin-left:var(--space-1)` | `text-[11px] text-muted ml-1` |
| `checkbox-row` | 1887 | `display:flex; align-items:center; gap:var(--space-2); font-size:13px; color:var(--fg); cursor:pointer; margin-top:6px` | `flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5` |
| `checkbox-row input[type="checkbox"]` | 1888 | `width:16px; height:16px; accent-color:var(--accent)` | input 上: `w-4 h-4 accent-accent` |
| `section-desc` | 1889 | `font-size:13px; color:var(--muted); margin-bottom:var(--space-4); line-height:1.6` | `text-[13px] text-muted mb-4 leading-relaxed` |
| `password-wrap` | 1890 | `position:relative` | `relative` |
| `password-wrap .form-input` | 1891 | `padding-right:36px` | form-input 上追加: `pr-9` |
| `password-toggle` | 1892 | `position:absolute; right:8px; top:50%; transform:translateY(-50%); background:none; border:none; color:var(--muted); cursor:pointer; padding:4px; display:flex; align-items:center; justify-content:center` | `absolute right-2 top-1/2 -translate-y-1/2 bg-transparent border-none text-muted cursor-pointer p-1 flex items-center justify-center` |
| `password-toggle:hover` | 1893 | `color:var(--fg-2)` | `hover:text-fg-2` |

### checkbox-label（伴随类，一并迁移）

| class | base.css 行 | 当前 CSS | 原子 class 串 |
|---|---|---|---|
| `checkbox-label` | 1143 | `display:inline-flex !important; align-items:center; gap:var(--space-2); cursor:pointer` | `inline-flex items-center gap-2 cursor-pointer` |
| `checkbox-label input[type="checkbox"]` | 1144 | `width:auto; accent-color:var(--accent)` | `w-auto accent-accent` |

---

## 涉及文件清单（74 个）

### 组件文件（3 个）
- `abt-web/src/components/customer_info.rs`
- `abt-web/src/components/entity_picker.rs`
- `abt-web/src/components/input_dialog.rs`

### 页面文件（71 个）
`bom_create.rs`, `bom_edit.rs`, `category_list.rs`, `customer_create.rs`, `customer_detail.rs`, `customer_edit.rs`, `department_list.rs`, `fms_expense_create.rs`, `fms_journal_create.rs`, `labor_process_dict_list.rs`, `md_work_calendar_create.rs`, `md_work_center_create.rs`, `mes_demand_pool_create.rs`, `mes_exception_list.rs`, `mes_inspection_create.rs`, `mes_inspection_detail.rs`, `mes_order_create.rs`, `mes_order_detail.rs`, `mes_plan_create.rs`, `mes_plan_detail.rs`, `mes_receipt_create.rs`, `mes_report_create.rs`, `misc_request_create.rs`, `om_outsourcing_create.rs`, `om_outsourcing_detail.rs`, `payment_request_create.rs`, `product_create.rs`, `product_detail.rs`, `product_list.rs`, `purchase_approval_rules.rs`, `purchase_demand_pool_create.rs`, `purchase_order_create.rs`, `purchase_order_edit.rs`, `purchase_quotation_create.rs`, `purchase_recon_create.rs`, `purchase_return_create.rs`, `purchase_settings.rs`, `qms_mrb_create.rs`, `qms_result_create.rs`, `qms_rma_create.rs`, `qms_spec_create.rs`, `quotation_create.rs`, `quotation_edit.rs`, `reconciliation_create.rs`, `role_create.rs`, `role_edit.rs`, `routing_create.rs`, `sales_order_create.rs`, `sales_order_edit.rs`, `sales_return_create.rs`, `shipping_create.rs`, `supplier_create.rs`, `supplier_detail.rs`, `supplier_edit.rs`, `supplier_price_catalog.rs`, `user_create.rs`, `user_detail.rs`, `user_edit.rs`, `wms_arrival_create.rs`, `wms_bin_create.rs`, `wms_conversion_create.rs`, `wms_cycle_count_create.rs`, `wms_lock_create.rs`, `wms_requisition_create.rs`, `wms_stock_in_create.rs`, `wms_stock_list.rs`, `wms_stock_out_create.rs`, `wms_transfer_create.rs`, `wms_warehouse_create.rs`, `wms_warehouse_detail.rs`

---

### Task 1: 高频表单结构类（form-section / form-section-title / form-grid / form-field）

**Files:**
- Modify: `abt-web/src/pages/purchase_settings.rs`（先改这一个做参照）
- Modify: `abt-web/src/pages/customer_create.rs`
- Modify: `abt-web/src/pages/customer_edit.rs`
- Modify: `abt-web/src/pages/bom_create.rs`
- Modify: `abt-web/src/pages/labor_process_dict_list.rs`
- Modify: `abt-web/src/pages/mes_demand_pool_create.rs`
- Modify: `abt-web/src/pages/fms_journal_create.rs`
- Modify: `abt-web/src/pages/md_work_calendar_create.rs`
- Modify: `abt-web/src/pages/md_work_center_create.rs`
- Modify: `abt-web/src/pages/mes_inspection_create.rs`
- Modify: `abt-web/src/components/customer_info.rs`
- Modify: `static/base.css:1111-1145`（删除 form-section-title / form-grid / form-field / checkbox-label 块）

- [ ] **Step 1: 替换 form-section**

将所有 `class="form-section"` 替换为：
```
class="bg-white border border-border rounded-md p-6 mb-6"
```

- [ ] **Step 2: 替换 form-section-title（基础版）**

将 `class="form-section-title"`（不在 form-section-card 内的）替换为：
```
class="text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft flex items-center gap-2"
```

注意：form-section-title 内的 svg 已通过 inline class（如 `w-[18px] h-[18px]`）设置尺寸与颜色，无需额外处理。

- [ ] **Step 3: 替换 form-grid（基础 2 列版）**

将 `class="form-grid"`（在 form-section / data-card 内、且无 .cols-2 修饰）替换为：
```
class="grid grid-cols-2 gap-x-6 gap-y-4 mb-6"
```

对于 `.form-section .form-grid`（在 form-section 内的 form-grid，需要 4 列），替换为：
```
class="grid grid-cols-4 gap-5 mb-0"
```

- [ ] **Step 4: 替换 form-grid 内的 span-2 / field-full**

将 `class="form-field span-2"` 中的 `span-2` 改为 `col-span-full`（基础 2 列 grid 时占满全行）。

在 4 列 grid 中（form-section / form-section-card 内），`span-2` 改为 `col-span-2`。

将 `class="form-field field-full"` 改为 `class="form-field col-span-full"`（不含 form-field 自身样式，见 Step 5）。

- [ ] **Step 5: 替换 form-field 及其子元素**

`form-field` 本身无样式，是后代选择器容器。策略：

(a) 将 `class="form-field"` 改为 `class="flex flex-col"`（保持块级布局，form-field 内 label/input 垂直排列）。如果 form-field 已有 span 类（如 `col-span-full`），合并为 `class="flex flex-col col-span-full"`。

(b) 在 form-field 内的 `<label>` 上，如果 label 没有 class，添加：`class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"`。如果 label 已有 class（如 `form-label` 或 `checkbox-label`），按对应映射处理。

(c) 在 form-field 内的 `<input>` / `<select>` / `<textarea>` 上，如果没有 class，添加：`class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]"`。对于 textarea 额外追加 `resize-y min-h-[72px]`。

注意：很多 form-field 内的 input/select 已有 `class="form-input"` 或 `class="form-select"`——这些在 Task 2 中统一处理。如果 input 既在 form-field 内又有 form-input class，只需保留 form-input 的原子化串（form-input 的样式与 form-field input 后代选择器一致）。

- [ ] **Step 6: 从 base.css 删除已迁移的 class**

删除以下行：
- 行 1111-1118（form-section-title 块 + svg 后代）
- 行 1119-1124（form-grid + .field-full + .span-2 + form-section 覆盖）
- 行 1131-1144（form-field label/input/select/textarea + focus + textarea + checkbox-label）
- 行 2670（.form-section .form-grid.cols-2 覆盖）

保留行 1125-1130（supplier-info-bar，不在本批次范围）。

- [ ] **Step 7: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

Expected: `[success] N utilities generated to static/app.css`

- [ ] **Step 8: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 9: 页面验证**

用 agent-browser 打开以下页面，检查表单结构渲染：
- `http://localhost:8000/admin/purchase/settings`（采购参数配置，含 form-section + form-grid + checkbox-row）
- `http://localhost:8000/admin/customers/new`（客户新建，含 data-card + form-section-title + form-grid + form-field）
- `http://localhost:8000/admin/mes/demand-pool/new`（需求计划新建，含 form-section + form-section-title svg + form-grid 4列）

检查点：
- form-section-title 字体大小、颜色、下划线、flex 布局
- form-grid 列数（2列基础 / 4列 form-section内）
- form-field label 字号、颜色、间距
- form-field input 边框、padding、focus 蓝色边框 + 阴影

---

### Task 2: 高频输入类（form-input / form-select / form-textarea）

**Files:**
- Modify: 所有 74 个文件中含 `class="form-input"` / `class="form-select"` / `class="form-textarea"` 的行
- Modify: `static/base.css:1819-1822`（form-input 块）
- Modify: `static/base.css:2257-2264`（form-select 块）
- Modify: `static/base.css:2659-2661`（form-textarea 块）
- Modify: `static/base.css:542-543`（line-items-table .form-input/.form-select 覆盖）
- Modify: `static/base.css:1242-1243, 2137-2139`（form-input-readonly / form-input-mono / form-input-disabled）

- [ ] **Step 1: 替换 form-input**

全局搜索 `class="form-input"`，替换为：
```
class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)] placeholder:text-muted placeholder:opacity-60"
```

对于 `class="form-input form-input-readonly"`，替换为：
```
class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)] opacity-50 cursor-not-allowed"
```

对于 `class="form-input form-input-mono"`，追加 `uppercase font-mono`。

对于 `class="form-input form-input-disabled"`，替换 disabled 变体为 `bg-surface text-muted cursor-not-allowed`。

- [ ] **Step 2: 替换 textarea.form-input**

`textarea class="form-input"` 替换为（在 form-input 基础上追加）：
```
class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)] min-h-[72px] resize-y"
```

- [ ] **Step 3: 替换 form-select**

全局搜索 `class="form-select"`，替换为：
```
class="h-9 w-full px-3 border border-border rounded-sm text-sm text-fg bg-white outline-none transition-colors duration-150 focus:border-accent focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12)]"
```

- [ ] **Step 4: 替换 form-textarea**

全局搜索 `class="form-textarea"`，替换为：
```
class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg min-h-[80px] resize-y leading-normal transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)] placeholder:text-muted placeholder:opacity-60"
```

- [ ] **Step 5: 处理 line-items-table 内的 form-input / form-select 覆盖**

行 542-543 定义了 `.line-items-table .form-input, .line-items-table .form-select { width:100%; padding:6px 8px; font-size:13px }`。

策略：在 line-items-table 内使用 form-input 的地方，原子化时直接用行内尺寸：
```
class="w-full px-2 py-1.5 text-[13px] border border-border rounded-sm bg-white text-fg transition-all duration-150 focus:outline-none focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)]"
```

涉及文件：`quotation_create.rs`, `quotation_edit.rs`, `sales_order_create.rs`, `sales_order_edit.rs`, `purchase_order_create.rs`, `purchase_order_edit.rs`, `purchase_quotation_create.rs`, `purchase_return_create.rs`, `sales_return_create.rs`, `reconciliation_create.rs` 中含 line-items-table 的行。

注意：部分文件可能已用 `.li-input` 系列 class（base.css 行 1918-1923），这些不在本批次范围（属 P9 域专属）。仅处理直接使用 `form-input`/`form-select` 在 line-items-table 内的情况。

- [ ] **Step 6: 从 base.css 删除**

删除：
- 行 1819-1822（form-input + focus + placeholder + textarea.form-input）
- 行 2257-2264（form-select + focus）
- 行 2659-2661（form-textarea + focus + placeholder）
- 行 542-543（line-items-table .form-input/.form-select 覆盖）
- 行 1242-1243（form-input-readonly / form-input-mono 重复定义）
- 行 2137-2139（form-input-readonly / form-input-mono / form-input-disabled）

- [ ] **Step 7: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 8: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 9: 页面验证**

用 agent-browser 打开：
- `http://localhost:8000/admin/customers/new` — 检查 form-input 边框/padding/focus 效果
- `http://localhost:8000/admin/purchase/settings` — 检查 form-select 高度/箭头
- `http://localhost:8000/admin/mes/inspections/new` — 检查 form-input 在 form-field 内的渲染

检查点：input focus 时蓝色边框 + 3px 阴影环；select 高度 36px；placeholder 颜色为 muted 半透明。

---

### Task 3: 表单辅助类（form-label / form-hint / form-group / form-actions / form-row）

**Files:**
- Modify: 所有含这些 class 的 Maud 文件（约 30 个，主要是 user_create/edit、role_create/edit、department_list、各 create/edit 页面）
- Modify: `static/base.css:1576`（form-hint）
- Modify: `static/base.css:1815-1818`（form-row 块）
- Modify: `static/base.css:1885-1886`（form-label + .required）
- Modify: `static/base.css:2671`（form-actions）

- [ ] **Step 1: 替换 form-label**

`class="form-label"` → `class="text-[13px] font-medium text-fg"`

`form-label` 内的 `<span class="required">` → `<span class="text-danger ml-0.5">`

- [ ] **Step 2: 替换 form-hint**

`class="form-hint"` → `class="text-xs text-muted mt-0.5"`

- [ ] **Step 3: 替换 form-group**

`class="form-group"` → `class="flex flex-col gap-1"`

- [ ] **Step 4: 替换 form-actions**

`class="form-actions"` → `class="flex items-center justify-end gap-3 py-5"`

- [ ] **Step 5: 替换 form-row（drawer 内的表单行）**

`class="form-row"` → `class="mb-4 last:mb-0"`

form-row 内的 `<label>`（无 class 时）→ `class="block text-[13px] font-medium text-fg mb-1"`

form-row 内 label 的 `<span class="req">` → `<span class="text-danger ml-0.5">`

- [ ] **Step 6: 从 base.css 删除**

删除：
- 行 1576（form-hint）
- 行 1815-1818（form-row + last-child + label + .req）
- 行 1885-1886（form-label + .required）
- 行 2671（form-actions）

- [ ] **Step 7: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 8: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 9: 页面验证**

打开 `http://localhost:8000/admin/users/new`：
- form-group 内 label（form-label）字号 13px、颜色 fg
- form-hint 字号 12px、颜色 muted
- required 星号红色

打开 `http://localhost:8000/admin/departments`（drawer 表单）：
- form-row 间距 16px、最后一行无底部间距
- form-row label 样式正确

---

### Task 4: form-section-card + 复选框/密码类

**Files:**
- Modify: `abt-web/src/pages/user_create.rs`
- Modify: `abt-web/src/pages/user_edit.rs`
- Modify: `abt-web/src/pages/role_create.rs`
- Modify: `abt-web/src/pages/role_edit.rs`
- Modify: `abt-web/src/pages/department_list.rs`（form-check / form-check-hint）
- Modify: `abt-web/src/pages/purchase_settings.rs`（checkbox-row）
- Modify: 其他含 checkbox-row / section-desc / password-wrap / password-toggle 的文件
- Modify: `static/base.css:1884-1893`（form-group 已在 Task 3 删，这里删 checkbox-row / section-desc / password-wrap / password-toggle）
- Modify: `static/base.css:2026-2030`（form-section-card 块）
- Modify: `static/base.css:2146-2148`（form-check / form-check-hint）
- Modify: `static/base.css:1937-1942`（form-section-card.flush 系列）

- [ ] **Step 1: 替换 form-section-card**

`class="form-section-card"` → `class="bg-white border border-border rounded-md p-6 mb-5 shadow-xs"`

- [ ] **Step 2: 替换 form-section-card 内的 form-section-title（覆盖版）**

在 form-section-card 内的 `class="form-section-title"` → `class="flex items-center gap-2 mb-5 pb-0 border-b-0 text-sm font-semibold text-fg"`

（无下划线、无底部 padding、底部间距 20px。svg 颜色通过 inline class 设置。）

- [ ] **Step 3: 替换 form-section-card 内的 form-grid（覆盖版）**

在 form-section-card 内的 `class="form-grid"` → `class="grid grid-cols-4 gap-5 mb-0"`

- [ ] **Step 4: 替换 form-check**

`class="form-check"` → `class="flex items-center gap-2 text-[13px] cursor-pointer py-1.5"`

form-check 内的 `<input type="checkbox">` → 追加 `class="w-4 h-4 accent-accent"`

- [ ] **Step 5: 替换 form-check-hint**

`class="form-check-hint"` → `class="text-[11px] text-muted ml-1"`

- [ ] **Step 6: 替换 checkbox-row**

`class="checkbox-row"` → `class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5"`

checkbox-row 内的 `<input type="checkbox">` → 追加 `class="w-4 h-4 accent-accent"`

- [ ] **Step 7: 替换 section-desc**

`class="section-desc"` → `class="text-[13px] text-muted mb-4 leading-relaxed"`

- [ ] **Step 8: 替换 password-wrap / password-toggle**

`class="password-wrap"` → `class="relative"`

password-wrap 内的 `class="form-input"` → 在 form-input 原子串后追加 `pr-9`

`class="password-toggle"` → `class="absolute right-2 top-1/2 -translate-y-1/2 bg-transparent border-none text-muted cursor-pointer p-1 flex items-center justify-center hover:text-fg-2"`

- [ ] **Step 9: 替换 checkbox-label**

`class="checkbox-label"` → `class="inline-flex items-center gap-2 cursor-pointer"`

checkbox-label 内的 `<input type="checkbox">` → 追加 `class="w-auto accent-accent"`

- [ ] **Step 10: 从 base.css 删除**

删除：
- 行 1884-1893（form-group 已删 / 剩余 checkbox-row / section-desc / password-wrap / password-toggle）

注意：行 1884 form-group 在 Task 3 已删。这里删除 1887-1893。

- 行 1937-1942（form-section-card.flush 系列）
- 行 2026-2030（form-section-card + .form-section-title 覆盖 + .form-grid 覆盖）
- 行 2146-2148（form-check / form-check-hint）

- [ ] **Step 11: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 12: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 13: 页面验证**

打开 `http://localhost:8000/admin/users/new`：
- form-section-card 白色背景、圆角、阴影
- form-section-title 在 card 内无下划线
- form-grid 4 列
- password-toggle 眼睛图标在 input 右侧居中
- 点击眼睛切换密码可见性
- checkbox-row 复选框 16px、accent 蓝色
- section-desc 灰色描述文字

打开 `http://localhost:8000/admin/departments` → 点击部门 → drawer：
- form-check 复选框样式
- form-check-hint 小字提示

---

### Task 5: FMS scoped 覆盖处理

**Files:**
- Modify: `static/base.css:3257-3265`（.fms-form-page .form-grid input/select focus 覆盖）
- Modify: 含 `class="fms-form-page"` 的页面（搜索确认）

- [ ] **Step 1: 搜索 fms-form-page 使用位置**

Run（用 search 工具）: 搜索 `abt-web/src` 中 `fms-form-page` 的引用。

- [ ] **Step 2: 处理 FMS focus 覆盖**

base.css 行 3257-3265 定义了 `.fms-form-page .form-grid input:focus` 的特殊 box-shadow（双环效果）。

策略：FMS 表单页面的 form-input / form-select 已经在 Task 2 中原子化为带 `focus:shadow-[0_0_0_3px_rgba(22,119,255,0.1)]`。FMS 版本需要替换为：
```
focus:shadow-[0_0_0_3px_rgba(37,99,235,0.12),0_0_0_1px_rgba(37,99,235,0.25)]
```

在 FMS 表单页面的 form-input / form-select 上，将 focus shadow 替换为上述双环值。可通过 Maud 中 `let fms_input_cls = "...";` 变量提取。

- [ ] **Step 3: 从 base.css 删除**

删除行 3257-3265（.fms-form-page .form-grid input/select transition + focus 覆盖）。

- [ ] **Step 4: 构建验证**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 5: 编译验证**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

- [ ] **Step 6: 页面验证**

打开 `http://localhost:8000/admin/fms/journals/new`：
- form-input focus 时双环阴影效果

---

### Task 6: 最终构建与回归验证

**Files:**
- 无新文件修改（验证步骤）

- [ ] **Step 1: 全量构建**

Run: `cd E:/work/abt && npm run build:css`

- [ ] **Step 2: 全量编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error"`

Expected: 无 error

- [ ] **Step 3: 确认 base.css 已删除所有 P1 class**

Run（用 search 工具）: 在 `static/base.css` 中搜索以下 class，确认全部不存在：
`form-field`, `form-input`, `form-select`, `form-label`, `form-grid`, `form-section`, `form-section-title`, `form-section-card`, `form-actions`, `form-textarea`, `form-hint`, `form-group`, `form-row`, `form-check`, `form-check-hint`, `checkbox-row`, `section-desc`, `password-wrap`, `password-toggle`, `checkbox-label`, `form-input-readonly`, `form-input-mono`, `form-input-disabled`

Expected: 全部 0 匹配（除非在注释中）。

- [ ] **Step 4: 回归验证 5 个代表性页面**

用 agent-browser 逐一打开并截图对比：

| 页面 | URL | 检查重点 |
|---|---|---|
| 采购参数配置 | `/admin/purchase/settings` | form-section-title + form-grid 2列 + form-input + form-select + checkbox-row |
| 客户新建 | `/admin/customers/new` | data-card 内 form-section-title + form-grid + form-field label/input |
| 用户新建 | `/admin/users/new` | form-section-card + form-grid 4列 + form-group + password-wrap + checkbox-row + section-desc |
| 日记账新建 | `/admin/fms/journals/new` | form-section + form-label + form-input + form-select + FMS focus 双环 |
| 部门 drawer | `/admin/departments` | form-row + form-input + form-check + form-check-hint |

- [ ] **Step 5: 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P1 — migrate form controls to atomic UnoCSS

Migrate 18 form control classes from base.css to inline atomic UnoCSS:
form-field, form-input, form-select, form-label, form-grid, form-section,
form-section-title, form-section-card, form-actions, form-textarea, form-hint,
form-group, form-row, form-check, form-check-hint, checkbox-row, section-desc,
password-wrap, password-toggle.

Updated 74 Maud files (3 components + 71 pages).
Deleted ~80 lines of CSS from base.css.
Verified: build:css success, cargo clippy clean, 5 pages render-checked."
```
