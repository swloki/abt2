# MES 两层架构方案 — 四视角迭代评审

> 日期: 2026-06-13
> 基础文档: `2026-06-13-mes-two-layer-architecture.md`
> 方法: 依次以 ERP 系统设计师 → 高级后端工程师 → 产品经理 → 使用者 四个视角评审，每轮只输出**修改项**，最终汇成修订版方案。

---

## 角色一：ERP 系统设计师 — 提出设计方案

### 1.1 重新定义问题

原文档把问题定义为"三层合并为两层"。更精确的表述是：**消除没有独立领域语义的实体**。

- Plan 不是"第三层"，是"壳"——在成熟 ERP（SAP PP / Oracle MES）中，计划层承担 MRP 运算、产能平衡、排程优化。这里的 `production_plan` 只做分组 + 薄弱的 `schedule_v1`（排序 + 标记逾期），不承担任何排程计算。
- Batch（流转卡）不是壳——它在车间有真实物理语义（一张卡跟一拨物料走完全部工序）。问题在于当前系统 1:1 不拆批（`release()` 只建 1 个 batch，`split_work_order` 全仓零调用），使 Batch 退化为 WorkOrder 的执行状态镜像。

### 1.2 模型设计（修订原文档 3 处缺陷）

#### 缺陷 ①：demand 关联——原文档 `demand_id: Option<i64>` 只能存一个需求

**事实**：`demand_handler/implt.rs:93` 的聚合逻辑按 `(product_id, source_id)` 合并多个需求为一个 plan_item → 一个 work_order。一个工单可能来源于**多个需求**。

```
当前路径: demand_ids[] → aggregate by (product_id, source_id) → 1 PlanItem → 1 WorkOrder
```

原文档的 `demand_id: Option<i64>` 是单值字段，会丢失多对一关系。

**修正**：使用关联表，而非单值字段。

```sql
CREATE TABLE production_order_demands (
    production_order_id BIGINT NOT NULL REFERENCES work_orders(id),
    demand_id           BIGINT NOT NULL,
    PRIMARY KEY (production_order_id, demand_id)
);
```

`work_orders` 表**不需要**加 `demand_id` 列——关系在关联表里。

#### 缺陷 ②：表名——不应重命名为 `production_orders`

**事实**：`work_orders` 表被 6 张表通过 FK 引用（work_order_routings / work_reports / production_batches / production_receipts / production_inspections / production_exceptions），还被 3 个视图引用（v_production_demands 等）。

重命名表 → 迁移面爆炸（全部 FK + 全部 SQL + 全部 view）。收益仅为语义更好听。

**修正**：表名保持 `work_orders`，代码层的 struct 命名为 `ProductionOrder`。ORM/查询里 `work_orders` 就是 ProductionOrder 的存储表——这在 ERP 里很常见（存储名与领域名分离）。

#### 缺陷 ③：`PendingReceipt` 状态被合并到 `Completed`，但语义模糊

**事实**：当前 Batch 状态流：
```
InProgress → PendingReceipt（最后工序完成，等入库）→ Completed（入库后）
```

原文档合并后的状态机把 `PendingReceipt` 省了，用 `Completed` 表示"工序完成"、`Closed` 表示"入库后"。语义上可行，但"Completed"在中文语境下容易和"完工入库"混淆。

**修正**：明确术语边界。

### 1.3 修订版状态机

```
ProductionOrder 合并状态机（7 值）:

主路径:
  Draft(1)      → 刚创建，未下达（吸收 Plan 的 Draft + Confirmed 缓冲）
  Released(2)   → 已下达（BOM快照+工序+流转卡+预留已创建）
  InProduction(3)→ 首次报工后
  Completed(4)  → 最后工序报工完成，等待入库（= 原 PendingReceipt）
  Closed(5)     → 完工入库后

分支/终态:
  Suspended(6)  → 暂停（从 Released/InProduction 暂停，可恢复——正交态，平面枚举表达）
  Cancelled(7)  → 取消（Draft/Released 阶段取消）
```

