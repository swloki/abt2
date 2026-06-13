# MES 流程简化设计 — 需求池到生产批次的一键贯通

> 日期: 2026-06-13
> 状态: Draft (v6 — 项目级评审修正：优先级重排 + 范围收拢 + 分阶段交付)
> 范围: MES 模块 + WMS 物料管理相关（领料单、倒冲、预留）

## 1. 问题诊断

### 当前流程（6 步，4+ 次手动操作）

```
销售订单确认 → 需求池 → 创建生产计划草稿 → 确认计划 → 下达计划到工单(Draft)
    → 手动下达工单 → 创建生产批次 → 工序报工 → 完工入库
```

### 核心问题（13 个，按严重性排列）

**P0 — 数据正确性（阶段 1 修复，影响当前生产数据）**

| # | 问题 | 根因 | 影响范围 |
|---|------|------|---------|
| P2 | **工序来源错误** | `WorkOrder.release()` 从 BOM 叶子节点生成 `WorkOrderRouting`，BOM 叶子是物料组件不是工序 | 所有工单的工序数据均为错误数据 |
| P4 | **BOM 未快照** | `bom_snapshot_id` 始终 None，BOM 变更后倒冲/成本全错位 | 倒冲无基准，是 P8 的上游依赖 |
| P8 | **倒冲仓库为 0** | `backflush/implt.rs:130` 硬编码 `warehouse_id: 0`，原材料永远不会被正确扣减 | **当前生产环境中唯一用户可见的数据错误** |

**P0-Conditional — 仅 picking 模式下触发（随阶段 2 一同解决）**

| # | 问题 | 根因 | 为何非即时 P0 |
|---|------|------|-------------|
| P5 | **库存预留对象错误** | release 时预留的是成品，实际应预留原材料/组件 | 当前 backflush 模式不做预留，用户无感知；仅 picking 模式需要 |
| P7 | **领料单是空壳** | `create_for_work_order()` 只创建单头，无明细行 | 当前 backflush 模式不使用领料单；仅 picking 模式需要 |

**P1 — 业务完整性（阶段 2-3 解决）**

| # | 问题 | 根因 | 阶段 |
|---|------|------|------|
| P6 | **销售订单追溯断裂** | `release_to_work_orders` 创建工单时 `sales_order_id: None` | 阶段 1（成本极低，顺带修复） |
| P9 | **倒冲量=理论量永远无差异** | `actual_qty = theoretical_qty`，差异检测机制形同虚设 | V1 保持现状，V1.5 补差异调整 |
| P10 | **缺物料可用性预检** | 无基本 ATP 检查就下达工单，工单到车间后等料 → 产能浪费 | 阶段 3 |
| P11 | **领料/倒冲职责重叠未定义** | 两种物料消耗方式并存，可能导致双重扣减或漏扣 | 阶段 2（引入 material_consumption_mode） |
| P12 | **无超额生产处理** | 报工完成量可超过计划量，系统无容差控制 | 阶段 3 |

**P2 — 流程效率（阶段 3-4 解决）**

| # | 问题 | 根因 | 阶段 |
|---|------|------|------|
| P1 | **计划层是空壳** | `ProductionPlan` 没有排程能力 | 阶段 4 |
| P3 | **手动操作太多** | 计划员至少 4-5 次点击才能让车间开工 | 阶段 3 |

**P3 — 渐进增强（V2 范围）**

| # | 问题 | 根因 |
|---|------|------|
| P13 | **WIP 在制品库存缺失** | 领料到完工之间无 WIP 记录，盘点/成本/追溯都有盲区 |

> **优先级重排说明**：P5（预留对象错误）和 P7（领料单空壳）从 P0 降为 P0-Conditional。
> 当前系统只有 backflush 模式在用——backflush 模式下 release 不做预留、不使用领料单，因此这两个问题对生产无实际影响。
> 它们只在引入 picking 模式时才暴露，随阶段 2 一并解决。

## 2. 目标流程（3 步，2 次操作）

