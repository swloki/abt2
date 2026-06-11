# Q2C E2E 集成冒烟测试 — 覆盖度审查报告

> **审查日期**：2026-06-10
> **审查范围**：`tests/e2e-q2c/` 全部测试脚本 + `docs/superpowers/plans/` 5 份计划文档 + `docs/superpowers/specs/` 4 份规格文档 + `lib/` 工具库 + `validation/sql/` CHK SQL
> **审查方法**：逐文件对比 Plan→Implementation、Spec→Test 的一致性，SQL 逻辑审查，工具库完整性检查

---

## 一、计划 vs 实现总览

| 计划 | 状态 | 备注 |
|---|---|---|
| **Plan 00** 基础设施 | ⚠️ 基本完成 | 缺 04_system_config.sql、无报告生成、无质量门禁 |
| **Plan 01** 销售+计划 | ✅ 全部完成 | U1-U5 均已实现 |
| **Plan 02** 采购+生产 | ⚠️ 基本完成 | PU6 来料检验被跳过、并行 relay 未拆分 |
| **Plan 03** 发货+财务 | 🔴 有关键 Bug | Happy Path 漏跑 W1-W2 + CHK SQL 降级 |
| **Plan 04** 异常+逆向+通知 | ⚠️ 部分完成 | 4 个审批桩 + 通知编号错位 + 30 个 Spec 场景缺失 |

---

## 二、🔴 关键 Bug（直接违反 Plan 定义）

### BUG-1: Happy Path 漏跑 W1-W2 拣货测试

**违反计划**：Plan 03 U8 明确定义"5. 运行 Phase 4（W1→W4，发货出库）"

**实际代码** (`tests/full/test_q2c_happy_path.sh`):

```bash
# 只执行了 W3-W4，跳过了 W1-W2
run_step 11 "W1-W4 发货与签收" "$PHASE4/test_w3_w4_ship_confirm.sh" || goto_summary
```

`test_w1_w2_pick_pack.sh` 文件存在但从未被 Happy Path 调用。

**影响**：
- 发货申请创建、库存确认、拣货操作在主链路中从未执行
- W3-W4 依赖 `shipping_request_id`（由 W1-W2 写入 relay），直接跳到 W3-W4 大概率失败

**修复方案**：将 Phase 4 拆为两步

```bash
run_step 11 "W1-W2 拣货" "$PHASE4/test_w1_w2_pick_pack.sh" || goto_summary
run_step 12 "W3-W4 发货签收" "$PHASE4/test_w3_w4_ship_confirm.sh" || goto_summary
```

并将 `TOTAL_STEPS` 调整，后续步骤编号顺延。

---

### BUG-2: `goto_summary` 函数定义在引用之后

`test_q2c_happy_path.sh` 中 `goto_summary` 函数定义在文件末尾（~第 145 行），但在 `run_step ... || goto_summary` 中被引用（第 108-120 行）。Bash 要求函数必须先定义后调用——当任何步骤失败时，`goto_summary` 会触发 "command not found" 错误，导致整个脚本崩溃而非优雅降级。

**修复方案**：将 `goto_summary` 函数定义移到第一个 `run_step` 调用之前。

---

### BUG-3: Happy Path 始终 exit 0

```bash
if [[ $FAIL_COUNT -eq 0 ]]; then
    exit 0
else
    exit 0  # 财务域失败不视为整体失败
fi
```

无论 PASS/FAIL，脚本都返回 exit 0。这完全破坏了 CI/CD 集成能力——CI pipeline 永远显示绿色。

**修复方案**：`FAIL_COUNT > 0` 时 `exit 1`，或至少对 P0 节点（Phase 1-4）的失败返回非零退出码。

---

### BUG-4: CHK-09 总账借贷平衡硬编码 PASS

```sql
-- CHK-09: 总账借贷平衡
-- 注: journal_entries 表尚未创建，使用空查询（自动 PASS）
SELECT 1 WHERE false;
```

`SELECT 1 WHERE false` 永远返回 0 行，CHK-09 在任何环境下都会 PASS。这违反了质量标准（quality-criteria.md GATE-03："总账借贷平衡：所有会计分录借贷相等"）。

**影响**：业财一体化最关键的验证点（借贷平衡）被绕过，整个数据一致性校验的可信度打折。

---

## 三、🟡 CHK SQL 逻辑降级（规格定义 vs 实际 SQL）

