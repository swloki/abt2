---
title: "Q2C E2E 测试 — 01 销售+计划 Happy Path"
date: 2026-06-10
type: feat
plan_depth: standard
origin: docs/superpowers/specs/2026-06-10-q2c-e2e-test-nodes.md
depends_on: 2026-06-10-00-feat-q2c-infrastructure-plan.md
---

## Summary

实现 Quote-to-Cash 主线的 Phase 1（销售域 S1-S5）和 Phase 2（计划域 P1-P3）的 Happy Path 测试：从客户询价 → 销售报价 → 报价审批 → 销售合同 → 销售订单 → BOM 展开 → MRP 运算 → 需求分解，共 8 个节点。测试通过 agent-browser 模拟销售专员、销售经理、计划员三个角色的接力操作。

## Problem Frame

这是 Q2C 全链路的起始阶段。销售订单是所有下游业务（采购、生产、发货、财务）的触发源头，MRP 运算将销售需求分解为采购需求和生产建议。如果这一阶段的数据传递有误，整个链路都会失败。

---

## Requirements

- R1. S1: 销售专员创建询价/报价（`/admin/quotations/new`）
- R2. S2: 报价单填写产品、数量、单价、折扣、税率，保存为草稿
- R3. S3: 报价审批——折扣率 > 15% 时触发审批流，销售经理审批通过
- R4. S4: 审批通过后报价转销售合同（如系统支持），或直接转销售订单
- R5. S5: 从合同/报价创建销售订单（`/admin/orders/create`），触发下游 MRP
- R6. P1: 计划员查看 SO 关联的 BOM 展开（如系统支持 MRP 自动展开）
- R7. P2: 执行 MRP 运算，生成采购需求 + 生产工单建议
- R8. P3: 需求分解完成，接力数据传递给采购和生产分支

---

## Key Technical Decisions

KTD1. **报价→订单的路径取决于系统实现** — 如果系统支持"报价直接转订单"（一键转换），则走该路径；如果不支持，则手动创建订单并引用报价信息。执行时需要先探测系统实际行为。

KTD2. **MRP 触发方式** — SO 创建后 MRP 是否自动触发？如果是，测试验证自动触发；如果需要手动触发，测试手动操作。根据系统实际行为调整测试步骤。

KTD3. **接力数据契约** — S5 完成后接力文件必须包含：`sales_order_id`、`customer_id`、`product_id`、`quantity`、`total_amount`。P3 完成后接力文件必须包含：`purchase_request_ids[]`、`work_order_suggestion_ids[]`。

---

## Implementation Units

### U1. S1-S2: 销售报价创建

**Goal:** 验证销售专员可以成功创建报价单，填写产品和价格信息。

**Requirements:** R1, R2

**Dependencies:** Plan 00 全部完成

**Files:**
- Create: `tests/e2e-q2c/tests/phase1/test_s1_s2_quotation.sh`

**Approach:**

测试步骤：
1. 以 `q2c_sales`（Agent-S1）登录
2. 导航到 `/admin/quotations/new`
3. 选择客户 CUS-001（通过搜索或下拉）
4. 添加明细行：产品 PRD-FG-001，数量 100，单价 1500
5. 填写折扣率 10%（不触发审批）
6. 填写有效期和交付条款
7. 保存报价
8. 断言：页面显示"创建成功"，获取报价单号
9. 写入接力文件：`quotation_id`

**Patterns to follow:** 参考 `.claude/skills/page-test/SKILL.md` 中 `form-test.md` 的 7 步表单测试流程。

**Test scenarios:**
- Happy path：完整填写所有必填字段 → 报价创建成功
- 验证报价单号格式正确（QT-YYYY-NNN）
- 验证明细行数据（产品、数量、单价）与输入一致
- 验证报价状态为"草稿"或"待审批"

**Verification:** 报价列表页（`/admin/quotations`）中出现刚创建的报价单。

---

### U2. S3: 报价审批

**Goal:** 验证报价审批流程——提交审批后销售经理可以审批通过。

**Requirements:** R3

**Dependencies:** U1

**Files:**
- Create: `tests/e2e-q2c/tests/phase1/test_s3_approval.sh`

**Approach:**

分两个阶段：
1. Agent-S1 提交报价审批（修改折扣率为 20% 以触发审批，或直接提交待审批报价）
2. Agent-S2（`q2c_sales_mgr`）在审批待办中找到该报价，执行审批通过

