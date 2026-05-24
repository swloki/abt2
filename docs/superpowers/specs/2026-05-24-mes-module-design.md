# MES 生产模块实现设计

> 日期：2026-05-24
> 设计文档权威来源：`docs/uml-design/04-mes.html`
> 实现范围：`abt-core` crate，不涉及 gRPC/Proto

## 1. 概述

MES（制造执行系统）模块覆盖从生产计划到完工入库的完整生产管理流程。模块位于 `abt-core/src/mes/`，严格遵循 `docs/uml-design/04-mes.html` 中的 UML 类图设计。

### 核心业务流

```
ProductionPlan → WorkOrder → ProductionBatch → WorkOrderRouting → WorkReport → ProductionReceipt
                                                    ↗ OutsourcingOrder (OM 模块管理)
                                                    ↗ ProductionInspection (报检)
```

### 关键决策

- **实现范围**：一次性全部实现 6 个 Service、7 个实体
- **跨模块调用**：使用 stub（与 WMS 模块相同模式），后续对接真实服务
- **代码组织**：方案 A — 扁平子模块，每个实体一个子目录，与 WMS 模块一致
- **数据库迁移**：单个迁移文件，包含所有表、枚举、索引
- **Proto/gRPC**：本次不涉及，仅实现 `abt-core` 层
- **WorkOrderRouting**：带 `batch_id`，每个批次拥有独立的工序列

## 2. 模块结构

```
abt-core/src/mes/
  mod.rs                     # pub mod 声明 + pub use
  enums.rs                   # 所有 MES 枚举
  stubs.rs                   # 跨模块 stub
  production_plan/           # ProductionPlanService (5 methods)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  work_order/                # WorkOrderService (6 methods)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  production_batch/          # ProductionBatchService (8 methods)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  work_report/               # WorkReportService (4 methods, 只读)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  production_inspection/     # ProductionInspectionService (3 methods)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  production_receipt/        # ProductionReceiptService (3 methods)
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
```

## 3. 枚举定义（enums.rs）

所有枚举 derives `Debug, Clone, PartialEq, sqlx::Type`，实现 `Display`。

| 枚举 | 变体 |
|------|------|
| PlanType | Mto, Mts |
| PlanStatus | Draft, Confirmed, InProgress, Completed, Cancelled |
| PlanItemStatus | Planned, Released, InProduction, Completed, Cancelled |
| WorkOrderStatus | Draft, Planned, Released, Closed, Cancelled |
| BatchStatus | Pending, InProgress, Suspended, PendingReceipt, Completed, Cancelled |
| RoutingStatus | Pending, InProgress, Completed, Skipped |
| ShiftType | Day, Night |
| InspectionType | FirstArticle, InProcess, Final |
| InspectionResultType | Pass, Fail, Conditional |
| ReceiptStatus | Draft, Confirmed, Cancelled |
| DefectReason | MaterialDefect, EquipmentFault, OperatorError, ProcessIssue |

`DefectReason` 额外实现 `affect_wage() -> bool`：
- MaterialDefect, EquipmentFault, ProcessIssue → `true`
- OperatorError → `false`

## 4. 实体与字段

严格按 `04-mes.html` 定义，以下仅记录关键约束。

### ProductionPlan + ProductionPlanItem
- `doc_number` 格式：`PP-2026-05-xxxxx`（DocumentSequence 生成）
- `plan_type`: MTO（按单）/ MTS（备货）

### WorkOrder
- `doc_number` 格式：`WO-2026-05-xxxxx`
- `version`: 乐观锁版本号，release/close/cancel 传入 `expected_version`
- `planned_qty`, `completed_qty`, `scrap_qty` 为 read-only 聚合（来自 ProductionBatch）

### ProductionBatch（流转卡）
- `batch_no` 格式：`WO-2026-05-xxxxx-01`
- `card_sn`: 流转卡序列号（二维码唯一标识）
- `current_step`: 工序状态机核心，禁止业务代码直接赋值
- `sum(batch_qty) <= work_order.planned_qty`

### WorkOrderRouting（批次级工序快照）
- `batch_id` FK → production_batches（每个批次独立工序列）
- 下达时从主数据复制，后续不同步（快照语义）
- `completed_qty` / `defect_qty` 由 SQL 原子增量维护

### WorkReport（报工）
- `doc_number` 格式：`WR-2026-05-xxxxx`
- 幂等约束：UNIQUE(batch_id, routing_id, worker_id, shift, report_date)
- 创建由 `ProductionBatchService.confirm_routing_step` 内部完成，不暴露独立创建接口

