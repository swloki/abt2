# MES 生产管理模块 — 功能测试总纲

> **测试环境**: http://127.0.0.1:3000
> **测试账号**: admin / 123456
> **测试工具**: agent-browser (Headless Chromium)
> **测试日期**: 2026-06-07
> **测试原则**: 对齐原型设计，逐功能验证，发现问题立即修复，修复后回归测试

---

## 1. 模块范围

MES 生产管理模块包含 **12 个侧栏入口、20+ 个页面路由**，覆盖从计划到入库的完整生产流程。

### 1.1 功能清单与实现状态

| # | 子功能 | 侧栏名称 | URL 前缀 | 原型文件 | 实现状态 |
|---|--------|----------|----------|---------|---------|
| 1 | 生产总览 | 生产总览 | /admin/mes | 04-index.html | ✅ 已实现（统计值为静态占位） |
| 2 | 生产计划 | 生产计划 | /admin/mes/plans | 04-plan-list/create/detail.html | ✅ 已实现 |
| 3 | 工单管理 | 工单管理 | /admin/mes/orders | 04-order-list/create/detail.html | ✅ 已实现 |
| 4 | 生产批次 | 生产批次 | /admin/mes/batches | 04-batch-list/detail.html | ✅ 已实现 |
| 5 | 流转卡查询 | 流转卡查询 | /admin/mes/cards | 04-card-query.html | ⚠️ 静态页面，无查询逻辑 |
| 6 | 排程看板 | 排程看板 | /admin/mes/schedule | 04-schedule-board.html | ❌ 显示"功能开发中" |
| 7 | 报工记录 | 报工记录 | /admin/mes/reports | 04-report-list/create/detail.html | ✅ 已实现 |
| 8 | 计件工资 | 计件工资 | /admin/mes/wages | 04-wage-list.html | ⚠️ 空表格 stub |
| 9 | 生产报检 | 生产报检 | /admin/mes/inspections | 04-inspection-list/create/detail.html | ✅ 已实现 |
| 10 | 完工入库 | 完工入库 | /admin/mes/receipts | 04-receipt-list/create/detail.html | ✅ 已实现 |
| 11 | 物料消耗 | 物料消耗 | /admin/mes/material-usage | 04-material-usage.html | ❌ 显示"功能开发中" |
| 12 | 生产异常 | 生产异常 | /admin/mes/exceptions | 04-exception-list/detail.html | ❌ 显示"功能开发中" |

### 1.2 原型设计但未实现的功能

| 功能 | 原型文件 | 说明 |
|------|---------|------|
| 排程看板（看板/甘特图） | 04-schedule-board.html | 看板视图+甘特图+产线切换 |
| 物料消耗追踪 | 04-material-usage.html | BOM对比+倒冲明细+领料记录 |
| 生产异常管理 | 04-exception-list/detail.html | 异常类型筛选+时间线+关联信息 |
| 委外管理 | 04-outsourcing-list/create/detail.html | 委外类型+供应商+收发记录 |
| 委外追踪 | 04-outsourcing-tracking-list.html | 委外进度跟踪 |

---

## 2. 测试文件索引

| 编号 | 文件 | 覆盖范围 |
|------|------|---------|
| 00 | `2026-06-07-mes-test-00-overview.md` | 本文档（总纲+公共测试项） |
| 01 | `2026-06-07-mes-test-01-dashboard.md` | 生产总览 Dashboard |
| 02 | `2026-06-07-mes-test-02-plan.md` | 生产计划（列表/创建/详情/状态流转） |
| 03 | `2026-06-07-mes-test-03-order.md` | 工单管理（列表/创建/详情/状态流转） |
| 04 | `2026-06-07-mes-test-04-batch.md` | 生产批次 + 流转卡查询 |
| 05 | `2026-06-07-mes-test-05-report.md` | 报工记录 + 计件工资 |
| 06 | `2026-06-07-mes-test-06-inspection.md` | 生产报检 |
| 07 | `2026-06-07-mes-test-07-receipt.md` | 完工入库 + 物料消耗 |
| 08 | `2026-06-07-mes-test-08-integration.md` | 端到端流程 + 跨模块集成 + 边界条件 |