**Test scenarios:**
- Agent-S1 提交审批 → 报价状态变为"待审批"
- Agent-S2 登录 → 在审批列表中看到待审批报价
- Agent-S2 审批通过 → 报价状态变为"已生效"
- 验证审批记录包含审批人和审批时间

**Verification:** 以 Agent-S1 查看报价详情，状态为"已生效"。

---

### U3. S4-S5: 销售订单创建

**Goal:** 从报价/合同创建销售订单，触发下游需求。

**Requirements:** R4, R5

**Dependencies:** U2

**Files:**
- Create: `tests/e2e-q2c/tests/phase1/test_s4_s5_sales_order.sh`

**Approach:**

测试步骤：
1. Agent-S1 导航到 `/admin/orders/create`
2. 选择客户 CUS-001
3. 添加明细行：产品 PRD-FG-001，数量 100，单价 1500
4. 填写交付日期（未来 30 天）
5. 提交销售订单
6. 断言：订单创建成功，状态为"已确认"
7. 验证：库存预留是否已创建（如果有预留机制）
8. 写入接力文件：`sales_order_id`、`quantity`、`total_amount`

**Test scenarios:**
- Happy path：创建销售订单成功
- 验证 SO 单号格式正确
- 验证明细行数据与输入一致
- 验证 SO 状态为"已确认"或"待处理"
- 验证客户信用额度检查通过

**Verification:** 订单列表页（`/admin/orders`）中出现新订单，状态正确。

---

### U4. P1-P3: MRP 运算与需求分解

**Goal:** 计划员基于 SO 执行 MRP 运算，生成采购需求和生产建议。

**Requirements:** R6, R7, R8

**Dependencies:** U3

**Files:**
- Create: `tests/e2e-q2c/tests/phase2/test_p1_p3_mrp.sh`

**Approach:**

测试步骤：
1. Agent-P1（`q2c_planner`）登录
2. 导航到生产计划或 MRP 页面（`/admin/mes/plans`）
3. 查看与 SO 关联的需求
4. 执行 MRP 运算（如果有手动触发功能）
5. 等待运算完成
6. 查看运算结果：采购需求（外购件）和生产建议（自制件）
7. 写入接力文件：`purchase_request_ids`、`work_order_suggestion_ids`

**Test scenarios:**
- MRP 运算成功完成
- 运算结果包含采购需求（PRD-RM-001/002/003）
- 运算结果包含生产建议（PRD-FG-001，可能包含 PRD-SFG-001）
- 需求数量与 SO 数量（100）× BOM 用量一致
- BOM 展开正确（多级展开）

**Verification:** MRP 结果页面显示采购和生产建议，数据与 BOM 计算一致。

---

### U5. Phase 1+2 接力链路验证

**Goal:** 验证从 S1 到 P3 的完整接力链路，数据在 Agent 间正确传递。

**Requirements:** R1-R8

**Dependencies:** U1, U2, U3, U4

**Files:**
- Create: `tests/e2e-q2c/tests/phase1/test_relay_s1_p3.sh` — 完整接力链路脚本

**Approach:**

整合 U1-U4 为一个完整脚本，按顺序执行 S1→S2→S3→S4→S5→P1→P2→P3，每个节点完成后写入接力文件，下一个节点读取并验证。如果任一节点失败，记录失败点并停止。

**Test scenarios:**
- 全链路 S1→P3 无阻断通过
- 每个节点的接力数据正确传递
- 接力文件中 `sales_order_id` 从 S5 写入后，在 P1 中被正确读取
- 接力文件中 `purchase_request_ids` 在 P3 完成后非空

**Verification:** 运行完整脚本，输出 "Phase 1+2 PASSED"，接力文件包含所有预期的 artifacts。

---

## Scope Boundaries

### In Scope
- Phase 1（S1-S5）和 Phase 2（P1-P3）的 Happy Path
- 3 个 Agent 角色的接力操作
- 接力数据传递验证

### Deferred to Follow-Up Work
- 报价审批驳回、重提等异常场景（Plan 04）
- SO 变更、取消等异常场景（Plan 04）
- 信用额度冻结场景（Plan 04）
- MRP 重跑、需求合并等边界场景（Plan 04）

---

## Open Questions

- Q1. 系统是否支持"报价一键转订单"？还是需要手动创建订单并关联报价？这将影响 S4→S5 的测试步骤。
- Q2. MRP 运算完成后，结果在哪里查看？是否有专门的 MRP 结果页面？还是在生产计划页面中？
- Q3. BOM 展开是否在 MRP 中自动执行？还是需要单独操作？
