# MES 三层状态联动设计

> 日期: 2026-06-13
> 状态: 设计完成，待评审
> 范围: ProductionPlan / WorkOrder / ProductionBatch 三层状态自动传播 + 详情页关联信息可见性
> 前提: 保留三层架构不变（不执行两层合并方案）

## 1. 问题分析

### 1.1 当前三层模型

```
ProductionPlan (生产计划)
  └── PlanItem[] → release_to_work_orders() → WorkOrder (工单)
                        └── release() → ProductionBatch (生产批次) + WorkOrderRouting[]
                                    └── confirm_routing_step() → 报工
                                        └── advance_to_receipt() → 完工入库
```

### 1.2 确认的断连点

| # | 位置 | 现状 | 期望 |
|---|------|------|------|
| 1 | `confirm_routing_step` 首次报工 | Batch: Pending→InProgress，上游不动 | WO→InProduction, PlanItem→InProduction |
| 2 | `ProductionReceipt::confirm` 完工入库 | Batch→Completed，WO 不动 | WO→Closed, PlanItem→Completed |
| 3 | PlanItem 全部完成后 | Plan 永远 InProgress | Plan→Completed |
| 4 | WorkOrder 缺中间态 | Released→Closed，无 InProduction | 新增 `WorkOrderStatus::InProduction` |
| 5 | 工单详情看不到批次进度 | 缺关联信息 section | 显示批次状态、工序进度 |
| 6 | 计划详情看不到工单执行情况 | 缺"下达结果"Tab | 显示工单列表+状态+进度 |

### 1.3 决策

- **架构方向**: 保留三层，修复联动（不执行两层合并方案 `docs/2026-06-13-mes-two-layer-architecture.md`）
- **传播模式**: 自动传播——批次事件直接驱动上游状态变更
- **UI 可见性**: 详情页打通上下游关联信息

## 2. 状态传播机制

### 2.1 方案选择：直接服务调用传播

在已有的 `confirm_routing_step()` 和 `ProductionReceipt::confirm()` 事务内，追加上游状态更新。

- 符合现有代码模式（`work_order/implt.rs` 的 `release()` 已大量跨模块调用）
- 同步、事务性、调试简单
- 不新增事件消费者、不新增协调器

### 2.2 状态机终态图

```
Plan:     Draft → Confirmed → InProgress ──→ Completed
                                   ↑              ↑
PlanItem:     Planned → Released → InProduction → Completed / Cancelled
                                   ↑              ↑
WorkOrder: Draft → Released → InProduction ──→ Closed / Cancelled
                              ↑                ↑
Batch:    Pending → InProgress → PendingReceipt → Completed
                    ↑                                   ↑
              首次报工                          完工入库确认
```

### 2.3 传播点详述

#### 传播点 1：批次首次报工

触发位置：`ProductionBatchServiceImpl::confirm_routing_step()` 末尾

触发条件：`batch.status == BatchStatus::Pending && step_no == 1`（即首次报工，batch 从 Pending → InProgress）

传播动作：
```
1. WorkOrder: Released → InProduction（条件 UPDATE，幂等）
2. PlanItem: Released → InProduction（通过 WorkOrder.plan_item_id 反查）
3. Plan: 不变（release_to_work_orders 时已是 InProgress）
```

数据路径：`batch.work_order_id` → `WorkOrder` → `plan_item_id` → `PlanItem`

#### 传播点 2：完工入库确认

触发位置：`ProductionReceiptServiceImpl::confirm()` 步骤 6 之后

触发条件：`ProductionReceipt::confirm()` 成功完成（Batch → Completed）

传播动作：
```
1. WorkOrder: InProduction → Closed（repo 级条件 UPDATE `WHERE id=$1 AND status=InProduction`，不经过 Service::close() 因为该方法需要 expected_version 参数，而 confirm() 事务内无此值）
2. PlanItem: InProduction → Completed（通过 `receipt.work_order_id` → `work_orders.plan_item_id` 反查，不依赖 batch_id）
3. recalculate_plan_status(plan_id):
     查该 Plan 下所有 PlanItem，
     若全部 Completed/Cancelled → Plan → Completed
```