12 个 CHK SQL 中，多个存在逻辑降级——SQL 只验证了 Spec 要求的一个子集，且容忍阈值远超标准：

| CHK | Spec 定义 | 实际 SQL 验证内容 | 问题 |
|---|---|---|---|
| **CHK-02** SO 与 AR 一致 | "AR 金额 = SO 金额 × (1+税率)" | 只验证 SO header vs SO items 内部一致 | **完全没有验证 AR**，名字叫"SO 与 AR"但实际不查 AR |
| **CHK-04** PO 与 AP 一致 | "AP 金额 = PO 金额（已收货部分）" | 只验证 PO header vs PO items 内部一致 | **完全没有验证 AP**，同上 |
| **CHK-05** 工单用料与 BOM | "偏差 < 10%" | 偏差 > 50% 才报 | **阈值放宽 5 倍**，49% 偏差也能 PASS |
| **CHK-06** 工单成本归集 | "总成本 = 材料 + 人工 + 制造费用" | 只检查工单有有效 planned_qty | **完全没有验证成本归集**，名字叫"成本归集完整"但只检查数量字段 |
| **CHK-07** 库存余额正确 | "期初 + 入库 - 出库 = 期末" | 只检查无负库存 | **完全没有验证余额方程**，只检查 quantity < 0 |
| **CHK-09** 总账借贷平衡 | "所有凭证借方 = 贷方" | `SELECT 1 WHERE false` | **硬编码 PASS**（见 BUG-4） |
| **CHK-12** 审计日志完整 | "关键操作有审计记录" | 检查业务表有无数据 | **完全没有查 audit_log 表**，只检查业务表 COUNT > 0 |

**CHK 有效覆盖率：12 个中仅 5 个（CHK-01/03/08/10/11）真正匹配 Spec 定义，其余 7 个降级或空壳。**

---

## 四、🟡 Plan 04 审批异常空壳桩（4 个）

**违反计划**：Plan 04 U1 定义 8 个审批异常场景，每个都有 "Setup→Inject→Action→Assert→Cleanup" 五步流程。

| 文件 | Plan 编号 | Plan 要求的预期行为 | 实际实现 |
|---|---|---|---|
| `test_approval_data_changed.sh` | AP-E7 | 审批中数据被修改 → 审批自动挂起 | 仅检查表是否存在，直接 `assert_pass` |
| `test_approval_history.sh` | AP-E8 | 多次驳回重提 → 历史完整按时间排列 | 同上 |
| `test_approval_concurrent.sh` | AP-E6 | 或签中两人同时审批 → 第一个生效 | 同上 |
| `test_approval_countersign_reject.sh` | AP-E5 | 会签中一人拒绝 → 整体驳回，待办清除 | 同上 |

**影响**：这 4 个脚本在任何环境下都会 PASS，虚假覆盖。表面审批异常覆盖率 8/8，实际 4/8。

**修复方案**：按 Plan U1 定义的五步流程重写 4 个脚本。

---

## 五、🟡 违反 Plan 定义的实现偏差

### DEVIATION-1: PU6 来料检验（IQC）被跳过

**Plan 02 U2 定义**：
> "Agent-Q1（q2c_qc）执行来料检验 → 记录合格"
> "质检通过后库存可用于生产领料"

**实际代码** (`tests/phase3a/test_pu5_pu6_goods_receipt.sh`):
```bash
# PU6: 来料检验（跳过 — 非关键路径）
```

仅做可选探测，没有验证质检流程。质量域的 `q2c_qc` 角色和 WH-QC 待检仓都已创建，系统支持此流程。

---

### DEVIATION-2: 并行 Relay 未拆分

**Plan 02 KTD3 定义**：
> "并行执行时使用两个接力文件：relay-purchase.json 和 relay-production.json，汇聚时合并到主 relay-state.json"

**实际实现**：只使用了一个 `relay-state.json`。`relay.sh` 中 `relay_init_branch` / `relay_merge_branch` 等并行分支函数已实现但从未被调用。

---

### DEVIATION-3: 通知测试未嵌入 Happy Path + 编号错位

**Plan 04 U6 定义**：
> "通知验证集成在 Happy Path 执行过程中。每完成一个业务节点，查询通知表验证"

**实际实现**：`test_notifications.sh` 是独立脚本，不在 Happy Path 链路中。

