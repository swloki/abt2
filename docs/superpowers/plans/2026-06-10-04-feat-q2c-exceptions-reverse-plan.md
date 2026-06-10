---
title: "Q2C E2E 测试 — 04 异常+审批+逆向+通知"
date: 2026-06-10
type: feat
plan_depth: deep
origin: docs/superpowers/specs/2026-06-10-q2c-e2e-test-approval-events.md
depends_on: 2026-06-10-03-feat-q2c-shipping-finance-plan.md
---

## Summary

实现 Batch 2（异常场景 + 审批流测试）和 Batch 3（逆向操作 + 通知验证）的测试，共约 42 个场景。覆盖审批驳回/超时/委托/撤回、库存不足、质检不合格、生产报废、销售退货、红字冲销、付款冲销等关键异常和逆向流程，以及 20 项通知送达验证。

## Problem Frame

ERP 系统的核心在于控制——不是只有 Happy Path，而是在各种异常情况下系统仍能正确处理数据、保护业务规则、提供审计追踪。异常和逆向操作是最容易出 bug 的地方，也是最需要自动化覆盖的地方。

---

## Requirements

- R1. 审批流完整测试：驳回重提、超时升级、委托代理、撤回、会签冲突（8 场景）
- R2. 销售域异常：报价改版、合同变更、订单变更/取消、信用冻结（6 场景）
- R3. 采购域异常：超额采购、PO 变更、超交/短交、拒收（6 场景）
- R4. 生产域异常：超领审批、退料、代料、工序返工、报废审批（5 场景）
- R5. 质量异常：质检不合格→MRB、让步接收、批量报废（3 场景）
- R6. 逆向操作：销售退货、采购退货、红字冲销、付款冲销（4 场景）
- R7. 通知验证：20 项通知规则的正确性验证
- R8. 边界场景：库存不足强行发货、信用超限下单、BOM 缺失（3 场景）

---

## Key Technical Decisions

KTD1. **异常注入策略** — 在 Happy Path 基础上，通过修改输入数据（如将折扣率设为 20%）、替换 Agent 行为（如审批时选择"驳回"而非"通过"）、或预置异常条件（如将客户信用额度设为 0）来注入异常。

KTD2. **独立场景文件** — 每个异常/逆向场景一个独立的测试脚本，不强依赖完整链路。场景使用独立的环境初始化（setup → 注入异常 → 执行测试 → 验证 → 清理）。

KTD3. **通知验证通过数据库查询** — 直接查询通知表验证通知是否生成、内容是否正确、接收人是否正确，而非通过 UI 检查。

---

## Implementation Units

### U1. 审批流异常场景（AP-E1~AP-E8）

**Goal:** 覆盖 8 个审批流异常场景。

**Requirements:** R1

**Dependencies:** Plan 00

**Files:**
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_reject_resubmit.sh` — AP-E1 驳回重提
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_timeout.sh` — AP-E2 超时升级
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_delegate.sh` — AP-E3 委托代理
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_withdraw.sh` — AP-E4 撤回
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_countersign_reject.sh` — AP-E5 会签部分拒绝
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_concurrent.sh` — AP-E6 并行审批竞态
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_data_changed.sh` — AP-E7 审批中数据变更
- Create: `tests/e2e-q2c/tests/exceptions/test_approval_history.sh` — AP-E8 审批历史完整性

**Approach:**

每个场景独立运行：
1. Setup：创建基础数据（如报价单）
2. Inject：注入异常条件（如提交审批）
3. Action：执行异常操作（如驳回、超时等）
4. Assert：验证系统行为（如状态变更、通知发送）
5. Cleanup：清理数据

AP-E2（超时升级）需要特殊处理：将系统超时参数临时设为 1 分钟，或通过 SQL 直接更新审批创建时间模拟超时。

**Test scenarios per file:**
- AP-E1: 驳回后重提 → 新审批实例创建，旧记录保留
- AP-E2: 超时后 → 自动升级到上级审批人
- AP-E3: 代理人审批 → 代理人可见待办，审批记录标注代理
- AP-E4: 提交人撤回 → 审批终止，单据恢复编辑状态
- AP-E5: 会签中一人拒绝 → 整体驳回
- AP-E6: 或签中两人同时审批 → 第一个生效
- AP-E7: 审批中数据被修改 → 审批挂起
- AP-E8: 多次驳回重提 → 历史完整

**Verification:** 每个场景输出 PASS/FAIL，审批状态和记录符合预期。

---

### U2. 销售域异常场景

**Goal:** 覆盖销售域的关键异常场景。

**Requirements:** R2

**Dependencies:** Plan 00

**Files:**
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_credit_freeze.sh` — 信用冻结客户下单
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_order_change.sh` — SO 变更（数量/交期）
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_order_cancel.sh` — SO 取消
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_quotation_expire.sh` — 报价过期
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_contract_change.sh` — 合同变更审批
- Create: `tests/e2e-q2c/tests/exceptions/test_sales_partial_delivery.sh` — 部分交付

