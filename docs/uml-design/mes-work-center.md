# MES 生产作业中心（WorkCenter）设计

> 关联原型：`04-work-center-hub.html`（Open Design，生产作业中心 Hub v2）。
> 现状：聚合「生产需求池 / 订单排期 / 工单」三视角于一屏，就地下达与报工。

## 1. 定位

MES 作业中心是车间执行入口：一进系统看到「待排产需求 → 待下达工单 → 生产中工单」全链路，就地完成「下达 / 分批 / 报工」，尽量不跳工单详情页。

与 WMS 作业中心（见 `wms-work-center.md`，纯待办计数 + 跳转）不同，MES 作业中心**就地承载操作**（drawer 下达/报工），不仅是导航看板。

## 2. 与原型 `04-work-center-hub.html` 的对应

| 原型区块 | 实现对应 | 一致性 |
|---|---|---|
| `detail-header` + 内嵌 `todo-nav` 锚点条 | `render_anchor_nav`（独立 sticky 锚点条） | 形态简化：未内嵌 header，未含 meta chips（日期/车间主任/班次）与「缺料/逾期」pill |
| 分组 1「生产需求池」3 tab（物料汇总 / 订单行明细 / 订单排期） | `wc-demand-card`，`view=material\|detail\|schedule` 三 tab | ✅ 一致（订单排期为需求池第 3 tab，**非独立 card**） |
| 分组 2「工单」行内 `detail-grid` 展开 | `wc-orders-card`，展开走独立 `row-detail` 端点 | 形态简化：未用原型行内 detail-grid |
| 报工 / 转化 / 下达 drawer | 下达 drawer + 报工 drawer | 「转化」drawer（需求→工单一键贯通）未实现 |

**当前为最小对齐版本**：结构上「订单排期并入需求池第 3 tab」与原型一致；原型的 disclosure 折叠分组、转化 drawer、工单行内展开等形态暂未实现（按需迭代）。

## 3. 页面结构

首页 `get_work_center`（`WcPath`）内联渲染：

- 锚点条 `render_anchor_nav`：活跃待办总数 + 「待下达 / 生产中」chip（锚定对应 card）
- 需求池 card shell（`#wc-demand-card`，懒加载 `WcDemandPath`）
- 工单 card shell（`#wc-orders-card`，懒加载 `WcOrdersPath`）
- 下达 drawer overlay（`release-overlay` / `release-drawer`）

## 4. 端点契约（TypedPath）

| 路径 | 方法 | handler | 说明 |
|---|---|---|---|
| `/admin/mes/work-center` | GET | `get_work_center` | 首页（2 card shell + drawer） |
| `/admin/mes/work-center/demand` | GET | `get_demand_card` | 需求池 card，`view=material\|detail\|orders\|batches` 四 tab + 搜索/筛选/分页；可选 `order_id`（从订单详情页「自制需求」按钮跳入）：detail 视图按销售订单过滤、显示该订单**全状态** demand（`MesDemandRepo::find_demands` 在 `order_id` 有值且 `status=None` 时不再强制 `demand_status=1`），筛选栏顶部显示「销售订单 #N·全部状态」chip + 清除 |
| `…/orders/{id}/drawer` | GET | `get_order_drawer` | 工单详情 drawer body（工单号点击，只读：头部摘要/工艺路线/物料/来源SO） |
| `…/orders/{id}/release-drawer` | GET | `get_release_drawer` | 下达 drawer body |
| `…/orders/{id}/release` | POST | `release_order` | 下达（release + 分批单事务），广播 `woChanged` |
| `…/orders/{id}/split-multi` | POST | `split_multi` | 多批分批，广播 `woChanged` |

工单 tab（`view=orders`）复用 `WorkOrderService.list`，行内「下达」入口走 `release-drawer`，工单号点击走 `drawer` 查看只读详情。

## 5. MesWorkCenterSummary

```rust
pub struct MesWorkCenterSummary {
    pub pending_release: u64,   // 待下达（Draft + Planned）
    pub in_production: u64,     // 生产中
    pub fn total(&self) -> u64; // 活跃待办总数
}
```

锚点条 chip 仅基于这两个字段。原型里的「缺料 / 逾期」pill 暂无数据来源（summary 未含 shortage/overdue），未实现——若要补需先在 `MesWorkCenterService` 扩字段 + repo 查询。

## 6. 单端点交互模式

