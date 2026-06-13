# MES 流程简化设计 — 需求池到生产批次的一键贯通

> 日期: 2026-06-13
> 状态: Draft (v2 — 含评审改进)
> 范围: MES 模块 — `production_plan`、`work_order`、`production_batch`、`demand_handler`

## 1. 问题诊断

### 当前流程（6 步，4+ 次手动操作）

```
销售订单确认 → 需求池 → 创建生产计划草稿 → 确认计划 → 下达计划到工单(Draft)
    → 手动下达工单 → 创建生产批次 → 工序报工 → 完工入库
```

### 核心问题（6 个，含 2 个数据正确性问题）

| # | 问题 | 严重性 | 根因 |
|---|------|--------|------|
| P1 | **计划层是空壳** | P2 | `ProductionPlan` 只做了创建草稿→确认→批量创建 Draft 工单，没有排程能力 |
| P2 | **工序来源错误** | P0 | `WorkOrder.release()` 从 BOM 叶子节点生成 `WorkOrderRouting`，BOM 叶子是物料组件不是工序 |
| P3 | **手动操作太多** | P2 | 计划确认→下达计划→逐个下达工单，计划员至少 4-5 次点击才能让车间开工 |
| P4 | **BOM 未快照** | P0 | `bom_snapshot_id` 始终 None，工单下达后 BOM 变更会破坏领料、倒冲、成本核算的数据一致性 |
| P5 | **库存预留对象错误** | P0 | release 时预留的是成品（product_id = 成品ID），实际应预留原材料/组件 |
| P6 | **销售订单追溯断裂** | P1 | `release_to_work_orders` 创建工单时 `sales_order_id: None`，MTO 场景无法追溯来源订单 |

## 2. 目标流程（3 步，2 次操作）

```
需求池(选需求) → 生成计划(含排程) → 确认并下达(一键生成 Released 工单 + 批次)
```

**"确认并下达"一步完成的原子操作序列**：
1. 预校验（Routing/BOM/产品有效性）
2. 创建工单（Draft）
3. BOM 快照（冻结用料清单）
4. 从 Routing 创建工序（或标记免工序）
5. 创建生产批次
6. 原材料 HARD 预留（从 BOM 快照展开）
7. 创建领料单（基于 BOM 快照）
8. 工单状态 → Released

## 3. 改动清单

### 3.1 修复工序来源 + 无 Routing 前置校验（P0 + P2）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

**当前逻辑（错误）**:
```rust
// 从 BOM 叶子节点（物料组件）当工序用
let bom_nodes = new_bom_query_service(...)
    .get_leaf_nodes(ctx, db, bom_id).await?;
let routing_steps = bom_nodes.iter().enumerate().map(|(i, node)| {
    WorkOrderRouting {
        process_name: node.product_code.clone(), // 物料编码当工序名！
        ...
    }
});
```

**新逻辑**:
```
1. 查找产品关联的工艺路线：RoutingService.get_bom_routing(product_code)
2. 如果找到 Routing：
   - 从 routing_steps 读取工序列表
   - 每个步骤映射到 WorkOrderRouting（process_name 从 LaborProcessDict 取）
   - routing_id 记录到 WorkOrder 上（溯源）
3. 如果没有 Routing：
   - 不创建假工序
   - WorkOrderRouting 为空（免工序模式）
   - 允许无工序的工单直接创建批次 + 入库（适用于简单加工/组装）
```

**数据映射**:

| RoutingStep 字段 | WorkOrderRouting 字段 | 说明 |
|---|---|---|
| `step_order` | `step_no` | 直接映射 |
| `process_code` | `process_name` | 从 LaborProcessDict 查 name |
| — | `work_center_id` | 从 Routing 关联的工作中心读取（如无则为 None） |
| — | `standard_time` | 暂为 None（排程 V2 扩展） |
| — | `standard_cost` | 暂为 None |
| — | `planned_qty` | = work_order.planned_qty |
| — | `completed_qty` | Decimal::ZERO |
| — | `status` | RoutingStatus::Pending |

**前置校验（在 `release_to_work_orders` 批量执行前）**:
```rust
/// 预校验结果
struct ReleaseValidation {
    plan_item_id: i64,
    product_id: i64,
    has_routing: bool,
    has_published_bom: bool,
    routing_id: Option<i64>,
    bom_snapshot_id: Option<i64>,
    warnings: Vec<String>,  // 如 "该产品无工艺路线"
}
```

预校验不阻断下达，但将 warnings 返回给前端展示。无 Routing 的计划行标记警告但不阻止。

### 3.2 BOM 快照（P0 — 数据正确性）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

**问题**: 当前 `bom_snapshot_id` 始终 None。BOM 变更后，领料单、倒冲、成本核算全部错位。

**改动**: release 时增加 BOM 快照步骤：
```
release() 新增步骤（在创建 WorkOrderRouting 之前）：
1. 查找产品当前已发布的 BOM：BomQueryService.find_published(product_code)
2. 如果存在已发布 BOM：
   - 创建 BomSnapshot（冻结当前 BOM 结构）
   - UPDATE work_orders SET bom_snapshot_id = snapshot_id
3. 如果无 BOM：
   - bom_snapshot_id 保持 None
   - 领料单允许手动创建或跳过
```

