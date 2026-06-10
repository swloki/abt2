---
title: "Q2C E2E 测试 — 03 发货+财务+数据一致性校验"
date: 2026-06-10
type: feat
plan_depth: deep
origin: docs/superpowers/specs/2026-06-10-q2c-e2e-test-nodes.md
depends_on: 2026-06-10-02-feat-q2c-purchase-production-plan.md
---

## Summary

实现 Phase 4（仓储发货 W1-W4）、Phase 5（财务 F1-F6 + 应付 FP1-FP4）的 Happy Path 测试，以及全链路的 12 项数据一致性校验。这是 Q2C 主线的收尾阶段，验证从发货到最终财务结算的完整闭环，并确保业财一体化下的数据一致性。

## Problem Frame

发货和财务是 Q2C 链路的最后两环，也是业财一体化的核心验证点。每一笔发货都必须产生应收凭证，每一笔收货都必须产生应付凭证，最终总账必须借贷平衡。数据一致性校验是整个测试方案的"验收关卡"——如果 CHK 不通过，前面的所有测试都不可信。

---

## Requirements

- R1. W1: 确认成品库存满足 SO 需求，锁定库存
- R2. W2: 创建拣货打包记录
- R3. W3: 执行发货出库，扣减库存，触发应收
- R4. W4: 记录客户签收确认
- R5. F1: 发货/签收后自动生成应收凭证（AR）
- R6. F2: 成本核算（材料+人工+制造费用）
- R7. F3: 开具销售发票
- R8. F4: 记录客户收款
- R9. F5: 对账核销（应收冲收款）
- R10. F6: 总账结算验证
- R11. FP1-FP4: 应付侧完整流程（应付确认→采购发票→付款→核销）
- R12. CHK-01~CHK-12: 全链路数据一致性校验

---

## Key Technical Decisions

KTD1. **财务功能实现度探测** — 财务域标记为 🟡 P1（部分实现），测试脚本需要先探测功能可用性。如果 AR/AP 自动生成未实现，则标记为"未实现跳过"并继续后续步骤。

KTD2. **一致性校验使用 SQL 直查** — CHK 校验直接查询数据库而非通过 UI，因为 UI 可能不展示所有底层细节。校验脚本使用 `psql` 命令执行 SQL。

KTD3. **总账结算（F6）标记为 P2** — 总账结算可能未实现，测试脚本遇到此节点时标记为"未来待测"并跳过，不影响其他节点的验证。

---

## Implementation Units

### U1. W1-W2: 库存确认与拣货

**Goal:** 确认成品库存满足 SO 需求，执行拣货打包。

**Requirements:** R1, R2

**Dependencies:** Plan 02 U6（成品已入库）

**Files:**
- Create: `tests/e2e-q2c/tests/phase4/test_w1_w2_pick_pack.sh`

**Approach:**

测试步骤：
1. 从接力文件读取 `sales_order_id`
2. Agent-W1 登录 → 导航到发货页面（`/admin/shipping/create`）
3. 创建发货申请：关联 SO，选择发货仓库 WH-FG
4. 断言：系统显示可用库存 100，满足 SO 需求
5. 执行拣货/打包操作（如系统支持）
6. 写入接力文件：`shipment_id`

**Test scenarios:**
- 发货申请创建成功
- 系统正确显示可用库存
- 库存被锁定（预留）

**Verification:** 发货列表（`/admin/shipping`）中出现待发货记录。

---

### U2. W3-W4: 发货出库与签收

**Goal:** 执行发货出库，记录客户签收。

**Requirements:** R3, R4

**Dependencies:** U1

**Files:**
- Create: `tests/e2e-q2c/tests/phase4/test_w3_w4_ship_confirm.sh`

**Approach:**

测试步骤：
1. Agent-W1 确认发货 → 触发出库
2. 断言：WH-FG 库存扣减 100 个
3. 断言：发货状态为"已发货"
4. 记录客户签收（如系统支持签收确认）
5. 断言：发货状态为"已签收"
6. 写入接力文件：`shipment_out=true`、`customer_received=true`

**Test scenarios:**
- 发货出库成功
- 库存正确扣减（WH-FG: PRD-FG-001 从 100 变为 0）
- 发货状态流转正确
- 签收确认记录存在

**Verification:** 库存页面显示 WH-FG 的 PRD-FG-001 为 0（或已扣减数量）。

---

### U3. F1-F2: 应收确认与成本核算

**Goal:** 验证发货后应收凭证自动生成，成本核算正确。