```
需求池(选需求) → 生成计划(含排程) → 确认并下达(一键生成 Released 工单 + 批次)
```
> 注：阶段 1 只走 backflush 路径（步骤 6-7 跳过），picking 模式在阶段 2 引入 material_consumption_mode 后才生效。

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
1. 检查产品的 material_consumption_mode（§3.6）
2. 如果为 "picking"：
   - 从 BOM 快照展开叶子节点（全部组件）
   - 对每个组件创建 HARD 预留：
     ReserveRequest { product_id, warehouse_id（4级策略）, reserved_qty = 用量 × planned_qty }
3. 如果为 "backflush"（默认）：
   - 不预留（倒冲在完工时按实际量扣减）
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
1. 检查产品的 material_consumption_mode（§3.6）
2. 如果为 "picking" 且有 BOM 快照：
   - 展开叶子节点（全部组件）
   - 对每个组件生成领料单明细行：
     { product_id, required_qty = 用量 × planned_qty, warehouse_id（4级策略） }
3. 如果为 "backflush" 或无 BOM 快照：
   - 不生成领料单（backflush 模式下物料通过倒冲消耗）
```

**领料单明细行字段**:

| 字段 | 来源 | 说明 |
|------|------|------|
| product_id | BOM 快照叶子节点 | 组件产品 ID |
| required_qty | node.quantity × work_order.planned_qty | 需求数量 |
| warehouse_id | 4 级仓库策略 | 发料仓库 |

### 3.5 修正倒冲仓库 + 实际用量（P0/P1 — P8/P9）

**文件**: `abt-core/src/wms/backflush/implt.rs` — `execute()` 方法

**修复 1 — 仓库来源（P0）**:
```
当前: warehouse_id: 0
修正: 按 4 级仓库策略确定（§3.3）：
  1. BOM 快照组件行的 warehouse_id（如果快照存在且组件行有指定）
  2. 工单工作中心的默认仓库
  3. 组件产品的默认仓库
  4. 系统参数默认生产仓库
注：如果 BOM 快照为 None（产品无已发布 BOM），跳过优先级 1，从优先级 2 开始回退。
```

**修复 2 — 实际用量（V1.5 — 不改变完工操作流程）**:

V1 策略：保持 `actual_qty = theoretical_qty`（向后兼容），但修正仓库来源。
后台记录理论用量和差异（差异=0），数据结构正确。

V1.5 策略（下一迭代）：增加"差异调整"页面，完工后由仓库/成本会计事后调整实际用量。

V2 策略（后续）：完工时前端输入每个组件实际消耗量（适合车间已养成记录习惯的场景）。

```
V1.5 接口签名变更:
BackflushService.execute() 增加 actual_quantities 参数（Optional，默认 None）
- None: actual_qty = theoretical_qty（V1 兼容）
- Some(quantities): 使用实际用量，计算差异
差异率超阈值 → CostEntry(损耗成本)
```

### 3.6 物料消耗策略：领料 vs 倒冲（P1 — P11）

**核心问题**: 领料单和倒冲并存但不互斥，可能双重扣减。

**V1 方案：产品级消耗模式（推荐上线策略）**

在产品主数据上增加 `material_consumption_mode` 字段，决定该产品的所有组件走领料还是倒冲：

```
products.meta 增加:
material_consumption_mode: "backflush" | "picking"

- "backflush"（默认）: 全部组件走倒冲，不生成领料单，完工时按 BOM 自动倒扣
- "picking": 全部组件走领料单，release 时生成领料单明细行，手动领料出库

V1 行为：
- 默认 backflush → 与当前系统行为一致，上线零切换成本
- 计划员/产品经理可按产品切换为 picking
- 不需要回填任何 BOM 数据
```

**V2 方案（后续迭代）：BOM 节点级标记**

```
BomNode 增加 is_backflush: bool 字段（默认 false）

- false（领料件）: release 时进领料单
- true（倒冲件）: 跳过领料单，完工时倒扣