#### 传播点 3：工单取消

触发位置：`WorkOrderServiceImpl::cancel()`

传播动作：
```
1. PlanItem: → Cancelled（通过 plan_item_id 反查）
2. recalculate_plan_status(plan_id):
     若全部 Completed/Cancelled → Plan → Completed
```

#### 传播点 4：工单反下达

触发位置：`WorkOrderServiceImpl::unrelease()`（已有方法，删除批次和工序后）

传播动作：
```
1. PlanItem: → Planned（回退到下达前状态）
```

### 2.4 recalculate_plan_status 逻辑

```
fn recalculate_plan_status(plan_id):
  items = SELECT status FROM production_plan_items WHERE plan_id = $1
  terminal_count = items.filter(status in [Completed, Cancelled]).count
  if terminal_count == items.count:
    UPDATE production_plans SET status = Completed WHERE id = $1 AND status = InProgress
```

幂等：条件 `WHERE status = InProgress` 保证只推进一次。

### 2.5 多工单场景

当前 `split_work_order()` 存在（可一个工单拆多个批次），但 `release_to_work_orders()` 是每个 PlanItem 创建一个工单。

如果一个 PlanItem 有多个工单（未来扩展），PlanItem 的 Completed 时机改为：
```
所有关联工单均为 Closed/Cancelled 时，PlanItem → Completed
```

当前 1:1 场景下，一个工单完成即 PlanItem 完成（`update_item_status_by_work_order` 直接更新）。未来如需多工单支持，可新增 `recalculate_item_status(item_id)` 方法，逻辑与 `recalculate_plan_status` 相同——检查所有关联工单是否终态，全部完成才推进 PlanItem。本设计不含此方法。

## 3. 后端接口变更

### 3.1 枚举变更

```rust
// abt-core/src/mes/enums.rs
define_mes_enum!(WorkOrderStatus {
    Draft = 1,
    Planned = 2,
    Released = 3,
    InProduction = 6,   // 新增，值=6 不影响存量数据
    Closed = 4,
    Cancelled = 5,
});
```

smallint 存储，无需数据库 migration。

### 3.2 新增方法

| 模块 | 方法签名 | 说明 |
|------|---------|------|
| `WorkOrderService` | `async fn mark_in_production(&self, ctx, db, id: i64) -> Result<()>` | Released → InProduction，条件 UPDATE 幂等 |
| `WorkOrderRepo` | `fn update_status_conditional(db, id: i64, from: WorkOrderStatus, to: WorkOrderStatus) -> Result<()>` | repo 级条件状态更新（不需 version，用于事务内传播） |
| `ProductionPlanRepo` | `fn update_item_status_by_work_order(db, wo_id: i64, status: PlanItemStatus) -> Result<()>` | 通过 `work_orders.plan_item_id` JOIN 反查更新 |
| `ProductionPlanRepo` | `fn find_plan_id_by_work_order(db, wo_id: i64) -> Result<Option<i64>>` | work_order_id → plan_item_id → plan_id 两级 JOIN |
| `ProductionPlanRepo` | `fn recalculate_plan_status(db, plan_id: i64) -> Result<()>` | 检查所有 PlanItem 终态，全部完成→Plan Completed |

### 3.3 修改的方法

