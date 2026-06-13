# 页面对齐修复计划 — MES 生产需求池

**日期**：2026-06-12 | **范围**：1 页面 | **已修复项**：2

## 总览

| 页面 | 原型 | 实现 | 🔴 | 🟡 |
|------|------|------|-----|-----|
| 生产需求池 | 04-demand-pool.html | mes_demand_pool.rs | 2 | 0 |

**整体匹配度：95%** | **已修复：2 项**

## 修复记录

### 1. 生产需求池（原型：04-demand-pool.html → 实现：mes_demand_pool.rs）

| # | 严重度 | 检查项 | 问题描述 | 修复方式 | 状态 |
|---|--------|--------|----------|----------|------|
| 1 | 🔴 | 视图切换按钮元素类型 | 物料汇总/订单行明细用了 `<a>` 链接，原型用 `<button>` | `mes_demand_pool.rs` L412-433: `<a>` → `<button type="button">` | ✅ |
| 2 | 🔴 | material-actions 按钮大小 | `.btn-sm` 的 CSS 在 base.css 中位于 `.btn` 之前，被 `.btn` 的 padding/font-size 覆盖，导致按钮比原型大（41px vs 30px） | `static/base.css`: 将 `.btn-sm` 和 `.btn svg` 移到 `.btn` 变体定义之后，确保后定义的 `.btn-sm` 覆盖 `.btn` 的值 | ✅ |

## 涉及文件

- `abt-web/src/pages/mes_demand_pool.rs` — 视图切换 `<a>` → `<button>`
- `static/base.css` — `.btn-sm` CSS 位置调整

## 不修改项

- 全局元素（sidebar、header、面包屑）— 使用全局控件
- 分页组件 — 使用全局 pagination 组件，不新建
- CSS 类定义值 — stat-mini、material-row、view-toggle-btn 等定义与原型完全一致
- 图标尺寸 — prototype 与实现已一致（stat-mini-icon 38×38 + svg 18×18, view-toggle-btn svg 15×15, btn svg 16×16）
