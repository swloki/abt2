# MES 流程简化设计 — 需求池到生产批次的一键贯通

> 日期: 2026-06-13
> 状态: Draft (v4 — 含技术评审 + ERP 业务评审全部改进)
> 范围: MES 模块 + WMS 物料管理相关（领料单、倒冲、预留）

## 1. 问题诊断

### 当前流程（6 步，4+ 次手动操作）

```
销售订单确认 → 需求池 → 创建生产计划草稿 → 确认计划 → 下达计划到工单(Draft)
    → 手动下达工单 → 创建生产批次 → 工序报工 → 完工入库
```

### 核心问题（13 个，按严重性排列）

**P0 — 数据正确性（上线即修复）**

| # | 问题 | 根因 |
|---|------|------|
| P2 | **工序来源错误** | `WorkOrder.release()` 从 BOM 叶子节点生成 `WorkOrderRouting`，BOM 叶子是物料组件不是工序 |
| P4 | **BOM 未快照** | `bom_snapshot_id` 始终 None，BOM 变更后领料/倒冲/成本全错位 |
| P5 | **库存预留对象错误** | release 时预留的是成品，实际应预留原材料/组件 |
| P7 | **领料单是空壳** | `create_for_work_order()` 只创建单头，无明细行（不知领什么料、领多少） |
| P8 | **倒冲仓库为 0** | `backflush/implt.rs:130` 硬编码 `warehouse_id: 0`，原材料库存永远不会被正确扣减 |

**P1 — 业务完整性（必须同步解决）**

| # | 问题 | 根因 |
|---|------|------|
| P6 | **销售订单追溯断裂** | `release_to_work_orders` 创建工单时 `sales_order_id: None` |
| P9 | **倒冲量=理论量永远无差异** | `actual_qty = theoretical_qty`，差异检测机制形同虚设 |
| P10 | **缺物料可用性预检** | 无基本 ATP 检查就下达工单，工单到车间后等料 → 产能浪费 |
| P11 | **领料/倒冲职责重叠未定义** | 两种物料消耗方式并存，可能导致双重扣减或漏扣 |
| P12 | **无超额生产处理** | 报工完成量可超过计划量，系统无容差控制 |

**P2 — 流程效率**

| # | 问题 | 根因 |
|---|------|------|
| P1 | **计划层是空壳** | `ProductionPlan` 没有排程能力 |
| P3 | **手动操作太多** | 计划员至少 4-5 次点击才能让车间开工 |

**P3 — 渐进增强（V2 范围）**

| # | 问题 | 根因 |
|---|------|------|
| P13 | **WIP 在制品库存缺失** | 领料到完工之间无 WIP 记录，盘点/成本/追溯都有盲区 |

## 2. 目标流程（3 步，2 次操作）

```
需求池(选需求) → 生成计划(含排程) → 确认并下达(一键生成 Released 工单 + 批次)
```

**"确认并下达"一步完成的原子操作序列**：
1. 预校验（Routing/BOM/物料可用性/产品有效性）
2. 创建工单（Draft）
3. BOM 快照（冻结用料清单）
4. 从 Routing 创建工序（或虚拟默认工序）
5. 创建生产批次
6. 原材料 HARD 预留（从 BOM 快照展开，区分倒冲件/领料件）
7. 创建领料单（含明细行，仅非倒冲件）
8. 工单状态 → Released

## 3. 改动清单

### 3.1 修复工序来源 + 虚拟默认工序（P0 — P2）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

**当前逻辑（错误）**: 从 BOM 叶子节点（物料组件）当工序用。

**新逻辑**:
```
1. 查找产品关联的工艺路线：RoutingService.get_bom_routing(product_code)
2. 如果找到 Routing：
   - 从 routing_steps 读取工序列表
   - 每个步骤映射到 WorkOrderRouting（process_name 从 LaborProcessDict 取）
   - routing_id 记录到 WorkOrder 上（溯源）
3. 如果没有 Routing：
   - 创建 1 个虚拟默认工序：{ step_no: 1, process_name: "生产", is_inspection_point: false }
   - 设计决策：使用虚拟默认工序而非"免工序"，保持 ProductionBatch 状态机统一
```

**数据映射**:

