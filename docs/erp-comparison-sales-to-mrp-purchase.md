# 三家 ERP 销售→生产/采购 流转机制对比分析

> 分析对象：ERPNext、Odoo、OFBiz  
> 分析目的：为 ABT 项目的销售订单→生产/采购流转设计选择参考方案  
> 日期：2026-06-15

## 1. 业务场景

销售订单确认后，系统需根据产品属性和库存情况智能分流：

```
SO Confirm
  │
  ├─ 外购物料
  │   ├─ 仓库有货 → 直接发货
  │   └─ 仓库无货 → 流转采购需求池
  │
  ├─ 自制物料
  │   ├─ 转生产 → 生产计划/工单
  │   └─ 转采购 → BOM 展开的全部原材料采购需求  ← 当前缺失
  │
  └─ 委外物料（预留）
      └─ 委外订单
```

**ABT 当前状态**：外购链路完整，自制链路只到生产计划，**BOM 原材料级联采购是缺口**。

---

## 2. 三家 ERP 架构总览

| 维度 | ERPNext | Odoo | OFBiz |
|---|---|---|---|
| **范式** | 人工触发 + Production Plan 做 MRP | 实时 pull-based 规则引擎 | 定时批量 MRP 批处理 |
| **触发时机** | 用户点按钮 | SO confirm 同步触发 | 定时调度（timeout 7200s） |
| **核心抽象** | Production Plan | StockRule + Procurement | MrpEvent + Requirement |
| **BOM 展开** | PP 内手动 "Get Raw Materials" | 递归级联（自动） | BOM level 逐层展开 |
| **技术栈** | Python / Frappe Framework | Python / OWL Framework | Java / MiniLang |

---

## 3. 各家处理链路详解

### 3.1 ERPNext — 人工触发型

**核心理念**：SO 确认后不自动创建任何下游单据，用户通过页面 Create 按钮手动触发。

```
SO on_submit()
  │  ❌ 不自动创建任何下游单据
  │  只做信用检查 + 库存预留 + 状态变更
  │
  ├─ Create > Delivery Note       ← 仓库有货，手动发货
  ├─ Create > Work Order          ← 手动建工单（不展开原材料）
  ├─ Create > Production Plan     ← 手动建 PP，PP 内做 MRP
  │     ├─ Get Raw Materials      ← 手动 BOM 爆炸 + 库存扣减
  │     ├─ Create Work Order      ← 手动从 PP 生成工单
  │     └─ Create Material Request ← 手动从 PP 生成采购申请
  ├─ Create > Material Request    ← 直接建采购申请
  └─ Create > Request for Raw Materials ← 直接 BOM 爆炸→采购申请
```

**关键源码**：

| 功能 | 源码位置 |
|---|---|
| SO Create 按钮组 | `selling/doctype/sales_order/sales_order.js` `refresh()` |
| SO → Work Order | `sales_order.py` `get_work_order_items()` |
| SO → Production Plan | `mapper.py` `make_production_plan()` |
| PP BOM 爆炸 | `production_plan/services/bom_explosion.py` `get_exploded_items()` |
| PP 原材料获取 | `production_plan/services/material_request.py` `get_items_for_material_requests()` |
| PP → 工单 | `production_plan/services/work_order_planning.py` `make_work_order()` |
| PP → 采购申请 | `production_plan/services/material_request.py` `MaterialRequestService.make_material_request()` |

**库存可用量计算**：`bin.projected_qty = actual_qty + ordered_qty + planned_qty − reserved_qty − reserved_qty_for_production`

**特点**：控制力最强，用户每步都有决策权。但效率低，依赖人工操作。

---

### 3.2 Odoo — 实时规则引擎型

**核心理念**：SO 确认同步触发 Procurement，StockRule 按产品配置的 route 自动分发到 pull/buy/manufacture，递归处理 BOM 全链路。