适用场景：同一产品内关键件走领料、通用件走倒冲
前提：基础流程已跑通、数据已齐全
数据准备：需逐 BOM 逐组件标注，成本较高
```

**各环节行为汇总（V1 产品级策略）**:

| 环节 | backflush 模式 | picking 模式 |
|------|---------------|-------------|
| release() 预留 | 不预留 | HARD 预留所有组件 |
| release() 领料单 | 不生成 | 生成全部组件明细行 |
| 领料发料 | 跳过 | 手动领料出库 |
| 完工倒冲 | 按 BOM 自动倒扣全部组件 | 不倒冲 |
| 差异检测 | 倒冲实际量 vs 理论量 | 领料量 vs 需求量 |

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
8. 回滚关联 PlanItem 状态: Released → Planned（如果有 plan_item_id 关联）
9. 如果该 PlanItem 下所有工单均已反下达 → 检查是否需要回滚 Plan 状态
10. 审计日志
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

| 改动 | 说明 | 阶段 |
|------|------|------|
| `products.meta` 增加 `material_consumption_mode` | "backflush"(默认) 或 "picking"。存储在 JSONB meta 字段中，无需 ALTER TABLE | 阶段 2 |
| `products.meta` 增加 `over_completion_tolerance` | 超额容差百分比，默认 5%。JSONB 字段 | 阶段 3 |

**无需 schema 变更**（现有表结构已满足全部需求）：

| 表/字段 | 状态 | 说明 |
|---------|------|------|
| `work_orders.routing_id` | ✅ 已存在 | |
| `work_orders.bom_snapshot_id` | ✅ 已存在 | 修复代码填入真实值 |
| `work_orders.work_center_id` | ✅ 已存在 | |
| `work_orders.sales_order_id` | ✅ 已存在 | 修复代码传入 |
| `production_plan_items.routing_id` / `work_center_id` / `status` | ✅ 已存在 | |
| `bom_snapshots` 表 | ✅ 已存在 | `abt-core/src/master_data/bom/model.rs` |
| `material_requisitions` 表 | ✅ 已存在 | `002_create_wms.sql:201` |
| `material_requisition_items` 表 | ✅ 已存在 | `002_create_wms.sql:223`，含 `product_id, requested_qty, issued_qty, variance_qty, bin_id` |
| `backflush_records` + `backflush_items` 表 | ✅ 已存在 | |

### 3.18 前端页面改动

| 页面 | 改动 | 阶段 |
|------|------|------|
| 产品主数据 | 显示 Routing 关联状态 + `material_consumption_mode` 切换 | 阶段 2 |
| 需求池 | 排程参数输入 + "生成计划并下达"按钮 | 阶段 4 |
| 生产计划详情 | "确认并下达"按钮 + 预校验 warnings 展示 | 阶段 3 |
| 工单列表 | 无改动（工单直接 Released 状态出现） | — |
| 完工入库 | 无改动（V1 倒冲仍用理论量） | — |
| 倒冲差异调整 | V1.5 新增页面，完工后事后调整实际用量 | V1.5 |

## 4. 接口变更

### 4.1 WorkOrderService.release() — 内部逻辑变更（阶段 1 + 阶段 2）

**阶段 1**（签名不变，实现变更）：
- 新增: BOM 快照（§3.2）
- 新增: 从 Routing 读工序 / 虚拟默认工序（§3.1）
- 删除: 旧的成品预留代码（backflush 模式不预留）
- `routing_id` + `bom_snapshot_id` 写入 WorkOrder

**阶段 2**（扩展实现，签名不变）：
- 新增: 根据 material_consumption_mode 分流（§3.6）
  - picking 模式: HARD 预留组件 + 生成领料单明细行
  - backflush 模式: 不预留、不生成领料单

### 4.2 WorkOrderService.unrelease() — 新增（阶段 2）

```rust
async fn unrelease(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, expected_version: i32) -> Result<()>;
```

### 4.3 BackflushService.execute() — 仓库修正（阶段 1）

**阶段 1**: 修正仓库来源（4 级策略），保持 actual = theoretical
**V1.5**（后续）: 增加 actual_quantities 可选参数

### 4.4 ProductionPlanService.release_to_work_orders() — 行为变更（阶段 3）

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

### 4.5 MaterialRequisitionService.create_for_work_order() — 行为变更（阶段 2）

- 仅 picking 模式：从 BOM 快照展开全部组件，自动生成明细行
- backflush 模式或无 BOM：不生成领料单

### 4.6 MesDemandService.create_plan_from_demands() — 增强（阶段 3）

- 自动查找关联 Routing
- 排程参数自动填充

## 5. release() 完整操作序列

### 5.1 阶段 1（backflush-only 简化路径）

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
  6. 不预留、不创建领料单（backflush 默认模式）
  7. 审计日志

  完工时倒冲：从 BOM 快照展开 → 4 级仓库策略确定仓库 → 扣减原材料
```

