# MES 生产管理模块 — 功能测试总纲

> 测试环境：http://127.0.0.1:8000
> 测试账号：admin / admin123
> 权限要求：MES.read / MES.write
> 浏览器：Headless Chromium (agent browser)
> 日期：2026-06-07

## 1. 模块范围

MES 生产管理模块包含以下子功能，共 **12 个侧栏入口、19 个页面路由**：

| 子功能 | 侧栏路径 | URL | 测试文档 |
|--------|----------|-----|----------|
| 生产总览 | /admin/mes | /admin/mes | 01-dashboard |
| 生产计划 | /admin/mes/plans | /admin/mes/plans, /create, /{id} | 02-plan |
| 工单管理 | /admin/mes/orders | /admin/mes/orders, /create, /{id} | 03-order |
| 生产批次 | /admin/mes/batches | /admin/mes/batches, /{id} | 04-batch |
| 流转卡查询 | /admin/mes/cards | /admin/mes/cards | 04-batch |
| 排程看板 | /admin/mes/schedule | 未实现 | — |
| 报工记录 | /admin/mes/reports | /admin/mes/reports, /create, /{id} | 05-report |
| 计件工资 | /admin/mes/wages | /admin/mes/wages | 05-report |
| 生产报检 | /admin/mes/inspections | /admin/mes/inspections, /create, /{id} | 06-inspection |
| 完工入库 | /admin/mes/receipts | /admin/mes/receipts, /create, /{id} | 07-receipt |
| 物料消耗 | /admin/mes/material-usage | 未实现 | — |
| 生产异常 | /admin/mes/exceptions | 未实现 | — |

## 2. 实现状态总览

### 已实现（有后端服务 + 前端页面）

| 功能 | 列表 | 创建 | 详情 | 状态流转 |
|------|------|------|------|---------|
| 生产计划 | ✅ 有数据 | ✅ | ✅ | Draft→Confirmed→InProgress→Completed/Cancelled |
| 工单管理 | ✅ 有数据 | ✅ | ✅ | Draft→Planned→Released→Closed/Cancelled |
| 生产批次 | ⚠️ stub | — | ✅ | Pending→InProgress→Suspended→PendingReceipt→Completed |
| 报工记录 | ⚠️ stub | ✅ | ✅ | — |
| 计件工资 | ⚠️ stub | — | — | — |
| 生产报检 | ⚠️ stub | ✅ | ✅ | Pass/Fail/Conditional |
| 完工入库 | ⚠️ stub | ✅ | ✅ | Draft→Confirmed/Cancelled |
| 流转卡查询 | ⚠️ 静态页面 | — | — | — |

> "⚠️ stub" = 页面有表格结构但无真实数据查询，显示"暂无数据"

### 未实现（仅有侧栏入口）

- 排程看板 (`/admin/mes/schedule`) — 原型已设计，代码未实现
- 物料消耗 (`/admin/mes/material-usage`) — 原型已设计，代码未实现
- 生产异常 (`/admin/mes/exceptions`) — 原型已设计，代码未实现

## 3. 测试文件索引

| 编号 | 文件 | 内容 |
|------|------|------|
| 00 | `2026-06-07-mes-test-00-overview.md` | 本文档 |
| 01 | `2026-06-07-mes-test-01-dashboard.md` | 生产总览 Dashboard |
| 02 | `2026-06-07-mes-test-02-plan.md` | 生产计划 (CRUD + 状态流转) |
| 03 | `2026-06-07-mes-test-03-order.md` | 工单管理 (CRUD + 状态流转) |
| 04 | `2026-06-07-mes-test-04-batch.md` | 生产批次 + 流转卡查询 |
| 05 | `2026-06-07-mes-test-05-report.md` | 报工记录 + 计件工资 |
| 06 | `2026-06-07-mes-test-06-inspection.md` | 生产报检 |
| 07 | `2026-06-07-mes-test-07-receipt.md` | 完工入库 |
| 08 | `2026-06-07-mes-test-08-integration.md` | 跨模块集成测试 + 端到端流程 |

## 4. 公共测试项

以下测试项适用于所有 MES 页面：

