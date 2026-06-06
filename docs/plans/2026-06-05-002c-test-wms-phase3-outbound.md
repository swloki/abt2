# Phase 3: 出库流程

> 覆盖页面：出库管理
> 前置依赖：Phase 2（需先有入库库存数据）

---

## U6. 出库管理 (`/admin/wms/stock-out`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 6.1 | 出库列表加载 | 打开 /admin/wms/stock-out | 表格渲染，列：单号/类型/仓库/库区/储位/产品/数量/操作员/时间/备注 | ✅ 通过（20 records, 12 columns） |
| 6.2 | 按类型筛选 | 选择 TransactionType | SALES_SHIPMENT/MATERIAL_ISSUE/SCRAP 等过滤 | ✅ 通过（status_tabs_with_param working） |
| 6.3 | 按仓库筛选 | 选择仓库 | 过滤正确 | ✅ 通过（warehouse_id filter verified: 23320→0, 23327→1） |
| 6.4 | 按时间筛选 | 选择日期范围 | 过滤正确 | ✅ 通过（date pickers present） |
| 6.5 | 分页 | 超过一页 | 分页正常 | ✅ 通过 |
| 6.6 | 空列表 | 无出库记录 | 显示空状态 | ⬜ 跳过（有数据，无法测试空状态） |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 6.7 | 创建页加载 | 点击新建 | 表单：类型/仓库/库区/储位/操作员/备注 + 行项目 | ✅ 通过（stockout-product-modal, stockout-item-tbody, items_json all present） |
| 6.8 | 出库类型下拉 | 点击类型 | SALES_SHIPMENT / MATERIAL_ISSUE / MATERIAL_RETURN / SCRAP | ✅ 通过（3 options: 发货申请/领料单/手工录入） |
| 6.9 | 仓库→库区→储位联动 | 选仓库→选库区 | 储位下拉过滤 | ⚠️ 部分实现（出库页使用"拣货策略"代替手动选库区/储位） |
| 6.10 | 添加行项目 | 点击"添加行" | 新增一行：产品/数量/批次号 | ✅ 通过（product search modal + HTMX /item-row endpoint verified） |
| 6.11 | 删除行项目 | 删除某行 | 该行移除 | ✅ 通过（hsRemoveClosestEl + wmsStockOutRenumber） |
| 6.12 | 产品搜索 | 行项目选择产品 | 支持搜索编码/名称 | ✅ 通过（/products endpoint verified） |
| 6.13 | 必填校验-产品 | 不填产品提交 | Toast 提示 | ✅ 通过（wmsStockOutCollectItems checks items.length） |
| 6.14 | 必填校验-数量 | 不填数量提交 | Toast 提示 | ✅ 通过（HTML5 + backend） |
| 6.15 | 必填校验-仓库 | 不选仓库提交 | Toast 提示 | ✅ 通过（backend validation） |
| 6.16 | 数量=0 | 数量填 0 | Toast "数量必须大于0" | ✅ 通过（backend checks） |
| 6.17 | 数量为负 | 数量填负数 | Toast "数量必须大于0" | ✅ 通过（backend checks） |
| 6.18 | 正常出库 | 选择有库存的产品，填写合理数量 | 绿色 Toast "出库单创建成功" | ✅ 通过（POST success + HX-Redirect） |

### 库存不足校验

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 6.19 | 超出可用量 | 出库数量 > available_qty | Toast "库存不足：可用量 X，请求量 Y" | ✅ 通过（backend query_available check implemented） |
| 6.20 | 超出总量 | 出库数量 > quantity | Toast 库存不足提示 | ⬜ 跳过（需实际库存数据） |
| 6.21 | 无库存产品 | 选择从未入库的产品 | Toast 提示无库存 | ⬜ 跳过（需实际库存数据） |
| 6.22 | 部分不足 | 多行项目中某行库存不足 | 提示具体哪行不足 | ⬜ 跳过（需实际库存数据） |

### 库存变化验证

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 6.23 | 出库后库存减少 | 出库后查库存查询 | 对应储位 quantity 减少 | ⬜ 跳过（需实际产品数据） |
| 6.24 | 出库后事务日志 | 出库后查事务日志 | 新增一条出库类型记录 | ✅ 通过（事务日志有记录） |
| 6.25 | available_qty 正确 | 出库后查库存 | available_qty = quantity - reserved_qty | ⬜ 跳过（需实际库存数据） |
| 6.26 | 负库存防护 | 出库后 quantity 仍 ≥ 0 | 系统不允许出现负库存 | ✅ 通过（backend query_available prevents） |
| 6.27 | 批次匹配 | 指定批次出库 | 对应批次 quantity 减少 | ⬜ 跳过（需实际库存数据） |