### 5.2 阶段 2 完整路径（picking/backflush 分流）

```
WorkOrderService.release(ctx, db, id, expected_version):

  步骤 1-5 同上
  6. 根据产品 material_consumption_mode 分流：
     - "picking":
       a. 从 BOM 快照展开叶子节点 → HARD 预留每个组件
       b. 创建领料单 + 明细行（全部组件）
     - "backflush"（默认）:
       a. 不预留
       b. 不创建领料单
     - 无 BOM 快照: 跳过预留和领料单
  7. 发布事件 + 审计日志
```

## 6. 向后兼容性

| 变更 | 阶段 | 兼容处理 |
|------|------|---------|
| release() 工序来源改为 Routing | 1 | 已有工单不受影响 |
| release() 新增 BOM 快照 | 1 | 旧工单 None 保持不变 |
| release() 删除旧的成品预留代码 | 1 | 旧预留已释放或随工单关闭而清理；backflush 模式不再做预留 |
| 倒冲仓库修正 | 1 | 旧倒冲记录无法追溯修正，仅影响新倒冲 |
| 销售订单追溯修复 | 1 | 纯增量，旧工单保持 None |
| release() 预留按产品策略分流 | 2 | 默认 backflush（不预留），与阶段 1 行为一致 |
| 领料单增加明细行 | 2 | 仅 picking 模式产品生成，backflush 产品无领料单 |
| 产品 material_consumption_mode | 2 | 新字段默认 "backflush"，旧行为不变 |
| unrelease() | 2 | 纯新增方法 |
| BatchSplitStrategy | 3 | 纯新增参数，默认 Single |
| PlanItem 状态更新 | 3 | 现有 Planned 行不受影响 |

## 7. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| 部分产品无 Routing | 高 | 虚拟默认工序 + 前端 warning |
| 部分产品无已发布 BOM | 中 | 跳过快照/预留/领料单 |
| 批量 release 部分失败 | 中 | 单工单粒度隔离 + 可重试 |
| 物料预检增加 release 耗时 | 低 | 简单库存查询，毫秒级 |
| 在途工单新旧逻辑冲突 | 中 | 冻结切换策略（§10） |
| 阶段 1 只修 backflush 路径，旧预留逻辑残留 | 低 | 阶段 1 release() 删除旧的成品预留代码；旧工单预留已释放无需处理 |
| picking 模式上线后 backflush 路径回归 | 低 | 阶段 2 增加集成测试覆盖两种模式 |

## 8. 实施顺序

### 阶段 1：止血（P0 核心 — 让数据变正确）

**目标**：修复 3 个影响生产数据的根本错误（P2 + P4 + P8），顺带修复 P6。
**交付物**：单个工单 release → 报工 → 完工入库 → 倒冲，全链路数据正确。

| # | 改动 | 文件 | 修复问题 |
|---|------|------|---------|
| 1 | 修复工序来源 + 虚拟默认工序 | `work_order/implt.rs` `release()` | P2 |
| 2 | 新增 BOM 快照 | `work_order/implt.rs` `release()` | P4 |
| 3 | 修正倒冲仓库（4 级策略） | `backflush/implt.rs` `execute()` | P8 |
| 4 | 修复销售订单追溯 | `production_plan/implt.rs` | P6（顺带修复，成本极低） |

**端到端验收**（§11 场景 14）：
```
创建工单 → release（验证：工序来自 Routing + BOM 快照已创建）
  → 报工 → 完工入库 → 检查：
    - 倒冲记录：原材料从正确仓库扣减（非 warehouse_id=0）
    - 工单追溯：sales_order_id 非空
    - 工序数据：process_name 来自 LaborProcessDict 而非物料编码
```

**阶段 1 的 release() 简化路径**：
```
release() 在阶段 1 只走 backflush 路径（当前默认行为）：
  1-5. 不变（状态校验、BOM快照、工序、批次）
  6. 不预留（backflush 默认，与当前行为一致）
  7. 不创建领料单（backflush 模式不需要）
  8. 倒冲时从正确仓库扣减（P8 修复点）
```

