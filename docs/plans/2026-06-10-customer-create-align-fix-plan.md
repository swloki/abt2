# 页面对齐修复计划 — 客户创建页

**日期**：2026-06-10 | **范围**：客户管理 / 新建客户页 | **待修复项**：8

## 总览

| 页面 | 类型 | 原型 | 实现 | 浏览器差异 | 代码定位 | 🔴 | 🟡 |
|------|------|------|------|-----------|---------|-----|-----|
| 新建客户 | 创建页 | customer-create.html | customer_create.rs | 8 项 | 8 项 | 2 | 6 |

**整体匹配度：73%** | **待修复：8 项**

## 逐项修复清单

### 🔴 严重（结构性缺失）

| # | 严重度 | 检查项 | 问题描述 | 代码位置 | 修复方式 |
|---|--------|--------|----------|---------|----------|
| 1 | 🔴 | C4 其他信息-字段 | 缺少"负责业务员" select 字段 | `customer_create.rs:267-286` | 在"其他信息"区的 form-grid 中、客户来源之前添加负责业务员 select |
| 2 | 🔴 | C10 底部操作栏 | 缺少"保存并继续"按钮（原型有3个按钮：取消、保存并继续、保存客户） | `customer_create.rs:288-294` | 在"取消"和"保存客户"之间添加"保存并继续"按钮（btn-default），点击后保存并重定向回新建页 |

### 🟡 轻微（属性/样式偏差）

| # | 严重度 | 检查项 | 问题描述 | 代码位置 | 修复方式 |
|---|--------|--------|----------|---------|----------|
| 3 | 🟡 | C5 必填-联系人 | input 缺少 `required` 属性 | `customer_create.rs:203` | 添加 `required` 属性 |
| 4 | 🟡 | C5 必填-手机号码 | input 缺少 `required` 属性 | `customer_create.rs:211` | 添加 `required` 属性 |
| 5 | 🟡 | C4 付款条款默认值 | 默认值"--请选择--"应改为"月结 30 天" | `customer_create.rs:243` | 将"月结30天" option 改为 selected，移除"--请选择--" |
| 6 | 🟡 | C10 取消按钮类型 | `<a>` 应改为 `<button type="button">` | `customer_create.rs:290` | 改为 `<button type="button" class="btn btn-default">` 配合 onclick 跳转 |
| 7 | 🟡 | 内联样式-卡片 | 4处 `style="margin-bottom:var(--space-4)"` 违反禁止内联样式规则，且 `.data-card` 已自带该 margin | `customer_create.rs:145,198,233,267` | 移除所有内联 `style` 属性 |
| 8 | 🟡 | 内联样式-备注 | `style="width:100%;min-height:80px;resize:vertical"` 应改用 `rows="4"` | `customer_create.rs:282` | 移除内联样式，改用 `rows="4"` |

## 涉及文件

- `abt-web/src/pages/customer_create.rs` — 主要修改（8项全在此文件）
- `abt-web/src/routes/customer.rs` — 可能需要添加"保存并继续"路由处理

## 修复顺序

1. 先修 🟡 轻微项（#3-#8）— 属性和样式，不影响功能
2. 再修 🔴 严重项（#1-#2）— 结构性添加，涉及功能逻辑

## 验证方式

每项修复后：
1. `cargo clippy -p abt-web` 编译检查
2. 刷新页面 `https://localhost:8000/admin/customers/new` 确认渲染
3. `agent-browser snapshot` 确认差异已消除
