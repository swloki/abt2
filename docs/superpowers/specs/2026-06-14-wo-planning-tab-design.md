# 工单规划工作台 — 设计文档

> **日期**：2026-06-14
> **状态**：待审核
> **关联 issue**：#52（生产计划下达弹窗修复，已完成 CSS/HTMX 层面修复）
> **关联评审**：`docs/plans/2026-06-14-plan-release-workorder-flow-review.md`
> **评审版本**：v2（经 6 角色评审修订）

---

## 1. 背景与目标

### 1.1 问题

当前生产计划"确认下达"（`release_to_work_orders()`）一步完成两件事：
1. 对每个计划明细项 `create()` 生成工单（Draft）
2. **立即** `release()` 下达（Draft → Released）

使用者没有任何控制窗口——不能选择哪些明细生成工单、不能拆分、不能调整参数。计划一旦"确认下达"，所有工单就不可逆地进入了生产流程。

### 1.2 行业标准

主流 ERP（SAP PP、金蝶 K/3、用友 U8）在计划→工单之间都有一个"转换/规划"层：
- **SAP**：计划订单 → 转换为生产订单(Created) → 主管 Release
- **金蝶 K/3**：任务单规划（合单/拆单/改单）→ 逐张下达
- **用友 U8**：已生成 → 已审核 → 已下达，三段式

共同原则：**计划明细是"需求"，工单是"执行方案"，中间有规划环节让使用者决定怎么执行。**

### 1.3 目标

在计划详情页新增"工单规划"tab，让使用者：
1. **选择性生成**：勾选明细项，决定哪些生成工单
2. **拆分**：一个大批量明细拆成多个工单（分批排产）
3. **参数调整**：修改排程日期
4. **分步下达**：生成 Draft 工单后，tab 内批量或逐个下达
5. **快速通道**：简单场景一键生成并下达，不强制走精细规划

不需要合并（需求池创建计划时已按产品合并）。

---

## 2. 整体流程

```
生产计划 (Confirmed)
  │
  ▼
计划详情页 → "工单规划" tab
  │
  │  ┌─────────────────────────────────────────┐
  │  │ 上方区块：待规划明细（始终可见）             │
  │  │  勾选 / 拆分 / 调排程                       │
  │  │  [生成草稿工单]  [一键生成并下达]           │
  │  └─────────────────────────────────────────┘
  │  ┌─────────────────────────────────────────┐
  │  │ 下方区块：已生成工单（有 Draft 时显示）      │
  │  │  逐个 [下达] / [取消]                      │
  │  │  [全部下达]                               │
  │  └─────────────────────────────────────────┘
  │
  ▼
工单 (Released) → 进入生产
```

**核心原则**：上下两个区块**同屏共存**，不互斥。使用者随时能在待规划明细和已生成工单之间切换操作。

---

## 3. 规划 tab 交互设计

### 3.1 上方区块：待规划明细

显示条件：计划状态为 Confirmed 或 InProgress。

**筛选规则**：只显示"无活跃工单"的明细项。活跃 = 存在 status 为 Draft/Released/InProduction 的 WorkOrder。已 Cancelled 的工单不计入——取消后明细自动回到可规划状态。

表格结构：

| 列 | 类型 | 说明 |
|----|------|------|
| ☑ 勾选 | checkbox | **默认全勾选**，使用者取消不需要的 |
| 产品 | 只读 | 产品名称 |
| 数量 | 只读 | 拆分后各行显示各自数量 |
| 排程(起→止) | date input | 可编辑排程日期 |
| 工艺路线 | 只读 | 从 `get_bom_routing(product_code)` 取，显示路线名称或"无（虚拟默认工序）" |
| 工作中心 | 只读 | 从 PlanItem 继承（工作中心主数据模块尚未建立，暂不支持选择） |
| 完整度 | 只读 | BOM/工艺/物料校验圆点 |
| 操作 | 按钮 | [拆分] |