| RoutingStep 字段 | WorkOrderRouting 字段 | 说明 |
|---|---|---|
| `step_order` | `step_no` | 直接映射 |
| `process_code` | `process_name` | 从 LaborProcessDict 查 name |
| — | `work_center_id` | 从 Routing 关联的工作中心读取 |
| — | `planned_qty` | = work_order.planned_qty |
| — | `completed_qty` | Decimal::ZERO |
| — | `status` | RoutingStatus::Pending |

### 3.2 BOM 快照（P0 — P4）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

```
release() 新增步骤（在创建 WorkOrderRouting 之前）：
1. 查找产品当前已发布 BOM：BomQueryService.find_published(product_code)
2. 如果存在已发布 BOM：
   - 创建 BomSnapshot（冻结当前 BOM 结构 — 完整树，所有层级节点）
   - UPDATE work_orders SET bom_snapshot_id = snapshot_id
3. 如果无 BOM：
   - bom_snapshot_id 保持 None
```

> `BomSnapshot` 模型已存在（`abt-core/src/master_data/bom/model.rs`），表 `bom_snapshots` 已有。
> 仅需补充 release() 中的调用逻辑。

**快照影响范围**:
- 领料单明细行 — 基于快照展开
- 倒冲 — 基于快照计算
- 成本核算 — 基于快照

### 3.3 修正库存预留（P0 — P5）

**文件**: `abt-core/src/mes/work_order/implt.rs` — `release()` 方法

**新逻辑**:
```
release() 步骤 6 — 库存预留：
1. 从 BOM 快照展开叶子节点（仅非倒冲件 — 参见 §3.8）
2. 对每个非倒冲组件创建 HARD 预留：
   ReserveRequest {
       product_id: component.product_id,
       warehouse_id: 按 4 级优先级策略确定（见下表）,
       reserved_qty: component.quantity × work_order.planned_qty,
       source_type: DocumentType::WorkOrder,
       source_id: work_order.id,
   }
3. 倒冲件不做预留（倒冲在完工时按实际量扣减）
4. 如果无 BOM 快照：跳过预留
```

**发料仓库确定策略**（优先级从高到低）:

| 优先级 | 来源 | 说明 |
|--------|------|------|
| 1 | BOM 行指定仓库 | 组件行如有 `warehouse_id` 则优先使用 |
| 2 | 工单工作中心的默认仓库 | `WorkOrder.work_center_id` → 工作中心关联仓库 |
| 3 | 产品主数据的默认仓库 | 组件 `product_id` 的默认仓库 |
| 4 | 系统参数默认生产仓库 | 全局配置的后备仓库 |

### 3.4 领料单明细行生成（P0 — P7）

**文件**: `abt-core/src/wms/material_requisition/implt.rs` — `create_for_work_order()` 方法

**当前问题**: 只创建单头，无明细行。仓管员不知该备什么料。

**新逻辑**:
```
create_for_work_order() 改造：
1. 读取工单的 bom_snapshot_id
2. 如果有 BOM 快照：
   - 展开叶子节点
   - 过滤：仅非倒冲件（is_backflush = false 的组件行）
   - 对每个组件生成领料单明细行：
     { product_id, required_qty = 用量 × planned_qty, warehouse_id（4级策略） }
3. 如果无 BOM 快照：
   - 创建空领料单（允许后续手动添加行）
```

**领料单明细行字段**:

| 字段 | 来源 | 说明 |
|------|------|------|
| product_id | BOM 快照叶子节点 | 组件产品 ID |
| required_qty | node.quantity × work_order.planned_qty | 需求数量 |
| warehouse_id | 4 级仓库策略 | 发料仓库 |
| is_backflush | BOM 节点标记 | false（倒冲件不进领料单） |

### 3.5 修正倒冲仓库 + 实际用量（P0/P1 — P8/P9）

**文件**: `abt-core/src/wms/backflush/implt.rs` — `execute()` 方法

**修复 1 — 仓库来源（P0）**:
```
当前: warehouse_id: 0
修正: 从 BOM 快照组件行取 warehouse_id（同 4 级仓库策略）
      如果组件行无仓库，使用工单工作中心默认仓库
```