**编号错位问题**：通知规则的编号与 Spec 的通知矩阵不一致：

| Spec N 编号 | Spec 定义 | 测试 N 编号 | 测试定义 |
|---|---|---|---|
| N1 | SO 创建 → 计划员、仓管 | N1 | 报价待审批 → 销售经理 |
| N2 | MRP 完成 → 采购、生产 | N2 | 报价已审批 → 销售专员 |
| N3 | 采购 PO 发送 → 供应商 | N3 | 报价已拒绝 → 销售专员 |
| ... | ... | ... | ... |

测试重新编排了 N1-N20 的内容（以"报价→订单→采购→生产→发货→财务"顺序），而 Spec 以事件类型分类。编号对不上，无法一一对应验证。

---

### DEVIATION-4: 缺少 04_system_config.sql 系统配置预置

**Spec data-env.md 第 2.2 节定义**：

```
fixtures/
├── 01_master_data.sql
├── 02_users_and_roles.sql
├── 03_initial_inventory.sql
├── 04_system_config.sql      # ← 系统参数（审批阈值、安全库存等）
└── 99_cleanup.sql
```

**实际实现**：`04_system_config.sql` 不存在。测试依赖的系统参数（折扣率 > 15% 触发审批、PO 金额 > 10 万触发审批、安全库存阈值等）从未被预置，完全依赖系统默认值。

---

### DEVIATION-5: 断言类型不完整

**Spec agent-strategy.md 第 4.3 节定义了 6 种断言类型**：

| 断言类型 | 说明 | 实现状态 |
|---|---|---|
| **页面断言** | 验证页面元素状态 | ✅ `abt_assert_visible/text/url/toast` |
| **数据断言** | 验证数据库数据 | ✅ `abt_assert_db/db_empty` |
| **事件断言** | 验证 Event Bus 事件是否触发 | ❌ 无 `abt_assert_event` |
| **通知断言** | 验证通知是否发送 | ❌ 无 `abt_assert_notification` |
| **财务断言** | 验证会计分录平衡 | ❌ 无 `abt_assert_accounting` |
| **审计断言** | 验证审计日志 | ❌ 无 `abt_assert_audit` |

缺少 4 种断言类型，无法验证 Event Bus 投递、通知送达、分录平衡、审计完整。

---

## 六、🟡 规格文档（Specs）定义但完全缺失的测试场景

规格 `docs/superpowers/specs/2026-06-10-q2c-e2e-test-nodes.md` 定义了 25 个主线节点 + 62 个异常分支 = 87 场景。以下按域列出有明确定义但测试完全缺失的场景。

### 6.1 销售域（缺失 2 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| S2-E4 | 报价版本管理 | 多版本文档，版本号递增，旧版本自动失效 |
| S4-E2 | 合同终止 | 正确清理关联的报价和库存预留 |

### 6.2 计划域（缺失 6 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| P1-E1 | BOM 包含已停产物料 | 提示替代料或阻止 |
| P1-E2 | 多级 BOM 循环引用 | 系统检测并阻止 |
| P2-E1 | MRP 合并多个 SO 同类物料需求 | 验证合并逻辑 |
| P2-E2 | 安全库存触发补货建议 | 验证补货数量计算 |
| P2-E3 | MRP 重跑（需求变更后） | 旧建议取消，新建议生成 |
| P3-E2 | 委外件识别 | 自制/外协/采购判断逻辑验证 |

### 6.3 采购域（缺失 4 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| PU1-E1 | MRP 建议被手动调整数量 | 验证调整记录 |
| PU2-E2 | 最低价供应商不在合格名录 | 验证阻止逻辑 |
| PU3-E2 | 审批驳回后修改供应商 | 验证是否需要重新比价 |
| PU4-E2 | PO 取消 | 到货通知清理、应付清理 |

### 6.4 生产域（缺失 4 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| M1-E2 | 工单暂停/恢复 | 状态流转和通知 |
| M2-E3 | 代料（替代物料） | 替代料记录和成本差异 |
| M3-E1 | 报工数量与领料不匹配 | 用量差异预警 |
| M3-E2 | 工序跳过（免检工序） | 自动流转到下一工序 |

### 6.5 仓储发货（缺失 5 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| W1-E2 | 库存被其他 SO 锁定 | 分配优先级规则 |
| W2-E1 | 拣货数量错误 | 拣货校验和纠错 |
| W2-E2 | 打包时发现破损 | 触发质量复检 |
| W3-E1 | 超额发货 | 触发审批 |
| W3-E2 | 发货后客户要求暂缓 | 拦截和库存处理 |

