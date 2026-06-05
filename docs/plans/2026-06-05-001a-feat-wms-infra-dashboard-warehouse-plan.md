---
title: "feat: WMS 基础设施 + 总览 + 仓库 + 储位"
type: feat
status: active
date: 2026-06-05
parent: 2026-06-05-001-feat-wms-frontend-plan.md
pages: 8
verified: true
---

# feat: WMS 基础设施 + 总览 + 仓库 + 储位 (Sub-Plan A)

## Summary

搭建 WMS 前端基础设施（state.rs 注册、路由注册、侧边栏），实现库存总览 Dashboard、仓库管理（列表/新建/详情=3页含库区CRUD）、储位管理（列表/新建/详情=3页），共 8 个页面。

---

## Implementation Units

### U1. 补充 abt-core 工厂函数

**Goal:** 为 `inventory`、`inventory_cascade`、`strategy` 三个缺少工厂函数的模块补充 `new_xxx_service(pool)`

**Files:**
- Modify: `abt-core/src/wms/inventory/mod.rs`
- Modify: `abt-core/src/wms/inventory_cascade/mod.rs`
- Modify: `abt-core/src/wms/strategy/mod.rs`

**Approach:** 添加 `pub fn new_xxx_service(pool: PgPool) -> impl XxxService` 工厂函数。`InventoryServiceImpl` 构造函数无参数（无 pool 字段），工厂直接返回 `InventoryServiceImpl::new()` 即可。`inventory_cascade` 和 `strategy` 的 implt 可能需要 pool，需检查。

**Verification:** `cargo check -p abt-core` 编译通过

---

### U2. state.rs 注册全部 WMS Service

**Goal:** 在 `AppState` 中注册所有 12 个 WMS Service getter

**Files:**
- Modify: `abt-web/src/state.rs`

**Approach:** 添加以下 getter（已有 `warehouse_service`）：
- `arrival_notice_service` → `abt_core::wms::arrival_notice::new_arrival_notice_service`
- `inventory_service` → `abt_core::wms::inventory::new_inventory_service`
- `inventory_transaction_service` → `abt_core::wms::inventory_transaction::new_inventory_transaction_service`
- `material_requisition_service` → `abt_core::wms::material_requisition::new_material_requisition_service`
- `backflush_service` → `abt_core::wms::backflush::new_backflush_service`
- `cycle_count_service` → `abt_core::wms::cycle_count::new_cycle_count_service`
- `transfer_service` → `abt_core::wms::transfer::new_transfer_service`
- `form_conversion_service` → `abt_core::wms::form_conversion::new_form_conversion_service`
- `inventory_lock_service` → `abt_core::wms::inventory_lock::new_inventory_lock_service`
- `stock_ledger_service` → `abt_core::wms::stock_ledger::new_stock_ledger_service`
- `strategy_service` → `abt_core::wms::strategy::new_strategy_service`
- `inventory_cascade_service` → `abt_core::wms::inventory_cascade::new_inventory_cascade_service`

**Verification:** `cargo check -p abt-web` 编译通过

---

### U3. 侧边栏 + 路由注册骨架

**Goal:** 更新侧边栏 inventory 模块展开完整导航，创建路由骨架

**Files:**
- Modify: `abt-web/src/layout/sidebar.rs`
- Create: `abt-web/src/routes/wms.rs`
- Modify: `abt-web/src/routes/mod.rs`

**Approach:**

侧边栏 `inventory` 模块 items 替换为：
```
库存总览 → /admin/wms
仓库管理 → /admin/wms/warehouses
储位管理 → /admin/wms/bins
库存查询 → /admin/wms/stock
入库管理 → /admin/wms/stock-in
出库管理 → /admin/wms/stock-out
来料通知 → /admin/wms/arrivals
库存调拨 → /admin/wms/transfers
领料单   → /admin/wms/requisitions
形态转换 → /admin/wms/conversions
倒冲记录 → /admin/wms/backflushes
循环盘点 → /admin/wms/cycle-counts
库存锁定 → /admin/wms/locks
策略管理 → /admin/wms/strategies
事务日志 → /admin/wms/transactions
级联查询 → /admin/wms/cascade
```

---

### U4. 库存管理总览 Dashboard

**Goal:** 实现 `03-index.html` — 5 个统计卡片 + 快捷入口网格 + 最近操作列表

**原型文件:** `03-index.html`

**Files:**
- Create: `abt-web/src/pages/wms_dashboard.rs`
- Create: `abt-web/src/routes/wms_dashboard.rs`

