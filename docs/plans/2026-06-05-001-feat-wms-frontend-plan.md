---
title: "feat: 库存管理（WMS）前端全部页面"
type: feat
status: active
date: 2026-06-05
origin: 原型设计 03-*.html + UML 设计 docs/uml-design/03-wms.html
sub-plans:
  - 2026-06-05-001a-feat-wms-infra-dashboard-warehouse-plan.md
  - 2026-06-05-001b-feat-wms-stock-operations-plan.md
  - 2026-06-05-001c-feat-wms-arrival-requisition-plan.md
  - 2026-06-05-001d-feat-wms-transfer-conversion-backflush-plan.md
  - 2026-06-05-001e-feat-wms-count-lock-strategy-log-plan.md
---

# feat: 库存管理（WMS）前端全部页面

## Summary

对照原型设计 `03-*.html`（35 个页面）和 UML 设计文档 `docs/uml-design/03-wms.html`，在 `abt-web` 中实现全部 WMS 前端页面。后端 `abt-core/src/wms/` 已有 12 个 Service 模块全部实现。前端需新建所有页面文件、注册 Service 到 `state.rs`、注册路由、更新侧边栏导航。严格使用 HTMX + Surreal.js 组件化模式，最小化手写 JS。

---

## Problem Frame

库存管理模块是 ERP 的核心操作层，涵盖仓库主数据、储位管理、入库/出库、来料通知、领料、调拨、形态转换、倒冲、盘点、锁库、策略、事务日志、级联查询等 14 个子域。后端 Service 层已全部就绪，但前端零页面存在，需从零构建 35 个页面。

---

## Requirements

- R1. 所有 35 个 `03-*.html` 原型页面对应在 `abt-web/src/pages/` 下有对应 Rust 页面
- R2. 所有 35 个页面在 `abt-web/src/routes/` 下有对应路由注册
- R3. `state.rs` 注册全部 WMS Service getter
- R4. 侧边栏 `inventory` 模块展开完整导航（14 个子页面）
- R5. 所有页面严格使用 HTMX + Surreal.js，最小化手写 JS
- R6. 所有样式通过 UnoCSS shortcuts/preflights 管理，禁止内联 style
- R7. **所有状态枚举值、表单字段、表格列严格以原型设计为准**，同时与 UML 设计文档对齐

---

## Scope Boundaries

- **范围内**：`03-*.html` 原型中全部 35 个页面的前端实现
- **范围外**：后端 Service 层修改（已实现）、新建 migration、非 WMS 模块页面
- **范围外**：MES/QMS/FMS 跨模块集成

---

## 原型页面清单（35 页）

| # | 子模块 | 页面 | 原型文件 | 后端 Service |
|---|--------|------|----------|-------------|
| 1 | 总览 | 库存管理总览 | `03-index.html` | inventory + warehouse |
| 2-4 | 仓库 | 列表/新建/详情 | `03-warehouse-*` | warehouse |
| 5-7 | 储位 | 列表/新建/详情 | `03-bin-*` | warehouse (zone/bin) |
| 8 | 库存 | 库存查询 | `03-stock-list.html` | inventory |
| 9-10 | 入库 | 列表/新建 | `03-stockin-*` | inventory_transaction |
| 11-12 | 出库 | 列表/新建 | `03-stockout-*` | inventory_transaction |
| 13-15 | 来料通知 | 列表/新建/详情 | `03-arrival-*` | arrival_notice |
| 16-18 | 调拨 | 列表/新建/详情 | `03-transfer-*` | transfer |
| 19-21 | 领料单 | 列表/新建/详情 | `03-requisition-*` | material_requisition |
| 22-24 | 形态转换 | 列表/新建/详情 | `03-conversion-*` | form_conversion |
| 25-26 | 倒冲 | 列表/详情 | `03-backflush-*` | backflush |
| 27-29 | 盘点 | 列表/新建/详情 | `03-cycle-count-*` | cycle_count |
| 30-32 | 锁库 | 列表/新建/详情 | `03-lock-*` | inventory_lock |
| 33 | 策略 | 策略管理 | `03-strategy-list.html` | strategy |
| 34 | 日志 | 事务日志 | `03-transaction-log.html` | inventory (query_logs) |
| 35 | 级联 | 级联库存查询 | `03-cascade-list.html` | inventory_cascade |

---

## UML 状态枚举参考（权威来源）

以下状态枚举来自 `docs/uml-design/03-wms.html`，前端必须与这些值对齐：