**修复 2 — 实际用量输入（P1）**:
```
当前: let actual_qty = theoretical_qty; // 永远无差异

修正: BackflushService.execute() 增加 actual_quantities 参数
struct BackflushExecuteReq {
    work_order_id: i64,
    completed_qty: Decimal,
    actual_quantities: Option<Vec<ComponentActualQty>>,  // 新增：手工输入实际用量
}

struct ComponentActualQty {
    product_id: i64,
    actual_qty: Decimal,
}

逻辑：
- 如果 actual_quantities 提供：使用实际用量，计算差异
- 如果未提供（默认）：actual_qty = theoretical_qty（保持向后兼容）
- 差异率超阈值 → CostEntry(损耗成本)
```

### 3.6 物料消耗策略：领料 vs 倒冲（P1 — P11）

**核心问题**: 领料单和倒冲并存但不互斥，可能双重扣减。

**解决方案**: BOM 节点增加 `is_backflush` 标记。

```
BOM 节点级标记（复用现有 BomNode 结构扩展）：

is_backflush: bool  // 默认 false

- false（领料件）: release 时进领料单，发料时手动扣减
- true（倒冲件）: release 时跳过领料单，完工时按 BOM 自动倒扣

适用场景：
- 关键件/高价值件 → is_backflush = false → 走领料单，精确管控
- 大宗通用件（螺丝、胶水）→ is_backflush = true → 走倒冲，简化操作
```

**数据存储**: `is_backflush` 存储在 `BomNode` 的现有结构中。当前 `BomNode` 使用 JSONB 存储，可以直接扩展字段。

**各环节行为汇总**:

| 环节 | 领料件 (is_backflush=false) | 倒冲件 (is_backflush=true) |
|------|---------------------------|--------------------------|
| release() 预留 | HARD 预留 | 不预留 |
| release() 领料单 | 生成明细行 | 不生成 |
| 领料发料 | 手动领料出库 | 跳过 |
| 完工倒冲 | 不倒冲 | 按 BOM 自动倒扣 |
| 差异检测 | 领料量 vs 需求量 | 倒冲实际量 vs 理论量 |

### 3.7 物料可用性预检（P1 — P10）

**文件**: `abt-core/src/mes/production_plan/implt.rs` — `pre_validate()`

**V1 基本版**（不需要复杂算法）:
```
pre_validate() 增加物料检查：
1. 从 BOM 快照展开组件清单
2. 对每个组件查当前可用库存（on_hand - hard_reserved）
3. 如果任一组件可用量 < 需求量：
   → 添加到 warnings: "物料不足: {组件名} 需要 {X}, 可用 {Y}, 缺口 {Z}"
4. 不阻断下达（计划员可能选择先备料），但必须让计划员看到
```

**ReleaseValidation 增强**:
```rust
struct ReleaseValidation {
    plan_item_id: i64,
    product_id: i64,
    has_routing: bool,
    has_published_bom: bool,
    routing_id: Option<i64>,
    warnings: Vec<String>,
    material_shortages: Vec<MaterialShortage>,  // 新增
}

struct MaterialShortage {
    product_id: i64,
    product_name: String,
    required_qty: Decimal,
    available_qty: Decimal,
    shortage_qty: Decimal,
}
```

### 3.8 超额生产容差控制（P2 — P12）

**文件**: `abt-core/src/mes/production_batch/implt.rs` — `confirm_routing_step()`

```
confirm_routing_step() 增加校验：
- 最后工序报工时：completed_qty + defect_qty 不超过 planned_qty × (1 + over_completion_tolerance)
- over_completion_tolerance 默认 5%（系统参数可配置）
- 超出容差 → DomainError::BusinessRule("报工量超出计划量允许偏差范围")
- 容差内的超额：正常入库，倒冲按实际完工量计算
```

**容差来源优先级**:
1. 工单级指定（如有）
2. 产品主数据 `meta.over_completion_tolerance`（如有）
3. 系统默认 5%

### 3.9 `release_to_work_orders` 增强（P1/P2 — P1/P3）

**文件**: `abt-core/src/mes/production_plan/implt.rs`

**新逻辑**: 预校验 → 创建工单 → 立即 release，单个工单失败不影响其他。