| 方法 | 文件 | 修改内容 |
|------|------|---------|
| `confirm_routing_step()` | `production_batch/implt.rs` | 步骤 l 返回前：首次报工时调用 `mark_in_production` + `update_item_status_by_work_order(InProduction)` |
| `ProductionReceipt::confirm()` | `production_receipt/implt.rs` | 步骤 6 后：repo 级 `update_status WHERE status=InProduction`（WO→Closed）+ `update_item_status_by_work_order(Completed)` + `recalculate_plan_status`。传播使用 `receipt.work_order_id`，即使 `batch_id` 为 None 也执行 |
| `WorkOrder::cancel()` | `work_order/implt.rs` | 追加：`update_item_status_by_work_order(Cancelled)` + `recalculate_plan_status` |
| `WorkOrder::unrelease()` | `work_order/implt.rs` | 删除批次后：`update_item_status_by_work_order(Planned)` |

### 3.4 幂等保证

所有状态更新使用条件 UPDATE（`WHERE status = $expected`），重复调用不会出错。`mark_in_production` 和 `recalculate_plan_status` 均为幂等操作。

### 3.5 依赖关系

```
production_batch/implt.rs
  └─ 调用 → WorkOrderService::mark_in_production (新增)
  └─ 调用 → ProductionPlanRepo::update_item_status_by_work_order (新增)

production_receipt/implt.rs
  └─ 调用 → WorkOrderService::close (已有)
  └─ 调用 → ProductionPlanRepo::update_item_status_by_work_order (新增)
  └─ 调用 → ProductionPlanRepo::recalculate_plan_status (新增)

work_order/implt.rs
  └─ 调用 → ProductionPlanRepo::update_item_status_by_work_order (新增)
  └─ 调用 → ProductionPlanRepo::recalculate_plan_status (新增)
```

无循环依赖：Repo 层调用是单向的（batch/receipt → work_order/plan repo），不经过 Service trait 回调。

## 4. 详情页关联信息

### 4.1 计划详情页 (`mes_plan_detail.rs`)

新增 **"下达结果" Tab**：

数据来源：`WorkOrderService::list_by_plan(plan_id)`（已有）

展示内容：
| 列 | 数据来源 |
|----|---------|
| 工单编号 | `WorkOrder.doc_number` |
| 产品名称 | JOIN products |
| 计划数量 | `WorkOrder.planned_qty` |
| 工单状态 | `WorkOrder.status`（含新增 InProduction） |
| 批次进度 | `completed_steps / total_steps`（WorkOrder 已有聚合字段） |

Tab 切换使用 Surreal.js `me().on('click')` 内联，遵循组件三原则。

### 4.2 工单详情页 (`mes_order_detail.rs`)

新增两个 section：

**来源追溯 section**：
| 字段 | 数据来源 |
|------|---------|
| 计划编号 | `WorkOrder.plan_item_id` → JOIN `production_plan_items.plan_id` → `production_plans.doc_number` |
| 销售订单 | `WorkOrder.sales_order_id` → `sales_orders.doc_number` |
| 客户名称 | JOIN `customers` |

**批次执行状态 section**：
| 字段 | 数据来源 |
|------|---------|
| 批次编号 | `ProductionBatch.batch_no`（`list_by_work_order` 已有） |
| 流转卡号 | `ProductionBatch.card_sn` |
| 批次状态 | `ProductionBatch.status` |
| 当前工序 | `ProductionBatch.current_step` |
| 完成量/报废量 | `ProductionBatch.completed_qty / scrap_qty` |
| 工序进度 | `completed_steps / total_steps`（JOIN `work_order_routings`） |

### 4.3 批次详情页 (`mes_batch_detail.rs`)

当前已显示 `work_order_id`，补全：
- 工单编号 + 链接跳转（`<a href="/admin/mes/orders/{wo_id}">WO-xxx</a>`）
- 计划编号 + 链接跳转（通过 `plan_item_id` → `plan_id`）

### 4.4 页面导航

详情页之间的跳转通过标准 `<a href>` 链接（非 HTMX），使用 TypedPath 定义路由。

## 5. 边界情况

