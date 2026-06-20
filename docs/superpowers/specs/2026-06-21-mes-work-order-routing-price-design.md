# 工单工序计件单价维护 + 工序删除

- 日期：2026-06-21
- 模块：MES（`abt-core/src/mes/production_batch` + `abt-web/src/pages/mes_order_detail`）
- 状态：待实现

## 1. 背景与问题

报工工资链路 `routing_steps.unit_price → work_order_routings.unit_price → confirm_routing_step 算 wage → 报工页 data-price` **技术上已全程打通**，但源头 `work_order_routings.unit_price` 在工单下达时从 `routing_steps.unit_price` 深拷贝，而后者全项目没有任何 UI 录入（migration 045 默认 0），导致报工单价永远显示 `—` / `¥0.00`。

工单详情页「工序明细」tab（`mes_order_detail.rs::tab_routing`）当前只读展示「计件单价」列。

## 2. 目标

1. 在工单详情页工序列表上**行内编辑计件单价**，单价 > 0 必填
2. 报工时自动带出该单价（已有逻辑，无需改动）
3. 提供**工序删除**：不想要的工序直接删，删后工序号连续重排

## 3. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 单价维护位置 | 工单工序（`work_order_routings`） | 一单一价，符合定制/试产场景 |
| 改价时机 | 该工序「本身未报工」即可改（per-step） | 已报工的 wage 已落库，不改历史 |
| 删除时机 | 整个工单「零报工记录」才可删 | 删除具破坏性，需更严守卫；零报工时无 batch_routing_progress、current_step=0，重排零风险 |
| step_no 处理 | 删除后重排为 1..N 连续 | 状态机依赖连续 step_no |
| 单价取值 | 必须 > 0（不允许 0/负数） | 不留「未定价」灰色态；不想要的工序直接删 |
| 删最后一条 | 拒绝（至少保留一道工序） | 避免空工序导致报工/状态机死锁 |

## 4. 数据模型变更

无表结构变更。`work_order_routings.unit_price DECIMAL(18,6)` 已存在（migration 003）。

## 5. 接口设计

### 5.1 Repo 层（`abt-core/src/mes/production_batch/repo.rs`，`WorkOrderRoutingRepo`）

```rust
/// 更新单条工序单价
async fn update_unit_price(db: PgExecutor<'_>, routing_id: i64, unit_price: Decimal) -> Result<()>;

/// 删除单条工序
async fn delete(db: PgExecutor<'_>, routing_id: i64) -> Result<()>;

/// 删除后重排：把该工单剩余工序 step_no 压成 1..N 连续
async fn renumber_steps(db: PgExecutor<'_>, work_order_id: i64) -> Result<()>;

/// 该工单是否存在任意报工记录（删除的全局守卫）
async fn has_any_report(db: PgExecutor<'_>, work_order_id: i64) -> Result<bool>;

/// 该工序是否存在报工记录（改价的逐行守卫）
async fn has_report(db: PgExecutor<'_>, routing_id: i64) -> Result<bool>;
```

SQL 要点：
- `has_any_report`：`SELECT EXISTS(SELECT 1 FROM work_reports wr JOIN work_order_routings wor ON wor.id = wr.routing_id WHERE wor.work_order_id = $1)`
- `has_report`：`SELECT EXISTS(SELECT 1 FROM work_reports WHERE routing_id = $1)`
- `renumber_steps`：用 CTE `ROW_NUMBER() OVER (ORDER BY step_no)` 生成新序号并 UPDATE

### 5.2 Service 层（`abt-core/src/mes/production_batch/service.rs`，`ProductionBatchService`）

```rust
/// 修改工序计件单价
async fn update_routing_unit_price(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>,
    work_order_id: i64, routing_id: i64, unit_price: Decimal,
) -> Result<WorkOrderRouting>;

/// 删除工序并重排工序号
async fn delete_routing(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>,
    work_order_id: i64, routing_id: i64,
) -> Result<()>;
```

#### `update_routing_unit_price` 守卫（按序）
1. `unit_price > 0`，否则 `DomainError::validation("计件单价必须大于 0")`
2. 工单状态 ∈ {Released, InProduction}（Draft 无工序行；Closed/Cancelled 不可改）
3. routing 属于该 `work_order_id`，否则 `not_found`
4. **事务内** `!has_report(routing_id)`，否则 `business_rule("该工序已报工，单价不可修改")`
5. `update_unit_price` 落库；写审计 `AuditAction::Update`，changes = `"unit_price: {old} → {new}"`

