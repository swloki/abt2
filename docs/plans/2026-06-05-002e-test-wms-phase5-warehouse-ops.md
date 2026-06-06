# Phase 5: 仓储内部操作

> 覆盖页面：库存调拨、形态转换、循环盘点、库存锁定
> 前置依赖：Phase 2（需先有库存数据）

---

## U9. 库存调拨 (`/admin/wms/transfers`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 9.1 | 调拨列表加载 | 打开 /admin/wms/transfers | 表格：doc_number/from_warehouse/to_warehouse/status/创建时间 | ✅ |
| 9.2 | 按状态筛选 | Tab 筛选 | Draft/InTransit/Completed/Cancelled | ✅ |
| 9.3 | 搜索 | 输入关键词 | 按单号搜索 | ✅ |
| 9.4 | 编号格式 | 查看记录 | 格式 TRF-2026-06-xxxxx | ✅ |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 9.5 | 创建页加载 | 点击新建 | 表单：调出仓库/库区/储位 + 调入仓库/库区/储位 + 行项目 | ✅ |
| 9.6 | 调出方联动 | 选调出仓库→库区→储位 | 储位下拉过滤 | ✅ |
| 9.7 | 调入方联动 | 选调入仓库→库区→储位 | 储位下拉过滤 | ✅ |
| 9.8 | 添加行项目 | 点击"添加行" | 新增一行：产品/数量 | ✅ |
| 9.9 | 删除行项目 | 删除某行 | 该行移除 | ✅ |
| 9.10 | 必填校验 | 不填仓库/行项目 | Toast 提示 | ✅ |
| 9.11 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ |

### 详情页 — 信息展示

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 9.12 | 详情加载 | 点击某调拨单 | 完整信息：调出方/调入方/行项目 | ⬜ |
| 9.13 | 行项目展示 | 查看行项目 | 产品/数量/批次号 | ⬜ |
| 9.14 | 工作流步骤条 | 查看页面 | Draft→InTransit→Completed 进度 | ⬜ |
| 9.15 | 状态标签 | 查看当前状态 | Draft/InTransit/Completed/Cancelled 不同颜色 | ⬜ |

### 详情页 — 状态流转

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 9.16 | Draft→InTransit | 点击"确认发出" | 状态变为 InTransit | ⬜ |
| 9.17 | InTransit→Completed | 点击"确认到达" | 状态变为 Completed | ⬜ |
| 9.18 | Draft→Cancelled | 点击"取消" | 确认框后状态变为 Cancelled | ⬜ |
| 9.19 | 非法流转(Completed→任何) | 已完成后尝试操作 | Toast "状态转换无效" | ⬜ |
| 9.20 | 非法流转(InTransit→Cancelled) | 在途时尝试取消 | Toast 错误提示 | ⬜ |

### 调拨后库存变化

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 9.21 | Completed 后 from 库存 | 查看调出储位库存 | quantity 减少 | ⬜ |
| 9.22 | Completed 后 to 库存 | 查看调入储位库存 | quantity 增加 | ⬜ |
| 9.23 | 事务记录 | Completed 后查事务日志 | 新增 TRANSFER 类型记录 | ⬜ |
| 9.24 | 调拨库存不足 | 调出储位库存不足 | 阻断并提示 | ⬜ |

---

## U10. 形态转换 (`/admin/wms/conversions`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 10.1 | 转换列表加载 | 打开 /admin/wms/conversions | 表格：doc_number/warehouse/status/创建时间 | ✅ |
| 10.2 | 按状态筛选 | Tab 筛选 | Draft/Completed/Cancelled | ✅ |
| 10.3 | 编号格式 | 查看记录 | 格式 FC-2026-06-xxxxx | ✅ |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 10.4 | 创建页加载 | 点击新建 | 表单：仓库 + 行项目（含 Consume/Produce 方向） | ✅ |
| 10.5 | 添加消耗行 | 点击"添加消耗行" | 新增一行：direction=CONSUME/产品/数量/单位成本 | ✅ |
| 10.6 | 添加产出行 | 点击"添加产出行" | 新增一行：direction=PRODUCE/产品/数量/单位成本 | ✅ |
| 10.7 | 删除行项目 | 删除某行 | 该行移除 | ✅ |
| 10.8 | 必填校验 | 不填仓库/行项目 | Toast 提示 | ✅ |
| 10.9 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ |

### 详情页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 10.10 | 详情加载 | 点击某转换单 | 完整信息 + Consume 和 Produce 两组行项目 | ⬜ |
| 10.11 | ConversionDir 标识 | 查看行项目 | Consume/Produce 有明确区分（不同颜色/图标/标签） | ⬜ |
| 10.12 | 工作流步骤条 | 查看页面 | Draft→Completed 进度 | ⬜ |
| 10.13 | Draft→Completed | 点击"确认转换" | 状态变为 Completed | ⬜ |
| 10.14 | Draft→Cancelled | 点击"取消" | 状态变为 Cancelled | ⬜ |
| 10.15 | 非法流转(Completed→任何) | 已完成后尝试操作 | Toast 错误提示 | ⬜ |
| 10.16 | Completed 后 Consume 库存 | 查看消耗产品库存 | quantity 减少 | ⬜ |
| 10.17 | Completed 后 Produce 库存 | 查看产出产品库存 | quantity 增加 | ⬜ |
| 10.18 | 事务记录 | Completed 后查事务日志 | Consume 有 FORM_CONVERSION 出库 + Produce 有入库 | ⬜ |
| 10.19 | 消耗产品库存不足 | Consume 行库存不足 | Toast 库存不足提示 | ⬜ |

---