> **工艺路线列为什么是只读**：`RoutingService` 只有 `get_bom_routing(product_code)` 返回单个 Optional（`service.rs:15`），一个产品只关联一条 routing，不存在多条可选的场景。换 routing 应在主数据的产品页面修改 `bom_routing` 关联。

> **工作中心列为什么是只读**：代码中 `work_center_id` 是 `Option<i64>`，但没有 WorkCenter 独立表/Service/管理页面。BOM 节点中的 `work_center` 是字符串不是 ID。等工作中心主数据模块建好后再支持编辑。

**拆分交互**：
- 点击行内 [拆分] → 弹出小窗输入拆分数量（如 2000 件拆成 1000 + 1000）
- 确认后：当前行数量改为第一份，新增一行（继承 product_id/routing_id/work_center_id，数量为第二份）
- 拆分行可继续拆分（递归）
- 拆分行可删除（合并回原行）

**底部操作栏**：
- **[生成草稿工单]**：收集所有勾选行 → JSON 提交 → 后端逐行 create(Draft) → 刷新整个 tab
- **[一键生成并下达]**：收集所有勾选行 → 后端逐行 create(Draft) + release() → 刷新整个 tab。**简单场景的快速通道**，等价于原来的"确认下达"行为但经过使用者确认

### 3.2 下方区块：已生成工单

显示条件：该计划存在 Draft 状态的工单。

表格结构：

| 列 | 类型 | 说明 |
|----|------|------|
| 工单号 | 只读 | WO-xxxxx |
| 产品 | 只读 | |
| 数量 | 只读 | |
| 排程 | 只读 | |
| 工艺路线 | 只读 | |
| 状态 | 只读 | Draft |
| 操作 | 按钮 | [下达] [取消] |

**底部操作栏**：
- **[全部下达]**：对该计划所有 Draft 工单逐个 release → 返回 BatchReleaseResult（成功/失败计数）→ 刷新整个 tab
- 逐个 **[下达]**：调用已有 `POST /orders/{id}/release` → 刷新整个 tab
- 逐个 **[取消]**：调用已有 `POST /orders/{id}/cancel` → 刷新整个 tab，该明细回到上方区块

> 已 Released 的工单不在此区块显示（它们属于"已下达"状态，可在"下达结果"tab 查看）。下方区块只管 Draft 工单。

### 3.3 边界处理

| 场景 | 处理 |
|------|------|
| 计划明细为空 | 上方区块显示"暂无计划明细" |
| 所有明细都有活跃工单 | 上方区块为空，仅显示下方区块 |
| 所有 Draft 工单已下达 | 下方区块消失，仅显示上方（若仍有待规划明细） |
| 产品无工艺路线 | 只读显示"无（虚拟默认工序）"，release 时自动用虚拟默认工序 |
| 拆分后数量之和 ≠ 原数量 | 前端校验：拆分弹窗禁止确认 |
| 排程结束日期 < 开始日期 | 前端校验 + 后端 `generate_work_orders()` 校验 |
| release-all 部分失败 | 返回 BatchReleaseResult，成功行 Released，失败行保留 Draft + 错误原因 |

---

## 4. 数据模型

**无需新增表或字段**。现有模型已支持：

- `WorkOrder.plan_item_id: Option<i64>` — 非唯一，一个 plan_item 可关联多个 WorkOrder（拆分）
- `WorkOrder.routing_id: Option<i64>` — 已有字段，create 时从 PlanItem 继承
- `WorkOrder.work_center_id: Option<i64>` — 已有字段，create 时从 PlanItem 继承
- `WorkOrder.status` — Draft 状态已存在于 `WorkOrderStatus` 枚举

### 4.1 状态流转