```rust
async fn release_to_work_orders(ctx, db, plan_id) -> BatchReleaseResult {
    let items = get_items(plan_id);

    // 1. 预校验（Routing + BOM + 物料可用性）
    let validations = pre_validate(ctx, db, &items).await;

    // 2. 逐个创建+下达（独立处理，单工单失败不影响其余）
    let mut successful = Vec::new();
    let mut failed = Vec::new();
    for (item, validation) in items.iter().zip(validations.iter()) {
        match release_single_item(ctx, db, item, validation).await {
            Ok(wo) => successful.push(wo),
            Err(e) => failed.push(BatchFailure { index: item.id, error: e }),
        }
    }

    // 3. 更新计划和计划行状态
    if !successful.is_empty() {
        update_plan_status(plan_id, InProgress).await;
        // 成功行 → PlanItemStatus::Released
        // 失败行保持 Planned（支持修正后重试）
    }

    BatchReleaseResult { successful, failed, validations, total: items.len() }
}
```

### 3.10 修复销售订单追溯（P1 — P6）

**文件**: `abt-core/src/mes/production_plan/implt.rs`

```rust
// 修复前: sales_order_id: None
// 修复后:
CreateWorkOrderReq {
    sales_order_id: item.sales_order_id,  // 从 PlanItem 传入
    plan_item_id: Some(item.id),
    ...
}
```

### 3.11 反下达 Unrelease（安全网）

**文件**: `abt-core/src/mes/work_order/service.rs` + `implt.rs`

```rust
async fn unrelease(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, expected_version: i32) -> Result<()>;
```

**拒绝条件**:
- 工单状态 ≠ Released
- 任一批次 current_step > 0（已开工）
- 领料单有已领记录（qty_issued > 0）

**逆操作序列**:
```
1. 乐观锁校验
2. 取消领料单（状态 → Cancelled）
3. 释放库存 HARD 预留
4. 删除 ProductionBatch（WHERE work_order_id = id）
5. 删除 WorkOrderRouting（WHERE work_order_id = id）
6. 清除 work_order.bom_snapshot_id（快照记录保留）
7. 工单状态 → Draft
8. 审计日志
```

### 3.12 事件发布策略

```
release() 发布的事件：
1. WorkOrderReleased — 下游: QMS(质检计划), FMS(成本归集)
2. InventoryReserved — 下游: WMS(库存可见性)
3. MaterialRequisitionCreated — 下游: WMS(备料通知)

不发布的内部步骤: BOM快照、WorkOrderRouting、ProductionBatch 创建
unrelease() 发布: WorkOrderUnreleased, InventoryReservationReleased
```

### 3.13 计划层排程增强（P2 — P1）

**排程 V1（本次）**:
- 按需求交期倒推排程日期
- 提前期：计划员在需求池手动指定 `scheduled_start`
- 按优先级排序，按工作中心分组
- `scheduled_start < today()` → 标记紧急（priority 最高）

### 3.14 批次灵活拆分

```rust
pub enum BatchSplitStrategy {
    Single,                      // 不拆分（默认）
    ByQuantity(Decimal),         // 每 N 件一批
    Custom(Vec<Decimal>),        // 指定数量列表
}
```

`Custom` 模式校验: `sum(quantities) == planned_qty`，每批 > 0。

### 3.15 三层状态流转映射

```
MesDemand       PlanItem          WorkOrder
─────────       ─────────         ─────────
Pending
  ↓ 创建计划    Planned
  ↓ 下达工单    Released          Draft → Released
                                  ↓ 报工
                InProduction      (Released)
                                  ↓ 完工入库
Fulfilled       Completed         Closed

Rejected        Cancelled         Cancelled
```

**状态推导**: 下达成功→PlanItem.Released，首次报工→InProduction，所有工单完工→Completed，需求满足→Fulfilled。

**部分失败**: 成功行=Released，失败行=Planned（保持），支持修正后重试。

### 3.16 WIP 在制品追踪（P3 — V2 范围，本次仅记录设计决策）

**当前问题**: 领料出库→完工入库之间是"黑洞"，无 WIP 记录。

**V2 设计方向**:
- 领料时: 原材料 → WIP 虚拟仓库
- 完工入库时: WIP → 成品仓库 + 倒冲消耗原材料
- 工单上记录 `material_issued_qty`（已领料量）和 `material_consumed_qty`（已消耗量）
- 月末 WIP 价值 = 已领料成本 - 已倒冲成本

