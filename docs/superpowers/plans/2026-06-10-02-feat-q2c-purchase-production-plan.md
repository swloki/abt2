---
title: "Q2C E2E 测试 — 02 采购+生产 Happy Path"
date: 2026-06-10
type: feat
plan_depth: standard
origin: docs/superpowers/specs/2026-06-10-q2c-e2e-test-nodes.md
depends_on: 2026-06-10-01-feat-q2c-sales-planning-plan.md
---

## Summary

实现 Phase 3A（采购域 PU1-PU6）和 Phase 3B（生产域 M1-M5）的 Happy Path 测试。采购和生产在 MRP 完成后可并行执行，最终在仓储阶段汇聚。测试覆盖 11 个业务节点，涉及采购专员、采购经理、生产主管、车间操作员、质检员、仓管员 6 个角色。

## Problem Frame

采购和生产是 Q2C 链路中物料流转的核心。采购确保外购件按时到位，生产确保自制件按计划完成。两条路径需要并行执行且最终汇聚——采购的物料需要用于生产领料，生产的成品需要用于发货。这个并行+汇聚是整个链路最复杂的编排点。

---

## Requirements

- R1. PU1: 采购专员根据 MRP 建议创建采购申请
- R2. PU2: 采购专员进行询价比价（选择供应商 SUP-001）
- R3. PU3: 采购经理审批采购订单
- R4. PU4: 审批后生成采购订单，发送供应商
- R5. PU5: 采购到货，创建到货通知
- R6. PU6: 仓管员收货入库，触发来料质检
- R7. M1: 生产主管下达生产工单
- R8. M2: 仓管员执行生产领料，库存扣减
- R9. M3: 车间操作员报工，记录工时和用料
- R10. M4: 质检员执行成品质检
- R11. M5: 质检通过后成品入库，工单完工

---

## Key Technical Decisions

KTD1. **并行执行策略** — 采购和生产在 MRP 完成后并行启动，但共享仓管员 Agent（Agent-W1）。采购分支的仓管操作（收货入库）优先于生产分支的仓管操作（领料）。实现方式：采购分支先执行到收货入库完成，再启动生产分支的领料。

KTD2. **质检集成点** — 采购收货触发来料检验，生产报工触发出品质检。两个质检都由 Agent-Q1 执行，但分属不同 Session 阶段。

KTD3. **接力文件拆分** — 并行执行时使用两个接力文件：`relay-purchase.json` 和 `relay-production.json`，汇聚时合并到主 `relay-state.json`。

---

## Implementation Units

### U1. PU1-PU4: 采购申请到采购订单

**Goal:** 从 MRP 采购需求创建采购申请，经过审批生成采购订单。

**Requirements:** R1, R2, R3, R4

**Dependencies:** Plan 01 U4（MRP 结果）

**Files:**
- Create: `tests/e2e-q2c/tests/phase3a/test_pu1_pu4_purchase_order.sh`

**Approach:**

测试步骤：
1. 从接力文件读取 `purchase_request_ids`
2. Agent-PU1（`q2c_buyer`）登录 → 导航到采购申请页面（`/admin/purchase/orders/create`）
3. 创建采购订单：选择供应商 SUP-001，添加明细行（PRD-RM-001、PRD-RM-002、PRD-RM-003）
4. 填写数量（来自 MRP 建议）和价格（来自采购价目表）
5. 提交采购订单 → 断言创建成功
6. 如果需要审批（金额 > 10万），Agent-PU2（`q2c_buyer_mgr`）登录审批
7. 审批通过 → 断言采购订单状态为"已确认"
8. 写入接力文件：`purchase_order_id`

**Test scenarios:**
- 采购订单创建成功
- PO 明细包含所有外购物料
- PO 金额计算正确（数量 × 单价 × (1+税率)）
- 审批通过后 PO 状态正确
- 供应商信息正确

**Verification:** 采购订单列表（`/admin/purchase/orders`）中可看到新 PO。

---

### U2. PU5-PU6: 收货入库与来料检验

**Goal:** 供应商发货后，仓管员收货入库，触发并完成来料质检。

**Requirements:** R5, R6

**Dependencies:** U1

**Files:**
- Create: `tests/e2e-q2c/tests/phase3a/test_pu5_pu6_goods_receipt.sh`

**Approach:**

测试步骤：
1. Agent-PU1 创建到货通知（或在 PO 页面标记到货）
2. Agent-W1（`q2c_warehouse`）登录 → 导航到入库页面（`/admin/wms/stock-in/create`）
3. 创建入库单：关联 PO，选择仓库 WH-RAW，填写收货数量
4. 提交入库 → 断言入库成功
5. 断言：库存增加（待检状态）
6. Agent-Q1（`q2c_qc`）执行来料检验 → 记录合格
7. 质检通过 → 断言库存状态转为"可用"
8. 写入接力文件：收货数量、质检结果

**Test scenarios:**
- 入库单创建成功
- 入库后 WH-RAW 库存增加
- 来料检验可以执行
- 质检通过后库存可用于生产领料

**Verification:** 库存页面（`/admin/wms/stock`）显示 PRD-RM-001/002/003 在 WH-RAW 中有库存。

---

### U3. M1: 生产工单下达

**Goal:** 生产主管根据 MRP 建议下达生产工单。

**Requirements:** R7

**Dependencies:** Plan 01 U4（MRP 结果）