```
SO action_confirm()
  │
  ├─ _action_confirm() [sale_stock override]
  │   └─ order_line._action_launch_stock_rule()
  │       ├─ 构建 Procurement 列表（每行一个需求）
  │       └─ StockRule.run(procurements)           ← 核心分发
  │           │
  │           ├─ _get_rule(product, location)       ← 查规则
  │           │   按优先级搜索: procurement group → 产品 route → 产品分类 → 仓库 route
  │           │
  │           ├─ action="pull" → _run_pull()        ← 库存调拨（有货直接移）
  │           │   └─ MTS/MTO 逻辑：mts_else_mto 先查库存，不够再下单
  │           │
  │           ├─ action="buy" → _run_buy()          ← 创建采购订单
  │           │   ├─ 按 supplier 分组合并（_make_po_get_domain）
  │           │   ├─ 查现有未确认 PO 追加行
  │           │   └─ 创建新 PO + PO 行
  │           │
  │           └─ action="manufacture" → _run_manufacture()  ← 创建制造订单
  │               ├─ 查 BOM (_get_matching_bom)
  │               ├─ 创建 MrpProduction
  │               ├─ mo.action_confirm()
  │               │   └─ 展开原材料 → 生成 stock.move
  │               │       └─ 每个原材料 move 又触发 StockRule.run()  ← 递归级联！
  │               │           ├─ 原材料 pull → 调拨
  │               │           ├─ 原材料 buy  → 采购
  │               │           └─ 原材料 manufacture → 子制造订单（再递归）
  │               └─ 已有同类 MO → 自动追加数量（不重复创建）
```

**关键源码**：

| 功能 | 源码位置 |
|---|---|
| SO 确认入口 | `sale_stock/models/sale_order_line.py:375` `_action_launch_stock_rule()` |
| 核心规则分发 | `stock/models/stock_rule.py:451` `StockRule.run()` |
| 规则查找 | `stock/models/stock_rule.py:537` `_search_rule()` |
| 库存调拨 | `stock/models/stock_rule.py:288` `_run_pull()` |
| 创建采购订单 | `purchase_stock/models/stock_rule.py:59` `_run_buy()` |
| 创建制造订单 | `mrp/models/stock_rule.py:81` `_run_manufacture()` |

**特点**：一次 confirm 全链路自动展开到底。route 可按产品/产品分类/仓库配置不同 action。递归 BOM 级联完全自动化。

---

### 3.3 OFBiz — 定时批量 MRP 型

**核心理念**：全局定时调度跑 MRP，把 SO + PO + MO + 预测 + 安全库存全量拉进 MrpEvent 表统一计算。

```
executeMrp (定时调度, transaction-timeout=7200s)
  │
  ├─ Step 1: initMrpEvents()                       ← 收集所有供需事件
  │   ├─ SALES_ORDER_SHIP  (负数=需求)  ← 已审批销售订单行
  │   ├─ PROD_REQ_RECP     (正数=供应)  ← 已审批请购单
  │   ├─ PUR_ORDER_RECP    (正数=供应)  ← 已审批采购订单
  │   ├─ MANUF_ORDER_REQ   (负数=需求)  ← 生产订单的原材料需求
  │   ├─ MANUF_ORDER_RECP  (正数=供应)  ← 生产订单的成品产出
  │   ├─ REQUIRED_MRP      (需求)       ← 低于安全库存的产品
  │   └─ SalesForecast     (需求)       ← 销售预测
  │
  └─ Step 2: executeMrp() 主循环                    ← 按 BOM 层级逐层处理
      │
      for each bomLevel (0, 1, 2, ... 直到连续 3 层无事件):
          for each product in MrpEvent (ordered by productId, eventDate):
              │
              ├─ stockTmp += eventQuantity           (正供应 / 负需求)
              │
              ├─ getManufacturingComponents()         ← BOM 展开
              │   └─ isBuilt = node.isManufactured()  ← 判断自制 vs 外购
              │
              └─ if stockTmp < minimumStock:          ← 缺货
                  ├─ ProposedOrder(isBuilt, qty)
                  │   ├─ calculateQuantityToSupply()  ← 考虑最小批量
                  │   ├─ calculateStartDate()         ← 反向排程: 成品→工序
                  │   │
                  │   ├─ if isBuilt:
                  │   │   processBomComponent()       ← BOM 展开子件
                  │   │   → 子件作为 MRP_REQUIREMENT 写入下一层 MrpEvent
                  │   │
                  │   └─ proposedOrder.create()       ← 创建 Requirement
                  │       ├─ isBuilt=true  → INTERNAL_REQUIREMENT (→ 生产运行)
                  │       └─ isBuilt=false → PRODUCT_REQUIREMENT  (→ 采购订单)
                  │
                  └─ stockTmp += proposedOrder.quantity
```