```
PlanStatus:
  Draft ──(confirm)──→ Confirmed ──(首个工单Released)──→ InProgress ──(所有工单Closed)──→ Completed

PlanItemStatus:
  Planned ──(对应工单Released)──→ Released ──(对应工单Closed)──→ Completed
                                   ↑ 生成Draft工单时不变，保持Planned
                                   ↑ 取消Draft工单后回到Planned（可重新规划）

WorkOrderStatus:
  Draft ──(release)──→ Released ──(mark_in_production)──→ InProduction ──(close)──→ Closed
    │                                                                            ↑
    └──(cancel)──→ Cancelled                                         (cancel)──→ Cancelled
```

**关键**：生成 Draft 工单时 PlanItem 状态不变（保持 Planned）。只有工单 Released 时 PlanItem 才变 Released。这样取消 Draft 后 PlanItem 自然回到可规划状态，无需额外回退逻辑。

---

## 5. Service 层改动

### 5.1 `release_to_work_orders()` 改为 `generate_work_orders()`

**文件**：`abt-core/src/mes/production_plan/service.rs` + `implt.rs` + `model.rs`

改前：对每个 item `create()` + `release()`，计划状态 → InProgress。

改后：对传入的规划项逐个 `create()`（Draft），不 release。整个操作在**同一事务**内执行，任一失败回滚（不产生孤儿 Draft 工单）。

接口签名变更：

```rust
// abt-core/src/mes/production_plan/model.rs 新增
pub struct WorkOrderPlanItem {
    pub plan_item_id: i64,
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub routing_id: Option<i64>,
    pub work_center_id: Option<i64>,
}

// service.rs 接口变更
async fn generate_work_orders(
    &self,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    plan_id: i64,
    items: Vec<WorkOrderPlanItem>,
) -> Result<Vec<i64>>; // 返回生成的工单 ID 列表
```

**校验**：`scheduled_end >= scheduled_start`，否则返回 `DomainError::Validation`。

### 5.2 批量下达

**文件**：`abt-web/src/pages/mes_plan_detail.rs` 新增 handler

```rust
// POST /admin/mes/plans/{id}/release-all
// 对该计划所有 Draft 工单逐个调用 work_order_svc.release()
// 返回 BatchReleaseResult（成功/失败计数 + 失败原因）
// 首个成功的 release 触发计划状态 → InProgress
```

### 5.3 不改动的部分

- `WorkOrderService.create()` — 已支持创建 Draft 工单，不变
- `WorkOrderService.release()` — 已支持 Draft → Released + 工序创建 + BOM 快照 + 库存预留，不变
- `WorkOrderService.cancel()` — 已有，用于取消不需要的 Draft 工单
- `RoutingService.get_bom_routing()` — 已有，规划 tab 用此接口只读显示

---

## 6. Web 层改动

### 6.1 新增路由

| 路由 | 方法 | 功能 |
|------|------|------|
| `POST /admin/mes/plans/{id}/generate` | POST | 接收规划数据(items_json)，生成 Draft 工单（事务） |
| `POST /admin/mes/plans/{id}/generate-and-release` | POST | 快速通道：生成 Draft + 立即 release 全部 |
| `POST /admin/mes/plans/{id}/release-all` | POST | 批量下达该计划所有 Draft 工单，返回 BatchReleaseResult |

**文件**：`abt-web/src/routes/mes_plan.rs` — 新增 TypedPath + 路由注册

### 6.2 计划详情页改动

**文件**：`abt-web/src/pages/mes_plan_detail.rs`

- Tab 列表增加 `("planning", "工单规划")`
- 新增 `tab_planning()` 渲染函数（上方区块 + 下方区块同屏）
- 原"确认并下达"按钮和 modal 删除（被规划 tab 取代）
- `plan_detail_page()` 需要额外加载：该计划的所有 WorkOrder（用于判断哪些明细有待规划/活跃工单）

### 6.3 HTMX 组件设计

整个规划 tab 作为一个 HTMX 组件（`hx-target="this"` + `hx-swap="outerHTML"`）：
- 生成操作 → 返回更新后的整个 tab（上方刷新 + 下方出现/刷新）
- 下达操作 → 返回更新后的整个 tab（下方刷新，不单行更新，保证上下区块计数同步）
- 取消操作 → 返回更新后的整个 tab（下方行消失，上方对应明细重新出现）

