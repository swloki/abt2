# MES 架构简化 — 三层合并为两层

> 日期: 2026-06-13
> 状态: V2 规划（V1 先按现有三层修复 P0，跑通后执行本方案）
> 前置: `2026-06-13-mes-flow-simplification-design.md` v6 的 P0 修复完成后
> 范围: MES 模块架构重构 — 合并 ProductionPlan / WorkOrder / ProductionBatch

## 1. 现状分析

### 当前三层模型

```
ProductionPlan (生产计划)
  └── PlanItem[] → 1:1 创建 → WorkOrder (工单)
                        └── release() 1:1 创建 → ProductionBatch (生产批次)
```

### 三层各自的独有职责

| 层 | 独有职责 | 当前实际行为 |
|---|---------|------------|
| **Plan** | 排程、产能分配、需求分组 | ❌ 空壳 — 只分组需求、改状态，无排程逻辑 |
| **WorkOrder** | BOM快照、Routing、预留、领料 | ✅ release() 做了真正的工作 |
| **Batch** | 执行进度、报工、流转卡 | ✅ 车间交互入口 |

### 三个核心判断

**判断 1：Plan 层是空壳**

Plan 的"业务价值"只是把几个需求打成一个组。分组功能用 `plan_group_id` 字段就能实现，不需要独立的状态机（Draft → Confirmed → Released → Completed）和独立的管理页面。

**判断 2：WorkOrder 和 Batch 永远 1:1**

`release()` 中只创建 1 个批次，`batch_qty = planned_qty`。当前系统没有拆批场景。不拆批时，Batch 就是 WorkOrder 的执行视图，两表大量字段重复（product_id, qty）。

**判断 3：三层对当前规模是过度设计**

适合三层的场景：大型制造企业的计划部/车间调度/班组三层管理、MRP排程、甘特图。
当前实际情况：计划员直接看需求决定做什么、不拆批、排程靠人工经验。

## 2. 目标模型：两层

```
需求池 → ProductionOrder（生产工单）→ 报工 → 完工入库
```

### 2.1 ProductionOrder = WorkOrder + ProductionBatch 合并

```
ProductionOrder（生产工单）
├── id
├── doc_number           编号 (来自 WorkOrder)
├── product_id           产品
├── planned_qty          计划数量
├── completed_qty        完成量 (来自 Batch)
├── scrap_qty            报废量 (来自 Batch)
├── current_step         当前工序 (来自 Batch)
├── card_sn              流转卡号 (来自 Batch)
├── team_id              班组 (来自 Batch)
├── bom_snapshot_id      BOM快照 (来自 WorkOrder)
├── routing_id           工艺路线 (来自 WorkOrder)
├── work_center_id       工作中心
├── sales_order_id       来源订单
├── demand_ids           来源需求 (来自 PlanItem，改为直接关联)
├── plan_group_id        分组标签 (替代独立的 Plan 实体)
├── scheduled_start/end  排程日期
├── actual_start/end     实际开始/结束 (来自 Batch)
├── status               合并状态机
├── version              乐观锁
├── remark
├── operator_id
├── created_at / updated_at / deleted_at
```

### 2.2 合并状态机

```
当前三层状态机（3个独立状态机）:
  Plan:     Draft → Confirmed → InProgress → Completed
  PlanItem: Planned → Released → InProduction → Completed
  WorkOrder: Draft → Released → Closed / Cancelled
  Batch:    Pending → InProgress → Completed → Suspended → Scrapped

目标两层状态机（1个状态机）:
  ProductionOrder:
    Draft          刚创建，未下达
    Released       已下达（BOM快照+工序+流转卡已创建）
    InProduction   首次报工后
    Completed      最后工序报工完成
    Closed         完工入库后
    Suspended      暂停
    Scrapped       报废
    Cancelled      取消
```

### 2.3 Plan 降级为分组标签

不再有独立的 `production_plans` 和 `production_plan_items` 表。

分组功能通过 `plan_group_id` + `plan_group` 表实现：

```
plan_groups (新增，极简):
  id, group_no, group_date, remark, operator_id, created_at

production_orders 中:
  plan_group_id: Option<i64>   -- 可空，表示独立创建的工单
  demand_id: Option<i64>       -- 直接关联需求（替代 PlanItem 中转）
```

**页面变化**：
- 不再有独立的"生产计划列表"和"生产计划详情"页
- 需求池下达后直接在工单列表中看到
- 如果需要"这批工单是一起下的"，工单列表按 `plan_group_id` 筛选

### 2.4 WorkOrderRouting 不变

工序明细仍挂在 ProductionOrder 上：
```
WorkOrderRouting
  work_order_id → 改名 production_order_id（或保持 work_order_id 不改名）
```

数据结构不变，仅外键指向合并后的表。

### 2.5 报工和完工入库不变

- `confirm_routing_step()` — 参数从 `batch_id` 改为 `production_order_id`（因为 1:1）
- `advance_to_receipt()` — 同理
- `WorkReport` — `batch_id` 改为 `production_order_id`

## 3. 数据库变更

### 3.1 删除的表

- `production_plans`
- `production_plan_items`
- `production_batches`

### 3.2 合并后的表