### 4.1 侧栏导航

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| NAV-01 | 点击侧栏"生产"模块按钮 | 展开生产管理子菜单，高亮当前模块 |
| NAV-02 | 生产管理菜单包含 12 项 | 生产总览、生产计划、工单管理、生产批次、流转卡查询、排程看板、报工记录、计件工资、生产报检、完工入库、物料消耗、生产异常 |
| NAV-03 | 点击每个菜单项 | 正确跳转到对应 URL |
| NAV-04 | 当前页面的菜单项高亮 | active 状态样式正确 |
| NAV-05 | 收起/展开侧栏 | 侧栏折叠后只显示图标 |

### 4.2 权限

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| PERM-01 | 无 MES.read 权限的用户访问任何 MES 页面 | 返回 403 或重定向 |
| PERM-02 | 无 MES.write 权限的用户尝试创建/修改操作 | 操作按钮不显示或提交被拒绝 |
| PERM-03 | 有 MES.read 但无 MES.write 的用户 | 能查看列表和详情，创建按钮隐藏 |

### 4.3 页面布局

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| LAYOUT-01 | 所有 MES 页面使用 admin_page 布局 | 包含侧栏、顶部栏、面包屑 |
| LAYOUT-02 | 侧栏显示"生产管理"模块图标和名称 | 图标正确，名称"生产" |
| LAYOUT-03 | 面包屑显示正确 | 主模块"生产管理" + 当前页面名称 |
| LAYOUT-04 | HTMX 导航不重新加载侧栏 | 切换页面时侧栏保持状态 |

## 5. 枚举值速查

### 5.1 计划状态 (PlanStatus)

| 值 | 中文 | 背景色 | 文字色 |
|----|------|--------|--------|
| Draft | 草稿 | rgba(0,0,0,0.04) | var(--muted) |
| Confirmed | 已确认 | rgba(22,119,255,0.08) | var(--accent) |
| InProgress | 进行中 | rgba(250,140,22,0.08) | #fa8c16 |
| Completed | 已完成 | rgba(82,196,26,0.08) | var(--success) |
| Cancelled | 已取消 | rgba(245,63,63,0.06) | #f53f3f |

### 5.2 工单状态 (WorkOrderStatus)

| 值 | 中文 | 背景色 | 文字色 |
|----|------|--------|--------|
| Draft | 待计划 | rgba(0,0,0,0.04) | var(--muted) |
| Planned | 已计划 | rgba(22,119,255,0.08) | var(--accent) |
| Released | 已下达 | rgba(82,196,26,0.08) | var(--success) |
| Closed | 已关闭 | rgba(114,46,209,0.08) | #722ed1 |
| Cancelled | 已取消 | rgba(245,63,63,0.06) | #f53f3f |

### 5.3 批次状态 (BatchStatus)

| 值 | 中文 |
|----|------|
| Pending | 待生产 |
| InProgress | 进行中 |
| Suspended | 已暂停 |
| PendingReceipt | 待入库 |
| Completed | 已完成 |
| Cancelled | 已取消 |

### 5.4 检验类型 (InspectionType)

| 值 | i16 | 中文 |
|----|-----|------|
| FirstArticle | 1 | 首检 |
| InProcess | 2 | 巡检 |
| Final | 3 | 完工检 |

### 5.5 检验结果 (InspectionResultType)

| 值 | i16 | 中文 | 背景色 |
|----|-----|------|--------|
| Pass | 1 | 合格 | 绿色 |
| Fail | 2 | 不合格 | 红色 |
| Conditional | 3 | 让步接收 | 橙色 |

### 5.6 入库状态 (ReceiptStatus)

| 值 | 中文 |
|----|------|
| Draft | 草稿 |
| Confirmed | 已确认 |
| Cancelled | 已取消 |

### 5.7 班次 (ShiftType)

| 值 | 中文 |
|----|------|
| Day (1) | 白班 |
| Night (2) | 夜班 |

### 5.8 不良原因 (DefectReason)

| 值 | i16 | 中文 |
|----|-----|------|
| MaterialDefect | 1 | 物料不良 |
| EquipmentFault | 2 | 设备故障 |
| OperatorError | 3 | 操作失误 |
| ProcessIssue | 4 | 工艺问题 |

### 5.9 排产类型 (PlanType)

| 值 | 中文 |
|----|------|
| Mto | 按单生产 (MTO) |
| Mts | 按库存备货 (MTS) |