**关于 Scrapped**：原文档列了 `Scrapped` 状态。但当前代码 `scrap()` 实际做的是 `BatchStatus::Cancelled`（`production_batch/implt.rs:484`），没有独立的 Scrapped 值。废弃与取消的区分在当前系统里没有实际意义（都释放预留、都终止）。**不新增 Scrapped**，统一用 `Cancelled` + `remark` 记录原因，与现状一致。

**关于 `Planned` 状态**：当前 WorkOrderStatus 有 `Planned(2)`，但 `release()` 允许 `Draft → Released` 跳过 Planned。合并后删除 `Planned`，只有 `Draft → Released`。

状态转换图：
```
Draft ──release──→ Released ──首次报工──→ InProduction ──末工序完成──→ Completed ──入库──→ Closed
  │                    │                        │
  └──cancel──→ Cancelled ←──cancel──┘   ↕ suspend/resume
                                         Suspended
Released/InProduction/Completed ──cancel──→ Cancelled
```

### 1.4 最终模型

```
ProductionOrder（存储于 work_orders 表）
├── 计划属性（原 WorkOrder）
│   ├── doc_number, product_id, planned_qty
│   ├── bom_snapshot_id, routing_id, work_center_id
│   └── scheduled_start, scheduled_end, sales_order_id
├── 执行属性（原 ProductionBatch）
│   ├── completed_qty, scrap_qty
│   ├── current_step, card_sn, team_id
│   └── actual_start, actual_end
├── 来源属性（原 PlanItem，简化）
│   └── plan_group_id: Option<i64>  -- 分组标签
├── 来源需求（关联表 production_order_demands）
├── status, version, remark, operator_id
└── created_at, updated_at, deleted_at

plan_groups（新增，极简）
├── id, group_no, group_date, remark, operator_id, created_at
└── 无 status —— 状态通过聚合查询工单推导
```

---

## 角色二：高级后端工程师 — 修改方案

### 2.1 反对双写策略——改用一次性迁移

原文档阶段 B（双写 3 天）+ C（迁移 2 天）+ D（切换 3 天）= 8 天渐进迁移。

**反对理由**：
1. `abt_v2` 是新库，migrations 才到 037，无稳定生产数据
2. MES 模块代码本周仍在频繁修改（plan/batch/work_order 最近 2h-5h 都有改动）
3. 双写要求两套写入路径都正确（release + confirm_routing_step + suspend + scrap 全部双写），bug 面反而更大
4. 双写是为"不能停机 + 大数据量"场景准备的，当前两者都不满足

**修正方案——一次性迁移（全库备份 → 迁移 → 切换）**：

```
阶段 A（1 天）: ALTER TABLE work_orders ADD COLUMN（加 Batch 字段 + plan_group_id）
                新增 plan_groups 表 + production_order_demands 表
                代码层用 #[allow(dead_code)] 标记新字段，不影响现有逻辑

阶段 B（1 天）: 一次性数据迁移脚本（单事务）
  1. work_orders 吸收 production_batches 字段（UPDATE FROM subquery）
  2. work_orders.plan_group_id 从 plan_items.plan_id 映射
  3. production_order_demands 从 plan_items → demand 关联重建
  4. work_reports: 删除 batch_id 列，UNIQUE 改为 (work_order_id, routing_id, ...)
  5. production_receipts: 删除 batch_id 列（work_order_id 已存在）
  6. production_exceptions: batch_id 列保留但置 NULL（历史数据，不再写入）

阶段 C（3 天）: 代码切换
  - 删除 production_plan/ + production_batch/ 模块
  - work_order/ 吸收执行逻辑
  - demand_handler 重写 create_plan_from_demands → create_orders_from_demands
  - work_report / production_receipt / production_exception 调整

阶段 D（1 天）: DROP TABLE production_plans, production_plan_items, production_batches
                删除 state_transition_defs 中 ProductionPlan/ProductionBatch 的行

阶段 E（1 天）: 端到端回归测试

总计: 7 天（比原文档 13 天省 6 天，且无双写一致性风险）
```

### 2.2 补全原文档遗漏的变更范围

