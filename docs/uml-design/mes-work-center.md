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
