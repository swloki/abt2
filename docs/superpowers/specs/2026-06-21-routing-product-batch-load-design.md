# 工单工序产出品批量加载（两种来源）

- 日期：2026-06-21
- 模块：`abt-core/src/mes/production_batch`、`abt-web/src/pages/{mes_order_detail,routing_create,routing_detail}`
- 状态：待实现
- 承接：`2026-06-21-routing-edit-drawer-design.md`（产出品编辑抽屉已实现，但逐行设太累）；`2026-06-21-issue67-outsourcing-routing-product-design.md`（产出品字段+下达快照已就绪，模板录入 UI 缺口在 A6 标注）

## 1. 背景与目标

工单工序的「产出品」目前只能逐行抽屉设（19 道工序很累），且多数为空（模板无产出品 → 下达快照为空）。目标：提供**两种批量加载来源**，一键填充整单的产出品：
- **① 从工艺路线加载**：从工单引用的工艺路径模板（`routing_steps`）按 `step_no` 映射 `product_id`
- **② 从最近同路径工单加载**：找同 `routing_id` 且已设过产出品的最近其他工单，按 `step_no` 1:1 复制（"上次怎么设的，这次照搬"）

① 的前提是模板有产出品，故一并补上工艺路径模板的产出品录入 UI（A6 缺口）。

## 2. 关键决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 场景②匹配键 | 同 `routing_id`（工艺路径模板） | 产出品是工序属性，同路径工序列表一致，按 step_no 1:1 映射最准 |
| 守卫粒度 | 仅填充未报工工序行（报工行跳过） | 与现有产出品编辑守卫一致；已报工的产出品已锁 |
| 加载行为 | 模板/历史工单某步无产出品 → 跳过该步（不覆盖已有值） | 避免清空已手动设好的行 |
| 模板录入 | `routing_create` 每步加产出品 `<select>` + `routing_detail` 显示名 | ① 的前提；补 A6 缺口 |
| 批量动作守卫 | 整单全部已报工 → 按钮禁用 | 无未报工行可填，加载无意义 |

## 3. 模板产出品录入（① 的前置）

### 3.1 `routing_create.rs`
- `StepWeb` 加 `product_id: Option<i64>`
- `get_routing_create` 额外 list 产品（`product_service.list`，page 500）传入页面
- `addStep()` JS 每行加产出品 `<select>`（选项 = 产品列表，与 OM 页产品下拉同范式）；`onStepChange` 收集 `product_id`
- 提交解析：`RoutingStepInput { product_id: s.product_id, .. }`（map 已用 `..Default::default()`，只需 StepWeb 带上）

### 3.2 `routing_detail.rs`
- 工序表「产出品」列：`step.product_id` 解析为产品名（`product_service.get_by_ids` 批量），无则 `—`（升级现有 `#id` 展示）

## 4. 工单详情批量加载

### 4.1 触发 UI（`mes_order_detail.rs::tab_routing`）
工序 tab 顶部加操作栏：
- 按钮「从工艺路线加载」(`hx-post=load-from-template`)、「从最近工单加载」(`hx-post=load-from-recent`)
- 均 `hx-target=#routing-tbody-wrap hx-swap=innerHTML`（刷新工序列表）
- 禁用条件：`order.routing_id.is_none()`（无模板路径）、或整单已全部报工；禁用时 tooltip 说明

### 4.2 Service（`ProductionBatchService` 新增）
```rust
/// 从工艺路径模板按 step_no 填充产出品（仅未报工行；模板无值则跳过）
async fn load_routings_from_template(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64
) -> Result<usize>;   // 返回实际填充的行数

/// 从最近同 routing_id 且有产出品的工单按 step_no 复制（仅未报工行）
async fn load_routings_from_recent(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, work_order_id: i64
) -> Result<usize>;
```

**`load_routings_from_template` 实现**：
1. 取工单 → `routing_id` 为 None → `business_rule("工单未关联工艺路线")`
2. 工单状态 ∈ {Released, InProduction}（否则拒绝）
3. 事务内：
   - `RoutingStepRepo::get_by_routing_id(routing_id)` 取模板步（含 product_id）
   - 本工单 `work_order_routings` 逐行：若 `!has_report` 且模板对应 `step_no` 有 product_id 且本行 product_id 为空 → UPDATE
   - 审计 `AuditAction::Update`（changes=`"批量加载产出品自模板 routing#{id}，{n}行"`）