原文档 4.2 改造模块表**遗漏了 4 个模块**：

| 遗漏模块 | 实际影响 | 证据 |
|---------|---------|------|
| `production_exception` | model + repo 有 `batch_id: Option<i64>`，JOIN `production_batches` | `production_exception/repo.rs:128, model.rs:15,67` |
| `production_receipt` | `batch_id` 是独立列（非 work_order_id 替代），confirm() 更新 batch 状态 | `production_receipt/implt.rs:40,236, repo.rs:16` |
| `demand_handler` | `create_plan_from_demands` 是**重写**不是小改——深度依赖 ProductionPlanService::create | `demand_handler/implt.rs:131` |
| `dashboard` | 统计 SQL 引用 production_plans / production_batches | `dashboard/repo.rs` |

### 2.3 幂等键迁移——利用已有字段

**事实**：`work_reports` 表**已有** `work_order_id` 列（migration 003 line 148），与 `batch_id` 并存。

当前唯一约束：`UNIQUE(batch_id, routing_id, worker_id, shift, report_date)`

因 1:1，batch_id 与 work_order_id 等价。迁移操作：

```sql
-- 1. 替换唯一约束
ALTER TABLE work_reports DROP CONSTRAINT work_reports_batch_id_routing_id_worker_id_shift_report_date_key;
ALTER TABLE work_reports ADD CONSTRAINT work_reports_uq 
    UNIQUE (work_order_id, routing_id, worker_id, shift, report_date);

-- 2. 删除冗余列
ALTER TABLE work_reports DROP COLUMN batch_id;
DROP INDEX idx_work_reports_batch;
```

代码中 `InsertWorkReportParams.batch_id` 字段删除，改用 `work_order_id`。

### 2.4 Service 边界——Command/Query 分离

合并后 `ProductionOrderService` 会承载全部写操作（release + confirm_routing_step + suspend + resume + scrap + advance_to_receipt + close + cancel + create）+ 读操作（find_by_id + list + find_by_card_sn + list_routings），预计 800+ 行。

**修正**：拆分为两个 trait，遵循现有 dashboard 独立 service 的趋势：

```
mes/work_order/
├── service.rs         # ProductionOrderCommandService trait（写）
├── query_service.rs   # ProductionOrderQueryService trait（读）
├── implt.rs           # 命令实现
├── query_implt.rs     # 查询实现
├── repo.rs            # 共用 repo（raw SQL）
├── model.rs           # 共用 model
└── mod.rs             # 两个 factory 函数
```

命令 trait（吸收原 WorkOrderService + ProductionBatchService 写方法）：
```rust
pub trait ProductionOrderCommandService: Send + Sync {
    async fn create(...) -> Result<i64>;
    async fn release(...) -> Result<()>;
    async fn unrelease(...) -> Result<()>;
    async fn confirm_routing_step(...) -> Result<StepConfirmationResult>;
    async fn advance_to_receipt(...) -> Result<()>;
    async fn suspend(...) -> Result<()>;
    async fn resume(...) -> Result<()>;
    async fn close(...) -> Result<()>;
    async fn cancel(...) -> Result<()>;
}
```

查询 trait：
```rust
pub trait ProductionOrderQueryService: Send + Sync {
    async fn find_by_id(...) -> Result<ProductionOrder>;
    async fn find_by_card_sn(...) -> Result<Option<ProductionOrder>>;
    async fn list(...) -> Result<PaginatedResult<ProductionOrder>>;
    async fn list_routings(...) -> Result<Vec<WorkOrderRouting>>;
}
```

### 2.5 demand_handler 重写细节

当前 `create_plan_from_demands` 的核心路径：
```
lock demands → aggregate by (product_id, source_id) → ProductionPlanService::create(plan_req)
```

合并后 `create_orders_from_demands`：
```
lock demands → aggregate by (product_id, source_id) → for each group:
    1. 创建 plan_group（一次调用生成 group_id）
    2. ProductionOrderCommandService::create（Draft 状态）
    3. 写入 production_order_demands 关联
    4. update demand.target_doc = (WorkOrder, order_id)
    5. publish DemandConfirmed event
```