```sql
-- production_orders (从 work_orders 扩展)
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS completed_qty DECIMAL(10,6) DEFAULT 0;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS scrap_qty DECIMAL(10,6) DEFAULT 0;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS current_step INT DEFAULT 0;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS card_sn VARCHAR(50);
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS team_id BIGINT;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS actual_start TIMESTAMPTZ;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS actual_end TIMESTAMPTZ;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS plan_group_id BIGINT;
ALTER TABLE work_orders ADD COLUMN IF NOT EXISTS demand_id BIGINT;

-- plan_groups (新增，替代 production_plans)
CREATE TABLE plan_groups (
  id BIGSERIAL PRIMARY KEY,
  group_no VARCHAR(50) NOT NULL,
  group_date DATE NOT NULL,
  remark TEXT DEFAULT '',
  operator_id BIGINT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 3.3 数据迁移策略

```
1. work_orders 吸收 production_batches 的字段
   UPDATE work_orders SET
     completed_qty = (SELECT completed_qty FROM production_batches WHERE work_order_id = work_orders.id),
     scrap_qty = ...,
     current_step = ...,
     card_sn = ...,
     actual_start = ...,
     actual_end = ...
   ;

2. work_orders 吸收 production_plan_items 的关联
   UPDATE work_orders SET
     plan_group_id = (SELECT plan_id FROM production_plan_items WHERE id = work_orders.plan_item_id),
     demand_id = (SELECT demand_id FROM production_plan_items WHERE id = work_orders.plan_item_id)
   ;

3. 更新 work_reports 的 batch_id → production_order_id
4. 验证数据一致性
5. 删除旧表（production_plans, production_plan_items, production_batches）
```

## 4. 代码变更范围

### 4.1 删除的模块

- `abt-core/src/mes/production_plan/` — 整个目录（Service/implt/repo/model）
- `abt-core/src/mes/production_batch/` — 整个目录（Service/implt/repo/model）
- `abt-core/src/mes/demand_handler/` — 中与 Plan 相关的逻辑

### 4.2 改造的模块

| 模块 | 改动 |
|------|------|
| `mes/work_order/` | 吸收 Batch 的执行状态字段和报工逻辑 |
| `mes/work_report/` | `batch_id` → `production_order_id` |
| `mes/production_receipt/` | 工单完工逻辑调整 |
| `wms/backflush/` | 倒冲关联调整 |
| `wms/material_requisition/` | 领料单关联调整 |
| `mes/enums.rs` | 合并状态枚举 |
| `mes/dashboard/` | 统计查询调整 |

### 4.3 新增模块

- `mes/plan_group/` — 极简分组服务（仅 CRUD，无状态机）

### 4.4 页面变更

| 删除 | 替代 |
|------|------|
| `mes_plan_list.rs` | 工单列表按 `plan_group_id` 筛选 |
| `mes_plan_detail.rs` | 工单详情展示分组信息 |
| `mes_plan_create.rs` | 需求池直接创建工单 |
| `mes_batch_list.rs` | 工单列表即批次列表（1:1 合并） |
| `mes_batch_detail.rs` | 工单详情即批次详情（执行状态在工单上） |

| 保留 | 调整 |
|------|------|
| `mes_demand_pool.rs` | 增加"直接下达工单"操作 |
| `mes_demand_pool_create.rs` | 直接创建 ProductionOrder（不再经过 Plan） |
| `mes_order_list.rs` | 吸收批次的执行状态展示 |
| `mes_order_detail.rs` | 吸收批次的报工/流转卡/进度信息 |
| `mes_card_query.rs` | card_sn 查询从批次改为工单 |

## 5. 实施前提和顺序

### 前提条件

1. **v6 设计文档的所有 P0 修复已完成并上线稳定运行**
2. **至少 1-2 个月的实际生产数据验证**
3. **团队有足够的重构时间窗口**（估计 2-3 周）

### 实施顺序

| 阶段 | 内容 | 周期 |
|------|------|------|
| A. 准备 | 在 `work_orders` 表增加 Batch 字段（ALTER TABLE，不影响现有功能） | 1 天 |
| B. 双写 | release() 同时写入旧 Batch 表和新 work_orders 字段 | 3 天 |
| C. 迁移 | 历史数据迁移脚本（batches → work_orders 字段） | 2 天 |
| D. 切换 | 报工/完工/倒冲改为读 work_orders 字段 | 3 天 |
| E. 清理 | 删除 production_plan / production_batch 模块和页面 | 2 天 |
| F. 验证 | 端到端回归测试 | 2 天 |

## 6. 风险

| 风险 | 缓解 |
|------|------|
| 数据迁移丢数据 | 双写期间保证一致性；迁移脚本在事务中执行，先验证后提交 |
| 删除 Plan 后无法回退 | 上线前备份全库；保留 Plan 相关代码分支 |
| 外部引用（报表/脚本） | 搜索所有引用 `production_plans`/`production_batches` 的代码和脚本 |
| WorkOrder 表字段膨胀 | 合并后约 25 个字段，在合理范围内；如后续需要拆批可再提取子表 |

## 7. 拆批扩展点（未来如果需要）

如果将来需要拆批（1 工单 = N 批次），在两层模型上扩展：

```
方案: production_sub_batches 表（可选，按需创建）
  id, production_order_id, batch_qty, team_id, 
  completed_qty, current_step, card_sn, status
```

- 不拆批时：不创建 sub_batch，执行状态直接在 production_orders 上
- 拆批时：创建 N 个 sub_batch，工单的 completed_qty = sum(sub_batches.completed_qty)

这样避免了现在为不存在的拆批场景预留复杂度。