### ProductionInspection（报检）
- `doc_number` 格式：`PI-2026-05-xxxxx`
- IPQC：由 `WorkOrderRouting.is_inspection_point` 自动触发
- FQC：完工入库前触发

### ProductionReceipt（完工入库）
- `doc_number` 格式：`PR-2026-05-xxxxx`
- confirm 前必须通过 QMS FQC 硬门校验

## 5. Service 接口

### ProductionPlanService（5 methods）

```rust
#[async_trait]
pub trait ProductionPlanService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreatePlanReq) -> Result<i64, DomainError>;
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<ProductionPlan, DomainError>;
    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn release_to_work_orders(ctx: ServiceContext<'_>, plan_id: i64) -> Result<BatchReleaseResult, DomainError>;
    async fn list(ctx: ServiceContext<'_>, filter: PlanFilter, page: u32, page_size: u32) -> Result<PaginatedResult<ProductionPlan>, DomainError>;
}
```

- `create`: DocSequence 生成编号，插入 plan + items
- `confirm`: Draft → Confirmed（状态校验）
- `release_to_work_orders`: ContinueOnError 模式，部分失败不影响其他行。为每个 PlanItem 创建 WorkOrder + ProductionBatch(min 1) + WorkOrderRouting（从主数据快照）+ InvRes.reserve(Hard) + WMS.MaterialRequisitionService.create_for_work_order()
- `list`: 返回 PaginatedResult

### WorkOrderService（6 methods）

```rust
#[async_trait]
pub trait WorkOrderService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreateWorkOrderReq) -> Result<i64, DomainError>;
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<WorkOrder, DomainError>;
    async fn release(ctx: ServiceContext<'_>, id: i64, expected_version: i32) -> Result<(), DomainError>;
    async fn close(ctx: ServiceContext<'_>, id: i64, expected_version: i32) -> Result<(), DomainError>;
    async fn cancel(ctx: ServiceContext<'_>, id: i64, expected_version: i32) -> Result<(), DomainError>;
    async fn list(ctx: ServiceContext<'_>, filter: WorkOrderFilter, page: u32, page_size: u32) -> Result<PaginatedResult<WorkOrder>, DomainError>;
}
```

- `release`: Draft/Planned → Released，乐观锁校验，创建 ProductionBatch(min 1) + WorkOrderRouting + InvRes.reserve(Hard)
- `close`: 所有 batch 完成后可关闭，InvRes.fulfill/cancel 释放剩余
- `cancel`: 同步释放当前 batch 的 Hard 预留，发布 WorkOrderCancelled 事件
- `completed_qty`/`scrap_qty` = read-only，聚合自 ProductionBatch

### ProductionBatchService（8 methods）

```rust
#[async_trait]
pub trait ProductionBatchService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreateBatchReq) -> Result<i64, DomainError>;
    async fn split_work_order(ctx: ServiceContext<'_>, work_order_id: i64, splits: Vec<SplitReq>) -> Result<Vec<i64>, DomainError>;
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<ProductionBatch, DomainError>;
    async fn list_by_work_order(ctx: ServiceContext<'_>, work_order_id: i64) -> Result<Vec<ProductionBatch>, DomainError>;
    async fn confirm_routing_step(ctx: ServiceContext<'_>, batch_id: i64, step_no: i32, req: StepConfirmationReq) -> Result<StepConfirmationResult, DomainError>;
    async fn advance_to_receipt(ctx: ServiceContext<'_>, batch_id: i64) -> Result<(), DomainError>;
    async fn suspend(ctx: ServiceContext<'_>, batch_id: i64, reason: String) -> Result<(), DomainError>;
    async fn resume(ctx: ServiceContext<'_>, batch_id: i64) -> Result<(), DomainError>;
}
```

- `confirm_routing_step` 是核心原子入口（单事务）：
  1. Guard: `current_step == step_no - 1`（防跳序）
  2. 幂等检查：UNIQUE 约束 → 重复返回已有结果（不报错）
  3. DocSequence 生成 WorkReport 编号 + INSERT work_report
  4. SQL 原子增量 `WorkOrderRouting.completed_qty/defect_qty`
  5. 若 `is_inspection_point` → 创建 ProductionInspection(IPQC) + 挂起批次
  6. 更新 `batch.current_step`
  7. 若最后工序 → 自动推进至 PendingReceipt
  8. 返回 `StepConfirmationResult`
- `split_work_order`: 验证 `sum(batch_qty) <= planned_qty`，DocSequence 生成 batch_no + card_sn
- `suspend`: 保留 Hard 预留，可由审批流接管
- `resume`: 从 Suspended 恢复

注意：设计文档中的 `scrap` 方法暂不实现（需审批流集成，超出当前范围）。cancel 由 WorkOrderService.cancel 级联处理。

