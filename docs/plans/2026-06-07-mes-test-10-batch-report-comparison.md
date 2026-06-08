# MES 批次 & 报工页面原型对比测试报告

**测试日期**: 2026-06-07
**测试范围**: 批次详情、报工创建（重点页面）+ 批次列表、报工列表、报工详情
**原型来源**: Open Design 本地文件（04-batch-list/detail、04-report-list/create/detail）

## 测试总览

| 页面 | 路径 | 状态 | 修复项 |
|------|------|------|--------|
| 批次列表 | /admin/mes/batches | ✅ | 操作列增加报工/入库按钮 |
| 批次详情 | /admin/mes/batches/7 | ✅ | 全面重构匹配原型 |
| 报工列表 | /admin/mes/reports | ⚠️ | P3 差异已记录 |
| 报工创建 | /admin/mes/reports/create | ✅ | 全面改造匹配原型 |
| 报工详情 | /admin/mes/reports/1 | ⚠️ | P2 差异已记录 |

## 核心修复

### P0/P1 修复

| # | 页面 | 问题 | 修复 | 文件 |
|---|------|------|------|------|
| 1 | 批次列表 | 操作列只有"查看" | 根据状态显示报工/入库/查看 | `mes_batch_list.rs` |
| 2 | 批次详情 | 用 `info-card`+`info-grid` 而非原型 `detail-header`+`detail-info-grid-5` | 重构为 `detail-header` + `detail-title-row` + `detail-doc-no` + `detail-info-grid-5` | `mes_batch_detail.rs` |
| 3 | 批次详情 | 工序路线用表格而非水平进度条 | 改为 `progress-track` + `progress-step` 水平进度条 | `mes_batch_detail.rs` |
| 4 | 批次详情 | 缺少工单链接、流转卡号、实际开始/结束等字段 | 补充10项信息网格 | `mes_batch_detail.rs` |
| 5 | 报工创建 | 全手动输入 ID | 改为工单下拉、批次只读、工序下拉、工人下拉 | `mes_report_create.rs` |
| 6 | 报工创建 | 班次用 select 下拉 | 改为 `shift-toggle` + `shift-btn` 按钮 | `mes_report_create.rs` |
| 7 | 报工创建 | 缺少基本信息/生产数据分区 | 分为基本信息（6字段）+ 生产数据（5字段） | `mes_report_create.rs` |

### CSS 新增（base.css）

在 `abt-web/static/base.css` 尾部添加了原型所需的 CSS class：

| Class | 用途 |
|-------|------|
| `detail-doc-no`, `detail-info-grid-5`, `detail-info-item`, `detail-info-label`, `detail-info-value` | 批次详情 5 列信息网格 |
| `sub-section`, `sub-section-title` | 白色卡片区块 |
| `progress-track`, `progress-step`, `progress-step-dot`, `progress-step-line`, `progress-step-label` | 水平工序进度条 |
| `.progress-step.completed`, `.progress-step.active` | 进度条状态样式 |
| `shift-toggle`, `shift-btn`, `.shift-btn.active` | 班次切换按钮 |
| `wage-display`, `wage-amount`, `wage-label` | 计件工资显示 |
| `form-actions` | 表单底部操作栏 |

## 与原型对比详情

### 批次详情（04-batch-detail.html）— 完全匹配 ✅

| 区域 | 原型 | 实现 | 状态 |
|------|------|------|------|
| detail-header + detail-title-row | ✅ | ✅ `detail-header` + `detail-doc-no` | ✅ |
| 状态pill + 流转卡号 | ✅ | ✅ `status-pill` + `time-cell` | ✅ |
| 操作按钮：暂停 + 工序报工 | ✅ | ✅ `btn-default` + `btn-primary` | ✅ |
| 5列信息网格 10 项 | ✅ | ✅ `detail-info-grid-5` + `detail-info-item` | ✅ |
| 工单链接 | ✅ | ✅ `link-cell` 可点击 | ✅ |
| 完成/报废 颜色区分 | ✅ | ✅ `text-success` + `text-danger` | ✅ |
| 工序流转进度（水平步骤条）| ✅ | ✅ `progress-track` + `progress-step` | ✅ |
| 报工历史表 | ✅ | ⚠️ 内联快速报工表单（保留） | P2 |
| 状态变更记录表 | ✅ | ❌ | P2 待实现 |

### 报工创建（04-report-create.html）— 高度匹配 ✅

| 区域 | 原型 | 实现 | 状态 |
|------|------|------|------|
| 基本信息 section | ✅ | ✅ 6 字段 | ✅ |
| 工单下拉（显示工单号） | ✅ | ✅ | ✅ |
| 批次下拉（只读预选） | ✅ | ✅ 只读输入框 | ✅ |
| 工序下拉（显示当前工序） | ✅ | ✅ 自动预选 | ✅ |
| 班次 toggle 按钮 | ✅ | ✅ `shift-toggle` + `shift-btn` | ✅ |
| 工人下拉 | ✅ | ✅ 显示姓名 | ✅ |
| 报工日期默认今天 | ✅ | ✅ | ✅ |
| 生产数据 section | ✅ | ✅ 5 字段 | ✅ |
| 计件单价（只读） | ✅ | ❌ | P3 |
| 预计工资自动计算 | ✅ | ❌ | P3 |
| form-actions 底栏 | ✅ | ⚠️ `create-action-bar` sticky | P3 差异 |

## 文件变更清单

| 文件 | 变更 |
|------|------|
| `abt-web/static/base.css` | 新增 MES 专用 CSS class（~40 行） |
| `abt-web/src/pages/mes_batch_list.rs` | 操作列条件显示报工/入库按钮 |
| `abt-web/src/pages/mes_batch_detail.rs` | 全面重构匹配原型布局 |
| `abt-web/src/pages/mes_report_create.rs` | 全面改造为下拉选择+分区布局 |