### 6.6 质量域（缺失 3 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| M5-E1 | 入库数量与报工数量不符 | 差异处理 |
| M5-E2 | 部分入库（分批完工） | 工单部分完工状态 |
| W4-E2 | 客户部分签收（短收） | 差异处理和补发流程 |

### 6.7 财务域（缺失 6 个）

| 规格编号 | 场景描述 | 预期行为 |
|---|---|---|
| F1-E1 | AR 金额与 SO 不一致 | 差异预警 |
| F2-E1 | 实际成本超标准成本 >10% | 成本差异预警 |
| F2-E2 | 在制品成本（WIP）计算 | 未完工工单成本归集 |
| F3-E2 | 开票金额与应收不一致 | 差异处理 |
| F4-E1 | 部分收款 | 部分核销逻辑 |
| F4-E2 | 多收/溢缴 | 预收账款处理 |

**Spec 异常场景缺失合计：30 个**

---

## 七、🟡 事件链验证缺失

规格 `docs/superpowers/specs/2026-06-10-q2c-e2e-test-approval-events.md` 定义了 5 条核心事件链（EC-1~EC-5），每条都有跨模块验证点。当前测试只验证单节点操作，没有测试验证事件链的跨模块传播。

| 事件链 | 核心验证点 | 当前状态 |
|---|---|---|
| **EC-1**: SO创建 → 库存预留 → 通知计划员 → 触发MRP | 预留记录、MRP来源SO、通知内容 | ❌ 无集成验证 |
| **EC-2**: MRP完成 → 采购需求 + 工单建议并行 | 并行通知、需求覆盖100% | ❌ 无集成验证 |
| **EC-3**: 收货 → 来料QC → 库存可用 → AP生成 | 待检→可用状态流转、暂估AP转正式 | ❌ 无集成验证 |
| **EC-4**: 报工 → 成品质检 → 入库 → 成本归集 | 工时归集、成本=材料+人工+制费 | ❌ 无集成验证 |
| **EC-5**: 发货 → AR → 开票 → 收款 → 核销 → SO已结算 | 自动核销、信用释放、SO状态链完整 | ❌ 无集成验证 |

Spec 还定义了每条事件链的验证矩阵（4-6 个验证点）和事件触发延迟标准（< 5s），均未实现。

---

## 八、🟡 基础设施层遗漏

### INFRA-1: 无测试报告生成

**Spec quality-criteria.md 第 5.3 节定义了 9 章测试报告**：

1. 执行摘要
2. 场景执行明细
3. 接力链路追踪
4. 数据一致性报告
5. 财务闭环报告
6. 审批流报告
7. 事件与通知报告
8. 缺陷清单
9. 建议与优化

**实际实现**：没有任何报告生成脚本。`tests/full/` 目录下无 `reports/` 目录，Spec 定义的 `reports/{run_id}/` 结构不存在。

---

### INFRA-2: 无质量门禁执行

**Spec quality-criteria.md 第 5.1 节定义了 5 个一票否决项**：

| 门禁 | 条件 | 实现状态 |
|---|---|---|
| GATE-01 | Happy Path 全链路跑通 | ❌ 始终 exit 0（见 BUG-3） |
| GATE-02 | 12 项跨表校验全部通过 | ❌ CHK-09 硬编码 PASS |
| GATE-03 | 总账借贷平衡 | ❌ CHK-09 空壳 |
| GATE-04 | 无 Critical 级别缺陷 | ❌ 无缺陷分级机制 |
| GATE-05 | 库存不出现负数 | ⚠️ CHK-07 部分覆盖（只查负数） |

没有任何门禁检查脚本，GATE 判定流程（quality-criteria.md 第 5.4 节）未实现。

---

### INFRA-3: 无数据快照机制

**Spec data-env.md 第 4 节定义了 5 个关键节点快照**：

| 快照点 | 时机 | 用途 |
|---|---|---|
| SNAP-0 | 测试开始前 | 主数据基线 |
| SNAP-1 | SO 创建后 | 订单数据 + 库存预留状态 |
| SNAP-2 | MRP 完成后 | 需求分解结果 + 库存快照 |
| SNAP-3 | 生产完工后 | 工单成本 + 库存快照 |
| SNAP-4 | 发货完成后 | 库存快照 + AR 凭证 |
| SNAP-5 | 结算完成后 | 全量数据快照（用于最终对账） |

