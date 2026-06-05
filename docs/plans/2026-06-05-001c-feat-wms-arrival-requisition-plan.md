---
title: "feat: WMS 来料通知 + 领料单"
type: feat
status: active
date: 2026-06-05
parent: 2026-06-05-001-feat-wms-frontend-plan.md
pages: 6
dependencies: 001a
verified: true
---

# feat: WMS 来料通知 + 领料单 (Sub-Plan C)

## Summary

实现来料通知（列表/新建/详情=3页）和领料单（列表/新建/详情=3页），共 6 个页面。

---

## Implementation Units

### U14. 来料通知 — 列表页

**原型文件:** `03-arrival-list.html`
**后端 Service:** `arrival_notice_service`

**Files:**
- Create: `abt-web/src/pages/wms_arrival.rs`
- Modify: `abt-web/src/routes/wms.rs`

- 状态 Tab(8个)：全部(24)/草稿(3)/已收货(4)/检验中(2)/已接收(8)/部分接收(3)/已拒收(2)/已取消(2)
- 筛选：搜索单号/供应商 + **仓库**下拉
- 表格列(7)：**单据编号**(链接)/来源采购单/供应商/到货仓库/到货日期/状态(pill)/操作
- 操作按钮（条件性）：**非草稿行**→查看+删除；**草稿行**→**编辑**+删除
  - ⚠️ 原型草稿行有「编辑」按钮跳转创建页编辑模式
- **新建来料通知**按钮
- 分页
**设计对齐修正：** 操作按钮因状态条件性变化；新增筛选器描述。
---

### U15. 来料通知 — 新建页

**原型文件:** `03-arrival-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_arrival.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- ⚠️ 原型没有「来源类型选择器」，直接是：
  - **供应商信息区**：供应商选择 → 联系人/电话自动填充 + 来源采购单选择
  - **到货信息区**：到货仓库、到货库区、到货日期、送货单号、操作员
  - **物料明细区**：行号/产品编码/产品名称/规格/单位/**申报数量**/**实收数量**/**合格数量**/**批次号**/操作
- 供应商选择后可关联采购订单（非必须）
- 保存草稿 / 确认收货

**设计对齐修正：** 无来源类型切换；表单字段以原型为准（联系人/电话/仓库/库区/送货单号/操作员）；行项有申报数量/实收数量/合格数量/批次号。

---

### U16. 来料通知 — 详情页

**原型文件:** `03-arrival-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_arrival.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 工作流步骤条：草稿 → 已收货 → 检验中 → **全部接收**
  - 分支标注：部分接收 / **拒收**（⚠️ 原型用「拒收」非「已拒收」）
- 信息卡片
- 行项明细表格：行号/产品编码/产品名称/规格/单位/申报数量/实收数量/合格数量/批次号
- **IQC 质检结果区**（黄色卡片）：检验标准、AQL等级、检验员、计划完成日期、MRB 硬门警告
- 操作栏：**取消** / **开始检验** / **确认接收** / **打印**（⚠️ 原型有「打印」按钮）
  - ⚠️ 原型没有单独的「收货」按钮（收货在创建页完成）
  - ⚠️ 原型没有「拒收」按钮，只有取消
**设计对齐修正：** 分支标签「拒收」非「已拒收」；新增「打印」按钮。

---

### U17. 领料单 — 列表页

**原型文件:** `03-requisition-list.html`
**后端 Service:** `material_requisition_service`

**Files:**
- Create: `abt-web/src/pages/wms_requisition.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 状态 Tab：全部 / 草稿 / **已确认** / **已发料** / 已取消
  - ⚠️ UML RequisitionStatus: DRAFT, CONFIRMED, ISSUED, CANCELLED
  - 原型用「已确认」对应 CONFIRMED，「已发料」对应 ISSUED
  - ⚠️ 原型**没有**「部分发料」和「已完成」Tab（计划中多出的）
- 表格列：领料单号、关联工单、仓库、状态、申请人、申请日期
- 分页

**设计对齐修正：** 状态 Tab 只有 5 个（原型），与 UML 4 个枚举值对齐。

---

### U18. 领料单 — 新建页

**原型文件:** `03-requisition-create.html`

**Files:**
- Modify: `abt-web/src/pages/wms_requisition.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- **工单信息区**：关联工单 → 自动加载 BOM 物料 + **领料日期** + **操作员**（⚠️ 原型有领料日期和操作员字段）
- 仓库选择器
- 行项表格：产品编码/产品名称/规格/单位/**BOM定额**/**实领数量**/**差异量**（自动计算）/储位
  - 差异量颜色：0=绿色(success)，负=琥珀色(warn)，正=红色(danger)
- 添加行 / 删除行
- 保存草稿 / **确认领料**（⚠️ 原型用「确认领料」非「提交」）

---
### U19. 领料单 — 详情页

**原型文件:** `03-requisition-detail.html`

**Files:**
- Modify: `abt-web/src/pages/wms_requisition.rs`
- Modify: `abt-web/src/routes/wms.rs`

**Approach:**
- 工作流步骤条：草稿 → 已确认 → 已发料
- 信息卡片
- 行项明细表格：行号/产品编码/产品名称/规格/单位/BOM定额/实领数量/差异量/储位
- 操作栏：确认（草稿→已确认）、发料（已确认→已发料）、取消

---

## Execution Order

U14 → U15 → U16（来料通知）
U17 → U18 → U19（领料单）
可并行。