**影响范围**:
- 领料单（`create_for_work_order`）应基于 `work_order.bom_snapshot_id` 展开
- 倒冲（`BackflushService`）应基于快照计算
- 成本核算应基于快照

### 3.3 修正库存预留：原材料而非成品（P0 — 数据正确性）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

**当前逻辑（错误）**:
```rust
// 预留的是成品本身！
ReserveRequest {
    product_id: work_order.product_id,  // 成品
    warehouse_id: 0,                     // 无效仓库
    reserved_qty: work_order.planned_qty,
    ...
}
```

**新逻辑**:
```
release() 步骤 6 — 库存预留：
1. 读取 work_order.bom_snapshot_id 对应的 BOM 快照
2. 展开叶子节点（组件清单）
3. 对每个组件创建预留：
   ReserveRequest {
       product_id: component.product_id,      // 原材料/组件
       warehouse_id: component.warehouse_id,   // 从 BOM 或默认仓库取
       reserved_qty: component.quantity × work_order.planned_qty,  // 用量 × 工单量
       reservation_type: ReservationType::Hard,
       source_type: DocumentType::WorkOrder,
       source_id: work_order.id,
       source_line_id: Some(component.id),     // 组件行号
       ...
   }
4. 如果无 BOM 快照：跳过预留（免物料产品）
```

### 3.4 `release_to_work_orders` 增强：一键到底 + 失败隔离（P1 + P3）

**文件**: `abt-core/src/mes/production_plan/implt.rs`

**当前逻辑**: 创建 Draft 工单就结束了，需要手动再 release。

**新逻辑**: 预校验 → 创建工单 → 立即 release，单个工单失败不影响其他。

```rust
// 伪代码
async fn release_to_work_orders(ctx, db, plan_id) -> BatchReleaseResult {
    let items = get_items(plan_id);

    // 1. 预校验所有计划行
    let validations = pre_validate(ctx, db, &items).await;
    // 收集 warnings，不阻断

    // 2. 逐个创建+下达（每个工单独立处理，单个失败不影响其余）
    let mut successful = Vec::new();
    let mut failed = Vec::new();
    for (item, validation) in items.iter().zip(validations.iter()) {
        match release_single_item(ctx, db, item, validation).await {
            Ok(wo) => successful.push(wo),
            Err(e) => failed.push(BatchFailure { index: item.id, error: e }),
        }
    }

    // 3. 更新计划状态
    if !successful.is_empty() {
        update_plan_status(plan_id, InProgress).await;
    }

    BatchReleaseResult { successful, failed, validations, total: items.len() }
}
```

**关键设计决策**: 单个工单 release 失败不回滚其他已成功的工单。`BatchReleaseResult` 返回详细的成功/失败/校验信息。

### 3.5 修复销售订单追溯（P6）

**文件**: `abt-core/src/mes/production_plan/implt.rs`

**当前代码**:
```rust
CreateWorkOrderReq {
    sales_order_id: None,  // 丢失了 MTO 追溯！
    ...
}
```

**修复**: 从 `ProductionPlanItem.sales_order_id` 传入：
```rust
CreateWorkOrderReq {
    sales_order_id: item.sales_order_id,  // 保留 MTO 追溯
    plan_item_id: Some(item.id),
    ...
}
```

### 3.6 计划层排程增强（P1）

**文件**: `abt-core/src/mes/production_plan/model.rs` + `implt.rs`

**改动**:

1. `CreatePlanItemReq` 的 `routing_id` 和 `work_center_id` 已存在，需自动填充
2. `ProductionPlanServiceImpl.create()` 中，根据 product_code 查 `BomRouting` 自动填充 `routing_id`
3. 排序逻辑：计划行按 `priority ASC, scheduled_start ASC` 排序后下达到工单

**排程 V1（本次）**:
- 按需求交期倒推排程日期
- 按优先级排序
- 按工作中心分组（同一工作中心的工单连续下达）

**排程 V2（后续迭代）**:
- 引入工作中心产能日历
- 引入物料可用性检查（ATP）
- 甘特图可视化

### 3.7 批次灵活拆分

**文件**: `abt-core/src/mes/production_plan/model.rs`

`CreatePlanFromDemandsReq` 或 `release_to_work_orders` 支持指定拆分策略：

```rust
/// 批次拆分策略
pub enum BatchSplitStrategy {
    /// 不拆分：1 工单 = 1 批次（默认）
    Single,
    /// 按数量拆分：每 N 件一批
    ByQuantity(Decimal),
    /// 指定批次数量列表
    Custom(Vec<Decimal>),
}
```

默认不拆分（`Single`），计划员可在下达时选择。

### 3.8 数据库改动

**无 schema 变更**。现有表结构已满足所有需求：
- `work_orders.routing_id` — 已存在
- `work_orders.bom_snapshot_id` — 已存在，修复代码逻辑填入真实值
- `work_orders.work_center_id` — 已存在
- `work_orders.sales_order_id` — 已存在，修复代码逻辑传入
- `production_plan_items.routing_id` / `work_center_id` — 已存在