**关键决策**：创建后工单保持 `Draft`，**不自动 release**。理由见使用者视角（角色四 §4.5）。

---

## 角色三：产品经理 — 修改方案

### 3.1 用户流程——从 7 步降到 4 步

```
当前: 需求池 → 创建计划 → 确认计划 → 下达计划(生成工单+批次) → 工单列表 → 批次详情 → 报工 → 入库
合并: 需求池 → 创建工单(Draft) → 下达工单(Released) → 工单详情(报工+流转卡) → 入库
```

删除的 3 个页面（`mes_plan_list` / `mes_plan_detail` / `mes_plan_create`）+ 2 个页面（`mes_batch_list` / `mes_batch_detail`）的功能必须**无损迁移**到工单页面，不能丢功能。

### 3.2 工单详情页——Tab 化，防止信息过载

`mes_order_detail.rs`（当前 26.5KB）合并后要吸收 `mes_batch_detail.rs`（16.8KB）+ `mes_plan_detail.rs`（25.8KB）的信息，预计 50KB+ 渲染逻辑。必须用 Tab 组织：

```
工单详情
├── 基本信息 Tab
│   产品、数量、BOM快照、工艺路线、排程日期、来源需求列表
├── 工序进度 Tab（吸收 batch_detail）
│   工序列表（step_no / 名称 / 状态 / 完成量 / 报工历史）
│   流转卡号（card_sn，支持打印/二维码）
│   报工入口（confirm_routing_step 的表单）
├── 物料 Tab
│   BOM 快照明细、领料单、倒冲记录
└── 质量与异常 Tab
    报检记录（IPQC/FQC）、异常记录
```

### 3.3 排程看板保留——不随 Plan 删除

`mes_schedule_board.rs`（排程看板）和 `mes_plan_create.rs`（创建计划）当前挂在 Plan 模块下。删除 Plan 后：

- **排程看板**：改为基于 ProductionOrder 的 `scheduled_start/end` 展示看板/甘特。这是计划员核心工具，**不能删**。
- **创建计划页** → 改为"批量创建工单"页（选需求 → 聚合 → 填排程 → 一次性创建多个 Draft 工单）。

### 3.4 分组功能的产品化

`plan_group_id` 对用户需要友好呈现：

- 工单列表增加"下达批次"列（显示 `group_no`）
- 支持"按批次筛选"筛选器
- 批次汇总：`WHERE plan_group_id = ?` → 聚合状态（X 进行中 / Y 完工 / Z 异常）
- 不需要独立的"分组管理"页面——它只是工单的一个属性

### 3.5 数据连续性保障

删除页面前必须确认：
- `mes_dashboard` 的统计 SQL 已切换到新表
- `mes_wage_list`（工资）的 `batch_id → work_order_id` 映射不影响已结算工资
- `mes_material_usage`（物料用量）的数据源切换
- 历史工单的 `card_sn` 保留可查

---

## 角色四：使用者（计划员 / 车间操作员）— 修改方案

### 4.1 流转卡（card_sn）——车间核心交互不能断

流转卡是车间物理存在的卡片/扫码标签。`mes_card_query.rs`（20.7KB）是扫码查工单入口。

**要求**：
- card_sn 从 batch 迁移到 work_orders 后，`find_by_card_sn` 查询接口不消失（移到 QueryService）
- 工单详情页醒目展示 card_sn，支持打印/二维码
- 扫码 → 查到工单 → 看到当前工序 → 报工，这条链路**前端交互零变化**

### 4.2 报工入口——心智模型不变

车间操作员的心智模型：**拿流转卡 → 扫码 → 看到当前工序 → 输入完工/不良数量 → 确认**。

后端 `batch_id → work_order_id` 的变化对操作员**不可见**。前端表单、提交流程、幂等提示（重复报工返回已有结果）全部保持一致。

### 4.3 暂停 / 恢复 / 报废——操作入口位置不变

这些按钮当前在批次详情页。合并后在工单详情页"工序进度 Tab"。交互流程（填原因 → 确认 → 状态更新）不变。

