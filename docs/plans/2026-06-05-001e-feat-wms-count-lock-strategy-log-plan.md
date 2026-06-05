---
title: "feat: WMS 盘点 + 锁库 + 策略 + 事务日志 + 级联查询"
type: feat
status: active
date: 2026-06-05
parent: 2026-06-05-001-feat-wms-frontend-plan.md
pages: 9
dependencies: 001a
verified: true
---

# feat: WMS 盘点 + 锁库 + 策略 + 事务日志 + 级联查询 (Sub-Plan E)

## Summary

实现循环盘点（列表/新建/详情=3页）、库存锁定（列表/新建/详情=3页）、策略管理（1页配置）、事务日志（1页只读）、级联库存查询（1页查询），共 9 个页面。

---

## Implementation Units

### U28. 循环盘点 — 列表页

**原型文件:** `03-cycle-count-list.html`
**后端 Service:** `cycle_count_service`

**Files:**
- Create: `abt-web/src/pages/wms_cycle_count.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态 Tab：全部/草稿/盘点中/已完成/已调整/已取消（✅ 与 UML CycleCountStatus 一致）
- 搜索 + 筛选
- 表格列：盘点单号、盘点仓库、盘点库区、**盘点日期**、状态、**盲盘**、**物料项数**、操作员、操作
  - ⚠️ 原型没有「盘点方法」列和「差异项数」列
  - ⚠️ 原型有「盘点日期」列和「盲盘」标记列
- 分页

**设计对齐修正：** 表格列以原型为准。

---

### U29. 循环盘点 — 新建页

**原型文件:** `03-cycle-count-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_cycle_count.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 仓库 + 库区联动
- **盘点范围**：按库区 / 按产品范围 / 按ABC分类（⚠️ 不是计划的「全面盘点/抽盘/ABC分类盘点」）
  - ⚠️ 原型用「盘点范围」而非「盘点方法」
- **盲盘模式**开关（⚠️ 原型有此字段，计划缺少；对应 UML `is_blind: bool`）
- 盘点日期（单个日期，非范围）
- 保存草稿 / 开始盘点

**设计对齐修正：** 盘点选项改为「按库区/按产品范围/按ABC分类」；新增盲盘模式开关。

---

### U30. 循环盘点 — 详情页

**原型文件:** `03-cycle-count-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_cycle_count.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 4 个统计卡片：**总项数** / **一致项** / **差异项** / **已调整项**
  - ⚠️ 原型标签：总项数/一致项/差异项/已调整项（计划写的是总数/匹配数/差异数/已调整数）
- 工作流步骤条：草稿 → 盘点中 → 已完成 → 已调整（✅ 与 UML 一致）
- 盘点明细表格：行号/储位/产品编码/产品名称/批次号/系统数量/实盘数量/差异数量/差异原因/已调整
  - 盘点中：实盘数量可编辑
  - 差异行有颜色标记（正差异=黄色，负差异=红色，零=绿色）
- 操作栏：确认盘点完成、调整库存、取消

**设计对齐修正：** 统计卡片标签名以原型为准。

---

### U31. 库存锁定 — 列表页

**原型文件:** `03-lock-list.html`
**后端 Service:** `inventory_lock_service`

**Files:**
- Create: `abt-web/src/pages/wms_lock.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态 Tab：全部 / **生效** / 已释放 / 已作废
  - ⚠️ UML LockStatus: ACTIVE, RELEASED, CANCELLED — 只有 3 个状态（无 Expired）
  - ⚠️ 原型 4 个 Tab（全部/生效/已释放/已作废），与 UML 一致
  - ⚠️ 计划之前写了 5 个 Tab（含「已过期」），UML 没有 Expired 状态
- 表格列：锁库单号/产品编码/产品名称/锁定仓库/锁定数量/锁定原因/**关联客户**/状态/操作员/操作
  - ⚠️ 原型有「关联客户」列（对应 UML `customer_id`），计划之前缺少
  - ⚠️ 原型没有「储位」和「过期日期」列