### 3.9 前端页面改动

| 页面 | 改动 |
|------|------|
| 需求池 | 增加排程参数输入（工作中心、优先级、日期），增加"生成计划并下达"按钮 |
| 生产计划详情 | 增加"确认并下达"按钮（调用 `release_to_work_orders`），展示预校验 warnings |
| 工单列表 | 无改动（工单直接是 Released 状态出现） |
| 产品主数据 | 在产品详情页显示是否有 Routing 关联，引导补全 |

## 4. 接口变更

### 4.1 WorkOrderService.release() — 内部逻辑变更

签名不变，内部实现变更：
- 新增：BOM 快照（3.2）
- 新增：从 Routing 读工序，无 Routing 时免工序（3.1）
- 修正：库存预留原材料而非成品（3.3）
- `routing_id` + `bom_snapshot_id` 写入 `WorkOrder` 记录

### 4.2 ProductionPlanService.release_to_work_orders() — 行为变更

签名变更：返回类型增强，包含 `validations` 字段：
```rust
struct BatchReleaseResult {
    plan_id: i64,
    successful_work_orders: Vec<WorkOrder>,
    failed_items: Vec<BatchFailure>,
    validations: Vec<ReleaseValidation>,  // 新增：预校验结果
    total: i32,
}
```

行为增强：
- 预校验（不阻断，返回 warnings）
- 创建工单后立即 `release()`（状态 Draft → Released）
- 传递 `sales_order_id`（3.5）
- 单个工单失败不影响其他（3.4）
- 支持传入 `BatchSplitStrategy`（可选）

### 4.3 MesDemandService.create_plan_from_demands() — 增强

- 自动根据 product_code 查找关联 Routing
- 排程参数（routing_id, work_center_id）自动填充到 CreatePlanItemReq

## 5. release() 完整操作序列（修订版）

```
WorkOrderService.release(ctx, db, id, expected_version):

  1. 加载工单，校验状态 Draft/Planned
  2. 乐观锁更新状态 → Released
  3. BOM 快照：
     - 查产品已发布 BOM → 创建 BomSnapshot → 写入 work_order.bom_snapshot_id
     - 无 BOM → 跳过
  4. 工序创建：
     - 查 Routing → WorkOrderRouting 从 RoutingStep 映射
     - 无 Routing → 免工序（WorkOrderRouting 为空）
  5. 创建默认 ProductionBatch（1 个，qty = planned_qty）
  6. 原材料库存预留：
     - 从 BOM 快照展开叶子节点
     - 每个组件: ReserveRequest { product_id = 组件, qty = 用量 × planned_qty }
     - 无 BOM 快照 → 跳过预留
  7. 创建领料单（基于 BOM 快照展开的组件清单）
  8. 审计日志
```

## 6. 向后兼容性

| 变更 | 影响 | 兼容处理 |
|------|------|---------|
| release() 工序来源改为 Routing | 已有工单的 WorkOrderRouting 不受影响 | 无 |
| release() 新增 BOM 快照 | 已有工单 `bom_snapshot_id` 为 None，不影响 | 新工单有快照，旧工单保持 None |
| release() 修正预留为原材料 | 已有预留是成品，不影响已有关闭/取消的工单 | 新工单正确预留原材料 |
| release_to_work_orders() 一步到底 | 已有 Draft 工单不受影响 | "确认并下达"是新增按钮，原有路径保留 |
| BatchSplitStrategy | 纯新增参数，默认值 Single | 无 |

## 7. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| 部分产品无 Routing | 高 | 免工序模式 + 前端 warnings 提示 |
| 部分产品无已发布 BOM | 中 | 跳过快照/预留/领料单，免物料模式 |
| BOM 快照增加 release 耗时 | 低 | 快照是 INSERT 操作，毫秒级 |
| 已有工单预留成品的影响 | 低 | 已关闭/取消的工单预留已释放；进行中的工单需数据修正脚本 |

## 8. 实施顺序

1. **修复工序来源 + 无 Routing 处理**（P0）— `work_order/implt.rs` `release()`
2. **新增 BOM 快照**（P0）— `work_order/implt.rs` `release()`
3. **修正库存预留**（P0）— `work_order/implt.rs` `release()`
4. **修复销售订单追溯**（P1）— `production_plan/implt.rs` `release_to_work_orders()`
5. **预校验 + 失败隔离**（P1）— `production_plan/implt.rs`
6. **一键到底**（P2）— `production_plan/implt.rs` `release_to_work_orders()`
7. **需求池自动填充 Routing** — `demand_handler/implt.rs`
8. **前端页面调整** — 需求池 + 计划详情 + 产品详情
9. **更新 UML 设计文档** — `docs/uml-design/04-mes.html`

## 9. 不在本次范围

- 排程 V2（产能日历、物料 ATP、甘特图）
- 自动排产算法（MRP）
- 批次工序报工流程优化
- 生产异常处理流程
- 已有工单的库存预留数据修正（需单独数据迁移脚本）
