# Phase 4: 生产相关出库

> 覆盖页面：领料单、倒冲记录
> 前置依赖：Phase 2（需先有库存数据）

---

## U7. 领料单 (`/admin/wms/requisitions`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 7.1 | 领料单列表加载 | 打开 /admin/wms/requisitions | 表格：doc_number/work_order/warehouse/status/创建时间 | ✅ |
| 7.2 | 按状态筛选 | Tab 筛选 | Draft/Confirmed/Issued/Cancelled | ✅ |
| 7.3 | 搜索 | 输入关键词 | 按单号/工单搜索 | ✅ |
| 7.4 | 编号格式 | 查看已有记录 | 格式 MR-2026-06-xxxxx | ✅ |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 7.5 | 创建页加载 | 点击新建 | 表单：工单/仓库/领料日期/操作员/备注 + 行项目 | ✅ |
| 7.6 | 工单选择 | 选择工单 | 显示工单编号和产品信息 | ✅ |
| 7.7 | 仓库选择 | 选择仓库 | 列出所有 ACTIVE 仓库 | ✅ |
| 7.8 | 添加行项目 | 点击"添加行" | 新增一行：产品/requested_qty | ✅ |
| 7.9 | 删除行项目 | 删除某行 | 该行移除 | ✅ |
| 7.10 | 必填校验 | 不填工单/仓库/行项目 | Toast 提示缺失字段 | ✅ |
| 7.11 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ |

### 详情页 — 信息展示

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 7.12 | 详情加载 | 点击某领料单 | 完整信息 + 行项目 | ⬜ |
| 7.13 | 行项目展示 | 查看行项目 | 产品/编码/requested_qty/issued_qty/variance_qty | ⬜ |
| 7.14 | 工作流步骤条 | 查看页面 | Draft→Confirmed→Issued 进度 | ⬜ |
| 7.15 | 状态标签 | 查看当前状态 | 不同颜色标签 | ⬜ |

### 详情页 — 状态流转

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 7.16 | Draft→Confirmed | 点击"确认领料" | 状态变为 Confirmed | ⬜ |
| 7.17 | Confirmed→Issued | 点击"发料"，填 issued_qty + bin_id | 状态变为 Issued | ⬜ |
| 7.18 | 发料后出库 | Issued 后查库存 | 对应产品库存减少（MATERIAL_ISSUE） | ⬜ |
| 7.19 | 发料后事务日志 | Issued 后查事务日志 | 新增 MATERIAL_ISSUE 记录 | ⬜ |
| 7.20 | 差异量计算 | issued_qty ≠ requested_qty | variance_qty = requested_qty - issued_qty 自动计算 | ⬜ |
| 7.21 | Draft→Cancelled | 取消草稿 | 状态变为 Cancelled | ⬜ |
| 7.22 | Confirmed→Cancelled | 取消已确认 | 状态变为 Cancelled | ⬜ |
| 7.23 | 非法流转(Issued→Cancelled) | 已发料后尝试取消 | Toast "状态转换无效: Issued -> Cancelled" | ⬜ |
| 7.24 | 非法流转(Issued→Confirmed) | 尝试回退状态 | Toast 错误提示 | ⬜ |
| 7.25 | 发料库存不足 | issued_qty 超过库存 | Toast 库存不足提示 | ⬜ |

### HARD 预留联动（如 MES 已实现）

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 7.26 | 工单下达→HARD 预留 | WO.release() | available_qty 减少 | ⏭ |
| 7.27 | 发料消耗 HARD 预留 | Requisition.issue() | reserved_qty 减少 | ⏭ |

---

## U8. 倒冲记录 (`/admin/wms/backflushes`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 8.1 | 倒冲列表加载 | 打开 /admin/wms/backflushes | 表格：doc_number/work_order/product/completed_qty/status | ✅ |
| 8.2 | 按状态筛选 | Tab 筛选 | Draft/Executed/Adjusted | ✅ |
| 8.3 | 搜索 | 输入关键词 | 按单号/工单搜索 | ✅ |
| 8.4 | 无创建按钮 | 检查页面 | 倒冲由 MES 完工触发，无手动创建入口 | ✅ |
| 8.5 | 编号格式 | 查看已有记录 | 格式 BF-2026-06-xxxxx | ✅ |

### 详情页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 8.6 | 详情加载 | 点击某倒冲记录 | 完整信息 + BackflushItem 行项目 | ⬜ |
| 8.7 | 行项目展示 | 查看行项目 | component(产品名称)/theoretical_qty/actual_qty/variance_qty/variance_rate | ⬜ |
| 8.8 | BOM 子件解析 | 查看行项目 | component_id 显示产品名称而非 ID | ⬜ |
| 8.9 | 差异计算 | theoretical ≠ actual | variance_qty = theoretical - actual；variance_rate = variance / theoretical | ⬜ |
| 8.10 | 超阈值标记 | variance_rate > threshold | is_over_threshold=true，醒目标记（红色/警告图标） | ⬜ |
| 8.11 | 工单关联 | 查看 work_order_id | 可点击跳转到工单详情（如 MES 已实现） | ⬜ |
| 8.12 | 状态枚举 | 查看不同记录 | Draft/Executed/Adjusted 有不同颜色标签 | ⬜ |

### 倒冲后库存联动（如 MES 已实现）

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 8.13 | 倒冲执行后库存 | Backflush.execute() | BOM 子件库存扣减（BACKFLUSH 类型事务） | ⬜ |
| 8.14 | 倒冲差异超阈值 | 超阈值倒冲 | CostEntry 记录损耗成本（独立事务） | ⬜ |