**关键源码**：

| 功能 | 源码位置 |
|---|---|
| MRP 服务定义 | `manufacturing/servicedef/services_mrp.xml:28` |
| executeMrp 主循环 | `MrpServices.java:618` |
| initMrpEvents 初始化 | `MrpServices.java:62` |
| processBomComponent BOM 展开 | `MrpServices.java:575` |
| ProposedOrder.create | `ProposedOrder.java:235` |
| Requirement 类型分流 | `ProposedOrder.java:261` isBuilt ? INTERNAL : PRODUCT |

**特点**：全局视角，供需统一建模为正负数量事件。但实时性差，依赖定时调度（timeout 2 小时）。

---

## 4. 核心场景对比：自制产品 BOM 级联

| | ERPNext | Odoo | OFBiz |
|---|---|---|---|
| **触发时机** | 手动点 "Get Raw Materials" | SO confirm 自动递归 | 定时 MRP 批处理 |
| **BOM 展开深度** | 多层（PP services） | 无限递归（stock.move 链式触发） | 逐层（bomLevel 递增） |
| **自制→采购切换** | 代码内 BOMNode.isManufactured | StockRule.action 自动路由 | ProposedOrder.isBuilt |
| **库存扣减** | projected_qty | move 状态机 + virtual_available | QOH (quantityOnHand) |
| **原材料需求合并** | 不合并（按 SO） | _make_po_get_domain 自动合并 | 按 productId 事件聚合 |
| **反向排程** | 无（PP 有 schedule_date） | date_planned 正向 | calculateStartDate 反向 |
| **重复展开防护** | ignore_existing_ordered_qty | 已有 MO 自动追加数量 | event 去重 createOrUpdate |

---

## 5. 推荐参考：Odoo

### 5.1 推荐理由

ABT 的架构基因和 Odoo 最接近：

**① 实时事件驱动 — 匹配 ABT 已有设计**

| ABT 概念 | Odoo 对应 |
|---|---|
| `acquire_channel` 枚举 | `StockRule.action` (pull/buy/manufacture) |
| `DemandCreated` 事件 | `Procurement` NamedTuple |
| MES/Purchase demand handler | `_run_manufacture` / `_run_buy` |
| `DemandService.create_from_order()` | `StockRule.run()` |

ABT 已有的链路：
```
SO confirm → DemandService.create_from_order() → DemandCreated 事件
  ├─ acquire_channel=1 → MES handler → 生产计划
  └─ acquire_channel=2 → Purchase handler → 采购通知
```

Odoo 的链路几乎同构，映射清晰。

**② 递归级联 — 正好补 ABT 的缺失**

Odoo 的递归在 `MrpProduction.action_confirm()` 内：成品 MO 创建后，其原材料的 stock.move 自动成为新的 Procurement，再次进入 `StockRule.run()`，原材料如果是外购件就自动走 `_run_buy()`。