---

## 3. 测试工作流

```
Phase 1: 登录验证 → Dashboard 验证
Phase 2: 生产计划 CRUD + 状态流转（Draft→Confirmed→InProgress）
Phase 3: 工单管理 CRUD + 状态流转（Draft→Planned→Released→Closed/Cancelled）
Phase 4: 生产批次 + 报工 + 检验
Phase 5: 完工入库 + 确认入库（触发倒冲）
Phase 6: 其他功能（流转卡查询、计件工资、排程看板、物料消耗、异常管理）
Phase 7: 端到端完整流程 + 跨模块验证
每个 Phase: 测试 → 记录结果 → 修复缺陷 → 重测 → 全部通过后进入下一 Phase
```

---

## 4. 公共测试项（适用于所有 MES 页面）

### 4.1 侧栏导航

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| NAV-01 | 展开生产管理菜单 | 点击侧栏"生产"模块 | 展开子菜单，显示 12 个子项 |
| NAV-02 | 子菜单项完整性 | 查看子菜单 | 包含：生产总览、生产计划、工单管理、生产批次、流转卡查询、排程看板、报工记录、计件工资、生产报检、完工入库、物料消耗、生产异常 |
| NAV-03 | 每个菜单项可点击 | 依次点击每个菜单 | 正确跳转到对应 URL |
| NAV-04 | 当前页高亮 | 访问某页面 | 对应菜单项有 active/highlight 样式 |
| NAV-05 | 侧栏收起/展开 | 点击折叠按钮 | 侧栏折叠后只显示图标 |

### 4.2 页面布局

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| LAYOUT-01 | 通用布局 | 所有 MES 页面包含侧栏 + 顶部栏 + 内容区域 |
| LAYOUT-02 | 面包屑 | 显示 "生产管理 > 当前页面名称"，可点击返回 |
| LAYOUT-03 | HTMX 导航 | 切换页面时侧栏保持状态，不重新加载 |

### 4.3 通用组件

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| COMP-01 | 状态标签颜色 | 各状态显示正确的颜色 pill（见枚举值速查） |
| COMP-02 | 空数据提示 | 无数据时显示居中提示文本（如"暂无生产计划"） |
| COMP-03 | 分页组件 | 数据 > 20 条时显示分页，点击翻页有效 |
| COMP-04 | 表格行可点击 | cursor:pointer + onclick 跳转详情 |
| COMP-05 | 返回链接 | 各详情页/创建页有返回列表链接 |

---

## 5. 枚举值速查

### 5.1 计划类型 (PlanType)
| 值 | i16 | 中文 | 标签色 |
|----|-----|------|--------|
| Mto | 1 | 按单生产 (MTO) | 蓝色 |
| Mts | 2 | 按库存备货 (MTS) | 紫色 |

### 5.2 计划状态 (PlanStatus)
| 值 | i16 | 中文 | 背景色 | 文字色 |
|----|-----|------|--------|--------|
| Draft | 1 | 草稿 | rgba(0,0,0,0.04) | var(--muted) |
| Confirmed | 2 | 已确认 | rgba(22,119,255,0.08) | var(--accent) |
| InProgress | 3 | 进行中 | rgba(250,140,22,0.08) | #fa8c16 |
| Completed | 4 | 已完成 | rgba(82,196,26,0.08) | var(--success) |
| Cancelled | 5 | 已取消 | rgba(245,63,63,0.06) | #f53f3f |

