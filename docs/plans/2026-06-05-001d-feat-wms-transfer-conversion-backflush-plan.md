---
title: "feat: WMS 调拨 + 形态转换 + 倒冲"
type: feat
status: active
date: 2026-06-05
parent: 2026-06-05-001-feat-wms-frontend-plan.md
pages: 8
dependencies: 001a
verified: true
---

# feat: WMS 调拨 + 形态转换 + 倒冲 (Sub-Plan D)

## Summary

实现库存调拨（列表/新建/详情=3页）、形态转换（列表/新建/详情=3页）、倒冲记录（列表/详情=2页），共 8 个页面。

---

## Implementation Units

### U20. 库存调拨 — 列表页

**原型文件:** `03-transfer-list.html`
**后端 Service:** `transfer_service`

**Files:**
- Create: `abt-web/src/pages/wms_transfer.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态 Tab：全部 / 草稿 / **在途** / 已完成 / 已取消
  - ⚠️ UML TransferStatus: DRAFT, IN_TRANSIT, COMPLETED, CANCELLED — 只有 4 个状态
  - ⚠️ 原型与 UML 一致，只有 5 个 Tab（含「全部」）
  - ⚠️ 计划之前写了 7 个 Tab（含待审核/已审核），这是**错误的**
- 搜索 + 筛选
- 表格列：调拨单号、调出仓库、调入仓库、产品数、总数量、状态、操作人、创建时间
- 分页

**设计对齐修正：** 状态 Tab 以原型和 UML 为准（5个），删除虚假的待审核/已审核。

---

### U21. 库存调拨 — 新建页

**原型文件:** `03-transfer-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_transfer.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 调出仓库 + 调入仓库选择器
- 状态流程条：草稿 → 在途 → 完成（⚠️ 原型只有 3 步）
- 行项表格：产品/数量/**批次号**
  - ⚠️ 原型行项**没有 per-line 的 from-bin/to-bin** 选择器，批次号是行级字段
  - 库区和储位信息在调出仓库/调入仓库级别设置
- 添加行 / 删除行
- 保存草稿 / 提交

**设计对齐修正：** 行项无独立 from-bin/to-bin，批次号在行项级别。

---

### U22. 库存调拨 — 详情页

**原型文件:** `03-transfer-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_transfer.rs`
- Modify: `abt-web/src/routes/wms.rs`

- 工作流步骤条：草稿 → 在途 → 已完成（3 步，与 UML TransferStatus 一致）
- 信息卡片：调拨单号/调出仓库(**→库区→储位**)/调入仓库(**→库区→储位**)/调拨日期/**操作员**
- 行项明细表格：行号/产品编码/产品名称/规格/单位/调拨数量/批次号
- 操作栏：取消 + 确认完成
---

### U23. 形态转换 — 列表页

**原型文件:** `03-conversion-list.html`
**后端 Service:** `form_conversion_service`

**Files:**
- Create: `abt-web/src/pages/wms_conversion.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态 Tab：全部 / 草稿 / **已完成** / 已取消
  - ⚠️ UML ConversionStatus: DRAFT, COMPLETED, CANCELLED — 只有 3 个状态
  - ⚠️ 计划之前写了「全部/草稿/待审核/已审核/已完成/已取消」，但 UML 没有待审核/已审核
- 表格列：转换单号、仓库、消耗物料数、产出物料数、状态、操作人、创建时间
- 分页

**设计对齐修正：** 状态 Tab 只有 4 个（含全部），删除虚假的待审核/已审核。

---

### U24. 形态转换 — 新建页

**原型文件:** `03-conversion-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_conversion.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 仓库选择器
- **消耗物料区**（红色标签）：产品搜索 → 数量 → 储位
- **产出物料区**（绿色标签）：产品搜索 → 数量 → 储位
- ⚠️ 原型**没有**转换比例说明区域（计划之前有，但原型不存在）
- 添加行 / 删除行
- 保存草稿 / 提交

**设计对齐修正：** 删除「转换比例说明」。

---

### U25. 形态转换 — 详情页

**原型文件:** `03-conversion-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_conversion.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 工作流步骤条：草稿 → 已完成
- 信息卡片
- 消耗物料区（红色标签）+ 产出物料区（绿色标签）
- 操作栏：完成、取消

---

### U26. 倒冲记录 — 列表页

**原型文件:** `03-backflush-list.html`
**后端 Service:** `backflush_service`

**Files:**
- Create: `abt-web/src/pages/wms_backflush.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 无状态 Tab（只读列表）
- 筛选：日期范围 + 搜索（搜索单据编号/工单号）
- 表格列：**单据编号**、关联工单、**完工产品**、**完工数量**、**倒冲日期**、状态、**差异预警**、操作员、操作
  - ⚠️ 原型列名与计划不同：倒冲单号→单据编号，成品名称→完工产品，倒冲数量→完工数量，执行时间→倒冲日期
  - ⚠️ 原型有额外的「差异预警」和「操作」列
- 无新建按钮
- 分页

**设计对齐修正：** 列名以原型为准。

---

### U27. 倒冲记录 — 详情页

**原型文件:** `03-backflush-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_backflush.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态流程：草稿 → 已执行 → 已调整（✅ 与 UML BackflushStatus 一致）
- 行项明细：物料/**应耗数量(theoretical_qty)**/**实耗数量(actual_qty)**/差异(variance_qty)/差异率(variance_rate)
  - **超耗高亮**：is_over_threshold=true 的行红色标记
- 汇总栏：总子件数 / 超标项数 / 最大差异率
- 打印按钮（占位）

---

## Execution Order

U20 → U21 → U22（调拨）
U23 → U24 → U25（形态转换）
U26 → U27（倒冲）
可并行。