**Approach:**

信用冻结场景：预置客户 CUS-002（信用额度 0），尝试创建 SO → 验证被拦截。
SO 取消场景：创建 SO 后取消 → 验证库存预留释放。
SO 变更场景：修改 SO 数量 → 验证 MRP 需求更新。

**Test scenarios:**
- 信用冻结：SO 被拦截或提示信用异常
- SO 变更：修改后数据正确，MRP 重新触发
- SO 取消：预留释放，下游单据清理
- 报价过期：系统提示过期，不能直接转订单

**Verification:** 每个场景输出 PASS/FAIL。

---

### U3. 采购域异常场景

**Goal:** 覆盖采购域的关键异常场景。

**Requirements:** R3

**Dependencies:** Plan 00

**Files:**
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_over_delivery.sh` — 超交处理
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_short_delivery.sh` — 短交处理
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_reject.sh` — 拒收（质量问题）
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_order_change.sh` — PO 变更审批
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_over_budget.sh` — 超额采购审批
- Create: `tests/e2e-q2c/tests/exceptions/test_purchase_single_source.sh` — 单一来源审批

**Approach:**

超交/短交：收货时设置数量与 PO 不一致 → 验证系统处理逻辑。
拒收：来料检验不合格 → 验证退货流程触发。

**Test scenarios:**
- 超交：收货数量 > PO 数量 → 系统拦截或触发审批
- 短交：收货数量 < PO 数量 → PO 未交数量更新
- 拒收：质检不合格 → 退货流程启动
- PO 变更：触发变更审批

**Verification:** 每个场景输出 PASS/FAIL。

---

### U4. 生产+质量异常场景

**Goal:** 覆盖生产报废、超领、返工和质检不合格场景。

**Requirements:** R4, R5

**Dependencies:** Plan 00

**Files:**
- Create: `tests/e2e-q2c/tests/exceptions/test_production_over_issue.sh` — 超领审批
- Create: `tests/e2e-q2c/tests/exceptions/test_production_return_material.sh` — 退料
- Create: `tests/e2e-q2c/tests/exceptions/test_production_rework.sh` — 返工报工
- Create: `tests/e2e-q2c/tests/exceptions/test_production_scrap.sh` — 报废审批
- Create: `tests/e2e-q2c/tests/exceptions/test_quality_reject_mrb.sh` — 质检不合格→MRB
- Create: `tests/e2e-q2c/tests/exceptions/test_quality_concession.sh` — 让步接收
- Create: `tests/e2e-q2c/tests/exceptions/test_quality_batch_scrap.sh` — 批量报废

**Approach:**

质检不合格→MRB：在报工完成后，质检时注入不合格结果 → 验证 MRB 流程启动 → 不合格品移入隔离仓。
报废审批：报废数量 > 工单 5% → 触发会签审批（质量+生产+财务）。

**Test scenarios:**
- 超领：领料超出 BOM 用量 → 触发审批
- 报废：报废品入废品仓 + 成本重算
- MRB：不合格品隔离 + 评审决定（返工/让步/报废）
- 让步接收：客户通知 + 价格调整

**Verification:** 每个场景输出 PASS/FAIL。

---

### U5. 逆向操作场景

**Goal:** 覆盖退货、冲销等逆向操作。

**Requirements:** R6

**Dependencies:** Plan 03（需要完整的 Happy Path 数据）

**Files:**
- Create: `tests/e2e-q2c/tests/reverse/test_sales_return.sh` — 销售退货
- Create: `tests/e2e-q2c/tests/reverse/test_purchase_return.sh` — 采购退货
- Create: `tests/e2e-q2c/tests/reverse/test_invoice_reversal.sh` — 红字冲销
- Create: `tests/e2e-q2c/tests/reverse/test_payment_reversal.sh` — 付款冲销

**Approach:**

销售退货：基于已完成的 SO 和签收 → 创建退货申请（`/admin/returns/new`）→ 仓库收到退货 → 质检 → 库存回仓（待检）→ AR 冲减。

采购退货：基于已完成的收货 → 创建退货 → 库存扣减 → AP 冲减 → 供应商退款。

红字冲销：基于已开发票 → 创建红字发票 → AR 负数冲抵。

**Test scenarios:**
- 销售退货：退货流程完整，AR 冲减正确
- 采购退货：退货后库存和 AP 正确更新
- 红字冲销：红字发票金额正确，AR 余额更新
- 付款冲销：银行退回，AP 恢复

**Verification:** 每个场景输出 PASS/FAIL，验证财务数据一致性。

---

### U6. 通知送达验证（N1-N20）

**Goal:** 验证 20 项通知规则的正确性。

**Requirements:** R7

**Dependencies:** Plan 00 + Happy Path 各节点

**Files:**
- Create: `tests/e2e-q2c/tests/notifications/test_notifications.sh` — 通知验证主脚本

**Approach:**

通知验证集成在 Happy Path 执行过程中。每完成一个业务节点，查询通知表验证：
1. 通知是否生成
2. 接收人是否正确
3. 通知内容是否包含必要信息（单号、金额等）
4. 通知渠道是否符合规则（站内信/邮件/企微）

验证方式：通过 SQL 查询通知表，而非 UI 检查。

验证矩阵（源自设计文档 `2026-06-10-q2c-e2e-test-approval-events.md` 第 3.2 节）：
- N1: SO 创建 → 计划员、仓管收到通知
- N2: MRP 完成 → 采购、生产收到通知
- N3-N6: 采购各节点通知
- N7-N9: 生产/质检通知
- N10-N11: 入库/发货通知
- N12-N15: 财务通知
- N16-N17: 审批通知
- N18-N20: 预警通知（库存不足、信用超限、成本差异）

**Test scenarios:**
- 每个 N 编号对应一个通知验证点
- 验证通知存在且内容正确
- 验证通知不重复（幂等性）
- 验证通知级别与渠道匹配

**Verification:** 输出 20 项通知验证结果 PASS/FAIL。

---

### U7. 边界条件场景

**Goal:** 覆盖关键边界条件。

**Requirements:** R8

**Dependencies:** Plan 00

**Files:**
- Create: `tests/e2e-q2c/tests/exceptions/test_boundary_insufficient_stock.sh` — 库存不足发货
- Create: `tests/e2e-q2c/tests/exceptions/test_boundary_credit_exceeded.sh` — 信用超限下单
- Create: `tests/e2e-q2c/tests/exceptions/test_boundary_bom_missing.sh` — BOM 缺失产品报价

**Approach:**

预置极端条件（库存清零、信用额度设为极低值、删除 BOM），验证系统的拦截和保护机制。

**Test scenarios:**
- 库存不足：发货被拦截或提示缺货
- 信用超限：SO 被拦截或触发信用审批
- BOM 缺失：报价被阻止或标记待补充

**Verification:** 系统正确拦截并给出明确错误提示。

---

### U8. 异常+逆向+通知 全量集成验证

**Goal:** 汇总所有异常场景测试结果，生成完整测试报告。

**Requirements:** R1-R8

**Dependencies:** U1-U7

**Files:**
- Create: `tests/e2e-q2c/tests/full/test_q2c_all_exceptions.sh` — 异常全量主脚本

**Approach:**

按顺序执行 U1-U7 的所有测试脚本，汇总结果：
- 审批流异常：8 场景
- 销售域异常：6 场景
- 采购域异常：6 场景
- 生产+质量异常：7 场景
- 逆向操作：4 场景
- 通知验证：20 项
- 边界条件：3 场景

输出汇总报告：通过/失败/跳过计数，按模块和级别分类。

**Test scenarios:**
- 所有场景按预期 PASS 或 FAIL（Fail 记录为缺陷，不阻塞后续场景）
- 跳过的场景（未实现功能）正确标记

**Verification:** 输出完整测试报告，含通过率和缺陷清单。

---

## Scope Boundaries

### In Scope
- 8 审批流异常 + 6 销售异常 + 6 采购异常 + 7 生产质量异常 + 4 逆向操作 + 20 通知验证 + 3 边界条件
- 每个场景独立可执行

### Deferred to Follow-Up Work
- 压力测试（大批量数据并发）
- 跨期间财务结转
- 多币种汇率场景
- 外部系统集成测试（邮件/企微实际发送）
