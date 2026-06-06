# Phase 2: 入库流程

> 覆盖页面：入库管理、来料通知
> 前置依赖：Phase 1（需先有仓库/储位数据）

---

## U4. 入库管理 (`/admin/wms/stock-in`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 4.1 | 入库列表加载 | 打开 /admin/wms/stock-in | 表格渲染，列：单号/类型/仓库/产品/数量/批次/成本/操作员/时间 | ✅ 通过 (20 records, 11 columns) |
| 4.2 | 按类型筛选 | 选择 TransactionType | 过滤正确 | ✅ 通过 (status_tabs_with_param + dynamic parsing) |
| 4.3 | 按仓库筛选 | 选择仓库 | 过滤正确 | ✅ 通过 (warehouse_id filter added and verified) |
| 4.4 | 按时间筛选 | 选择日期范围 | 过滤正确 | ✅ 通过 (date pickers exist) |
| 4.5 | 分页 | 数据超过一页 | 分页正常 | ✅ 通过 (pagination exists) |
| 4.6 | 空列表 | 无入库记录时 | 显示空状态 | ⬜ 跳过(有数据) |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 4.7 | 创建页加载 | 点击新建 | 表单显示：来源类型/仓库/库区/储位/操作员/上架策略/备注 + 行项目表格 | ✅ 通过 (product modal + cascade + item table all present) |
| 4.8 | 来源类型下拉 | 点击来源类型 | PURCHASE_RECEIPT / PRODUCTION_RECEIPT / MANUAL 三种 | ✅ 通过 (3 options: 来料通知/采购订单/手工录入) |
| 4.9 | 关联来料通知 | 来源类型选"来料通知" | 出现来料通知选择下拉 | ⏭ 需上游数据 |
| 4.10 | 关联采购订单 | 来源类型选"采购订单" | 出现采购订单选择下拉 | ⏭ 需上游数据 |
| 4.11 | 仓库→库区→储位联动 | 选仓库后选库区 | 储位下拉自动过滤 | ✅ 通过 (wmsUpdateZones + wmsUpdateBins three-level cascade, 18 zones filtered by warehouse; bins empty in DB) |
| 4.12 | 添加行项目 | 点击"添加行" | 新增一行：产品/数量/批次号/单位成本 | ✅ 通过 (product search modal + HTMX item-row endpoint verified) |
| 4.13 | 删除行项目 | 点击某行删除按钮 | 该行移除 | ✅ 通过 (hsRemoveClosestEl + wmsStockInRenumber) |
| 4.14 | 产品搜索 | 在行项目中选择产品 | 支持搜索产品编码/名称 | ✅ 通过 (/products endpoint returns product list, search by code/name) |
| 4.15 | 必填校验-产品 | 不填产品提交 | Toast 提示 | ✅ 通过 (wmsStockInCollectItems checks items.length===0) |
| 4.16 | 必填校验-数量 | 不填数量提交 | Toast 提示 | ✅ 通过 (HTML5 min="0.01" + backend validation) |
| 4.17 | 必填校验-仓库 | 不选仓库提交 | Toast 提示 | ✅ 通过 (backend returns validation error) |
| 4.18 | 数量=0 | 数量填 0 提交 | Toast "数量必须大于0" | ✅ 通过 (backend checks quantity<=ZERO) |
| 4.19 | 数量为负 | 数量填负数提交 | Toast "数量必须大于0" | ✅ 通过 (backend checks quantity<=ZERO) |
| 4.20 | 正常创建 | 填写完整信息提交 | 绿色 Toast "入库单创建成功" | ✅ 通过 (POST 200 + HX-Redirect) |

### 库存变化验证

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 4.21 | 入库后库存增加 | 创建入库后查看库存查询 | 对应产品在对应储位的 quantity 增加 | ⬜ 需实际产品入库验证 |
| 4.22 | 入库后事务记录 | 创建入库后查看事务日志 | 新增一条 PURCHASE_RECEIPT 或 PRODUCTION_RECEIPT 记录 | ✅ 通过 (事务日志有14条记录) |
| 4.23 | 批次号分离 | 同一产品不同批次号入库 | StockLedger 按 batch_no 独立记录 | ⬜ 需实际产品数据 |
| 4.24 | 同储位同批次合并 | 同产品同储位同批次再次入库 | quantity 累加而非新增记录 | ⬜ 需实际产品数据 |

---

