---
module: mes
tags: [manufacturing, work-order, production-batch, modeling, erpnext, odoo, ofbiz]
problem_type: design_decision
---

# 生产建模：需求 → 工单(WO) → 批次(Batch) — 「一个工单 + 多批次」，不拆多工单

> **🔒 2026-06-27 扁平化更新（Issue #116）**：本文原描述「PP → WO → Batch 三层」。
> 经三家 ERP（ERPNext/Odoo/OFBiz）对比，**生产订单(PP)层已废弃**——PP 与 WO 1:1 透传冗余，
> 改为**需求(销售订单驱动)→ Draft 工单直达**（`MesDemandService::create_work_orders_from_demands`），
> 对标 Odoo/OFBiz（无独立计划层）/ ERPNext（计划层可选）。本文核心结论「一个需求一个工单、数量走批次」**仍然成立**，
> 仅前置从 PP 改为需求直达；下文涉及 PP / ProductionPlan 的段落已过时，以本节为准。

## 背景

ABT 生产作业中心「创建生产计划」drawer 曾计划加「工单数 N」字段，把一个生产计划按数量均分成 N 个工单。

质疑：**工单已经能拆多个批次（流转卡 ProductionBatch），为什么还要拆多个工单？**

带着这个问题对比了 `E:\work\erp` 下三个开源 ERP 的制造模型。

## 三 ERP 对比

| | 工单层（1 个需求） | 工序执行 | 数量分批 / 追溯 |
|---|---|---|---|
| **ERPNext** | 1 Work Order（可 `combine_items` 合并多 SO） | Job Card（每工序 1 张报工卡） | Batch（按 `batch_size`）+ Serial No |
| **Odoo** | 1 mrp.production（MO） | mrp.workorder（每工序） | stock.lot（批次） |
| **OFBiz** | 1 ProductionRun | ProductionRunTask（每工序） | InventoryItem.lotId |

### 三家共识（一致到惊人）

1. **一个生产需求 → 一个工单**（Work Order / MO / ProductionRun）。需求聚合在**计划层**（PP / Production Plan / MRP），不下沉到多工单。
2. **工单内部分两个正交维度拆分**：
   - **工序维度**：Job Card / workorder / Task —— 每个工序一个执行单元，**报工在这里**
   - **数量维度**：Batch / lot / InventoryItem —— 按数量分批追溯，**入库在这里**
3. **拆成多个工单是例外，不是常态**：只在明确业务隔离时（不同交期、不同仓库、不同项目、分批交货、产能瓶颈）才**显式**拆（Odoo backorder、ERPNext 多次生成 WO）。**没有一家按「用户填个 N 就均分成 N 个工单」**。

## 回答核心问题

**不需要拆多工单。** 按「数量」拆分，三家都落在**批次 / 流转卡**层，不是再开多个工单。「工单数 N 均分」和批次拆批语义完全重叠，只是位置上移一层，徒增实体。

## ABT 现状对照

ABT 的工单（WO）**已经具备**这两条拆分维度：

| 维度 | ABT 实体 | 对应三 ERP |
|---|---|---|
| 工序 | `WorkOrderRouting` | Job Card / workorder / Task |
| 数量分批 | `ProductionBatch`（流转卡） | Batch / lot / InventoryItem |

所以 ABT 的工单拆分能力 = 三 ERP。「工单数 N 拆多工单」是多余的。

## ABT 决策（已落地）

- **一个物料一批生产指令 → 1 个工单**（不按数量拆多工单）
- **按数量分批**投料/入库 → `ProductionBatch`（流转卡），在**工单下达 drawer** 拆批（已有 `addSplitRow`）
- **工序报工** → `WorkOrderRouting`，报工 drawer（已有）
- **真要多个工单** → 只在「不同交期/车间/项目」时手动建多个，不靠 N 均分

### 落地（创建生产计划 drawer）

- 建 PP（`create_plan_from_demands` 按 `product_id` 聚合 → 一个物料一个 plan_item）→ `generate_work_orders` → **1 个 Draft 工单**（不 release）
- 「工单数 N」字段**取消**（不引入）
- 数量分批交给工单下达时的流转卡

## 代码位置

- `abt-core/src/mes/demand_handler/implt.rs::create_plan_from_demands` —— 需求按 `product_id` 聚合到一个 plan_item（不再按 `(product, source)` 多 SO 拆），sales_order_id 关联首个 SO
- `abt-web/src/pages/mes_work_center.rs::create_plan` —— 建 PP + `generate_work_orders`（1 个工单）
- `abt-web/src/pages/mes_work_center.rs::get_release_drawer` —— 下达 drawer 流转卡拆批（`render_split_row` + `addSplitRow`）
- `abt-core/src/mes/production_plan::generate_work_orders` —— 按 `Vec<WorkOrderPlanItem>` 生成工单（核心层**仍支持**多工单：传 N 个 item 生成 N 个；web 层当前传 1 个）
- `abt-core/src/mes/production_batch::split_work_order` —— 工单拆流转卡（数量维度）

## 参考实现

- ERPNext：`E:\work\erp\erpnext\apps\erpnext\manufacturing\` （Work Order / Job Card / Production Plan）
- Odoo：`E:\work\erp\odoo\addons\mrp\` （mrp.production / workorder / lot）
- OFBiz：`E:\work\erp\ofbiz-framework\applications\manufacturing\` （ProductionRun / ProductionRunTask）
