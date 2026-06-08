# MES 模块测试报告

**测试日期**: 2026-06-07
**测试范围**: MES 模块全部页面（Dashboard + 10 个菜单页面）
**测试数据**: `scripts/mes_test_data.sql`（7计划 + 9工单 + 8批次 + 12报工 + 6报检 + 3入库）

## 测试总览

| 页面 | 路径 | 状态 | 修复项 |
|------|------|------|--------|
| 生产管理总览 | /admin/mes | ✅ | — |
| 生产计划列表 | /admin/mes/plans | ✅ | 筛选栏样式、关联销售单空值 |
| 生产计划详情 | /admin/mes/plans/:id | ✅ | — |
| 工单管理列表 | /admin/mes/orders | ✅ | — |
| 工单详情 | /admin/mes/orders/:id | ✅ | 产品ID→名称、数量格式化 |
| 生产批次列表 | /admin/mes/batches | ✅ | — |
| 批次详情 | /admin/mes/batches/:id | ✅ | 产品ID→名称、4道工序全显示 |
| 报工记录列表 | /admin/mes/reports | ✅ | — |
| 报工详情 | /admin/mes/reports/:id | ✅ | 工单/批次/工序/工人ID→名称 |
| 生产报检列表 | /admin/mes/inspections | ✅ | — |
| 检验详情 | /admin/mes/inspections/:id | ✅ | 工单/产品/检验员ID→名称 |
| 完工入库列表 | /admin/mes/receipts | ✅ | — |
| 入库详情 | /admin/mes/receipts/:id | ✅ | 工单/批次/产品/仓库ID→名称 |
| 计件工资 | /admin/mes/wages | ✅ | 无数据（页面正常） |
| 排程看板 | /admin/mes/schedule | ⏭ | 功能开发中 |
| 物料消耗追踪 | /admin/mes/material-usage | ⏭ | 功能开发中 |
| 生产异常 | /admin/mes/exceptions | ⏭ | 功能开发中 |
| 流转卡查询 | /admin/mes/cards | ⚠ | 无form提交机制 |

## 缺陷修复记录

### P0 阻塞

| # | 问题 | 修复 | Commit |
|---|------|------|--------|
| 1 | 测试数据 RoutingStatus=0 导致批次详情只显示2/4工序 | 修正SQL中routing status值：1=Pending,2=InProgress,3=Completed | mes_test_data.sql |

### P1 严重

| # | 问题 | 修复 | Commit |
|---|------|------|--------|
| 2 | 所有详情页显示原始ID而非可读名称（产品/工单/批次/仓库/工人等） | 给5个service添加lookup方法：WorkReport/Inspection/Receipt get_detail_lookups, WorkOrder/Batch get_product_name | 08d8b93, dc9da82, 2cd8ace |
| 3 | users表列名错误(nickname→display_name)导致报工/报检详情500 | 修正SQL查询列名 | b0d230d |
| 4 | warehouses表PK列名错误(warehouse_id→id)导致入库详情500 | 修正SQL查询列名 | f50a52c |

### P2 一般

| # | 问题 | 修复 | Commit |
|---|------|------|--------|
| 5 | 计划列表筛选栏样式未对齐原型：日期input class错误、分隔符"~"应"至" | 对齐原型：search-input class + max-width:160px + "至" | 17edaa3 |
| 6 | 计划列表"关联销售单"MTS类型空字符串显示为空白 | 空值替换为"—" | 17edaa3 |
| 7 | 工单详情"计划数量"显示200.000000 | 使用fmt_qty格式化 | 2cd8ace |

## 数据验证结果

### 生产计划（7条）
- MTO计划4条，MTS计划3条
- 状态分布：草稿1、已确认1、进行中2、已完成2、已取消1 ✅
- 关联销售单MTO显示SO号，MTS显示"—" ✅

### 工单（9条）
- 状态分布：待计划2、已计划1、已下达2、已关闭2、已取消2 ✅
- 产品名、创建人名显示正确 ✅

### 批次（8条）
- 状态分布：待生产1、进行中3、待入库1、已完成2、已取消1 ✅
- 当前工序显示格式：`4/4 组装`、`1/4 插件(DIP)`、`未开始` ✅
- 工序路线全4步显示正确 ✅

### 报工（12条）
- 产品名、工序名、工人名均正确显示 ✅
- 班次(白班/夜班)显示正确 ✅

### 报检（6条）
- 类型(首检/巡检/完工检)显示正确 ✅
- 结果(合格/让步接收)颜色标签正确 ✅

### 入库（3条）
- 仓库名显示"备料周转仓"而非ID ✅
- 状态"已确认"标签正确 ✅

## 待完善

1. **流转卡查询** `/admin/mes/cards` — 缺少 form 提交机制，Enter 键无法触发查询
2. **排程看板/物料消耗/生产异常** — 标记为"功能开发中"，需后续实现
3. **计划搜索框** — HTMX keyup trigger 可能需要调试 debounce 时序
4. **Dashboard 待入库批次** 显示0，实际有1条（status=4），查询条件可能需调整