### 6.4 新增 JS 文件

**文件**：`static/wo-planning.js`

- 拆分行管理（增删行、数量校验之和 = 原数量）
- 日期校验（end >= start）
- `collectItems()` 收集勾选行数据为 JSON
- 提交时填入 `<input type="hidden" name="items_json">`
- 与 `bom-edit.js` / `lineItemCalc` 同一模式（Hyperscript `call` 调用）

---

## 7. 实现清单

| 序号 | 修改项 | 涉及文件 | 修改类型 | 优先级 |
|------|--------|---------|---------|--------|
| 1 | `release_to_work_orders()` 改为 `generate_work_orders()`，只 create 不 release，事务保护 | `abt-core/src/mes/production_plan/service.rs` + `implt.rs` + `model.rs` | 修改 | P0 |
| 2 | 新增规划 tab 渲染函数 `tab_planning()`（上下双区块同屏） | `abt-web/src/pages/mes_plan_detail.rs` | 新增 | P0 |
| 3 | 删除原"确认并下达"按钮和 modal | `abt-web/src/pages/mes_plan_detail.rs` | 删除 | P0 |
| 4 | 新增 `POST /plans/{id}/generate` 路由 + handler | `abt-web/src/routes/mes_plan.rs` + `mes_plan_detail.rs` | 新增 | P0 |
| 5 | 新增 `POST /plans/{id}/generate-and-release` 快速通道 | `abt-web/src/routes/mes_plan.rs` + `mes_plan_detail.rs` | 新增 | P0 |
| 6 | 新增 `POST /plans/{id}/release-all` 返回 BatchReleaseResult | `abt-web/src/routes/mes_plan.rs` + `mes_plan_detail.rs` | 新增 | P0 |
| 7 | 新增 `static/wo-planning.js`（拆分/收集/校验逻辑） | `static/wo-planning.js` | 新增 | P0 |
| 8 | 规划 tab 整体作为 HTMX 组件（操作后整 tab 刷新） | `abt-web/src/pages/mes_plan_detail.rs` | 新增 | P0 |
| 9 | 拆分弹窗（数量输入 + 校验） | `mes_plan_detail.rs` + `wo-planning.js` | 新增 | P1 |
| 10 | 日期校验（end >= start，前后端双重） | `wo-planning.js` + `generate_work_orders()` | 新增 | P1 |
| 11 | 设计文档同步 `docs/uml-design/04-mes.html` | `docs/uml-design/04-mes.html` | 修改 | P1 |

---

## 8. 风险与注意事项

1. **已有数据兼容**：已 Released/InProgress 的工单不受影响。已用旧 `release_to_work_orders()` 下达的计划保持现状。
2. **计划状态语义**：生成 Draft 工单时计划保持 Confirmed。首个工单 Released 时触发 InProgress（在 release-all / generate-and-release / 逐个 release 的 handler 里判断）。所有工单 Closed 时触发 Completed（后续迭代）。
3. **PlanItem 状态语义**：生成 Draft 时 PlanItem 保持 Planned（不变）。工单 Released 时 PlanItem → Released。取消 Draft 工单时 PlanItem 保持 Planned（自然回到可规划状态）。
4. **部分规划**：使用者可以先勾选 3 个明细生成工单，剩余 2 个仍在上方区块，下次再处理。上方区块只显示"无活跃工单"的明细项。
5. **权限**：规划生成需要 `WORK_ORDER.create`，下达需要 `WORK_ORDER.update`，已有权限定义。
6. **并行操作**：多人同时操作同一计划时，由 WorkOrder 的 `version` 乐观锁保护 release 操作。generate 操作是 INSERT（新工单），无并发冲突。
7. **工作中心主数据依赖**：工作中心列暂时只读。待 WorkCenter 主数据模块建立后，改为 dropdown 选择。
