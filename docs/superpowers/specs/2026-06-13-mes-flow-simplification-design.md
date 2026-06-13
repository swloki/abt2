# MES 流程简化设计 — 需求池到生产批次的一键贯通

> 日期: 2026-06-13
> 状态: Draft
> 范围: MES 模块 — `production_plan`、`work_order`、`production_batch`、`demand_handler`

## 1. 问题诊断

### 当前流程（6 步，4+ 次手动操作）

```
销售订单确认 → 需求池 → 创建生产计划草稿 → 确认计划 → 下达计划到工单(Draft)
    → 手动下达工单 → 创建生产批次 → 工序报工 → 完工入库
```

### 三个核心问题

| # | 问题 | 根因 |
|---|------|------|
| P1 | **计划层是空壳** | `ProductionPlan` 只做了创建草稿→确认→批量创建 Draft 工单，没有排程能力 |
| P2 | **工序来源错误** | `WorkOrder.release()` 从 BOM 叶子节点生成 `WorkOrderRouting`，而不是从 Routing 工艺路线读取。BOM 叶子是物料组件，不是工序 |
| P3 | **手动操作太多** | 计划确认→下达计划→逐个下达工单，每步都需要人工点击。计划员至少操作 4-5 次才能让车间开工 |

## 2. 目标流程（3 步，2 次操作）

```
需求池(选需求) → 生成计划(含排程) → 确认并下达(一键生成 Released 工单 + 批次)
```

计划员操作：
1. 在需求池选择需求，指定排程参数（产品、数量、日期、工作中心、优先级）
2. 点击"确认并下达" → 系统自动：创建计划 → 创建已 Released 工单 → 从 Routing 读工序 → 创建默认批次 → 库存预留 → 创建领料单

## 3. 改动清单

### 3.1 修复工序来源（P2 — 最关键）

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
   - 创建 1 个默认工序"生产"（process_name = "生产", step_no = 1）
   - 后续可手工在工单上补充工序
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

### 3.2 `release_to_work_orders` 增强：一键到底（P1 + P3）

**文件**: `abt-core/src/mes/production_plan/implt.rs`

**当前逻辑**: 创建 Draft 工单就结束了，需要手动再 release。

**新逻辑**: 创建工单后立即调用 `release()`，状态直接到 Released。

```rust
// 伪代码
for item in &items {
    // 创建工单
    let wo_id = work_order_svc.create(ctx, db, CreateWorkOrderReq { ... }).await?;
    
    // 立即下达（内部：读 Routing → 创建 WorkOrderRouting → 创建批次 → 预留 → 领料单）
    let wo = work_order_svc.find_by_id(ctx, db, wo_id).await?;
    work_order_svc.release(ctx, db, wo_id, wo.version).await?;
}
```

**批量创建的事务保证**: 整个 `release_to_work_orders` 调用方（handler）已在外层开启事务，无需额外处理。

### 3.3 计划层排程增强（P1）

**文件**: `abt-core/src/mes/production_plan/model.rs` + `implt.rs`

**改动**:

1. `CreatePlanItemReq` 增加 `routing_id: Option<i64>` 和 `work_center_id: Option<i64>` 字段（已存在，但需要前端传入）
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

### 3.4 批次灵活拆分

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

### 3.5 数据库改动

**无 schema 变更**。现有表结构已满足所有需求：
- `work_orders.routing_id` — 已存在，修复代码逻辑即可
- `work_orders.work_center_id` — 已存在
- `production_plan_items.routing_id` / `work_center_id` — 已存在

### 3.6 前端页面改动

| 页面 | 改动 |
|------|------|
| 需求池 | 增加排程参数输入（工作中心、优先级、日期），增加"生成计划并下达"按钮 |
| 生产计划详情 | 增加"确认并下达"按钮（调用 `release_to_work_orders`，现在会一步到底） |
| 工单列表 | 无改动（工单直接是 Released 状态出现） |

## 4. 接口变更

### 4.1 WorkOrderService.release() — 内部逻辑变更

签名不变，内部实现变更：
- 工序来源：BOM 叶子 → Routing 工艺路线
- 无 Routing 时：创建默认"生产"工序
- `routing_id` 写入 `WorkOrder` 记录

### 4.2 ProductionPlanService.release_to_work_orders() — 行为变更

签名不变，行为增强：
- 创建工单后立即 `release()`（状态 Draft → Released）
- 支持传入 `BatchSplitStrategy`（可选）

### 4.3 MesDemandService.create_plan_from_demands() — 增强

- 自动根据 product_code 查找关联 Routing
- 排程参数（routing_id, work_center_id）自动填充到 CreatePlanItemReq

## 5. 向后兼容性

| 变更 | 影响 | 兼容处理 |
|------|------|---------|
| release() 工序来源改为 Routing | 已有工单的 WorkOrderRouting 不受影响（只改 release 创建新工序的逻辑） | 无 |
| release_to_work_orders() 一步到底 | 已有 Draft 工单不受影响 | 前端增加"确认并下达"按钮，原有的"确认"和"下达"两步仍可用 |
| BatchSplitStrategy | 纯新增参数，默认值 Single | 无 |

## 6. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| 部分产品无 Routing 导致工序为空 | 高（用户确认"部分有"） | 无 Routing 时自动创建默认"生产"工序，保证流程不断 |
| release_to_work_orders 事务过长 | 低 | 单个工单 release 已是原子操作，批量按单工单粒度隔离 |
| 前端页面改动影响现有流程 | 低 | "确认并下达"是新增按钮，不替换原有操作路径 |

## 7. 实施顺序

1. **修复工序来源**（P2）— 改 `work_order/implt.rs` 的 `release()` 方法
2. **release_to_work_orders 一步到底**（P3）— 改 `production_plan/implt.rs`
3. **需求池自动填充 Routing** — 改 `demand_handler/implt.rs`
4. **前端页面调整** — 需求池增加排程参数、计划详情增加"确认并下达"
5. **更新 UML 设计文档** — 同步 `docs/uml-design/04-mes.html`

## 8. 不在本次范围

- 排程 V2（产能日历、物料 ATP、甘特图）
- 自动排产算法（MRP）
- 批次工序报工流程优化
- 生产异常处理流程