| 实体 | 枚举 | 值 |
|------|------|-----|
| Warehouse | WarehouseStatus | ACTIVE, INACTIVE |
| Bin | BinStatus | EMPTY, OCCUPIED, LOCKED, DISABLED |
| ArrivalNotice | ArrivalStatus | DRAFT, RECEIVED, INSPECTING, ACCEPTED, PARTIALLY_ACCEPTED, REJECTED, CANCELLED |
| InventoryTransaction | TransactionType | PURCHASE_RECEIPT, PRODUCTION_RECEIPT, SALES_SHIPMENT, MATERIAL_ISSUE, MATERIAL_RETURN, BACKFLUSH, TRANSFER, FORM_CONVERSION, ADJUSTMENT, LOCK, UNLOCK, SCRAP |
| MaterialRequisition | RequisitionStatus | DRAFT, CONFIRMED, ISSUED, CANCELLED |
| BackflushRecord | BackflushStatus | DRAFT, EXECUTED, ADJUSTED |
| CycleCount | CycleCountStatus | DRAFT, COUNTING, COMPLETED, ADJUSTED, CANCELLED |
| FormConversion | ConversionStatus | DRAFT, COMPLETED, CANCELLED |
| InventoryLock | LockStatus | ACTIVE, RELEASED, CANCELLED |
| PutawayStrategy | PutawayType | SAME_MERGE, NEAREST, FIXED_BIN, EMPTY_FIRST |
| PickStrategy | PickType | FIFO, FEFO, SHORTEST_PATH, FULL_PALLET |

---

## Context & Research

### Relevant Code and Patterns

- **已有模式参考**：`abt-web/src/pages/purchase_*.rs` — 采购模块前端，完整的 list/create/detail 页面范式
- **路由模式**：`abt-web/src/routes/purchase_dashboard.rs` — TypedPath + Router 注册
- **Service 注册模式**：`abt-web/src/state.rs` — `pub fn xxx_service(&self) -> impl XxxService`
- **侧边栏**：`abt-web/src/layout/sidebar.rs` — `modules()` 函数中 `inventory` 模块已有占位，需扩展
- **后端 Service 工厂函数**：`abt-core/src/wms/*/mod.rs` — 10 个模块有 `new_xxx_service(pool)` 工厂

### 缺少工厂函数的模块

以下 3 个模块没有 `new_xxx_service()` 工厂函数，需要先补充：
- `inventory` — `InventoryServiceImpl::new()` 无参数，需要加工厂
- `inventory_cascade` — 无工厂
- `strategy` — 无工厂

### 设计对齐验证结果（2026-06-05 二次验证）

已完成全部 35 个原型页面的两轮三方交叉验证（原型 × 计划 × UML）。

**第一轮**：发现并修正 45+ 处差异（虚假状态值、列名不匹配、缺少字段、工作流步骤错误等）
**第二轮**：逐页详细验证，又发现并修正 ~25 处差异：
- 仓库模块：列名（仓库类型/管理员）、新建页表单字段（虚拟仓库checkbox无状态开关）、详情页储位明细子表格
- 入库新建：来源类型3种（AN/PO/手工）、额外表单字段（库区/储位/操作员/上架策略/汇总/备注）、行项额外列
- 出库列表：完整表格定义（13列）、条件操作按钮、导出功能
- 来料详情：分支标签（拒收 vs 已拒收）、打印按钮
- 领料新建：领料日期/操作员字段、按钮文案（确认领料 vs 提交）

**核心原则：原型设计是 UI 的权威来源，UML 是数据模型的权威来源。计划文档必须同时与两者对齐。**

---

## Key Technical Decisions

- **HTMX + Surreal.js 优先**：所有交互优先用 HTMX 属性和 Surreal.js 内联实现
- **页面文件组织**：每个子域一个 `routes/wms_xxx.rs` + 一个 `pages/wms_xxx.rs`
- **分阶段交付**：按子计划拆分为 5 个阶段
- **路由前缀**：所有 WMS 路由使用 `/admin/wms/` 前缀
- **状态枚举对齐**：前端 Tab/筛选/工作流状态值严格使用 UML 枚举对应的中文标签

---

## Sub-Plan 分解

| 子计划 | 文件 | 覆盖页面 | 依赖 |
|--------|------|----------|------|
| A | `001a-infra-dashboard-warehouse` | 基础设施 + 总览 + 仓库(4) + 储位(3) = 8 页 | 无 |
| B | `001b-stock-operations` | 库存查询(1) + 入库(2) + 出库(2) = 5 页 | A |
| C | `001c-arrival-requisition` | 来料通知(3) + 领料单(3) = 6 页 | A |
| D | `001d-transfer-conversion-backflush` | 调拨(3) + 形态转换(3) + 倒冲(2) = 8 页 | A |
| E | `001e-count-lock-strategy-log` | 盘点(3) + 锁库(3) + 策略(1) + 日志(1) + 级联(1) = 9 页 | A |

---

## Acceptance Criteria

1. 所有 35 个页面与原型设计完全对齐（字段、列名、状态值）
2. `state.rs` 注册全部 WMS Service getter（含补充工厂函数）
3. 侧边栏 `inventory` 模块显示完整导航
4. 所有列表页有状态筛选、搜索、分页
5. 所有创建页有表单联动、行项增删、草稿/提交
6. 所有详情页有工作流步骤条、状态操作
7. `cargo clippy` 无错误
8. 最小化手写 JS — 优先 HTMX 属性 + Surreal.js 内联