- 每个 card 一个 GET 端点；card 内 tab/搜索/分页走该端点 + `hx-select="#wc-xxx-card"` + `hx-swap="outerHTML"` 局部刷新。
- 写操作 POST 广播 `HX-Trigger: woChanged`；需求池 schedule view 与工单 card 监听 `woChanged from:body` 自刷新。
- 工序由工单创建时从 BOM 关联工艺路线自动加载（`WorkOrderService::create` 内 `try_load_routings_from_bom`），只读不可编辑；BOM 未关联工艺路线时下达 drawer 引导用户去「工艺路线管理」关联。
- 工单 tab 工单号点击 → `get_order_drawer` 弹工单详情 drawer（只读，区块参考 ERPNext Work Order / Odoo MO：头部摘要 + 来源销售订单 + 工艺路线 + 物料齐套 + 备注）。
- 批次 tab 支持工单号搜索（`wo_no` → `BatchListFilter.work_order_no` → `wo.doc_number ILIKE`），按工单筛选其所有生产批次。

## 7. 批次工序流转：领料 → 收料 → 报工

批次 drawer 工序矩阵第 3 列动作位按「批次状态 + 领料/发料状态」推进，因果链。

**每道工序的状态判定**：
- `has_req`（领料单存在，picking `Confirmed(2)/Done(3)`）—— 防重复领料 + InProgress 报工前置；查 `PickingService::list_requisitioned_routing_ids`（`find_routing_ids_by_batch`，`status IN (2,3)`）。
- `has_issued`（仓库已发料完成，picking `Done(3)`）—— **收料前置**；查 `PickingService::list_issued_routing_ids`（`find_issued_routing_ids_by_batch`，`status = 3`）。
- `has_output`（工序有产出品 `product_id`）—— 无产出工序（检测/检验）视同已领料，跳过领料。

**收料前置规则**（本次修复）：

> 生产端点「领料」只创建领料单并 `confirm`（`Draft→Confirmed`），单据进入仓库「待领料」队列——**此时物料尚未到手**。必须等仓库执行 `issue()` 发料、所有行 `qty_done ≥ qty_requested`、picking 置 `Done` 后，生产端才能点「收料」开工（`Pending→InProgress`）。仓库未发齐前，动作位显示「⏳ 待仓库发料」，不显示收料按钮。

工序矩阵动作位（批次 `Pending` + 当前工序，即第 1 道）四态：

| 状态 | 条件 | 动作 |
|---|---|---|
| 收料 | `has_issued` 或 `!has_output` | 收料按钮（开工 `start_batch`） |
| 待仓库发料 | `has_req && !has_issued` | ⏳ 灰态（已领料、仓库未发齐） |
| 领料 | `!has_req && has_output && 齐套` | 领料按钮（建领料单→`confirm`） |
| 欠料 | `!has_req && !齐套` | 领料置灰 |

**service 层兜底**：`ProductionBatchService::start_batch` 在 `Pending→InProgress` 前校验——开工对象（第 1 道工序）若有领料单（Active picking），须 `Done` 才能开工，防绕过前端直接 POST `batch_receive`。Backflush（倒冲，无领料单）/无产出工序放行（无消耗物料或倒冲，不领料）。部分发料（`Confirmed` + 部分 `qty_done`，未 `Done`）不可收料。

## 8. 委外工序门控（Issue #277）

委外工序（`work_order_routings.is_outsourced`）走独立动作流，不参与车间领料/收料/报工。批次 drawer 工序矩阵动作位按 OSA（OutsourcingOrder）状态判定：

| Action | OSA 状态 | 渲染 |
|---|---|---|
| `OsaCreate` | 无 OSA + 当前道 | 创建委外单（drawer） |
| `OsaDraft` | `Draft` | 灰态「待仓库发料」（仓库作业中心 OutsourceIssue 域执行发料） |
| `OsaWaitReceipt` | `Sent` | 灰态「待仓库入库」（仓库作业中心 OutsourceReceipt 域执行产出品入库） |
| `OsaReceive` | `Received` + 工序未完成 | 绿色「委外收货」（writeback 推进工序） |
| `Done` | `Received` + 工序已完成 | ✅ 已完成 |

**Issue #277 控制链**（产出品入库与工序闭环解耦）：
- 仓库「委外产出品入库」（OutsourceReceipt 域 `osa_receipt`）调 `om.receive`：IQC 门禁 + 产品入目标仓（Process→WIP-SHOP，Full/Material→仓库选）+ 消耗虚拟仓 + 立应付 + 成本，OSA `Sent→Received`。**不推进工序**。
- MES「委外收货」（`osa_receive`）瘦身为工序闭环确认：校验 OSA=`Received` 后同步直调 `OutsourcingReceivedHandler::writeback`（合格量取 `OSA.completed_qty`，非 `planned_qty`），推进 `batch_routing_progress` + `current_step`。
- 仓库未产出品入库前（OSA=`Sent`），MES 收货按钮置灰（`OsaWaitReceipt`），实现「账实相符」硬约束。
- `om.receive` 不再发布 `OutsourcingReceived` 事件（writeback 全靠同步直调）；OM 详情页 `receive_order`（Process 类型）亦同步调 writeback。

