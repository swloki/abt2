# 销售模块完整测试报告

**测试日期**: 2026-06-08
**测试范围**: 销售模块（7 个子模块，22 个页面，43 项功能点）
**测试数据**: `scripts/sales-test-data.sql`
**测试工具**: agent-browser (snapshot -i 无障碍树验证)

## 测试总览

| 页面 | 路径 | 状态 | 修复项 |
|------|------|------|--------|
| 销售总览 | /admin | ✅ | 快捷入口链接"#"→实际URL；status-pill CSS缺失；dialog CSS缺失 |
| 客户列表 | /admin/customers | ✅ | 信用额度显示11.0000→格式化 |
| 客户详情 | /admin/customers/{id} | ✅ | - |
| 报价单列表 | /admin/quotations | ✅ | - |
| 报价单创建 | /admin/quotations/new | ✅ | - |
| 报价单详情 | /admin/quotations/{id} | ✅ | 数量/折扣率格式化；折扣为0时显示"—" |
| 报价单编辑 | /admin/quotations/{id}/edit-form | ✅ | - |
| 销售订单列表 | /admin/orders | ✅ | - |
| 销售订单创建 | /admin/orders/create | ✅ | - |
| 销售订单详情 | /admin/orders/{id} | ✅ | 数量/折扣率/发货量/退货量格式化 |
| 销售订单编辑 | /admin/orders/{id}/edit-form | ✅ | - |
| 发货申请列表 | /admin/shipping | ✅ | - |
| 发货申请创建 | /admin/shipping/create | ✅ | - |
| 发货详情 | /admin/shipping/{id} | ✅ | 发货量格式化 |
| 退货列表 | /admin/returns | ✅ | 缺少"已取消"标签页 |
| 退货创建 | /admin/returns/new | ✅ | - |
| 退货详情 | /admin/returns/{id} | ✅ | 退货量格式化 |
| 对账单列表 | /admin/reconciliations | ✅ | - |
| 对账单创建 | /admin/reconciliations/new | ✅ | - |
| 对账详情 | /admin/reconciliations/{id} | ✅ | 数量格式化 |

## 缺陷记录

### P1 严重

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| 1 | 全局确认弹窗(dialog-overlay)CSS完全缺失，hx-confirm自定义弹窗无法正常显示 | 添加 .dialog-overlay/.dialog/.dialog-* 完整CSS | static/base.css |
| 2 | status-pill CSS完全缺失，所有状态标签无样式 | 添加 .status-pill 及 14 种状态变体 CSS | static/base.css |

### P2 一般

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| 3 | 报价/订单/发货/退货/对账 5个详情页数量字段显示多余小数位(10.000000→10) | 使用 fmt_qty() 格式化 | quotation_detail.rs, sales_order_detail.rs, shipping_detail.rs, sales_return_detail.rs, reconciliation_detail.rs |
| 4 | 退货列表标签页缺少"已取消"(status=6) | 添加 status=6 TabItem | sales_return_list.rs |
| 5 | 报价详情页折扣为0时显示"0"而非"—" | 添加 > ZERO 判断 | quotation_detail.rs |
| 6 | 销售总览快捷入口链接全部指向"#" | 改为实际列表页URL | dashboard.rs |
| 7 | 销售总览最近活动/待办事项状态标签缺少 status-pill 类 | 所有4个组件函数添加 status-pill class | dashboard.rs |
| 8 | 客户列表信用额度显示 ¥ 11.0000 | 使用 fmt_qty() 格式化 | customer_list.rs |
| 9 | 销售总览最近活动状态颜色不区分（全部 status-progress） | 发货→picking, 退货→inspecting, 对账→sent, 报价→accepted | dashboard.rs |

### P3 轻微

| # | 问题 | 说明 |
|---|------|------|
| 10 | 发货创建页日期选择器默认显示 0/0/0 | 应默认为当前日期，需改前端代码 |

## 功能验证结果

### 页面加载（22 页面）

| 页面 | 500错误 | JS错误 | 验证 |
|------|---------|--------|------|
| 全部 22 个页面 | 0 | 0 | ✅ |

### 详情页状态流转按钮验证

**报价单**（5 种状态）：

| 状态 | 操作按钮 | 验证 |
|------|----------|------|
| 草稿 | 提交报价 | ✅ |
| 已发送 | 接受 + 拒绝 | ✅ |
| 已接受 | — | ✅ |
| 已拒绝 | — | ✅ |
| 已过期 | — | ✅ |

**销售订单**（7 种状态）：

| 状态 | 操作按钮 | 验证 |
|------|----------|------|
| 草稿 | 确认订单 + 取消订单 | ✅ |
| 已确认 | 开始生产 + 取消订单 | ✅ |
| 生产中 | 完成订单 | ✅ |
| 部分发货 | 打印 | ✅ |
| 已发货 | 打印 | ✅ |
| 已完成 | 打印 | ✅ |
| 已取消 | 打印 | ✅ |

