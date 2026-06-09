# 采购模块测试报告

**测试日期**: 2026-06-09
**测试范围**: 采购模块（19 个页面，含付款申请、零星请购）
**测试数据**: 数据库现有数据 + 测试创建数据
**测试层级**: Full（页面加载 + 表单交互 + 状态流转 + 完整提交）

## 测试总览

| 页面 | 路径 | 状态 | 备注 |
|------|------|------|------|
| 采购总览 | /admin/purchase | ✅ | 仪表盘、快捷入口、最近活动正常 |
| 采购报价列表 | /admin/purchase/quotations | ✅ | 筛选栏、状态 Tab、搜索、删除操作正常 |
| 采购报价新建 | /admin/purchase/quotations/create | ✅ | 修复 items_json + 重复字段名后完整提交流程通过 |
| 采购报价详情 | /admin/purchase/quotations/{id} | ✅ | 激活报价、转采购订单、取消、删除按钮正常 |
| 采购订单列表 | /admin/purchase/orders | ✅ | 修复金额格式后正常 |
| 采购订单新建 | /admin/purchase/orders/create | ✅ | 修复预计到货默认值后正常 |
| 采购订单详情 | /admin/purchase/orders/{id} | ✅ | 修复零值显示后正常，确认/取消按钮仅在草稿状态显示 |
| 采购对账列表 | /admin/purchase/reconciliations | ✅ | 空数据提示正常 |
| 采购对账新建 | /admin/purchase/reconciliations/create | ✅ | 表单结构完整 |
| 采购退货列表 | /admin/purchase/returns | ✅ | 空数据提示正常 |
| 采购退货新建 | /admin/purchase/returns/create | ✅ | 修复 JS 缺失后订单行加载、供应商填充、行项目渲染正常 |
| 采购退货详情 | /admin/purchase/returns/{id} | ⏭ | 无退货数据，无法测试 |
| 采购对账详情 | /admin/purchase/reconciliations/{id} | ⏭ | 无对账数据，无法测试 |
| 付款申请列表 | /admin/purchase/payments | ✅ | 筛选栏、状态 Tab、付款方式筛选正常 |
| 付款申请新建 | /admin/purchase/payments/create | ✅ | 供应商自动填充、完整提交流程通过，HX-Redirect 正确 |
| 付款申请详情 | /admin/purchase/payments/{id} | ✅ | 标题、审批/取消按钮正常 |
| 零星请购列表 | /admin/purchase/misc-requests | ✅ | 部门筛选、数据正确 |
| 零星请购新建 | /admin/purchase/misc-requests/create | ✅ | 表单结构完整（部门、用途、行项目） |
| 零星请购详情 | /admin/purchase/misc-requests/{id} | ✅ | 行项目显示正确 |

## 缺陷记录

### P0 阻塞（1 项 → 全部修复 ✅）

| # | 问题 | 修复方案 | 涉及文件 |
|---|------|---------|----------|
| 1 | 订单详情不存在的 ID 返回 "PurchaseOrder" 而非友好消息 | 所有采购子模块 implt.rs 添加 ENTITY_DISPLAY 常量，not_found 使用友好名 | abt-core/src/purchase/*/implt.rs（6 个文件） |

### P1 严重（3 项 → 全部修复 ✅）

| # | 问题 | 修复方案 | 涉及文件 |
|---|------|---------|----------|
| 2 | 报价单新建 items_json 始终为空导致提交无反应 | 表单改用 onsubmit HTML 属性收集数据（优先于 HTMX addEventListener） | purchase_quotation_create.rs |
| 3 | 退货新建选择订单后供应商/订单行不渲染 | 创建缺失的 return-create.js（HTMX afterSettle 处理数据、填充表单、渲染行项目） | static/return-create.js |
| 7 | 报价单新建行项目 name 与主表单字段重复导致 422 | 行项目 name 加 `item_` 前缀，onsubmit JS 收集时去掉前缀 | purchase_quotation_create.rs |

### P2 一般（2 项，已记录未修复）

| # | 问题 | 状态 | 涉及文件 |
|---|------|------|----------|
| 4 | 订单列表总金额多余小数位（1000.0000） | ✅ 已修复 | purchase_order_list.rs |
| 5 | 订单详情已收货/已检验/已退货零值应显示 — | ✅ 已修复 | purchase_order_detail.rs |
| 8 | 零星请购列表预估金额 `3.0000` 多余小数位 | ✅ 已修复 | misc_request_list.rs |
| 9 | 零星请购详情数量列 `1.000000` 多余小数位 | ✅ 已修复 | misc_request_detail.rs |

### P3 轻微（1 项 → 全部修复 ✅）

| # | 问题 | 修复方案 | 涉及文件 |
|---|------|---------|----------|
| 6 | 订单新建预计到货日期默认为 0/0/0 | 默认值设为订单日期 +15 天 | purchase_order_create.rs |

## 交互测试验证

| 测试项 | 结果 |
|--------|------|
| 报价单搜索筛选（HTMX keyup） | ✅ HTMX 配置正确 |
| 报价单删除草稿 | ✅ API 返回 200 + HX-Redirect |
| 报价单激活报价（草稿→已生效） | ✅ 状态变更正确 |
| 报价单完整提交流程（新建→详情跳转） | ✅ 创建成功并跳转 |
| 退货新建选择订单后供应商/订单行填充 | ✅ JS afterSettle 正常处理 |
| 付款申请完整提交流程（新建→HX-Redirect→详情） | ✅ 创建成功 |
| 订单详情状态按钮（草稿显示确认/取消，其他状态隐藏） | ✅ 逻辑正确 |

## 同类排查扩展

P0 修复同时覆盖了采购模块全部 6 个子模块的 not_found 错误消息：
- 采购订单 → "采购订单"
- 采购报价单 → "采购报价单"
- 采购退货单 → "采购退货单"
- 采购对账单 → "采购对账单"
- 付款申请 → "付款申请"
- 零星请购 → "零星请购"

## 待修复项

全部修复完成，无待修复项。