**本次不实施，但领料单明细行（§3.4）和倒冲修正（§3.5）已为 V2 铺路。**

### 3.17 数据库改动

**本次需新增**:

| 改动 | 说明 |
|------|------|
| `bom_nodes` 增加 `is_backflush` 字段 | boolean, 默认 false。存储在 JSONB `bom_detail` 的 nodes 数组中，无需 ALTER TABLE |

**其余无 schema 变更**，现有表结构已满足：
- `work_orders.routing_id` / `bom_snapshot_id` / `work_center_id` / `sales_order_id` — 均已存在
- `production_plan_items.routing_id` / `work_center_id` / `status` — 均已存在
- `bom_snapshots` 表 — 已存在
- `material_requisition_lines` 表 — 需确认是否存在，如不存在需新增（见下方）

**领料单明细行**:
> 如果 `material_requisition_lines` 表不存在，需新增 migration。领料单明细行应包含：
> `id, requisition_id, product_id, required_qty, issued_qty, warehouse_id, is_backflush, remark`

### 3.18 前端页面改动

| 页面 | 改动 |
|------|------|
| 需求池 | 排程参数输入 + "生成计划并下达"按钮 |
| 生产计划详情 | "确认并下达"按钮 + 预校验 warnings 展示 |
| 工单列表 | 无改动（工单直接 Released 状态出现） |
| 产品主数据 | 显示 Routing 关联状态，引导补全 |
| BOM 编辑 | 增加节点级 `is_backflush` 切换（领料件/倒冲件） |
| 完工入库 | 倒冲实际用量输入（可选，默认=理论量） |

## 4. 接口变更

### 4.1 WorkOrderService.release() — 内部逻辑变更

签名不变，实现变更：
- 新增: BOM 快照（§3.2）
- 新增: 从 Routing 读工序 / 虚拟默认工序（§3.1）
- 修正: 预留原材料而非成品，区分倒冲件（§3.3）
- `routing_id` + `bom_snapshot_id` 写入 WorkOrder

### 4.2 WorkOrderService.unrelease() — 新增

```rust
async fn unrelease(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, expected_version: i32) -> Result<()>;
```

### 4.3 ProductionPlanService.release_to_work_orders() — 行为变更

返回类型增强:
```rust
struct BatchReleaseResult {
    plan_id: i64,
    successful_work_orders: Vec<WorkOrder>,
    failed_items: Vec<BatchFailure>,
    validations: Vec<ReleaseValidation>,
    total: i32,
}
```

行为: 预校验 → 一键到底 → PlanItem 状态更新 → 失败行可重试

### 4.4 MaterialRequisitionService.create_for_work_order() — 行为变更

- 从 BOM 快照展开非倒冲组件，自动生成明细行
- 无 BOM → 空领料单

### 4.5 BackflushService.execute() — 签名变更

```rust
// 新增 actual_quantities 可选参数
async fn execute(ctx, db, work_order_id, completed_qty, actual_quantities: Option<Vec<ComponentActualQty>>) -> Result<i64>;
```

- 仓库来源: BOM 快照组件行（4 级策略）
- 实际用量: 可选输入，默认=理论量

### 4.6 MesDemandService.create_plan_from_demands() — 增强

- 自动查找关联 Routing
- 排程参数自动填充

## 5. release() 完整操作序列

```
WorkOrderService.release(ctx, db, id, expected_version):

  1. 加载工单，校验状态 Draft/Planned
  2. 乐观锁更新状态 → Released
  3. BOM 快照：
     - 查产品已发布 BOM → 创建 BomSnapshot（完整 BOM 树）
     - 写入 work_order.bom_snapshot_id
     - 无 BOM → 跳过
  4. 工序创建：
     - 查 Routing → WorkOrderRouting 从 RoutingStep 映射
     - 无 Routing → 虚拟默认工序 { step_no: 1, "生产" }
  5. 创建默认 ProductionBatch（1 个，qty = planned_qty）
  6. 原材料库存预留（仅非倒冲件）：
     - 从 BOM 快照展开叶子节点，过滤 is_backflush = false
     - 每个组件: ReserveRequest { product_id, qty = 用量 × planned_qty }
     - 仓库: 4 级优先级策略
     - 无 BOM 快照 → 跳过
  7. 创建领料单（含明细行）：
     - 从 BOM 快照展开非倒冲叶子节点
     - 生成明细行: { product_id, required_qty, warehouse_id }
     - 无 BOM 快照 → 空领料单
  8. 发布事件 + 审计日志
```