## U11. 循环盘点 (`/admin/wms/cycle-counts`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 11.1 | 盘点列表加载 | 打开 /admin/wms/cycle-counts | 表格：doc_number/warehouse/zone/count_date/status | ✅ |
| 11.2 | 按状态筛选 | Tab 筛选 | Draft/Counting/Completed/Adjusted/Cancelled | ✅ |
| 11.3 | 编号格式 | 查看记录 | 格式 CC-2026-06-xxxxx | ✅ |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 11.4 | 创建页加载 | 点击新建 | 表单：仓库/库区/盲盘开关 + 行项目 | ✅ |
| 11.5 | 仓库→库区联动 | 选仓库 | 库区下拉过滤 | ✅ |
| 11.6 | 盲盘开关 | 勾选 is_blind | 标记为盲盘模式 | ✅ |
| 11.7 | 添加行项目 | 点击"添加行" | 新增一行：储位/产品/批次号 | ✅ |
| 11.8 | 删除行项目 | 删除某行 | 该行移除 | ✅ |
| 11.9 | 必填校验 | 不填仓库/行项目 | Toast 提示 | ✅ |
| 11.10 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ |

### 详情页 — 信息展示

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 11.11 | 详情加载 | 点击某盘点单 | 完整信息 + 行项目 | ⬜ |
| 11.12 | 行项目展示 | 查看行项目 | 储位/产品/批次号/system_qty/counted_qty/variance_qty | ⬜ |
| 11.13 | 工作流步骤条 | 查看页面 | Draft→Counting→Completed→Adjusted 进度 | ⬜ |

### 详情页 — 状态流转

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 11.14 | Draft→Counting | 点击"开始盘点" | 状态变为 Counting | ⬜ |
| 11.15 | 填写 counted_qty | 在 Counting 阶段填写实际数量 | 每行可输入 counted_qty | ⬜ |
| 11.16 | Counting→Completed | 填写完所有 counted_qty 后提交 | 自动计算 variance_qty，状态变为 Completed | ⬜ |
| 11.17 | 差异计算 | counted ≠ system | variance_qty = counted - system，自动计算显示 | ⬜ |
| 11.18 | 差异原因填写 | variance_qty ≠ 0 时 | 可填写 variance_reason | ⬜ |
| 11.19 | Completed→Adjusted | 点击"确认调整" | 状态变为 Adjusted | ⬜ |
| 11.20 | Adjusted 后库存更新 | 确认调整后查库存 | StockLedger 更新为 counted_qty | ⬜ |
| 11.21 | Draft→Cancelled | 取消草稿 | 状态变为 Cancelled | ⬜ |
| 11.22 | Counting→Cancelled | 取消盘点中 | 状态变为 Cancelled | ⬜ |
| 11.23 | 非法流转(Adjusted→任何) | 已调整后尝试操作 | Toast 错误提示 | ⬜ |

### 盲盘模式

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 11.24 | 盲盘 system_qty 隐藏 | is_blind=true 的盘点单 | system_qty 列隐藏或显示为 "—" | ⬜ |
| 11.25 | 非盲盘 system_qty 可见 | is_blind=false 的盘点单 | system_qty 正常显示 | ⬜ |
| 11.26 | 盲盘 Completed 后显示 | 完成盘点后 | 可查看 variance（此时 system_qty 才显示） | ⬜ |

---

## U12. 库存锁定 (`/admin/wms/locks`)

### 列表页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 12.1 | 锁定列表加载 | 打开 /admin/wms/locks | 表格：doc_number/product/warehouse/locked_qty/lock_reason/status | ✅ |
| 12.2 | 按状态筛选 | Tab 筛选 | Active/Released/Cancelled | ✅ |
| 12.3 | 编号格式 | 查看记录 | 格式 LCK-2026-06-xxxxx | ✅ |

### 创建页

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 12.4 | 创建页加载 | 点击新建 | 表单：产品/仓库/锁定数量/锁定原因/客户（可选） | ✅ |
| 12.5 | 产品选择 | 选择产品 | 支持搜索 | ✅ |
| 12.6 | 仓库选择 | 选择仓库 | 列出 ACTIVE 仓库 | ✅ |
| 12.7 | 必填校验 | 不填产品/仓库/数量 | Toast 提示 | ✅ |
| 12.8 | 数量校验 | 锁定数量 > available_qty | Toast "可用库存不足" | ✅ |
| 12.9 | 正常创建 | 完整填写提交 | 编号自动生成，Toast 成功 | ✅ |

### 详情页 — 状态流转

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 12.10 | 详情加载 | 点击某锁定记录 | 完整信息 + 操作按钮 | ✅ |
| 12.11 | Active→Released | 点击"正常释放" | 状态变为 Released | ⬜ |
| 12.12 | Active→Cancelled | 点击"作废"（管理员） | 状态变为 Cancelled | ⬜ |
| 12.13 | 非法流转(Released→任何) | 已释放后尝试操作 | Toast 错误提示 | ⬜ |
| 12.14 | 非法流转(Cancelled→任何) | 已作废后尝试操作 | Toast 错误提示 | ⬜ |

### 锁定后库存变化

| # | 测试项 | 操作 | 期望结果 | 状态 |
|---|--------|------|----------|------|
| 12.15 | 锁定后 reserved_qty 增加 | 锁定后查库存 | reserved_qty 增加 | ⬜ |
| 12.16 | 锁定后 available_qty 减少 | 锁定后查库存 | available_qty = quantity - reserved_qty 减少 | ⬜ |
| 12.17 | 释放后 reserved_qty 减少 | Released 后查库存 | reserved_qty 减少，available_qty 恢复 | ⬜ |
| 12.18 | 作废后 reserved_qty 减少 | Cancelled 后查库存 | reserved_qty 减少（预留量退回） | ⬜ |