ABT 缺的正是这一步：
```
ABT 需补充的链路（借鉴 Odoo 递归模式）：
DemandService.create_from_order()
  │
  ├─ 自制行创建 Demand(acquire_channel=1)
  │   │
  │   └─ 🔥 新增：BOM 展开（多层递归）
  │       for each 原材料 in BOM.explode(product_id, shortage_qty):
  │           ├─ 查原材料 ATP（借鉴 ERPNext projected_qty 公式）
  │           ├─ 扣减已有在途/在制
  │           ├─ 原材料 acquire_channel=2 → Demand(acquire_channel=2)
  │           └─ 原材料 acquire_channel=1 → 递归展开（子件的子件）
  │       └── 原材料 Demand 的 source 标记为 SO 行
  │
  └─ 外购行创建 Demand(acquire_channel=2)
```

**③ 不推荐另外两家的原因**

- **ERPNext**：人工触发太重，SO 确认后要人工点 3-4 次按钮才能走完"自制→原材料采购"。ABT 已是事件驱动架构，退回人工触发是倒退。但 ERPNext 的 `projected_qty` 库存公式值得借鉴。
- **OFBiz**：定时批量 MRP 实时性太差（timeout 7200s）。ABT 场景是"SO 确认后立即分流"。但 OFBiz 的 `MrpEvent` 供需统一事件模型是远期 MRP 的参考。

### 5.2 设计要素参考矩阵

| 设计要素 | 推荐参考 | 理由 |
|---|---|---|
| **整体架构** | **Odoo** | 实时事件驱动 + 规则路由，与 ABT 架构基因一致 |
| **BOM 级联** | **Odoo** `_run_manufacture` 递归 | 正好补 ABT 缺失的自制→原材料→采购链路 |
| **库存可用量计算** | **ERPNext** `projected_qty` 公式 | 公式清晰：actual + ordered + planned − reserved |
| **供需事件建模** | **OFBiz** `MrpEvent` 正负数量 | 远期做全局 MRP 时可参考的统一事件模型 |
| **采购自动合并** | **Odoo** `_make_po_get_domain` | 多个需求按 supplier 合并到同一 PO |
| **需求→下游单据** | **ABT 自有**（Demand + Event Handler） | ABT 已有的设计比三家都更优雅 |
| **反向排程** | **OFBiz** `calculateStartDate` | 成品交期→工序反推开始日期 |

---

## 6. UI 参考：Odoo 设计模式

Odoo 的前端 UI 有几个值得 ABT 借鉴的设计模式。

### 6.1 值得参考的 Odoo UI 模式

#### ① Header 状态栏 + 条件按钮

```xml
<!-- Odoo SO Form Header -->
<header>
    <button string="Confirm" name="action_confirm" 
            class="btn-primary" invisible="state != 'draft'"/>
    <button string="Create Invoice" 
            invisible="invoice_status != 'to invoice'"/>
    <field name="state" widget="statusbar" statusbar_visible="draft,sent,sale"/>
</header>
```

- 按钮按状态条件显隐（`invisible="state != 'draft'"`）
- 状态栏 widget 可视化生命周期
- 主操作高亮 `btn-primary`，次操作 `btn-secondary`

**ABT 对比**：ABT 目前按钮逻辑分散在页面渲染中。可借鉴 Odoo 的声明式条件显隐模式，但用 HTMX 的 `hx-disable` + Hyperscript `_="on click if ..."` 实现。

#### ② Smart Button Box（关联单据统计）

```xml
<div class="oe_button_box" name="button_box">
    <button name="action_view_invoice" icon="fa-pencil-square-o" 
            invisible="invoice_count == 0">
        <field name="invoice_count" widget="statinfo" string="Invoices"/>
    </button>
</div>
```

- 表单顶部一行统计按钮，点击跳转关联单据列表
- 计数为 0 时隐藏，减少视觉噪音
- 每个按钮一个图标 + 计数 + 标签