## 6. 向后兼容性

| 变更 | 兼容处理 |
|------|---------|
| release() 工序来源改为 Routing | 已有工单不受影响 |
| release() 新增 BOM 快照 | 旧工单 None 保持不变 |
| release() 预留改为原材料 | 旧工单预留已释放/独立处理 |
| 领料单增加明细行 | 旧领料单无行不受影响 |
| 倒冲增加实际用量参数 | Optional 参数，默认=理论量（向后兼容） |
| BOM 节点 is_backflush | 新字段默认 false，旧行为不变 |
| unrelease() | 纯新增方法 |
| BatchSplitStrategy | 纯新增参数，默认 Single |
| PlanItem 状态更新 | 现有 Planned 行不受影响 |

## 7. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| 部分产品无 Routing | 高 | 虚拟默认工序 + 前端 warning |
| 部分产品无已发布 BOM | 中 | 跳过快照/预留/领料单 |
| material_requisition_lines 表不存在 | 低 | 需新增 migration |
| 倒冲件标记需要回填现有 BOM | 中 | 新字段默认 false，不影响现有行为；可渐进回填 |
| 批量 release 部分失败 | 中 | 单工单粒度隔离 + 可重试 |
| 物料预检增加 release 耗时 | 低 | 简单库存查询，毫秒级 |

## 8. 实施顺序

### 阶段 1：核心数据正确性修复（P0，可独立上线）

1. 修复工序来源 + 虚拟默认工序 — `work_order/implt.rs` `release()`
2. 新增 BOM 快照 — `work_order/implt.rs` `release()`
3. 修正库存预留（原材料 + 区分倒冲件） — `work_order/implt.rs` `release()`
4. 领料单明细行生成 — `material_requisition/implt.rs` `create_for_work_order()`
5. 修正倒冲仓库 — `backflush/implt.rs` `execute()`
6. 验证: 单个工单 release → 领料 → 倒冲 全流程正确

### 阶段 2：业务完整性（P1）

7. 实现 unrelease() — `work_order/service.rs` + `implt.rs`
8. 修复销售订单追溯 — `production_plan/implt.rs`
9. 倒冲实际用量输入 — `backflush/implt.rs` + 前端
10. BOM 节点 is_backflush 标记 — `bom/model.rs` + BOM 编辑页
11. 物料可用性预检 — `production_plan/implt.rs` `pre_validate()`
12. 超额生产容差 — `production_batch/implt.rs` `confirm_routing_step()`
13. 验证: 领料件/倒冲件分流 + 倒冲差异检测 + 物料预检

### 阶段 3：流程简化（P2）

14. release_to_work_orders 一键到底 — `production_plan/implt.rs`
15. 预校验 + 失败隔离 + PlanItem 状态 — `production_plan/implt.rs`
16. 需求池自动填充 Routing — `demand_handler/implt.rs`
17. 事件发布 — release/unrelease 事件集成
18. 验证: 需求池到工单完整贯通 + 反下达

### 阶段 4：前端 + 文档

19. 前端页面调整（需求池 + 计划详情 + BOM 编辑 + 完工入库）
20. 排程 V1（交期倒推 + 优先级 + 工作中心分组）
21. 更新 UML 设计文档 `docs/uml-design/04-mes.html`

## 9. 不在本次范围

- 排程 V2（产能日历、物料 ATP 算法、甘特图）
- 自动排产算法（MRP）
- WIP 在制品追踪（P13 — V2 范围，§3.16 记录了设计方向）
- 批次工序报工流程优化
- 生产异常处理流程
- 已有工单的库存预留数据修正（需单独数据迁移脚本）
- 产品主数据增加 `lead_time` 字段（排程 V2）
- 反下达的权限控制（V1 仅做操作层面限制）