`relay.sh` 有 `relay_snapshot` 函数，但它只写入一个标记字符串到 relay 文件，**不创建任何数据快照**。无法在异常发生时回溯历史状态。

---

### INFRA-4: 无环境隔离

**Spec data-env.md 第 3 节定义了环境隔离策略**（Schema 隔离或数据隔离），但实际测试直接操作主数据库 `abt_v2`，与开发/生产数据共存。多轮测试间的数据隔离完全依赖 `99_cleanup.sql` 的完整性。

---

### INFRA-5: Session 无过期恢复

**Spec agent-strategy.md 第 4.1 节定义**：
> "错误恢复（Session 过期重新登录）"

`session.sh` 有 `init_all_sessions` 和 `verify_sessions_ready` 但没有自动重新登录机制。如果长时间测试中 Session 过期，后续操作会静默失败。

---

### INFRA-6: Agent-QM1 和 Agent-GM 角色未被使用

**Spec agent-strategy.md 定义了 15 个 Agent**，其中 Agent-QM1（质量主管 `q2c_qc_mgr`）和 Agent-GM（总经理 `q2c_gm`）在 MRB 评审、报废审批、高金额终审等场景中扮演关键角色。但 fixture 创建了这两个用户，**没有任何测试脚本使用他们**。

---

## 九、🟡 结构与质量问题

### ISSUE-1: 接力数据只验证 Key 存在，不验证值

`test_relay_s1_p3.sh` 只检查 key 存在：
```bash
KEYS=("quotation_id" "quotation_status" "sales_order_id" "work_order_id")
```
缺少对值的合理性校验（如 `quotation_id > 0`，`quotation_status == "accepted"`）。

### ISSUE-2: 半成品 B 无独立生产链路

BOM 有两级展开（成品A → 半成品B → 原材料C），但只创建了一个工单（成品A）。如果系统需要先生产半成品B再组装成品A，单工单方案无法验证多级 BOM 联动。

### ISSUE-3: 缺少权限反向验证

15 个角色 + 精细权限已创建（01_users_and_roles.sql），但只测试了"正确角色做正确操作"。没有验证越权操作被拦截：
- 操作员（q2c_operator）无法审批报价
- 仓管员（q2c_warehouse）无法创建采购订单
- 质检员（q2c_qc）无法执行付款

### ISSUE-4: 供应商 SUP-002 未被使用

fixture 创建了 SUP-001（主力）和 SUP-002（备选），但所有测试只用了 SUP-001。多供应商比价、分配采购量等场景完全缺失。

### ISSUE-5: 税额计算未验证

报价 100 × ¥1500 × 0.9 = ¥135,000，但没有测试验证增值税（13%）计算是否正确，发票金额是否含税。Plan 03 U3 明确写了 "AR 金额 = 100 × 1500 × 1.13 = 169,500"。

### ISSUE-6: 接力文件 Spec 定义的字段未完全写入

**Spec agent-strategy.md 第 2.2 节定义了完整的接力数据结构**，包括 `context`（客户名/产品名/数量/单价/折扣率/税率）和 `next_agent`/`next_action`。实际写入 relay 的只有 `quotation_id`、`sales_order_id` 等少量字段，`context` 和 `next_*` 字段从未写入。

---

## 十、Plan 明确 Deferred 的场景（不在遗漏范围内）

以下场景在 Plan 03/04 的 "Deferred to Follow-Up Work" 中明确标注为延迟实现，不属于遗漏：

| 场景 | 来源 |
|---|---|
| 外币收款/汇兑损益 | Plan 03 Scope |
| 跨 SO 核销 | Plan 03 Scope |
| 部分收款/部分核销 | Plan 03 Scope |
| 期间结账验证 | Plan 03 Scope |
| 压力测试（大批量并发） | Plan 04 Scope |
| 跨期间财务结转 | Plan 04 Scope |
| 多币种汇率场景 | Plan 04 Scope |
| 外部系统集成（邮件/企微实际发送） | Plan 04 Scope |

---

## 十一、优先级排序与修复计划

### P0 — 立即修复（阻塞主链路或虚假通过）