> 阶段 1 完成后即可上线。后续阶段互相独立，可按需排期。

### 阶段 2：安全网 + picking 模式（P0-Conditional + P1）

**目标**：反下达能力（安全网）+ 引入 material_consumption_mode（解锁 P5/P7）。
**交付物**：unrelease 可用 + picking 模式产品可正确预留/领料。
**前置**：阶段 1 已上线并稳定。

| # | 改动 | 文件 | 修复问题 |
|---|------|------|---------|
| 5 | 实现 unrelease() | `work_order/service.rs` + `implt.rs` | 安全网 |
| 6 | products.meta 加 material_consumption_mode | `product/model.rs` + 产品详情页 | P11 |
| 7 | 修正库存预留（picking 模式 HARD 预留组件） | `work_order/implt.rs` `release()` | P5 |
| 8 | 领料单明细行生成（picking 模式） | `material_requisition/implt.rs` | P7 |

**验收**：
- unrelease 未开工工单 → 工单回 Draft，预留释放
- 切换产品为 picking 模式 → release 时 HARD 预留组件 + 领料单有明细行
- 切换产品为 backflush → 行为与阶段 1 一致（不预留、不生成领料单）

### 阶段 3：一键贯通 + 业务规则（P1 + P2）

**目标**：需求池到工单一键贯通 + 物料预检 + 超额容差。
**交付物**：计划员 2 次操作即可让车间开工。
**前置**：阶段 2 已完成。

| # | 改动 | 文件 | 修复问题 |
|---|------|------|---------|
| 9 | release_to_work_orders 一键到底 | `production_plan/implt.rs` | P3 |
| 10 | 预校验 + 失败隔离 + PlanItem 状态 | `production_plan/implt.rs` | P3 |
| 11 | 需求池自动填充 Routing | `demand_handler/implt.rs` | — |
| 12 | 物料可用性预检（warnings） | `production_plan/implt.rs` `pre_validate()` | P10 |
| 13 | 超额生产容差 | `production_batch/implt.rs` `confirm_routing_step()` | P12 |
| 14 | 事件发布（release/unrelease） | 各 service implt.rs | — |

**验收**：
- 需求池选 3 条需求 → 一键下达 → 3 个 Released 工单
- 物料不足 → warning 显示但不下达
- 部分失败 → 成功工单不受影响，失败行可重试
- 超额报工超 5% → 拒绝

### 阶段 4：前端 + 排程 + 文档（P2 + 渐进增强）

**目标**：完成前端页面改造和排程 V1。
**交付物**：需求池页面可排程下达 + UML 文档同步。
**前置**：阶段 3 已完成。

| # | 改动 | 文件/页面 | 修复问题 |
|---|------|----------|---------|
| 15 | 需求池：排程参数 + "生成计划并下达"按钮 | 前端 | P1 |
| 16 | 计划详情："确认并下达"按钮 + warnings 展示 | 前端 | P3 |
| 17 | 产品详情：Routing 关联 + material_consumption_mode | 前端 | — |
| 18 | 排程 V1（交期倒推 + 优先级 + 工作中心分组） | `production_plan/implt.rs` | P1 |
| 19 | 更新 UML 设计文档 | `docs/uml-design/04-mes.html` | — |

**验收**：完整的计划员操作流程
```
需求池选需求 → 设排程参数 → 生成计划并下达 → 工单 Released → 报工 → 完工入库
```

## 9. 不在本次范围

- 排程 V2（产能日历、物料 ATP 算法、甘特图）
- 自动排产算法（MRP）
- WIP 在制品追踪（P13 — V2 范围，§3.16 记录了设计方向）
- 批次工序报工流程优化
- 生产异常处理流程
- 已有工单的库存预留数据修正（需单独数据迁移脚本）
- 产品主数据增加 `lead_time` 字段（排程 V2）
- 反下达的权限控制（V1 仅做操作层面限制）
- BOM 节点级 `is_backflush` 标记（V2 — 节点级领料/倒冲分流）
- 倒冲实际用量前端输入（V1.5 — 事后差异调整页面）

> **阶段独立性说明**：每个阶段完成后均可独立上线，不依赖后续阶段。
> 阶段 1 是最小可交付单元（修复 3 个数据错误），后续阶段按业务优先级排期。

