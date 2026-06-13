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

  // 分支 A：全部 Cancelled → Plan = Cancelled
  if items.all(status == Cancelled):
    UPDATE production_plans SET status = Cancelled WHERE id = $1 AND status = InProgress

  // 分支 B：全部终态（Completed/Cancelled）且至少有一个 Completed → Plan = Completed
  elif items.all(status in [Completed, Cancelled]) and items.any(status == Completed):
    UPDATE production_plans SET status = Completed WHERE id = $1 AND status = InProgress
```

幂等：条件 `WHERE status = InProgress` 保证只推进一次。

**SQL 实现注意**：`production_plan_items.status` 列是 PlanItemStatus 类型，`production_plans.status` 是 PlanStatus 类型。NOT IN 子句必须用 PlanItemStatus 值（Completed=4, Cancelled=5），不能用 PlanStatus::Completed 复用——两者值碰巧相同但语义不同。

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

**关键修正**：`update_item_status_by_work_order` 的 SQL **必须**增加前向状态守卫 `AND ppi.status IN (2, 3)`（Released, InProduction），防止将已终态（Cancelled=5）的 PlanItem 回退为 Completed。WorkOrder 的 `update_status_conditional` 已有 `WHERE status = $from` 条件，天然幂等。

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

### 4.1 计划详情页 (`mes_plan_detail.rs`)

**"下达结果" Tab 已存在**——`mes_plan_detail.rs:337` 已注册 Tab、`:342` 已渲染面板、`:481-527` 已有 `tab_result()` 函数。Tab 切换使用现有 `detail_tabs()` + `tab_panel()` 机制。

唯一改进：`tab_result()` 的 `:511-512` 当前仅显示 `total_steps`，补充 `completed_steps` 进度显示：

```rust
@if let (Some(done), Some(total)) = (wo.completed_steps, wo.total_steps) {
    span { "工序: " (done) "/" (total) "步" }
}
```

**禁止引入 Surreal.js `me().on('click')`**——AGENTS.md 明令禁止，且会与现有 Tab 系统冲突。


### 4.2 工单详情页 (`mes_order_detail.rs`)

新增两个 section（纯模板渲染，**无需新增查询**）：
- `order.source_plan_doc` / `order.source_plan_id` / `order.source_so_doc` / `order.source_customer` 已由 `WorkOrderRepo::get_by_id` SQL JOIN 填充（repo.rs:64-65）
- `batches`（handler line 123）和 `routings`（handler line 117）已在 handler 中加载

**来源追溯 section**（直接读 `order` 字段）：
| 字段 | 数据来源 |
|------|---------|
| 计划编号 | `order.source_plan_doc` + 链接到 `/admin/mes/plans/{source_plan_id}` |
| 销售订单 | `order.source_so_doc` |
| 客户名称 | `order.source_customer` |

**批次执行状态 section**（直接读已加载的 `batches` 数组）：
| 字段 | 数据来源 |
|------|---------|
| 批次编号 | `ProductionBatch.batch_no`（`list_by_work_order` 已有） |
| 流转卡号 | `ProductionBatch.card_sn` |
| 批次状态 | `ProductionBatch.status` |
| 当前工序 | `ProductionBatch.current_step` |
| 完成量/报废量 | `ProductionBatch.completed_qty / scrap_qty` |
| 工序进度 | `order.completed_steps / order.total_steps`（`get_by_id` 已填充） |

**禁止**：abt-web 中使用 `sqlx::query_as` 直接查询（AGENTS.md 数据访问禁令）。

### 4.3 批次详情页 (`mes_batch_detail.rs`)

**工单编号链接已存在**——`mes_batch_detail.rs:196` 已有 `a href="/admin/mes/orders/{wo.id}"`。`wo` 已在 handler line 40 加载。

唯一补充：计划编号链接（`wo.source_plan_doc` + `wo.source_plan_id` 已由 `get_by_id` 填充）：

```rust
@if let (Some(pid), Some(pdoc)) = (wo.source_plan_id, &wo.source_plan_doc) {
    div class="detail-info-item" {
        span class="detail-info-label" { "计划" }
        span class="detail-info-value" {
            a href=(format!("/admin/mes/plans/{}", pid)) class="link-cell" { (pdoc) }
        }
    }
}
```

### 4.4 页面导航

详情页之间的跳转通过标准 `<a href>` 链接（非 HTMX），使用 TypedPath 定义路由。

## 5. 边界情况

| 场景 | 处理 |
|------|------|
| 一个 PlanItem 对应多个工单 | PlanItem 在所有关联工单 Closed/Cancelled 后才→Completed（`recalculate_item_status`） |
| **一个工单有多个批次**（split_work_order） | `confirm()` 传播 WO→Closed 前必须检查 `list_by_work_order` 所有批次是否终态。有活跃批次则不关闭 WO，仅更新已完工批次状态 |
| 工单 unrelease 后重新 release | PlanItem 回退→Planned（条件 `status IN (Released, InProduction)`），重新 release 后→Released |
| 批次 scrap | 仅标记批次 Cancelled，不自动关闭工单（工单可建新批次重做） |
| `recalculate_plan_status` 并发 | 条件 UPDATE（`WHERE status = InProgress`），无需全局锁 |
| 完工入库 receipt 无 batch_id | 跳过 batch 传播，仍更新 WO→Closed + PlanItem→Completed（通过 work_order_id） |
| 报工后 inspection 触发批次 Suspended | 不影响传播——Suspended 是 InProgress 的子态，WO 保持 InProduction |
| **事务原子性** | web handler 必须用 `pool.begin()` 包裹 `confirm_routing_step` / `confirm` 调用，传播用 `?` 传播错误。裸 `&mut conn` 是 autocommit，不构成事务 |
| **全部 PlanItem 被 Cancelled** | Plan 应标记 Cancelled 而非 Completed（`recalculate_plan_status` 分支 A） |
| 历史数据修复 | 独立 SQL 脚本：根据已有 batch 状态回填 WO 和 PlanItem 状态 |

## 6. 实施范围

### 6.1 abt-core 变更文件

| 文件 | 变更类型 |
|------|---------|
| `mes/enums.rs` | 新增 `WorkOrderStatus::InProduction = 6` |
| `mes/dashboard/repo.rs` | 修改：`status IN (2,3)` → `IN (2,3,6)`（评审 P0） |
| `mes/work_order/service.rs` | 新增 `mark_in_production` 方法 |
| `mes/work_order/implt.rs` | 实现 `mark_in_production`；修改 `cancel`、`unrelease` 追加传播 + 回退条件扩展 |
| `mes/production_batch/implt.rs` | 修改 `confirm_routing_step` 追加传播 |
| `mes/production_receipt/implt.rs` | 修改 `confirm` 追加传播（含多批次守卫） |
| `mes/production_plan/repo.rs` | 新增 `update_item_status_by_work_order`（含状态守卫）、`recalculate_plan_status`（含 Cancelled 分支）、`find_plan_id_by_work_order` |
| `mes/production_plan/service.rs` | 新增 `recalculate_plan_status` 方法（暴露给 web 层历史数据修复用） |
| `mes/production_plan/implt.rs` | 实现 `recalculate_plan_status` |

### 6.2 abt-web 变更文件

| 文件 | 变更类型 |
|------|---------|
| `pages/mes_plan_detail.rs` | 修改：`wo_status_label` 增 InProduction 臂 + `tab_result` 补 `completed_steps`（Tab 已存在） |
| `pages/mes_order_detail.rs` | 修改：`wo_status_label` + 取消按钮条件 + 新增来源追溯/批次状态 section（数据已加载） |
| `pages/mes_order_list.rs` | 修改：`wo_status_label` + `parse_wo_status` 增 InProduction（评审 P0） |
| `pages/mes_batch_detail.rs` | 修改：补设计划编号链接（工单链接已存在） |
| `pages/mes_report_create.rs` | 修改：报工 handler 包显式事务（评审 P0） |
| `pages/mes_receipt_detail.rs` | 修改：入库确认 handler 包显式事务（评审 P0） |

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
| InProduction 值=6 打破 UI 状态映射硬编码 | 搜索所有 `WorkOrderStatus` 的 exhaustive match，确保新变体有分支。已确认 3 处 `wo_status_label()` 无 `_ =>` 通配臂，必须同步更新：`mes_order_detail.rs:27`、`mes_order_list.rs:32`、`mes_plan_detail.rs:57` |
| Dashboard 硬编码 `status IN (2,3)` 漏掉 InProduction | `dashboard/repo.rs:53` 改为 `IN (2,3,6)` |
| 历史数据状态不一致 | 上线前执行 `mes-status-backfill.sql` |
| 传播失败导致状态不一致 | **必须**在 web handler 中用显式事务包裹 `confirm_routing_step` 和 `confirm` 调用（`pool.begin()` / `tx.commit()`），传播代码用 `?` 传播错误。裸 `&mut conn` 是 autocommit，不构成事务——设计原先"同一事务回滚"的承诺在当前代码中无法兑现，已修正 |
| 多批次 WO 被提前关闭 | `confirm()` 传播 WO→Closed 前必须检查该 WO 下所有批次是否终态（Completed/Cancelled），否则不关闭 |
| unrelease 后 PlanItem 回退但工单仍有残留 | unrelease 已删除批次和工序，回退 PlanItem→Planned 是安全的。但回退条件需同时接受 Released 和 InProduction（`status IN (2,3)`） |
| `update_item_status_by_work_order` 状态回退 | SQL 必须增加 `AND ppi.status IN (2,3)` 前向守卫，防止 Cancelled→Completed 回退 |

## 8. 验收标准

1. **状态传播**：批次首次报工后，工单列表中该工单显示"生产中"；完工入库后，工单显示"已关闭"；所有工单完成后，计划显示"已完成"
2. **UI 可见性**：计划详情"下达结果"Tab 显示工单列表+进度（**Tab 已存在**，仅需补 `completed_steps` 显示）；工单详情显示来源计划+批次执行状态（数据已通过 `get_by_id` JOIN 加载，无需新查询）
3. **幂等性**：重复调用 `confirm_routing_step`（幂等报工）不会重复传播状态
4. **边界**：批次报废不关闭工单；工单反下达后 PlanItem 回退；多批次工单仅在全终态后关闭
5. **编译**：`cargo clippy` 无 warning

## 9. 评审修订记录（2026-06-13）

> 以下为 feature-review 六角色评审后的修订项，与原文冲突处以本节为准。

### P0 修订（不改会出 Bug / 编译失败）

1. **枚举爆破半径**：新增 InProduction=6 会导致 `mes_order_detail.rs:27`、`mes_order_list.rs:32`、`mes_plan_detail.rs:57` 的 `wo_status_label()` exhaustive match 编译失败。必须在加枚举变体的同一 commit 中更新这 3 处。
2. **Dashboard 遗漏**：`dashboard/repo.rs:53` 的 `status IN (2,3)` 必须改为 `IN (2,3,6)`。
3. **事务策略修正**：web handler 传 `&mut conn`（裸连接 autocommit），不构成事务。必须在 `mes_report_create.rs:101` 和 `mes_receipt_detail.rs:71` 中用 `pool.begin()` 包裹，传播代码用 `?` 而非 `tracing::warn!`。
4. **`recalculate_plan_status` 枚举类型混淆**：原设计用 `PlanStatus::Completed` 绑定到 `production_plan_items.status` 列（PlanItemStatus 类型），靠巧合（值同为 4）不出错。必须用独立 bind 参数区分。
5. **`update_item_status_by_work_order` 缺状态守卫**：SQL 无 `AND ppi.status IN (...)` 条件，可将 Cancelled 回退为 Completed。必须增加前向守卫。
6. **计划详情 Tab 已存在**：`mes_plan_detail.rs` 已有完整的 `tab_result()` 函数和 Tab 注册。Task 9 大幅缩减——仅补 `completed_steps` 显示，**禁止引入 Surreal.js `me().on('click')`**（AGENTS.md 明令禁止）。
7. **工单详情数据已加载**：`order.source_plan_doc` 等字段已由 `get_by_id` SQL JOIN 填充。Task 10 纯模板渲染，**禁止 abt-web 直接 SQL**。

### P1 修订（正确性/健壮性）

8. **多批次 WO 提前关闭**：`confirm()` 传播 WO→Closed 前需检查 `list_by_work_order` 所有批次终态。
9. **审计日志误触发**：Task 6 审计 `else` 分支在 `Ok(false)` 时也触发。改为 `Ok(true)` 才记录。
10. **unrelease PlanItem 回退**：`implt.rs:414-419` 回退条件需同时接受 `Released(2)` 和 `InProduction(3)`。
11. **recalculate_plan_status 全 Cancelled 分支**：所有 PlanItem 都 Cancelled 时 Plan 应标 Cancelled 而非 Completed。
12. **UI 取消按钮**：`mes_order_detail.rs:343` 的 cancel 按钮条件需增加 InProduction。
13. **领域事件**：`mark_in_production` 和自动关闭应发布领域事件。