**设计对齐修正：** Tab 数量以 UML 为准（4个）；表格列以原型为准（含关联客户）。

---

### U32. 库存锁定 — 新建页

**原型文件:** `03-lock-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_lock.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 产品选择 + 仓库选择（⚠️ 原型用扁平的仓库选择器，**不是三级联动**）
- 锁定数量
- 锁定原因下拉
- **关联客户**（⚠️ 原型有此字段，对应 UML `customer_id`，计划之前缺少；条件显示）
- 备注
- 保存草稿 / 确认锁定

**设计对齐修正：** 无三级联动，用扁平仓库选择器；新增「关联客户」字段；删除「过期日期」。

---

### U33. 库存锁定 — 详情页

**原型文件:** `03-lock-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_lock.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 锁定信息卡片：锁库单号/产品编码/产品名称/锁定仓库/锁定数量/锁定原因/**关联客户**/操作员/创建时间
  - ⚠️ 原型没有「储位」和「过期日期」字段
- 操作按钮：释放锁定 / 作废（⚠️ 原型用「作废」而非「void」）

**设计对齐修正：** 详情字段以原型为准。

---

### U34. 策略管理页

**原型文件:** `03-strategy-list.html`
**后端 Service:** `strategy_service`

**Files:**
- Create: `abt-web/src/pages/wms_strategy.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 两区：
  - **上架策略 (Putaway Strategies)**：
    - 类型：**SAME_MERGE / NEAREST / FIXED_BIN / EMPTY_FIRST**
    - ⚠️ UML PutawayType 有 4 个值，计划之前只写了 2 个
  - **拣货策略 (Pick Strategies)**：
    - 类型：**FIFO / FEFO / SHORTEST_PATH / FULL_PALLET**
    - ⚠️ 计划之前写了 NEAREST，但 UML 是 SHORTEST_PATH
- 每行：优先级 badge、启用/禁用 toggle、编辑按钮
- 新建策略按钮（模态框）

**设计对齐修正：** 策略类型值全部使用 UML 枚举（4 个上架 + 4 个拣货）。

---

### U35. 事务日志页

**原型文件:** `03-transaction-log.html`
**后端 Service:** `inventory_service.query_logs()`

**Files:**
- Create: `abt-web/src/pages/wms_transaction_log.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 事务类型下拉：采购入库/生产入库/销售出库/生产领料/生产退料/系统倒冲/调拨/形态转换/盘点调整/锁库/解锁/报废（✅ 12 种类型与 UML TransactionType 一致）
- 仓库筛选 + 日期范围
- 表格列：时间、事务类型（彩色 tag）、产品、仓库、储位、数量（+/-前缀）、来源单号（链接）、操作人
- 类型颜色：入=绿/出=红/移=蓝/调=橙/锁=紫/转=青
- 分页

---

### U36. 级联库存查询页

**原型文件:** `03-cascade-list.html`
**后端 Service:** `inventory_cascade_service`

**Files:**
- Create: `abt-web/src/pages/wms_cascade.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 产品搜索栏
- 选中产品后显示：
  - 产品信息卡 + 总库存量
  - **BOM 表格（按 BOM 版本分组，扁平表格形式）**
    - ⚠️ 原型用**扁平表格**展示 BOM 组件，不是树形结构
    - 每行：子件产品编码/名称/用量/**当前库存**/**损耗率**/**是否缺料**标记
    - 缺料 = 红色，充足 = 绿色
  - 表格按 BOM 版本分组显示

**设计对齐修正：** BOM 展示用扁平表格（非树形），增加损耗率和缺料标记列。

---

## Execution Order

U28 → U29 → U30（盘点）
U31 → U32 → U33（锁库）
U34（策略）
U35（事务日志）
U36（级联查询）
可并行。