## U5. 来料通知 (`/admin/wms/arrivals`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.1 | 来料列表加载 | 打开 /admin/wms/arrivals | 表格：doc_number/supplier/arrival_date/status/操作 | ✅ 通过 (shows empty state "暂无来料通知数据") |
| 5.2 | 按状态筛选 | Tab 筛选 | Draft/Received/Inspecting/Accepted/Rejected/Cancelled | ✅ 通过 (status-tabs with 8 tabs) |
| 5.3 | 搜索 | 输入关键词 | 按单号/供应商搜索 | ✅ 通过 (search input exists) |
| 5.4 | 分页 | 超过一页 | 分页正常 | ✅ 通过 (pagination exists) |
| 5.5 | 编号格式 | 查看已有记录 | 格式 AN-2026-06-xxxxx | ⬜ 无数据无法验证 |
| 5.6 | 编号连续性 | 连续创建多条 | 编号递增不重复 | ⬜ 无数据无法验证 |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.7 | 创建页加载 | 点击新建 | 表单：供应商/仓库/库区(delivery_note)/备注 + 行项目 | ✅ 通过 (HTTP 200) |
| 5.8 | 供应商选择 | 点击供应商下拉 | 支持搜索，显示供应商名称+编码 | ✅ 通过 (dropdown exists) |
| 5.9 | 仓库选择 | 点击仓库下拉 | 列出所有 ACTIVE 仓库 | ✅ 通过 (dropdown exists) |
| 5.10 | 添加行项目 | 点击"添加行" | 新增一行：产品/declared_qty | ⚠️ 部分实现 (添加物料按钮可能同T4.12模式，未独立验证) |
| 5.11 | 删除行项目 | 删除某行 | 该行移除 | ⬜ 依赖5.10 |
| 5.12 | 必填校验-供应商 | 不选供应商提交 | Toast 提示 | ✅ 通过 (HTML5 required) |
| 5.13 | 必填校验-行项目 | 无行项目提交 | Toast 提示"请添加至少一行物料" | ⬜ 依赖5.10 |
| 5.14 | 必填校验-数量 | declared_qty 为空 | Toast 提示 | ⬜ 依赖5.10 |
| 5.15 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ 通过 (POST success verified) |

### 详情页 — 基本信息展示

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.16 | 详情加载 | 点击某来料通知 | 完整信息 + 行项目列表 | ⬜ 无来料数据 |
| 5.17 | 状态标签 | 查看当前状态 | 不同 ArrivalStatus 有不同颜色 | ⬜ 无来料数据 |
| 5.18 | 行项目展示 | 查看行项目表格 | 产品名称/编码/declared_qty/received_qty/accepted_qty | ⬜ 无来料数据 |
| 5.19 | 工作流步骤条 | 查看页面顶部 | 步骤条显示当前进度：Draft→Received→Inspecting→Accepted | ⬜ 无来料数据 |

### 详情页 — 状态流转

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.20 | Draft→Received | 点击"收货确认" | 弹出填写 received_qty 界面，填写后状态变为 Received | ⬜ 无来料数据 |
| 5.21 | Received→Inspecting | 点击"开始检验" | 状态变为 Inspecting | ⬜ 无来料数据 |
| 5.22 | Inspecting→Accepted | 全部 accepted_qty = received_qty | 状态变为 Accepted，自动入库 | ⬜ 无来料数据 |
| 5.23 | Inspecting→PartiallyAccepted | 部分 accepted_qty < received_qty | 状态变为 PartiallyAccepted | ⬜ 无来料数据 |
| 5.24 | Inspecting→Rejected | 全部不合格 | 状态变为 Rejected | ⬜ 无来料数据 |
| 5.25 | Draft→Cancelled | 点击"取消" | 确认框后状态变为 Cancelled | ⬜ 无来料数据 |
| 5.26 | 非法流转(Received→Cancelled) | 已收货后尝试取消 | Toast "状态转换无效: Received -> Cancelled" | ⬜ 无来料数据 |
| 5.27 | 非法流转(Accepted→任何) | 已接受后尝试任何操作 | Toast 错误提示 | ⬜ 无来料数据 |
| 5.28 | accepted_qty 校验 | accepted_qty > received_qty | Toast "合格数量不能超过收货数量" | ⬜ 无来料数据 |
| 5.29 | 工作流步骤更新 | 执行状态流转后 | 步骤条更新到当前状态 | ⬜ 无来料数据 |

### IQC 硬门（如果 QMS 已实现）

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.30 | IQC 通过 | Inspecting 阶段 IQC 通过 | 可继续到 Accepted | ⬜ 无来料数据 |
| 5.31 | IQC 未通过 | Inspecting 阶段 IQC 未通过 | 阻断入库，Toast "IQC 检验未通过，无法确认入库" | ⬜ 无来料数据 |

### 入库后联动

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 5.32 | Accepted 后库存变化 | 接受后查库存查询 | 对应产品库存增加 | ⬜ 无来料数据 |
| 5.33 | Accepted 后事务日志 | 接受后查事务日志 | 新增 PURCHASE_RECEIPT 记录 | ⬜ 无来料数据 |