**Requirements:** R5, R6

**Dependencies:** U2

**Files:**
- Create: `tests/e2e-q2c/tests/phase5/test_f1_f2_ar_cost.sh`

**Approach:**

测试步骤：
1. Agent-F1（`q2c_accountant`）登录 → 导航到财务/应收页面（`/admin/fms/journals`）
2. 查找与 SO 关联的应收凭证
3. 断言：AR 金额 = 发货数量 × 单价 × (1+税率) = 100 × 1500 × 1.13 = 169,500
4. Agent-F2（`q2c_cost_acct`）登录 → 导航到成本页面（`/admin/fms/cost-control`）
5. 查看产品成本核算结果
6. 断言：成本 = 材料成本 + 人工成本 + 制造费用
7. 写入接力文件：`ar_amount`、`product_cost`

**Test scenarios:**
- AR 凭证存在且金额正确
- 成本分录借贷平衡
- 产品单位成本在合理范围内（参考标准成本 ¥800）

**Verification:** 财务页面显示 AR 凭证，金额与计算一致。

---

### U4. F3-F5: 开票、收款与核销

**Goal:** 开具销售发票，记录收款，执行对账核销。

**Requirements:** R7, R8, R9

**Dependencies:** U3

**Files:**
- Create: `tests/e2e-q2c/tests/phase5/test_f3_f5_invoice_collect.sh`

**Approach:**

测试步骤：
1. Agent-F1 创建销售发票：关联 AR，填写发票信息
2. 断言：发票金额与 AR 一致
3. Agent-F3（`q2c_cashier`）登录 → 记录收款：金额 169,500
4. 断言：收款凭证创建成功
5. Agent-F1 执行核销：AR 冲收款
6. 断言：核销成功，AR 余额为 0
7. 断言：客户信用额度释放
8. 写入接力文件：`invoice_id`、`receipt_amount`、`write_off_done=true`

**Test scenarios:**
- 发票创建成功，金额正确
- 收款记录正确
- 核销后 AR 余额为 0
- 客户信用额度恢复

**Verification:** SO 状态更新为"已结算"或"已完成"，AR 已核销。

---

### U5. FP1-FP4: 应付侧完整流程

**Goal:** 验证采购收货后的应付确认、采购发票、付款、对账核销。

**Requirements:** R11

**Dependencies:** Plan 02 U2（采购收货已完成）

**Files:**
- Create: `tests/e2e-q2c/tests/phase5/test_fp1_fp4_ap_payment.sh`

**Approach:**

测试步骤：
1. Agent-F1 查看 AP 凭证（收货后自动生成）
2. 断言：AP 金额 = 收货数量 × 采购单价 × (1+税率)
3. 创建采购发票（关联 AP 和供应商 SUP-001）
4. Agent-F3 执行付款
5. Agent-F1 执行核销：AP 冲付款
6. 断言：核销成功，AP 余额为 0
7. 写入接力文件：`ap_amount`、`payment_amount`、`ap_write_off_done=true`

**Test scenarios:**
- AP 凭证自动生成
- AP 金额计算正确
- 付款记录正确
- 核销后 AP 余额为 0

**Verification:** 供应商 SUP-001 的应付余额为 0。

---

### U6. F6: 总账结算验证（P2 — 可能未实现）

**Goal:** 验证总账借贷平衡。如果未实现则标记跳过。

**Requirements:** R10

**Dependencies:** U4, U5

**Files:**
- Create: `tests/e2e-q2c/tests/phase5/test_f6_gl_settlement.sh`

**Approach:**

探测性测试：先检查系统是否有总账功能。如果有，验证所有凭证借贷平衡；如果没有，输出 "SKIPPED: F6 not implemented" 并继续。

**Test scenarios:**
- 所有凭证借方合计 = 贷方合计
- 或者：功能未实现，标记跳过

**Verification:** 输出借贷平衡校验结果，或跳过标记。

---

### U7. CHK-01~CHK-12: 全链路数据一致性校验

**Goal:** 执行 12 项跨表数据一致性校验，确保全链路数据无误。

**Requirements:** R12

**Dependencies:** U1-U6 全部完成