#### `delete_routing` 守卫（按序）
1. 工单状态 ∈ {Released, InProduction}
2. routing 属于该 `work_order_id`，否则 `not_found`
3. **事务内** `!has_any_report(work_order_id)`，否则 `business_rule("工单已有报工记录，不可删除工序")`
4. 删除后剩余工序数 ≥ 1，否则 `business_rule("至少保留一道工序")`
5. `delete(routing_id)` → `renumber_steps(work_order_id)`；写审计 `AuditAction::Delete`，changes = `"删除工序 {step_no} {process_name}"`

两个方法均使用事务，守卫 4/3 必须在事务内复查以防与报工并发竞争。

### 5.3 Web 层

#### 路由（`abt-web/src/routes/mes_order.rs`，新增 TypedPath）

```rust
#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/price")]
pub struct OrderRoutingPricePath { pub order_id: i64, pub routing_id: i64 }

#[typed_path("/admin/mes/orders/{order_id}/routings/{routing_id}/delete")]
pub struct OrderRoutingDeletePath { pub order_id: i64, pub routing_id: i64 }
```

> 删除用独立 `/delete` POST 路径而非 HTTP DELETE，与项目既有 HTMX `hx-post` 范式一致（HTMX `hx-delete` 仍发 DELETE，但项目内统一 POST 更稳）。

#### Handler（`abt-web/src/pages/mes_order_detail.rs`）

```rust
#[require_permission("WORK_ORDER", "update")]
pub async fn update_routing_price(
    path: OrderRoutingPricePath, ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingPriceForm>,
) -> Result<Html<String>>;   // 返回该行 <tr> outerHTML

#[require_permission("WORK_ORDER", "update")]
pub async fn delete_routing(
    path: OrderRoutingDeletePath, ctx: RequestContext,
) -> Result<Html<String>>;   // 返回重排后的整个 <tbody> outerHTML
```

`RoutingPriceForm { unit_price: Decimal }`。

### 5.4 前端（`mes_order_detail.rs::tab_routing`）

`get_order_detail` 额外计算并传入 `tab_routing`：
- `reported_routing_ids: HashSet<i64>` —— 已报工的 routing_id 集合
- `order_has_report: bool` —— 整单是否有报工

> 数据来源：从已加载的 `reports: Vec<ReportListItem>` 推导。若 `ReportListItem` 缺 `routing_id` 字段，则补一次 `WorkOrderRoutingRepo` 查询取集合（实现期确认）。

工序列表表格变化：
- **单价列**：
  - `!reported_routing_ids.contains(r.id)` → `<input type="number" step="any" name="unit_price" value={p} hx-post={price_path} hx-trigger="change" hx-target="closest tr" hx-swap="outerHTML">`
  - 否则 → 只读文本 `¥{p}`
- **新增「操作」列**（仅 `!order_has_report` 时整列可见）：
  - 每行一个删除图标按钮 `hx-post={delete_path} hx-confirm="..." hx-target="closest tbody" hx-swap="outerHTML"`

前端只读/隐藏仅是体验层，**并发安全与守卫完全由 service 事务内复查保证**。

## 6. 错误处理

| 场景 | 处理 |
|---|---|
| 并发：报工与改价/删除竞争 | service 事务内复查 has_report/has_any_report，已报工则 `business_rule` 拒绝 |
| 单价 ≤ 0 | `validation` |
| 删除最后一条工序 | `business_rule("至少保留一道工序")` |
| routing 不属于该工单 / 不存在 | `not_found` |
| 工单状态不符（Draft/Closed/Cancelled） | `business_rule` |
| 非法数值格式 | `Form` 解析 → 400 |

Web handler 错误按既有 `DomainError → HTTP` 映射，前端由 toast 展示（`htmx:responseError`）。

## 7. 测试

### Service 单测（`abt-core`）
- `update_routing_unit_price`：未报工可改且单价更新 + 审计落库；已报工拒绝；状态不符拒绝；越权（routing 不属该单）拒绝；单价 ≤0 拒绝
- `delete_routing`：零报工可删且 step_no 重排连续 + 审计落库；有报工拒绝；删最后一条拒绝；越权拒绝；状态不符拒绝

### Repo 单测
- `update_unit_price` 单列生效
- `delete` + `renumber_steps` 后 step_no 连续无缺口
- `has_report` / `has_any_report` 真假分支

### 手工/E2E 验证
- 工单下达 → 工序 tab 行内改单价 → 报工页 data-price 带出正确单价、wage 正确
- 零报工时删一道工序 → 序号重排 → 报工流程不受影响
- 首道报工后 → 删除按钮消失、报过工行单价变只读

## 8. wage_amount 冻结（报工即冻结工资）