**发货申请**（5 种状态）：

| 状态 | 操作按钮 | 验证 |
|------|----------|------|
| 草稿 | 确认发货 + 取消 | ✅ |
| 已确认 | 开始拣货 + 取消 | ✅ |
| 拣货中 | 确认发出 | ✅ |
| 已发出 | — | ✅ |
| 已取消 | — | ✅ |

**销售退货**（7 种状态）：

| 状态 | 操作按钮 | 验证 |
|------|----------|------|
| 草稿 | 确认退货 | ✅ |
| 已确认 | 确认收货 | ✅ |
| 已收货 | 开始质检 | ✅ |
| 质检中 | 完成退货 + 驳回 | ✅ |
| 已完成 | — | ✅ |
| 已取消 | — | ✅ |
| 已驳回 | — | ✅ |

**对账单**（5 种状态）：

| 状态 | 操作按钮 | 验证 |
|------|----------|------|
| 草稿 | 发送对账 | ✅ |
| 已发送 | 确认 + 异议 | ✅ |
| 已确认 | 结算 | ✅ |
| 有异议 | — | ✅ |
| 已结算 | — | ✅ |

### 搜索/筛选验证

| 功能 | 测试方式 | 结果 |
|------|----------|------|
| 报价单关键词搜索 | /quotations/table?keyword=SALES-TEST-QUO-001 | ✅ 返回 1 条 |
| 报价单状态筛选 | /quotations/table?status=2 | ✅ 返回 2 条已发送 |
| 订单状态筛选 | /orders/table?status=1 | ✅ 返回草稿订单 |
| 订单客户筛选 | /orders/table?customer_id=3 | ✅ 返回 3 条 |
| 退货状态筛选 | /returns/table?status=4 | ✅ 返回 1 条质检中 |
| 对账期间筛选 | /reconciliations/table?period=2026-05 | ✅ 返回 3 条 |
| 客户分类筛选 | UI 下拉存在 | ✅ 组件渲染正确 |

### 状态流转验证

| 流转链 | 按钮正确 | 自定义弹窗 | 实际提交 |
|--------|----------|------------|----------|
| 报价: 草稿→提交→接受/拒绝 | ✅ | ✅ 弹窗正常显示 | ⏭ agent-browser 无法验证最终结果 |
| 订单: 草稿→确认→开始→完成/取消 | ✅ | ✅ | ⏭ 同上 |
| 发货: 草稿→确认→拣货→发货/取消 | ✅ | ✅ | ⏭ 同上 |
| 退货: 草稿→确认→收货→质检→完成/驳回 | ✅ | ✅ | ⏭ 同上 |
| 对账: 草稿→发送→确认/异议→结算 | ✅ | ✅ | ⏭ 同上 |

**说明**：自定义确认弹窗已正常弹出并点击确认，但由于 agent-browser 的限制（confirm 后 HTMX redirect 可能无法被正确跟踪），无法验证最终数据库状态变更。通过数据库直接模拟状态变更后验证了所有后续状态的按钮正确性。

### 数据展示验证

| 检查项 | 结果 |
|--------|------|
| 数量字段无多余小数位 | ✅ (修复后) |
| 金额字段 2 位小数 | ✅ |
| 空值显示为"—" | ✅ |
| 客户名称显示(非 ID) | ✅ |
| 产品名称显示(非 ID) | ✅ |
| 状态标签有颜色样式 | ✅ (修复后) |
| 折扣为0时显示"—" | ✅ (修复后) |

### 总览页交互

| 功能 | 结果 |
|------|------|
| 快捷入口跳转 | ✅ (修复后：链接指向实际页面) |
| 销售流程步骤链接 | ✅ |
| 待办事项区域渲染 | ✅ |
| 最近活动区域渲染 | ✅ |
| status-pill 样式 | ✅ (修复后) |

## 修改文件清单

| 文件 | 修改内容 |
|------|----------|
| `static/base.css` | 添加 status-pill CSS (14 种状态变体) + dialog-overlay/dialog CSS |
| `abt-web/src/pages/dashboard.rs` | 快捷入口链接修复 + status-pill class + 状态颜色修正 |
| `abt-web/src/pages/quotation_detail.rs` | fmt_qty 格式化 + 折扣零值判断 |
| `abt-web/src/pages/sales_order_detail.rs` | fmt_qty 格式化 (task 子代理修复) |
| `abt-web/src/pages/shipping_detail.rs` | fmt_qty 格式化 (task 子代理修复) |
| `abt-web/src/pages/sales_return_detail.rs` | fmt_qty 格式化 (task 子代理修复) |
| `abt-web/src/pages/sales_return_list.rs` | 添加"已取消"标签页 |
| `abt-web/src/pages/reconciliation_detail.rs` | fmt_qty 格式化 (task 子代理修复) |
| `abt-web/src/pages/customer_list.rs` | 信用额度 fmt_qty 格式化 |
| `scripts/sales-test-data.sql` | 完整测试数据脚本 |