**ABT 可借鉴**：在 SO 详情页用 smart button 展示关联的工单数、采购单数、发货单数，点击跳转。

#### ③ 行内装饰着色（Decoration）

```xml
<field name="components_availability"
    decoration-success="reservation_state == 'assigned'"
    decoration-warning="reservation_state != 'assigned' and components_availability_state in ('expected', 'available')"
    decoration-danger="reservation_state != 'assigned' and components_availability_state in ('late', 'unavailable')"/>
```

- 根据字段值动态着色：绿(success)/橙(warning)/红(danger)/灰(muted)
- 用户一眼看到缺货风险，不需要读数字

**ABT 对比**：ABT 的履约工作台设计中已有"库存充足绿色高亮，不足橙色/红色高亮"的计划（设计文档 §3.8），但尚未实现。Odoo 的 decoration 模式是很好的实现参考。

#### ④ MO 组件可用性 Widget（forecast_widget）

```xml
<field name="forecast_availability" widget="forecast_widget"/>
```

在工单组件列表中，每行显示一个可视化的"预计可用性"指示器，表示生产开始时该原材料是否有足够库存。

**ABT 可借鉴**：在 MES 工单的 BOM 组件列表中，每个原材料旁边显示库存充足/不足的状态指示。

#### ⑤ Notebook 多 Tab 组织复杂表单

```xml
<notebook>
    <page string="Order Lines" name="order_lines"> ... </page>
    <page string="Other Info" name="other_information"> ... </page>
</notebook>
```

复杂表单用 Tab 分区，避免页面过长。

**ABT 对比**：ABT 已用 Tab/抽屉模式（设计文档 §3.8），方向一致。

### 6.2 不建议照搬的 Odoo UI 模式

| Odoo 模式 | 不建议原因 | ABT 替代 |
|---|---|---|
| XML 视图声明 | Odoo 特有框架，不适用 ABT 的 Maud + HTMX | Maud 宏渲染 + HTMX 局部刷新 |
| OWL 组件框架 | JS 前端框架，与 ABT SSR 架构不兼容 | HTMX + Hyperscript |
| Chatter 沟通面板 | 功能过重，ABT 用审计日志替代 | AuditLogService |
| 列表内联编辑 | 需要复杂前端状态管理 | HTMX `hx-post` + `outerHTML` swap |

### 6.3 ABT UI 可落地的 Odoo 借鉴点

按优先级排序：

| 优先级 | 借鉴点 | 实现方式 | 对应 ABT 组件 |
|---|---|---|---|
| P0 | **行状态着色** | UnoCSS `text-success` / `text-warning` / `text-danger` 条件 class | 履约工作台表格行 |
| P0 | **Smart Button 跳转** | SO 详情页顶部统计按钮，HTMX `hx-get` 跳转关联单据列表 | SO 详情页 |
| P1 | **状态条件按钮** | Hyperscript `_="on click if status is 'draft' ..."` 控制显隐 | SO/MO/PO 详情页 header |
| P1 | **库存可用性指示器** | 原材料行右侧显示 `status-pill`（充足/不足/缺货） | MES 工单组件列表 |
| P2 | **进度条** | `div class="w-full bg-gray-200 rounded-full"` + 内层宽度百分比 | SO 详情页加权进度条（设计文档 §3.8） |

---

## 7. 结论

1. **后端架构参考 Odoo**：实时事件驱动 + 递归 BOM 级联，补全自制→原材料→采购链路
2. **库存公式参考 ERPNext**：`projected_qty = actual + ordered + planned − reserved`
3. **UI 交互参考 Odoo**：行状态着色、Smart Button、库存指示器，用 HTMX + Hyperscript 落地
4. **远期 MRP 参考 OFBiz**：MrpEvent 供需统一事件模型
5. **需求池和事件驱动保持 ABT 自有设计**：Demand + Event Handler 架构比三家都更优雅