### 5.3 工单状态 (WorkOrderStatus)
| 值 | i16 | 中文 | 背景色 | 文字色 |
|----|-----|------|--------|--------|
| Draft | 1 | 待计划 | rgba(0,0,0,0.04) | var(--muted) |
| Planned | 2 | 已计划 | rgba(22,119,255,0.08) | var(--accent) |
| Released | 3 | 已下达 | rgba(82,196,26,0.08) | var(--success) |
| Closed | 4 | 已关闭 | rgba(114,46,209,0.08) | #722ed1 |
| Cancelled | 5 | 已取消 | rgba(245,63,63,0.06) | #f53f3f |

### 5.4 批次状态 (BatchStatus)
| 值 | i16 | 中文 |
|----|-----|------|
| Pending | 1 | 待生产 |
| InProgress | 2 | 进行中 |
| Suspended | 3 | 已暂停 |
| PendingReceipt | 4 | 待入库 |
| Completed | 5 | 已完成 |
| Cancelled | 6 | 已取消 |

### 5.5 检验类型 (InspectionType)
| 值 | i16 | 中文 |
|----|-----|------|
| FirstArticle | 1 | 首检 |
| InProcess | 2 | 巡检 |
| Final | 3 | 完工检 |

### 5.6 检验结果 (InspectionResultType)
| 值 | i16 | 中文 | 颜色 |
|----|-----|------|------|
| Pass | 1 | 合格 | 绿色 |
| Fail | 2 | 不合格 | 红色 |
| Conditional | 3 | 让步接收 | 橙色 |

### 5.7 入库状态 (ReceiptStatus)
| 值 | i16 | 中文 |
|----|-----|------|
| Draft | 1 | 草稿 |
| Confirmed | 2 | 已确认 |
| Cancelled | 3 | 已取消 |

### 5.8 班次 (ShiftType)
| 值 | i16 | 中文 |
|----|-----|------|
| Day | 1 | 白班 |
| Night | 2 | 夜班 |

### 5.9 不良原因 (DefectReason)
| 值 | i16 | 中文 | 影响工资 |
|----|-----|------|---------|
| MaterialDefect | 1 | 物料不良 | 是 |
| EquipmentFault | 2 | 设备故障 | 是 |
| OperatorError | 3 | 操作失误 | 否 |
| ProcessIssue | 4 | 工艺问题 | 是 |

---

## 6. 数据库表速查

| 表名 | 说明 | 关键字段 |
|------|------|---------|
| production_plans | 生产计划主表 | doc_number, plan_date, plan_type, status |
| production_plan_items | 计划明细 | plan_id, product_id, planned_qty, priority |
| work_orders | 工单 | doc_number, plan_item_id, product_id, status, version |
| work_order_routings | 工单工序 | work_order_id, step_no, process_name, unit_price |
| production_batches | 生产批次 | batch_no, card_sn, work_order_id, current_step |
| work_reports | 报工记录 | batch_id, routing_id, worker_id, completed_qty |
| production_inspections | 检验记录 | work_order_id, inspection_type, result |
| production_receipts | 入库记录 | batch_id, received_qty, backflush_triggered |

---

## 7. 测试结果记录规范

### 状态标记
- ✅ 通过 — 功能正常
- ⚠️ 部分实现 — 功能存在但有偏差
- ❌ 未实现 — 功能缺失
- 🐛 缺陷 — 发现 bug
- ⏭ 无法测试 — 依赖未实现模块

### 缺陷优先级
- **P0 阻塞** — 核心功能不可用（如页面 500、提交失败）
- **P1 严重** — 主要功能异常（如数据不正确、状态流转失败）
- **P2 一般** — 次要问题（如 UI 显示偏差、缺少非关键字段）
- **P3 轻微** — 美观/体验问题

### 测试流程
1. 按 Phase 顺序测试
2. 每个测试项执行后记录结果
3. 发现缺陷立即修复代码
4. 修复后 `cargo clippy` 确认编译通过
5. 回归测试确认修复生效
6. 当前 Phase 全部通过后进入下一 Phase