4. 返回填充行数

**`load_routings_from_recent` 实现**：
1. 取工单 → `routing_id` 为 None → `business_rule`
2. 状态守卫同上
3. 事务内：
   - 查最近其他工单：`SELECT wor2.work_order_id FROM work_order_routings wor2 JOIN work_orders wo2 ON wo2.id=wor2.work_order_id WHERE wo2.routing_id=$1 AND wo2.id<>$2 AND wor2.product_id IS NOT NULL ORDER BY wo2.created_at DESC LIMIT 1`（取同 routing_id、非本单、有产出品的最近单的工序集）
   - 取该工单的 `work_order_routings`（product_id by step_no）
   - 本工单逐行：`!has_report` 且源工单同 step_no 有 product_id 且本行为空 → UPDATE
   - 审计（changes=`"批量加载产出品自工单#{src_id}，{n}行"`）
4. 返回填充行数；无源工单 → 返回 0（不报错）

> 「仅当本行 product_id 为空才填」避免覆盖手动已设值；用户想全覆盖可先清空再加载（YAGNI：暂不做"覆盖"开关）。

### 4.3 Web（`abt-web`）
- TypedPath：`OrderRoutingLoadTemplatePath = /admin/mes/orders/{order_id}/routings/load-from-template`、`OrderRoutingLoadRecentPath = /admin/mes/orders/{order_id}/routings/load-from-recent`，POST
- handler：`#[require_permission("WORK_ORDER","update")]`，调 service → 重新 `list_routings` + 解析产品名 → 返回 `routing_tbody_fragment`（替换 `#routing-tbody-wrap`）
- 填充 0 行 → 响应头 `HX-Trigger: notifyToast` 提示「未找到可加载的产出品」（沿用项目 toast 机制）

### 4.4 数据流
```
tab「从工艺路线加载」 --POST--> load_routings_from_template
   routing_steps(wo.routing_id).product_id 按 step_no → work_order_routings（未报工 + 原空行）
tab「从最近工单加载」 --POST--> load_routings_from_recent
   最近同 routing_id 工单.work_order_routings.product_id 按 step_no → 本工单（未报工 + 原空行）
两者 → 刷新 #routing-tbody-wrap + 审计 + toast
```

## 5. 错误处理

| 场景 | 处理 |
|---|---|
| 工单无 routing_id | `business_rule("工单未关联工艺路线")`；UI 按钮禁用 |
| 状态不符（Draft/Closed/Cancelled） | `business_rule` |
| 模板/历史工单都无产出品 | 返回 0 行 + toast「未找到可加载的产出品」（不报错） |
| 报工行 | 跳过（不覆盖） |
| 并发（加载与报工竞争） | 事务内 `has_report` 复查 |

## 6. 测试（DB 集成，`abt-web/tests/mes_routing_price.rs`，串行）

- `load_routings_from_template`：模板设产出品 → 新工单加载后对应行 product_id 正确；报工行不被覆盖；已有值行不被覆盖
- `load_routings_from_recent`：建两同 routing_id 工单 A/B，A 设产出品 → B 加载后等于 A；B 报工行不覆盖
- 无 routing_id → `business_rule`；无历史工单 → 返回 0 不报错
- 模板录入（routing_create）：StepWeb 带 product_id → RoutingStepInput 持久化（service 层断言 routing_steps.product_id 落库）

## 7. 设计文档同步
`docs/uml-design/04-mes.html`：`ProductionBatchService` 加 `load_routings_from_template` / `load_routings_from_recent`。`09-master-data.html`（若有 routing 部分）：`RoutingStep` 已有 product_id（063），标注模板录入入口。

## 8. 不做（YAGNI）
- 不做"覆盖已有值"开关（默认只填空行；想全覆盖先清空）
- 不做加载预览/dry-run（直接加载 + 审计可追溯）
- 不改产出品编辑抽屉（保留逐行精修能力）
- 不动委外 suggest 链路（消费方不变）