### WorkReportService（4 methods，只读查询）

```rust
#[async_trait]
pub trait WorkReportService: Send + Sync {
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<WorkReport, DomainError>;
    async fn list_by_work_order(ctx: ServiceContext<'_>, work_order_id: i64) -> Result<Vec<WorkReport>, DomainError>;
    async fn list_by_batch(ctx: ServiceContext<'_>, batch_id: i64) -> Result<Vec<WorkReport>, DomainError>;
    async fn calculate_wage(ctx: ServiceContext<'_>, worker_id: i64, date_range: DateRange) -> Result<WageSummary, DomainError>;
}
```

- WorkReport 创建由 `ProductionBatchService.confirm_routing_step` 内部完成
- `calculate_wage`: `(completed_qty + non_operator_defect_qty) * unit_price`

### ProductionInspectionService（3 methods）

```rust
#[async_trait]
pub trait ProductionInspectionService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, req: CreateInspectionReq) -> Result<i64, DomainError>;
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<ProductionInspection, DomainError>;
    async fn record_result(ctx: ServiceContext<'_>, id: i64, result: InspectionResultType) -> Result<(), DomainError>;
}
```

- 由 `confirm_routing_step`（IPQC）和 `ProductionReceipt.confirm`（FQC）内部调用
- 也可独立手动创建

### ProductionReceiptService（3 methods）

```rust
#[async_trait]
pub trait ProductionReceiptService: Send + Sync {
    async fn create(ctx: ServiceContext<'_>, work_order_id: i64, batch_id: Option<i64>, received_qty: Decimal, warehouse_id: i64) -> Result<i64, DomainError>;
    async fn find_by_id(ctx: ServiceContext<'_>, id: i64) -> Result<ProductionReceipt, DomainError>;
    async fn confirm(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
```

- `confirm` 流程：
  1. PRE-CHECK: QMS FQC 硬门（stub 调用，失败返回 `BusinessRule`）
  2. WMS.InventoryTransactionService.record(ProductionReceipt)
  3. CostEntry(成品入库成本)
  4. BackflushService.execute() → 失败发 BackflushShortageEvent 到 DeadLetter（不阻断入库）
  5. 成本差异 → CostEntry(VARIANCE)
  6. InvRes.fulfill/cancel(释放 Hard)
  7. 更新 batch.status = Completed

## 6. 跨模块 Stub（stubs.rs）

| Stub | 调用场景 | 返回 |
|------|----------|------|
| DocumentSequenceStub | 所有单据编号生成 | doc_number |
| DocumentLinkStub | 单据关联（WO→PB→WR→PR） | link id |
| InventoryReservationStub | 工单下达 reserve(Hard) / 完工 fulfill/cancel | () |
| AuditLogStub | 所有写操作的审计日志 | log id |
| QmsInspectionStub | FQC 硬门校验 / IPQC 创建 | is_passed / inspection_id |
| WmsInventoryTransactionStub | 入库事务记录 | () |
| WmsMaterialRequisitionStub | 工单下达时创建领料单 | requisition_id |
| BackflushStub | 完工倒冲 | () |
| CostEntryStub | 成本记录 | entry_id |
| BomServiceStub | BOM 展开（工单创建时） | bom snapshot |
| ProductServiceStub | 产品信息查询 | product |

## 7. 并发控制

- **乐观锁**：WorkOrder.version — release/close/cancel 传入 expected_version，冲突返回 `ConcurrentConflict`
- **SQL 原子增量**：WorkOrderRouting.completed_qty/defect_qty 使用 `SET qty = qty + $delta`
- **幂等约束**：work_reports UNIQUE(batch_id, routing_id, worker_id, shift, report_date)

## 8. 事件边界

| 操作 | 边界 | 说明 |
|------|------|------|
| 库存 HARD 预留 | 同步 | InvRes.reserve 同一事务 |
| QMS FQC 硬门 | 同步 | 未通过直接阻断入库 |
| 防跳序 Guard | 同步 | current_step == step_no - 1 |
| 报工原子增量 | 同步 | SQL SET qty = qty + delta |
| 审计日志 | 同步 InCallerTx | 同事务 record |
| 成本记录 CostEntry | 异步 Outbox | 允许短暂延迟 |
| 单据关联 DocumentLink | 异步 Outbox | 可延迟建立 |
| 倒冲失败告警 | 异步 Outbox | BackflushShortageEvent → DeadLetter |
| 委外收货推进工序 | 异步 Outbox | OM → MES handler |
| Workflow 审批触发 | 异步 Outbox | suspend/scrap 由 WorkflowEngine 接管 |