**Files:**
- Create: `tests/e2e-q2c/tests/phase3b/test_m1_work_order.sh`

**Approach:**

测试步骤：
1. 从接力文件读取 `work_order_suggestion_ids`
2. Agent-M1（`q2c_prod_mgr`）登录 → 导航到生产工单页面（`/admin/mes/orders`）
3. 查看工单建议，确认下达
4. 断言：工单状态为"已下达"
5. 写入接力文件：`work_order_id`

**Test scenarios:**
- 工单下达成功
- 工单关联的 BOM 和工艺路线正确
- 工单计划开工/完工时间合理

**Verification:** 工单列表（`/admin/mes/orders`）中显示已下达工单。

---

### U4. M2: 生产领料

**Goal:** 仓管员根据工单 BOM 执行领料出库。

**Requirements:** R8

**Dependencies:** U3, U2（采购物料已入库且质检通过）

**Files:**
- Create: `tests/e2e-q2c/tests/phase3b/test_m2_material_requisition.sh`

**Approach:**

**注意**：生产领料依赖采购入库的原材料。因此 U4 必须在 U2（采购收货+质检）完成之后执行。这是并行→汇聚的关键同步点。

测试步骤：
1. 从接力文件读取 `work_order_id`
2. Agent-W1 登录 → 导航到领料/出库页面（`/admin/wms/requisition` 或 `/admin/wms/stock-out`）
3. 创建领料单：关联工单，按 BOM 标准用量领料（PRD-RM-001×200KG、PRD-RM-002×50KG、PRD-RM-003×100个）
4. 提交领料 → 断言出库成功
5. 断言：WH-RAW 库存减少
6. 写入接力文件：领料数量

**Test scenarios:**
- 领料单创建成功
- 领料数量与 BOM 标准用量一致
- 领料后库存正确减少
- 领料记录关联到工单

**Verification:** 库存页面显示原材料库存减少，工单中可见领料记录。

---

### U5. M3-M4: 车间报工与成品质检

**Goal:** 车间操作员完成报工，质检员执行成品质检。

**Requirements:** R9, R10

**Dependencies:** U4

**Files:**
- Create: `tests/e2e-q2c/tests/phase3b/test_m3_m4_work_report_qc.sh`

**Approach:**

测试步骤：
1. Agent-M2（`q2c_operator`）登录 → 导航到报工页面（`/admin/mes/report`）
2. 选择工单，填写报工信息：工序 10（注塑）完成数量 100
3. 继续报工工序 20（组装）、工序 30（检验）
4. 断言：报工记录保存成功
5. Agent-Q1 登录 → 导航到质检页面（`/admin/mes/inspection` 或 `/admin/qms/results/create`）
6. 创建质检记录：工单关联、检验数量 100、合格数量 100
7. 断言：质检通过

**Test scenarios:**
- 报工成功，工时记录正确
- 各工序按顺序报工
- 质检记录关联到工单
- 质检结果为"合格"

**Verification:** 工单详情中可见报工和质检记录。

---

### U6. M5: 成品入库

**Goal:** 质检通过后成品入库，工单完工。

**Requirements:** R11

**Dependencies:** U5

**Files:**
- Create: `tests/e2e-q2c/tests/phase3b/test_m5_finished_goods_receipt.sh`

**Approach:**

测试步骤：
1. Agent-W1 登录 → 导航到入库页面（`/admin/wms/stock-in/create`）
2. 创建成品入库单：关联工单，仓库 WH-FG，数量 100
3. 提交入库 → 断言入库成功
4. 断言：WH-FG 库存增加（PRD-FG-001 数量 100）
5. 断言：工单状态为"完工"
6. 写入接力文件：成品入库数量

**Test scenarios:**
- 成品入库成功
- WH-FG 库存正确增加
- 工单状态变更为"完工"
- 入库数量与报工合格数量一致

**Verification:** 库存页面显示 PRD-FG-001 在 WH-FG 中有 100 个可用库存。

---

### U7. 采购+生产并行链路验证

**Goal:** 验证完整的采购+生产并行执行，数据在接力文件中正确传递。

**Requirements:** R1-R11

**Dependencies:** U1-U6

**Files:**
- Create: `tests/e2e-q2c/tests/phase3/test_relay_pu_m.sh`

**Approach:**

整合 U1-U6 为完整脚本，执行顺序：
1. 采购分支：PU1→PU2→PU3→PU4→PU5→PU6（含来料检验）
2. 等待采购分支完成（同步点）
3. 生产分支：M1→M2→M3→M4→M5
4. 合并接力文件
5. 验证：采购收货完成 + 生产成品入库完成

**Test scenarios:**
- 全链路 PU1→M5 无阻断通过
- 并行汇聚点正确同步
- 成品库存满足 SO 需求

**Verification:** 运行完整脚本，输出 "Phase 3 PASSED"，WH-FG 中有 100 个 PRD-FG-001。

---

## Scope Boundaries

### In Scope
- Phase 3A（PU1-PU6）和 Phase 3B（M1-M5）Happy Path
- 6 个 Agent 角色接力
- 采购与生产并行→汇聚

### Deferred to Follow-Up Work
- 超领/退料/代料等异常场景（Plan 04）
- 采购退货/拒收（Plan 04）
- 生产报废/返工（Plan 04）
- 质检不合格/MRB 流程（Plan 04）