**Files:**
- Create: `tests/e2e-q2c/tests/validation/test_chk_all.sh` — 12 项校验主脚本
- Create: `tests/e2e-q2c/tests/validation/sql/chk_01_so_shipping.sql` — SO 与发货一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_02_so_ar.sql` — SO 与 AR 一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_03_po_receipt.sql` — PO 与收货一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_04_po_ap.sql` — PO 与 AP 一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_05_wo_bom.sql` — 工单用料与 BOM 一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_06_wo_cost.sql` — 工单成本归集完整
- Create: `tests/e2e-q2c/tests/validation/sql/chk_07_inventory_balance.sql` — 库存余额正确
- Create: `tests/e2e-q2c/tests/validation/sql/chk_08_inventory_reservation.sql` — 库存预留一致
- Create: `tests/e2e-q2c/tests/validation/sql/chk_09_gl_balance.sql` — 总账借贷平衡
- Create: `tests/e2e-q2c/tests/validation/sql/chk_10_ar_writeoff.sql` — AR 核销完整
- Create: `tests/e2e-q2c/tests/validation/sql/chk_11_ap_writeoff.sql` — AP 核销完整
- Create: `tests/e2e-q2c/tests/validation/sql/chk_12_audit_log.sql` — 审计日志完整

**Approach:**

每项 CHK 是一个独立的 SQL 脚本，查询数据库并输出结果。如果查询返回 0 行（无差异），则 PASS；返回任何行则 FAIL，输出差异明细。

主脚本 `test_chk_all.sh` 按顺序执行 12 个 SQL 脚本，汇总 PASS/FAIL 结果。

校验逻辑示例（CHK-07）：
```sql
-- 验证库存余额：期初 + 入库 - 出库 = 期末
SELECT product_code, warehouse_code,
       opening + COALESCE(received,0) - COALESCE(shipped,0) AS expected,
       current_balance AS actual,
       current_balance - (opening + COALESCE(received,0) - COALESCE(shipped,0)) AS diff
FROM ... WHERE diff <> 0;
-- 预期：0 行返回（无差异）
```

**Test scenarios:**
- CHK-01: SO 明细数量 ≥ 发货数量
- CHK-02: AR 金额 = SO 金额 × (1+税率)
- CHK-03: 收货数量 ≤ PO 数量
- CHK-04: AP 金额 = PO 金额（已收货部分）
- CHK-05: 实际领料 ≈ BOM 标准用量（偏差 < 10%）
- CHK-06: 工单总成本 = 材料+人工+制造费用
- CHK-07: 期初+入库-出库 = 期末
- CHK-08: 预留总额 = 未发货 SO 预留之和
- CHK-09: 总账借方 = 贷方
- CHK-10: AR 已核销 + 未核销余额 = AR 总额
- CHK-11: AP 已核销 + 未核销余额 = AP 总额
- CHK-12: 关键操作（创建/审批/状态变更）有审计记录

**Verification:** 运行 `test_chk_all.sh`，输出 12 项结果全部 PASS。

---

### U8. 全链路 Happy Path 集成验证

**Goal:** 从 S1 到 F6 的完整链路集成测试 + 数据一致性校验。

**Requirements:** R1-R12

**Dependencies:** U1-U7

**Files:**
- Create: `tests/e2e-q2c/tests/full/test_q2c_happy_path.sh` — 全链路主脚本

**Approach:**

整合 Plan 01~03 的所有测试脚本为一个可一键运行的完整链路脚本。执行顺序：
1. 运行 Plan 00 setup（环境初始化）
2. 运行 Phase 1+2（S1→P3）
3. 运行 Phase 3A（PU1→PU6，含来料检验）
4. 运行 Phase 3B（M1→M5，含成品检验和入库）
5. 运行 Phase 4（W1→W4，发货出库）
6. 运行 Phase 5（F1→F6，FP1→FP4，财务闭环）
7. 运行 CHK-01~CHK-12（数据一致性校验）
8. 输出汇总报告

**Test scenarios:**
- 全链路 S1→F6 无阻断通过
- 12 项 CHK 全部 PASS
- SO 状态最终为"已结算"
- 库存最终状态正确（原材料减少，成品出库）
- 财务闭环：AR 和 AP 均已核销

**Verification:** 脚本输出 "Q2C Happy Path PASSED" 和 CHK 汇总。

---

## Scope Boundaries

### In Scope
- Phase 4（W1-W4）Happy Path
- Phase 5（F1-F6 + FP1-FP4）Happy Path
- 12 项数据一致性校验
- 全链路集成验证

### Deferred to Follow-Up Work
- 红字发票/冲销（Plan 04）
- 外币收款/汇兑损益（Plan 04）
- 部分收款/部分核销（Plan 04）
- 跨 SO 核销（Plan 04）
- 期间结账验证（Plan 04）