| 场景 | 处理 |
|------|------|
| 一个 PlanItem 对应多个工单 | PlanItem 在所有关联工单 Closed/Cancelled 后才→Completed（`recalculate_item_status`） |
| 工单 unrelease 后重新 release | PlanItem 回退→Planned，重新 release 后→Released，符合状态机 |
| 批次 scrap | 仅标记批次 Cancelled，不自动关闭工单（工单可建新批次重做） |
| `recalculate_plan_status` 并发 | 条件 UPDATE（`WHERE status = InProgress`），无需全局锁 |
| 完工入库 receipt 无 batch_id | 跳过 batch 传播，仍更新 WO→Closed + PlanItem→Completed（通过 work_order_id） |
| 报工后 inspection 触发批次 Suspended | 不影响传播——Suspended 是 InProgress 的子态，WO 保持 InProduction |
| 历史数据修复 | 独立 SQL 脚本：根据已有 batch 状态回填 WO 和 PlanItem 状态 |

## 6. 实施范围

### 6.1 abt-core 变更文件

| 文件 | 变更类型 |
|------|---------|
| `mes/enums.rs` | 新增 `WorkOrderStatus::InProduction = 6` |
| `mes/work_order/service.rs` | 新增 `mark_in_production` 方法 |
| `mes/work_order/implt.rs` | 实现 `mark_in_production`；修改 `cancel`、`unrelease` 追加传播 |
| `mes/production_batch/implt.rs` | 修改 `confirm_routing_step` 追加传播 |
| `mes/production_receipt/implt.rs` | 修改 `confirm` 追加传播 |
| `mes/production_plan/repo.rs` | 新增 `update_item_status_by_work_order`、`recalculate_plan_status`、`find_plan_id_by_item` |
| `mes/production_plan/service.rs` | 新增 `recalculate_plan_status` 方法（暴露给 web 层历史数据修复用） |
| `mes/production_plan/implt.rs` | 实现 `recalculate_plan_status` |

### 6.2 abt-web 变更文件

| 文件 | 变更类型 |
|------|---------|
| `pages/mes_plan_detail.rs` | 新增"下达结果"Tab |
| `pages/mes_order_detail.rs` | 新增来源追溯 + 批次执行状态 section |
| `pages/mes_batch_detail.rs` | 补全工单/计划编号链接 |

### 6.3 数据同步脚本

`scripts/mes-status-backfill.sql`：
- 根据已有 batch 状态回填 WO 状态（有 InProgress batch → WO InProduction；有 Completed batch → WO Closed）
- 根据已有 WO 状态回填 PlanItem 状态
- 根据已有 PlanItem 状态回填 Plan 状态

### 6.4 设计文档同步

更新 `docs/uml-design/04-mes.html`：
- `WorkOrderStatus` 枚举增加 InProduction
- 状态机图增加三层联动标注

## 7. 风险

| 风险 | 缓解 |
|------|------|
| InProduction 值=6 打破 UI 状态映射硬编码 | 搜索所有 `WorkOrderStatus` 的 match/display 代码，确保新变体有分支 |
| 历史数据状态不一致 | 上线前执行 `mes-status-backfill.sql` |
| 传播失败导致状态不一致 | 所有传播在同一事务（PgExecutor）内，事务回滚则全部回滚 |
| unrelease 后 PlanItem 回退但工单仍有残留 | unrelease 已删除批次和工序，回退 PlanItem→Planned 是安全的 |

## 8. 验收标准

1. **状态传播**：批次首次报工后，工单列表中该工单显示"生产中"；完工入库后，工单显示"已关闭"；所有工单完成后，计划显示"已完成"
2. **UI 可见性**：计划详情"下达结果"Tab 显示工单列表+进度；工单详情显示来源计划+批次执行状态
3. **幂等性**：重复调用 `confirm_routing_step`（幂等报工）不会重复传播状态
4. **边界**：批次报废不关闭工单；工单反下达后 PlanItem 回退
5. **编译**：`cargo clippy` 无 warning