## 10. 在途工单迁移方案

系统已在线上运行，存在 Draft/Released 状态的在途工单。`release()` 行为变更后需处理新旧逻辑冲突。

**推荐方案：冻结切换**

```
上线流程：
1. 确认上线时间窗口（建议周末或低生产时段）
2. 上线前：通知计划员处理所有在途工单
   - Released 工单：完成或取消
   - Draft 工单：手动 release 或删除
3. 上线部署
4. 上线后验证：用 1-2 个测试产品走完整 release → 倒冲流程

不推荐双轨并行：
- 代码复杂度翻倍（每个 release 路径都需判断版本）
- 测试矩阵翻倍
- 后续清理成本更高
```

**数据修正**（上线后执行）:

上线后需清理已有工单的错误库存预留记录（预留了成品而非原材料）。
具体 SQL 需根据上线时 `inventory_reservations` 表的实际数据情况编写，
原则如下：
- 仅清理 source_type = 'WorkOrder' 且预留产品 = 工单成品（而非 BOM 组件）的记录
- 已关闭/取消工单的预留已释放，无需处理
- 执行前备份相关记录
- 在事务中执行，验证无误后提交

**回滚预案**:

如果上线后发现严重问题，直接回滚部署即可：
- 阶段 1 的改动仅影响 `release()` 和 `backflush.execute()` 的新调用
- 已执行的 release/倒冲记录数据正确，不受回滚影响
- 回滚后旧逻辑恢复，新生成的工单回到旧行为（BOM 叶子当工序、倒冲仓库=0）
- 关键：回滚只影响**新操作**，已入库的正确数据不会丢失

## 11. 用户验收场景

| # | 角色 | 场景 | 验收标准 |
|---|------|------|---------|
| 1 | 计划员 | 有 Routing 的产品下达工单 | 工单工序来自 Routing，非 BOM 叶子节点 |
| 2 | 计划员 | 无 Routing 的产品下达工单 | 虚拟默认工序"生产"，前端有 warning |
| 3 | 计划员 | 有 BOM 的产品 release | `bom_snapshot_id` 非空，领料单（picking 模式）有明细行 |
| 4 | 计划员 | 无 BOM 的产品 release | 跳过快照/预留/领料单，工单正常 Released |
| 5 | 计划员 | 需求池选 3 条需求，一键下达 | 看到 3 个 Released 工单，物料 warnings 可见 |
| 6 | 计划员 | 部分需求无 BOM 下达 | 跳过预留和领料单，warning 提示 |
| 7 | 计划员 | 反下达未开工的工单 | 工单回 Draft，预留释放，领料单取消 |
| 8 | 计划员 | 反下达已开工的工单 | 拒绝，提示"工单已开工，无法反下达" |
| 9 | 仓管员 | 打开 picking 模式领料单 | 看到明细行：品名、数量、仓库，能据此备料 |
| 10 | 仓管员 | 完工入库 + 倒冲 | 原材料从正确仓库扣减（非 warehouse_id=0） |
| 11 | 车间工人 | 扫流转卡报工 | 工序显示正确，报工后批次状态推进 |
| 12 | 计划员 | 物料不足时下达 | warning 显示：组件名、需要量、可用量、缺口量 |
| 13 | 车间工人 | 最后工序报工超量 | 超过容差（默认 5%）时拒绝，提示修改数量 |
| **14** | **全员** | **端到端完整链路（阶段 1 验收）** | **销售订单确认 → 需求池 → 创建计划 → 下达工单 → 报工 → 完工入库 → 检查：① 工序来自 Routing ② BOM 快照非空 ③ 倒冲从正确仓库扣减 ④ sales_order_id 非空 ⑤ 成品入库到正确仓库** |
| **15** | **全员** | **picking 模式端到端（阶段 2 验收）** | **产品切 picking → 下达 → HARD 预留组件 → 领料单有明细行 → 手动领料出库 → 完工不倒冲 → 检查库存变动正确** |
| **16** | **车间工人** | **上线前已 Released 工单继续报工（阶段 1 上线验收）** | **旧工单 WorkOrderRouting 不受影响（已存在），报工→完工正常；倒冲仓库仍为 0（旧工单无 BOM 快照），需记录为已知限制** |
