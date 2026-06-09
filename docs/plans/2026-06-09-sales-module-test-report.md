# 销售管理模块测试报告（深度测试）

**测试日期**: 2026-06-09
**测试范围**: 销售管理模块 5 个新建表单深度功能测试
**测试数据**: `scripts/sales-test-data.sql` + 测试中动态创建的数据

## 测试总览

| 新建表单 | 路径 | 深度测试 | 修复项 |
|---------|------|---------|--------|
| 报价单新建 | /admin/quotations/new | ✅（上次已完成） | — |
| 销售订单新建 | /admin/orders/create | ✅ | 0 |
| 发货申请新建 | /admin/shipping/create | ✅ | 2 项 |
| 退货单新建 | /admin/returns/new | ✅ | 2 项 |
| 对账单新建 | /admin/reconciliations/new | ✅ | 4 项 |

## 新建表单功能验证

| 表单 | 客户选择 | 关联单据选择 | 数据填写 | 金额计算 | 提交成功 | 详情验证 |
|------|---------|------------|---------|---------|---------|---------|
| 销售订单 | ✅ 联系人+电话自动填充 | ✅ 产品Modal选择 | ✅ | ✅ 小计/总额 | ✅ → /admin/orders/45 | ✅ SO-2026-06-000002 |
| 发货申请 | ✅ | ✅ 订单Modal选择 | ✅ 发货数量+仓库 | ✅ 汇总统计 | ✅ → /admin/shipping/20 | ✅ SR-2026-06-000001 |
| 退货单 | ✅ | ✅ 订单下拉选择 | ✅ 退货数量 | ✅ 小计/总额 | ✅ → /admin/returns/25 | ✅ SRT-2026-06-000001 |
| 对账单 | ✅ | ✅ 期间预览 | — | ✅ 汇总金额 | ✅ → /admin/reconciliations/23 | ✅ REC-2026-06-000001 |

## 缺陷记录与修复

### P1 严重（阻塞表单提交）

| # | 页面 | 问题 | 修复方案 | 文件 |
|---|------|------|----------|------|
| 1 | 发货申请 | 客户 select 缺少 `name="customer_id"`，表单提交时 customer_id=0 → 服务端验证失败 | 添加 `name="customer_id"` | `shipping_create.rs` |
| 2 | 退货单 | IIFE 闭包函数（calcRow/removeRow/addReturnRow/handleSubmit）未暴露到全局作用域，内联事件处理器 `oninput="calcRow(this)"` 等无法调用 | 添加 `window.xxx = xxx` 全局导出 | `sales_return_create.rs` |
| 3 | 对账单 | `triggerPreview()` 函数未定义，客户/期间 onchange 无法触发预览加载 | 在页面 JS 中添加 `triggerPreview()` 函数 | `reconciliation_create.rs` |
| 4 | 对账单 | `preview_empty()` 和 `preview_table()` 返回的 HTML 缺少 HTMX 属性，swap 后无法再次触发预览 | 在两个函数的外层 div 上添加完整的 hx-get/hx-trigger/hx-include/hx-target/hx-swap 属性 | `reconciliation_create.rs` |
| 5 | 对账单 | 表单有两个 `name="remark"` 字段（input + textarea），Axum Form 反序列化报 422 | 移除基本信息区 input 的 name 属性 | `reconciliation_create.rs` |

### P2 一般

| # | 页面 | 问题 | 修复方案 | 文件 |
|---|------|------|----------|------|
| 6 | 发货申请 | `handleSubmit()` 使用 `me('#shipping-form')` 在外部 JS 中不可靠 | 改为 `document.getElementById('shipping-form')` | `shipping-create.js` |
| 7 | 退货单 | 提交按钮 surreal.js 使用 `me('#return-form')` 在回调内不可靠 | 改为 `document.getElementById('return-form')` | `sales_return_create.rs` |
| 8 | 对账单 | 提交按钮 surreal.js 使用 `me('#rec-create-form')` 在回调内不可靠 | 改为 `document.getElementById('rec-create-form')` | `reconciliation_create.rs` |

### 未修复的已知问题

| # | 页面 | 问题 | 级别 |
|---|------|------|------|
| 1 | 发货申请 | 初始客户 select 没有 HTMX hx-get 属性，联系人/电话不会自动填充 | P2 |
| 2 | 发货申请 | 订单关联的第 3 行产品编码/名称为空（数据库中产品数据不完整） | P3 |

## 设计决策

- **shipping-create.js 保留为外部 JS 文件**：该文件包含复杂的状态管理（selectedCustomer/selectedOrder）、动态 DOM 生成（fillItemsTable）、多函数协作（getWarehouseOptions/updateTotals/collectItems），属于 CLAUDE.md 定义的"不能一两行 surreal.js 表达的逻辑"场景，外部 JS 是正确的做法。

## 测试环境

- **应用**: http://localhost:8000
- **数据库**: PostgreSQL abt_v2
- **编译验证**: `cargo clippy` 全部通过（58 warnings 均为预存）
- **浏览器测试**: agent-browser (headless Chrome)
- **交互限制**: agent-browser click 不触发 JS 事件监听器（onclick），测试中使用 eval 替代
