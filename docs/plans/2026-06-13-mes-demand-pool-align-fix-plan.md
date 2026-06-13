# MES 需求池页面对齐修复计划

**日期**：2026-06-13 | **范围**：MES 需求池列表页 | **待修复项**：5

## 总览

| # | 严重度 | 检查项 | 问题描述 | 代码位置 | 修复方式 |
|---|--------|--------|----------|---------|----------|
| 1 | 🔴 | 物料行 script 文本暴露 | Surreal.js script 作为 div 直接子元素，文本被浏览器渲染为可见内容 | `mes_demand_pool.rs:529-536` | 将 script 放在 div 首个子元素之前（Surreal.js me() 绑定到父 div） |
| 2 | 🔴 | "创建生产计划"按钮样式偏差 | `<form>` 包裹 `<button>` 导致 flex 布局下尺寸异常，原型用 `<a>` 链接 | `mes_demand_pool.rs:571-575` | 改用 `<a>` 链接，href 拼接 product_id |
| 3 | 🟡 | 统计卡片图标不匹配 | 第1个卡片用 clipboard_list 应为 gear/settings，第2个卡片用 box 应为 cube | `mes_demand_pool.rs:361,370` | 改用 `tool_icon` 和 `cube_icon` |
| 4 | 🟡 | batch-bar 按钮类型 | "创建生产计划"用 `<a>` 而原型用 `<button>`，导致 `.batch-bar .btn` CSS 选择器匹配不同 | `mes_demand_pool.rs:762-768` | 改用 `<button>` + onclick 跳转 |
| 5 | ⚪ | 视图切换 `<a>` vs `<button>` | HTMX 需要 `<a>` 标签处理 GET+pushUrl，原型用 `<button>` | `mes_demand_pool.rs:412-433` | **不修复**（功能正确，HTMX 要求） |

**涉及文件**：`abt-web/src/pages/mes_demand_pool.rs`, `abt-web/src/pages/purchase_demand_pool.rs`

## 逐项修复指引

### 1. 🔴 修复 script 文本暴露

**文件**：`mes_demand_pool.rs` material_row 函数

**当前代码**（script 在 div.material-row 内部作为直接子元素，被渲染为文本）：
```rust
div class="material-row" {
    (PreEscaped(format!("<script>me().on('click',...)</script>")))
    div class="material-info" { ... }
    ...
}
```

**修复**：将 script 放在 `material-info` div 内部作为首元素（或用 Surreal.js 标准 `me().on` 绑定到 material-row 的 onclick 事件）

### 2. 🔴 "创建生产计划"改用 `<a>` 链接

**文件**：`mes_demand_pool.rs` material_row 函数的 material-actions

**当前代码**：
```rust
form method="get" action=(MesDemandPoolCreatePath::PATH) onclick="event.stopPropagation()" {
    input type="hidden" name="product_id" value=(pid) {}
    button type="submit" class="btn btn-primary btn-sm" { "创建生产计划" }
}
```

**修复**：
```rust
a class="btn btn-primary btn-sm"
    href=(format!("{}?product_id={}", MesDemandPoolCreatePath::PATH, pid))
    onclick="event.stopPropagation()" {
    "创建生产计划"
}
```

### 3. 🟡 统计卡片图标修正

**文件**：`mes_demand_pool.rs` stat_mini_cards 函数

**修复**：
- 第1个卡片：`icon::clipboard_list_icon` → `icon::tool_icon`（黄色齿轮）
- 第2个卡片：`icon::box_icon` → `icon::cube_icon`（蓝色立方体）

### 4. 🟡 batch-bar 按钮修正

**文件**：`mes_demand_pool.rs` batch_action_bar 函数

**修复**：`<a>` → `<button type="button">` + onclick 跳转