### 4.4 术语变化——需要培训

当前车间习惯说"这个批次怎么样了"。合并后没有批次概念。

**要求**：
- 界面中"批次"统一改为"工单"
- 工单号 `WO-2026-06-xxxxx` 成为唯一标识
- 流转卡号 `SN-xxx` 作为辅助标识保留（车间扫的就是这个）
- 上线前给车间做一次简短培训（15 分钟够了——"以后没有批次了，工单就是批次"）

### 4.5 Draft 作为"确认缓冲"——不能省

当前流程有"确认计划"步骤。虽然语义薄弱（纯状态翻转），但它给了计划员**"检查后再放行"**的心理缓冲。

合并后省略了 Plan，这个缓冲由 **Draft 状态**承担：

```
需求池创建工单 → Draft（计划员检查物料、工艺、排程）
计划员批量选择 Draft 工单 → Release（下达）
```

**反对自动 release**：`create_orders_from_demands` 创建后保持 Draft，不自动下达。理由：一旦 release 就会冻结 BOM 快照、创建预留、生成领料单——这些是有成本的副作用，不应自动触发。

### 4.6 来源需求透明度

一个工单可能来自多个需求（聚合）。工单详情"基本信息 Tab"要展示**来源需求列表**（需求编号、订单号、需求数量），让计划员知道这个工单是为哪些订单生产的。不能只看到一个聚合后的总数。

---

## 最终汇总：修订版方案关键决策

| # | 决策项 | 原文档 | 修订后 | 依据 |
|---|--------|--------|--------|------|
| 1 | demand 关联 | `demand_id: Option<i64>` 单值 | 关联表 `production_order_demands` | 一个工单可来自多个需求（聚合逻辑） |
| 2 | 表名 | 暗示重命名 `production_orders` | 保持 `work_orders` | 6 张表 FK + 3 视图引用，重命名风险 > 收益 |
| 3 | 迁移策略 | 双写 8 天 | 一次性迁移 7 天 | 新库无稳定数据，双写是过度工程 |
| 4 | Scrapped 状态 | 新增 | 不新增，统一用 Cancelled | 当前代码 scrap() 实际就是 Cancelled |
| 5 | PendingReceipt | 合并到 Completed | Completed = 待入库，Closed = 已入库 | 明确术语边界 |
| 6 | Service 划分 | 暗示一个胖 Service | Command/Query 双 trait | 合并后 800+ 行，需拆分 |
| 7 | 幂等键 | 未提及 | `(work_order_id, routing_id, worker_id, shift, report_date)` | work_reports 已有 work_order_id 列 |
| 8 | production_receipts.batch_id | "改为 production_order_id" | 直接删除（work_order_id 已存在） | 冗余字段 |
| 9 | 遗漏模块 | 缺 4 个 | 补全 exception/receipt/demand_handler/dashboard | 代码验证 |
| 10 | 自动 release | 暗示自动 | 保持 Draft，人工 release | release 有副作用，需人工确认 |
| 11 | 工单详情页 | 未设计 | 4 Tab 组织 | 合并后信息量大 |
| 12 | 排程看板 | 未提及 | 保留，改为基于 ProductionOrder | 计划员核心工具 |

### 修订版实施顺序

```
Phase 0 — 设计确认（本文档评审通过）
Phase 1 — Schema 准备（ALTER + 新表，1 天）
Phase 2 — 数据迁移脚本（1 天，全库备份后执行）
Phase 3 — 代码切换（3 天）
  3a. 删除 production_plan/ + production_batch/ 模块
  3b. work_order/ 吸收执行逻辑 + 拆 Command/Query
  3c. demand_handler 重写
  3d. work_report / receipt / exception / dashboard 调整
Phase 4 — 页面合并（2 天）
  4a. 工单详情页 4 Tab 改造
  4b. 删除 plan/batch 页面，路由重定向
  4c. 排程看板改造
Phase 5 — DROP 旧表 + 回归测试（1 天）

总计: 8 天（含页面），比原文档 13 天省 5 天
```