**问题**：`work_reports` 表当前**无 `wage_amount` 字段**，工资在 `calculate_wage`（`abt-core/src/mes/work_report/implt.rs:102-130`）每次查询时用**实时** `work_order_routings.unit_price` 重算。本设计引入"可编辑 unit_price"后，虽有"报过工即锁"守卫保证稳定，但工资仍依赖实时单价——任何绕过守卫的修改（未来新代码、SQL 直改）都会让历史工资静默漂移。这是调研③的实质，必须根治。

**方案**：报工落库时把算出的 `wage_amount` 冻结写入 `work_reports`，读取直接用冻结值。

| 改动 | 位置 | 内容 |
|---|---|---|
| Migration | `abt-core/migrations/`（新文件） | `ALTER TABLE work_reports ADD COLUMN wage_amount NUMERIC(20,4) NOT NULL DEFAULT 0;` + 回填（见下） |
| Model | `abt-core/src/mes/work_report/model.rs` | `WorkReport`、`ReportListItem` 增加 `wage_amount: Decimal` |
| Row | `abt-core/src/mes/production_batch/repo.rs` `WorkReportRow` + `InsertWorkReportParams` | 增加 `wage_amount` 字段 |
| 写入 | `WorkReportRepo::insert_or_get_existing`（`production_batch/repo.rs:596`） | INSERT 列与 RETURNING 增加 `wage_amount`，绑定参数 |
| 计算→传入 | `confirm_routing_step`（`production_batch/implt.rs:222`） | 已算出 `wage_amount`，传入 `InsertWorkReportParams` |
| 读取 | `calculate_wage`（`work_report/implt.rs:102-130`） | 改为累加 `report.wage_amount`，**不再实时重算**；`WageDetail.unit_price` 仍从 routing 读，仅展示 |
| 详情页 | `abt-web/src/pages/mes_report_detail.rs` 工资计算卡 | 展示冻结的 wage_amount |

**回填脚本**（migration 内，对历史报工近似冻结）：
```sql
UPDATE work_reports wr
SET wage_amount = (wr.completed_qty +
      CASE WHEN <defect_reason 属于 affect_wage> THEN wr.defect_qty ELSE 0 END)
    * COALESCE(wor.unit_price, 0)
FROM work_order_routings wor
WHERE wor.id = wr.routing_id AND wr.wage_amount = 0;
```
> `affect_wage` 的 defect_reason 集合与运行时一致；回填是一次性近似（历史 defect_reason 已存于 `wr.defect_reason`，可直接判断）。

**与守卫的关系**：冻结是**根本保障**，"报过工即锁 unit_price"是**体验层防误改**。两者叠加，历史工资完全不受后续单价变动影响——这正是调研③要求的"报工即冻结费率"。

## 9. 设计文档同步（CLAUDE.md「双向同步」要求）

实现时同步更新 `docs/uml-design/04-mes.html` 中 `ProductionBatchService` 的接口定义（新增 `update_routing_unit_price` / `delete_routing` 两个 trait 方法）。

## 10. 不做（YAGNI）

- 不在 `routing_steps`（工艺模板）层加单价录入 UI —— 已明确走「工单一单一价」
- 不清理 `bom_labor_processes.unit_price`（与本需求无关，另立）
- 不做单价版本/历史（审计日志已留 old→new，足够；wage 已冻结落库）
- 不做「新增工序」（工单工序来源于下达时深拷贝，本需求只做改价与删）
- **不做产品成本核算（cost accounting）** —— 调研①②属于另一子系统，单独立项：
  - ① `unit_price`（计件工资）与 `costs_hour`（工作中心小时费率）的 labor 成本策略未定（`costs_hour` 当前是死数据，无 `work_hours × costs_hour` 链路）
  - ② 报工不写 `CostType::Labor` 成本分录（`confirm_routing_step` 不调 `CostEntryService`，全项目无 Labor 写入点，FMS 成本分析 labor_cost 恒为 0）
  - 这两项紧耦合（定价策略决定分录用哪个费率），将在独立的 cost-accounting brainstorm + spec 中一起设计

## 11. 已核实澄清（调研勘误）

- **调研③「缺历史费率快照」不成立**：`work_order_routings.unit_price` 是值拷贝快照（工单下达时从 `routing_steps` 深拷贝，`work_order/implt.rs:169`；查询不 JOIN `routing_steps`，`production_batch/repo.rs:338`）。改 routing 模板价**不会**污染已下达工单。本设计的"报过工即锁"+ §8 wage 冻结，进一步保证历史工资不可漂移。
- **调研①「两条线混算」实为「labor 线未接」**：`costs_hour`/`work_hours` 是死数据，当前产品成本只走 `unit_price`（计件），不存在混算，只是 labor 成本链路整体缺失（见 §10）。