- 5 个 stat-card：仓库总数、库存品类、本月入库、本月出库、低库存预警
- 快捷入口：**14** 个链接卡片（⚠️ 原型是14个非16个：无级联查询和事务日志快捷入口）
  - 仓库管理/储位管理/库存查询/来料通知/入库管理/出库管理/领料单/循环盘点/库存调拨/形态转换/倒冲记录/库存锁定/事务日志/策略管理
- 最近操作列表表格：时间/**操作类型**(pill)/**单号**(可点击链接)/仓库/操作人 — 取最近 5 条
- **导出报表**按钮（⚠️ 原型有此按钮，计划未提）
---

### U5. 仓库管理 — 列表页

**原型文件:** `03-warehouse-list.html`

**Files:**
- Create: `abt-web/src/pages/wms_warehouse_list.rs`
- Create: `abt-web/src/routes/wms_warehouse.rs`

- 状态 Tab：全部 / 启用 / 停用
- 搜索框 + 类型下拉筛选
- 表格列：仓库编码、仓库名称、**仓库类型**、状态、地址、**管理员**、**库区数**、**储位数**、操作
  - ⚠️ 列名以原型为准：「仓库类型」非「类型」，「管理员」非「负责人」，原型有额外「库区数」「储位数」列
- 操作列：**编辑**（icon）、删除（icon）
  - ⚠️ 原型用「编辑」非「查看」，行点击跳转详情
- 新建按钮 → 跳转 create 页
- 分页

---

### U6. 仓库管理 — 新建/编辑页

**原型文件:** `03-warehouse-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_warehouse_list.rs`
- Modify: `abt-web/src/routes/wms_warehouse.rs`
- 表单区：编码、名称、**仓库类型**下拉、**是否虚拟仓库（委外）**checkbox、地址、**管理员**、备注
  - ⚠️ 原型无「状态开关」，有「是否虚拟仓库」checkbox（对应 UML `is_virtual: bool`）
  - ⚠️ 虚拟仓库选中后显示说明信息区
- 编辑模式：加载已有数据
- `hx-post` 提交，成功后重定向到详情页

---

### U7. 仓库管理 — 详情页（含库区 CRUD）

**原型文件:** `03-warehouse-detail.html`

- 返回链接 + 详情头部 + 状态标签 + **虚拟仓 tag**（⚠️ 原型有虚拟仓标签）
- 信息网格：编码/名称/**仓库类型**/状态/地址/**管理员**/创建时间
- 库区子表格：库区编码/名称/类型/储位数/排序/备注/操作 — 可增删改
  - 新增：`hx-post` 返回更新后的表格
  - 编辑：模态框 `hx-put` 保存（字段：编码/名称/类型/排序/备注）
  - 删除：`hx-delete`
- 库区统计小卡片（总库存量/品种数/低库存项/安全库存预警）
- **储位明细子表格**（⚠️ 原型有额外的 bin 列表，显示选中库区下的储位：储位编码/名称/行/列/层/容量上限/状态/温控要求）
  - 储位点击可查看库存明细
---

### U8. 储位管理 — 列表/新建/详情 (3 页)

**原型文件:** `03-bin-list.html`, `03-bin-create.html`, `03-bin-detail.html`

**Files:**
- Create: `abt-web/src/pages/wms_bin.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**

**列表页** (`/admin/wms/bins`)：
- 搜索 + 仓库筛选 + **状态筛选**（⚠️ 原型没有库区筛选，有状态筛选）
- 表格列：储位编码、**储位名称**、所属仓库、所属库区、**行号(row_no)**、**列号(column_no)**、**层号(layer_no)**、容量、状态
  - ⚠️ 原型没有 type 列，有 row/column/layer 三列

**新建页** (`/admin/wms/bins/create`)：
- 坐标式编码：区号 + 行号 + 列号 + 层号
- 仓库/库区联动下拉
- **允许物料类型**（checkbox 多选）— ⚠️ 原型用「允许物料类型」而非「储位类型」
- 容量上限

**详情页** (`/admin/wms/bins/{id}`)：
- **3 个 Tab**：基本信息 / 库存明细 / **操作历史**（⚠️ 原型有3个Tab，不是2个）
  - 基本信息：坐标显示框、容量进度条
  - 库存明细：`inventory_service.get_by_bin()`
  - 操作历史：`inventory_service.list_logs_by_bin()`

**设计对齐修正（验证发现）：**
- 列表筛选用「状态」而非「库区」
- 表格有 bin_name / row_no / column_no / layer_no 列
- 新建页用「允许物料类型」而非「储位类型」
- 详情页有 3 个 Tab（基本信息 + 库存明细 + 操作历史）

---

## Execution Order

U1 → U2 → U3 → U4 → U5 → U6 → U7 → U8