| 编号 | 问题 | 修复量 | 涉及文件 |
|---|---|---|---|
| BUG-1 | Happy Path 漏跑 W1-W2 | 小 | `tests/full/test_q2c_happy_path.sh` |
| BUG-2 | `goto_summary` 定义在引用之后 | 小 | 同上 |
| BUG-3 | Happy Path 始终 exit 0 | 小 | 同上 |
| BUG-4 | CHK-09 硬编码 PASS | 中 | `tests/validation/sql/chk_09_gl_balance.sql` |
| CHK-降级 | 7 个 CHK SQL 逻辑降级 | 中 | `tests/validation/sql/chk_*.sql` |

### P1 — 高优先级（虚假覆盖或核心流程缺失）

| 编号 | 问题 | 修复量 | 涉及文件 |
|---|---|---|---|
| AP-桩 | 4 个审批异常重写 | 大 | `tests/exceptions/test_approval_*.sh` |
| DEV-1 | PU6 来料检验实现 | 中 | `tests/phase3a/test_pu5_pu6_goods_receipt.sh` |
| DEV-3 | 通知测试嵌入 Happy Path + 编号对齐 | 中 | `tests/full/` + `tests/notifications/` |
| EC | 5 条事件链集成验证 | 大 | 新增 `tests/validation/test_ec_*.sh` |
| DEV-5 | 补齐 4 种断言类型 | 中 | `tests/lib/assert.sh` |

### P2 — 中优先级（规格完整性）

| 编号 | 问题 | 修复量 |
|---|---|---|
| Spec 缺失 | 30 个异常分支场景 | 大（30 个脚本） |
| DEV-2 | 并行 Relay 拆分 | 中 |
| DEV-4 | 创建 04_system_config.sql | 小 |
| INFRA-3 | 数据快照机制 | 中 |
| ISSUE-1 | 接力数据值验证 | 小 |
| ISSUE-2 | 半成品B独立生产 | 中 |
| ISSUE-5 | 税额计算验证 | 小 |
| ISSUE-6 | 接力 context/next 字段写入 | 小 |

### P3 — 低优先级（增强覆盖 + 基础设施完善）

| 编号 | 问题 | 修复量 |
|---|---|---|
| INFRA-1 | 测试报告生成 | 大 |
| INFRA-2 | 质量门禁执行 | 中 |
| INFRA-4 | 环境隔离 | 中 |
| INFRA-5 | Session 过期恢复 | 中 |
| INFRA-6 | Agent-QM1/GM 角色使用 | 中 |
| ISSUE-3 | 权限反向验证 | 中 |
| ISSUE-4 | 多供应商比价 | 中 |

---

## 十二、覆盖率统计

### 按场景类型

| 类型 | 计划数 | 已实现 | 有效覆盖 | 覆盖率 |
|---|---|---|---|---|
| Happy Path 主线节点 | 25 | 25 | 23 (W1-W2 未在主链路执行) | 92% |
| 异常分支场景 | 62 | 33 | 29 (4 审批空壳) | 47% |
| 事件链验证 | 5 | 0 | 0 | 0% |
| 通知验证 | 20 | 1 独立脚本 | 待定（编号错位） | 待定 |
| 数据一致性 CHK | 12 | 12 | 5 (7 个降级/空壳) | 42% |
| 断言类型 | 6 | 2 | 2 | 33% |
| 质量门禁 GATE | 5 | 0 | 0 | 0% |
| 测试报告章节 | 9 | 0 | 0 | 0% |

### 按业务域异常场景

| 域 | Spec 异常数 | 已实现 | 缺失 |
|---|---|---|---|
| 销售 | 12 | 10 | 2 |
| 计划 | 6 | 0 | 6 |
| 采购 | 14 | 10 | 4 |
| 生产 | 12 | 8 | 4 |
| 仓储/发货 | 8 | 3 | 5 |
| 质量 | 6 | 3 | 3 |
| 财务 | 10 | 4 | 6 |
| 审批（跨域） | 8 | 4 | 4 |
| **合计** | **62** | **33** | **29+5(EC)** |

### 总体评估

- **Happy Path 有效覆盖率**：92%（W1-W2 缺失）
- **异常场景有效覆盖率**：47%（含空壳和降级）
- **CHK 有效覆盖率**：42%（7/12 降级）
- **基础设施完整度**：~60%（缺报告、门禁、快照、隔离、4 种断言）
- **整体方案执行度**：~55%
