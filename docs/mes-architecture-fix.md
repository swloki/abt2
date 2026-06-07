# MES 架构违规修复计划

## 问题描述
`abt-web` 中6个页面文件直接使用 `sqlx::query` 操作数据库，违反了"abt-web 不能操作数据库，只能调用 abt-core 接口"的约定。

## 涉及文件

| 文件 | SQL 查询内容 | 需要的 abt-core 接口 |
|------|-------------|---------------------|
| `mes_dashboard.rs` | Dashboard统计(10个COUNT查询) + 最近操作(UNION) | `MesDashboardService::get_stats()` + `get_recent_ops()` |
| `mes_batch_list.rs` | 批次列表分页+筛选 | `ProductionBatchService::list_filtered()` |
| `mes_inspection_list.rs` | 报检列表分页+筛选 | `ProductionInspectionService::list_filtered()` |
| `mes_receipt_list.rs` | 入库列表分页+筛选 | `ProductionReceiptService::list_filtered()` |
| `mes_plan_list.rs` | 计划行统计+关联销售单 | `ProductionPlanService::get_plan_stats()` |
| `mes_report_list.rs` | 报工列表分页+筛选 | `WorkReportService::list_filtered()` |

## 修复方案

### 第1步：在 abt-core 中添加 service 方法和 repo 实现

1. **新建 `abt-core/src/mes/dashboard/` 模块**
   - `model.rs` — `DashboardStats`, `RecentOp` 结构体
   - `service.rs` — `MesDashboardService` trait
   - `repo.rs` — SQL 查询实现
   - `implt.rs` — 实现

2. **给现有 service trait 添加 list 方法**
   - `ProductionBatchService::list_batches(ctx, db, filter, page, size) -> PaginatedResult<BatchListItem>`
   - `ProductionInspectionService::list_inspections(ctx, db, filter, page, size) -> PaginatedResult<InspectionListItem>`
   - `ProductionReceiptService::list_receipts(ctx, db, filter, page, size) -> PaginatedResult<ReceiptListItem>`
   - `WorkReportService::list_reports(ctx, db, filter, page, size) -> PaginatedResult<ReportListItem>`
   - `ProductionPlanService::get_plan_stats(ctx, db, plan_ids) -> HashMap<i64, PlanStats>`

### 第2步：在 abt-core 对应的 repo 中实现 SQL

把 `abt-web` 中的 SQL 搬到 `abt-core` 的 repo 层。

### 第3步：abt-web 改为调用 service 接口

把 `abt-web` 中的 `sqlx::query` 替换为 service 调用。

### 第4步：确保 AppState 提供所有 service

检查 `abt-web/src/state.rs` 是否已注册新的 service。

## 优先级
先修 dashboard（影响最大），然后修各列表页面。
