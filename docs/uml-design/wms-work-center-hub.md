# WMS 作业中心 Hub 设计（WorkCenter Hub）

> 关联：#93 已上线 WorkCenter（dashboard 式计数墙）→ 升级为「就地可执行工作台」
> 参照：`03-shipping-hub.html`（Doc Hub 范式，本设计同款）；[`wms-doc-hub.md`](wms-doc-hub.md)（Doc Hub 契约，互为镜像）
> 配套原型：`03-work-center-hub.html`
> 状态：**已实现**（单端点 + hx-select-oob，2026-06 落地，见 §5）；**2026-06 tab 化 + 数据库分页**：锚点条 + tab 主体（5 tab + 分页 + 过滤），`WorkCenterRepo` 跨域聚合 SQL 真正分页（只查当前页 20 条），见 §2.2 / §4.4 / §5；**2026-07 出库流合并**：删「待拣货」并入「待出库」（6→5 tab），消除重复计数 + 对齐主流 ERP，见 §3 / §9.6；**2026-07 职责二分**：移除 `view_toggle`（待办/全部二级切换），作业中心回归纯待办；全量查询独立到「单据台账」页（`/admin/wms/ledger`，5 类型 tab + 状态 + 单号 + 日期），见 §1.2 / §11
> 现行实现：`abt-web/src/pages/wms_work_center.rs` + `abt-core/src/wms/work_center/{service,implt,repo,model}.rs`（`WorkCenterService::{summary, list_pending}` 委托 `WorkCenterRepo` 数据库分页；共享 `#wc-drawer-overlay`）

## 1. 背景与定位

### 1.1 问题：当前 WorkCenter 是 dashboard，不是工作台

`wms_work_center.rs` 现状是「计数墙 + 跳转」：

- 7 张卡片只显示待办计数，点击 → 跳各域列表页 → 再进详情 → 才能操作，**两段断点**
- 底部「紧急/临期」区是**示意假数据**（#93 followup P1 item 4 至今未接）
- htmx 几乎没用：整页一次 `summary()` SSR；原型里 `setInterval` 局部刷新注释掉没做
- 仓库员「一个页面完成更多工作」做不到——看一眼 7 个数字就跳走

### 1.2 目标：就地可执行工作台（纯待办）+ 全量查询外移

**2026-07 职责二分**（对齐三家 ERP 共识）：作业中心只处理待办，全量查询独立到「单据台账」页。

- 左侧菜单：「作业中心」（处理待办）+ 「单据台账」（全量检索，`/admin/wms/ledger`）
- 作业中心采用 `03-shipping-hub.html` 同款「少即是多」范式（摘要带 + tab + drawer 就地操作），**摒弃卡片网格与「待办/全部」二级切换**：
  - **摘要带**：总待办 / 紧急 / 逾期，替代「您有 N 项待办」
  - **5 环节 tab 主体**：切 tab 看该环节待办队列（分页 + 过滤 + 紧急度排序），**不是跳转**
  - **队列行就地 drawer 操作**：拣货 / 收货 / 确认发出，复用 Doc Hub 的 drawer 组件，不离开本页
  - 各 tab 右上角「查看全部」→ 跳单据台账对应类型 tab（不再在作业中心内做全量视图）
- 仓库员进 WorkCenter 即可逐项处理日常作业，**一页完成**

### 1.3 与 Doc Hub 的镜像关系

两者 disclosure 结构对称，组成 WMS 前端统一范式：

| | WorkCenter Hub（本设计） | Doc Hub（`wms-doc-hub.md`） |
|---|---|---|
| 聚合维度 | 按**环节**聚合多张单据 | 按**单据**拆解多个子域 |
| disclosure 内容 | 该环节 top N 待办单据队列 | 该单据的明细/拣货/事务/日志 |
| Header | 摘要带（无状态步骤条/来源链） | 单号 + 状态步骤条 + 来源链 + 摘要带 |
| 共享 | **drawer 组件**（拣货 / 收货 / 确认发出） | |
| 跳转 | 队列行点单据号 → Doc Hub 看全生命周期 | — |

## 2. 页面骨架（对齐 shipping-hub，无卡片）

### 2.1 Header 层
- 标题：仓库作业中心 + 当班员 + 日期
- **待办通知锚点条** `todo-nav`（sticky 吸顶）：左侧总待办数 + 横向环节 chip 列表。**只显示 `count > 0` 的环节**（无待办的环节不露脸，少即是多）；每个 chip 带计数 + 异常染色（逾期红 / 临期橙）+ 脉冲点。点击 chip → `toggleAndScroll` 平滑滚动并展开对应 disclosure。替代原「您有 N 项待办」单一数字
- 就地操作后（`taskDone` 事件）整条重拉：计数下降、处理完毕的 chip 自动消失
- **异常信息架构下沉**：逾期/临期**不**汇总成独立聚合 pill，而是下沉到三处——① chip 染色（逾期 danger / 临期 warn）+ 脉冲点；② disclosure 图标右上角角标（红点 overdue / 橙点 soon）；③ disclosure 摘要文案「N 笔 · X 逾期 · Y 临期 · hint」。`UrgentSummary` 按环节拆分驱动（见 §4.2）
- **无状态步骤条、无来源链**（WorkCenter 不是单据，是聚合视图）

### 2.2 Tab 主体层（5 环节，分页 + 过滤）
锚点条下方单个 tab 主体卡片 `#wc-domain-card`，5 环节平铺为 tab（`status_tabs_with_param`：每个 tab 带计数 badge，切 tab 强制 `page=1` 并携带当前 filter）：
- **tab 栏**：待收货 / 待出库 / 待领料 / 待调拨 / 待盘点（**2026-07**：原待拣货 + 待发货合并为待出库，队内按 `OutboundStage` 分阶段显示拣货/发货按钮）；点击锚点条 chip 同效切 tab（`hx-vals={"domain","page":"1"}`）
- **过滤表单** `#wc-domain-filter`：keyword 搜索（防抖 300ms）+ 紧急度筛选（逾期/临期/正常）+ 来源筛选（仅待收货 PO/工单）；hidden `domain` 携带当前 tab
- **队列表格**：复用 `render_task_table`（紧急度染色 + 行内 drawer 操作），展示当前 tab 经 `list_pending(domain, filter, page)` 查询的待办队列
- **分页**：`pagination` 组件（每页 20 条），携带 filter + domain
- **废弃原 disclosure 折叠卡片**（固定懒加载 50 条、无分页、无过滤）；todo-nav 锚点跳转改为 HTMX 切 tab
- **2026-07：移除「待办/全部」二级切换**（`view_toggle`，曾塞在每个 tab 内），全量查询走单据台账（§11）；作业中心 tab 主体只渲染待办队列，各 tab 右上「查看全部」跳台账对应类型

### 2.3 Drawer 层（就地操作，复用 Doc Hub drawer）
队列每行带快操作按钮，点击弹出 drawer（与 shipping-hub 同套 drawer 组件）。

### 2.4 整页
- 首屏 `GET /admin/wms/work-center` 查 `summary()` + `urgent_summary()` + 默认 tab（arrival）`list_pending`，渲染 Header + todo-nav + tab 主体
- 切 tab / 搜索 / 分页才按需 `list_pending` 查其他环节；操作后 htmx 片段级刷新（`#wc-domain-card` + `#todo-nav`）

## 3. 5 环节映射 + 就地操作范围

> **2026-07 变更：原「待拣货」+「待发货」合并为「待出库」（6 环节 → 5 环节）**。三条动因：
> ① **去重复计数**——`PickList(Draft)` 是 `Shipping(Picking)` 的严格子集（`outbound.pick()` 在生成拣货单的同一事务把发货单 Confirmed→Picking，见 `outbound/implt.rs::pick`），而待发货又含 Picking，导致同一张出库单在两个 chip 各计一次，摘要总数虚高；
> ② **纠语义错位**——原待发货把 `Confirmed`（尚未拣货）也算进去，而真正「拣完待发」（Picking + PickList Picked）反而与「拣货中」混在一个数里看不出来；
> ③ **对齐主流 ERP**——Odoo 一张 `stock.picking` 用状态走全程、ERPNext Pick List→Delivery Note 顺序串联、OFBiz Picklist→Shipment，无一将拣货/发货拆成两个并列待办。合并后「待出库」一条队列按 `OutboundStage` 分阶段，就地动作按阶段分发。

| 环节 | pending 状态（对齐 summary） | 就地操作（drawer） | 复杂操作（跳 Doc Hub） |
|---|---|---|---|
| 待收货 | 采购 PO(Confirmed/PartiallyReceived) + 工单(完工未入库)，`PendingTask.source_kind` 区分 | **收货入库**（PO 调 `PurchaseStockInService` / 工单 record 库存，见 §10） | — |
| 待出库 | `ShippingStatus::Confirmed` / `Picking`，LEFT JOIN `pick_lists` 取拣货单状态，按 `OutboundStage` 分阶段（见下） | Unpicked→**直接发货**（选仓 direct_ship）/ Picking→**录入拣货**（pick_list_id）/ ReadyToShip→**确认发出**（shipping_id） | — |
| 待领料 | `RequisitionStatus::Confirmed` / `PartiallyIssued` | 发料 | 退料 |
| 待调拨 | `TransferStatus::Draft` / `InTransit` | 确认收货 | — |
| 待盘点 | `CycleCountStatus::Draft` / `Counting` / `PendingReview` | — | 盘点录入（跳 Doc Hub） |

**待出库阶段判定**（`OutboundStage`，由 repo SQL `CASE` 算出，对应 `outbound.pick()` → `complete_pick` → `ship` 的生命周期）：
- `Unpicked`：`shipping.status = Confirmed(2)`（尚未 pick，无拣货单）→ **就地「直接发货」**（选仓 drawer → `direct_ship`，跳过拣货）。参考三 ERP（Odoo ship_only / ERPNext / OFBiz quick ship）——拣货非强制，ABT 原强制拣货是过度设计。拣货（pick）保留可选（多库位/需走动场景仍可走）
- `Picking`：`shipping.status = Picking(3)` 且 `pick_list.status = Draft(1)`（拣货录入中）→ **就地「录入拣货」**（drawer hidden id = pick_list_id，复用原待拣货 tab 的录入能力）
- `ReadyToShip`：`shipping.status = Picking(3)` 且 `pick_list.status = Picked(2)`（拣货完成待发）→ **就地「确认发出」**（drawer hidden id = shipping_id）

**批量直接发货**（2026-07，未拣单）：待出库队列未拣（Unpicked/Confirmed）行带 checkbox（`.wc-ship-cb`），勾选多张 → 底部固定批量栏（`#wc-batch-bar`，复用 MES `.show` 显隐范式）选发货仓 → 提交 `action=batch_ship` + `ids` + `warehouse_id`。服务端循环调 `direct_ship`，外层 `post_work_center_action` 事务任一失败**整体回滚**。其他域不批量（收货行级仓库复杂）。

**就地操作范围克制**：只有「快操作」（单一动作或一条闭环）就地；复杂操作（盘点录多库位、拣货中多库位部分拣、形态转换）跳 Doc Hub。点单据号始终可跳 Doc Hub 看全貌。**待收货 drawer** 按 `source_kind` 分 PO 收货（调 `PurchaseStockInService` 直收入库 + 回写 PO/立应付）与工单入库（record 库存），一次提交闭环（见 §10），仍属就地快操作。

> 「待质检」环节已随来料通知取消而移除（工厂不做来料质检）。来料通知模块/`ArrivalAcceptedHandler` 本步保留（处理历史/手动来料通知），第二步再删。

## 4. 接口设计（WorkCenterService 扩展）

### 4.1 trait
```rust
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 各环节 (total/overdue/soon) 统计（锚点条 chip + tab badge + 染色）
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary>;

    /// 某 tab 环节的待办队列（**数据库分页** + keyword/紧急度/来源过滤，按紧急度排序）
    async fn list_pending(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        domain: WorkCenterDomain, filter: PendingTaskFilter, page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>>;
}
```

> `urgent_summary` / `urgent_of_domain` / `list_pending_with_stats` 已移除：紧急度统计合并进 `summary`（每域 `DomainStats`）；`list_pending` 改数据库分页后无需「一次 fetch 同时拿 urgent」。

### 4.2 model 新增
```rust
pub enum WorkCenterDomain {
    Arrival, Outbound, Requisition, Transfer, CycleCount,
    // 2026-07：删 Pick（合并入 Outbound，见 §3）；Inspection 早已随来料质检取消移除
}

pub enum Urgency { Normal, Soon, Overdue }

/// 出库流阶段（仅 Outbound domain 有意义；2026-07 合并待拣货/待发货后新增，
/// 驱动待出库队列的就地动作分发）。对应 outbound.pick() → complete_pick → ship 生命周期。
pub enum OutboundStage { Unpicked, Picking, ReadyToShip }

/// 跨域统一待办视图（各域实体在 WorkCenterService 内映射成此结构）
pub struct PendingTask {
    pub doc_id: i64,
    pub doc_number: String,
    pub domain: WorkCenterDomain,
    pub source_kind: TaskSourceKind,           // 仅 Arrival 有意义（PO/工单）；其他域占位
    pub counterparty: String,                  // 客户 / 供应商 / 产品名
    pub summary: String,                       // "待收 320 件"
    pub expected_at: Option<DateTime<Utc>>,
    pub received_at: Option<DateTime<Utc>>,    // 收到时间（单据 created_at，进入待办的时刻）
    pub urgency: Urgency,
    pub outbound_stage: Option<OutboundStage>, // 仅 Outbound 有意义；其他域 None
}

/// 单环节统计（`WorkCenterSummary` 各域字段）
pub struct DomainStats { pub total: u64, pub overdue: u64, pub soon: u64 }

/// 作业中心待办汇总（5 域各一个 DomainStats）
pub struct WorkCenterSummary {
    pub arrivals: DomainStats, pub outbounds: DomainStats,
    pub requisitions: DomainStats, pub transfers: DomainStats, pub cycle_counts: DomainStats,
}
impl WorkCenterSummary {
    pub fn of(&self, d: WorkCenterDomain) -> DomainStats;   // 某 domain 统计
    pub fn total(&self) -> u64;                              // 跨环节待办总数
}

/// 待办队列过滤（`list_pending` 用；过滤下推到 `WorkCenterRepo` SQL）。三者 AND 组合。
pub struct PendingTaskFilter {
    pub keyword: Option<String>,            // 模糊匹配 doc_number / counterparty（大小写不敏感）
    pub urgency: Option<Urgency>,           // 紧急度筛选
    pub source_kind: Option<TaskSourceKind>,// 仅 Arrival：PO / 工单
}
```

### 4.3 紧急度口径（消化 #93 followup P1 item 4）
| 环节 | 逾期 `Overdue` | 临期 `Soon` |
|---|---|---|
| 待收货 | `arrival_notice.expected_date < today` 且 Draft | `≤ today+N` |
| 待出库 | `outbound.expected_ship_date < today` 且 Confirmed/Picking | `≤ today+N` |
| 其他 | 按各自 `expected_date` | — |

> **2026-07**：合并删去原「待拣货」行（`pick_list.created_at < now − 阈值` 的 age 判超时）。待出库统一用 `expected_ship_date` 单一口径——承诺发货日是最有业务意义的 Deadline，未拣/拣货中/待发任一阶段到期都同样紧急。tradeoff 见 §9.6：丢失「拣货 created_at 超时」的独立预警（该阈值本就是临时硬编码，Phase D 待进 settings）。相应 `repo.rs` 删 `PICK_TIMEOUT_HOURS` 常量与 `use_age` 分支，urgency CASE 单一化。

### 4.4 数据库分页（`WorkCenterRepo` 聚合视图）

`WorkCenterService` 委托 `repo.rs::WorkCenterRepo` 跨域聚合查询（dashboard 只读 SQL），实现**真正数据库分页**——每域只查当前页 20 条（`LIMIT/OFFSET`），不再拉 `FETCH_LIMIT` 全量到内存。各业务域 service 不受影响。

- **`count_domain(domain, today) -> (total, overdue, soon)`**：`COUNT(*)` + `COUNT(*) FILTER (WHERE urgency_rank=2/1)`，`summary` 用（5 域各一条轻量 count）
- **`list_domain_page(domain, filter, today, page)`**：`status = ANY(...)` + keyword `ILIKE`（doc_number/counterparty）+ urgency CASE 筛选 + `ORDER BY urgency_rank DESC, expected ASC NULLS LAST` + `LIMIT/OFFSET`，counterparty 走 JOIN（suppliers/customers/warehouses/work_orders/products）
- **urgency 排序下推**：`urgency_rank`（Overdue=2 > Soon=1 > Normal=0）与 `expected_date` 单调对应，`CASE WHEN expected<today THEN 2 WHEN expected<=today+SOON_DAYS THEN 1 ELSE 0 END` 一列同时驱动排序与筛选（**2026-07**：删原 Pick 的 `created_at < now() - interval 'PICK_TIMEOUT_HOURS hours'` age 分支，urgency CASE 收敛为单一日期口径，`use_age` / `PICK_TIMEOUT_HOURS` 废弃）
- **待出库阶段下推**（Outbound 域，2026-07 新增）：`LEFT JOIN pick_lists pl ON pl.outbound_id = t.id AND pl.deleted_at IS NULL`，SELECT 多带 `CASE WHEN t.status=2 THEN 'Unpicked' WHEN pl.status=1 THEN 'Picking' WHEN pl.status=2 THEN 'ReadyToShip' ELSE 'Unpicked' END AS stage`，映射成 `PendingTask.outbound_stage`（前端按阶段显示「拣货」/「发货」按钮，替代原"待拣货 / 待发货"两个独立 tab）
- **Arrival UNION ALL**：PO 子查询（JOIN suppliers）+ WO 子查询（JOIN products + LEFT JOIN `inventory_transactions` 算 received，`WHERE completed_qty > received`），外层 ORDER BY + LIMIT
- 各域待办状态集 / 到期日字段 / counterparty JOIN 见 `repo.rs::simple_cfg`
- **架构权衡**：`work_center` 作为跨域聚合视图，repo 直接查各业务域表（只读），等同 reporting 层；业务域 service/列表页不受影响
- 查询失败容错沿用 `cnt` 模式（`tracing::warn`，该环节记空队列，不连累整页）

## 5. HTMX 契约（单端点 + hx-select / hx-select-oob）

**单一端点** `/admin/wms/work-center`（GET + POST），所有交互收敛到一个地址，对齐「列表页单端点」约束。

### 5.1 GET 分支（query 参数）
| query | 返回 | 客户端用法 |
|---|---|---|
| 无（非 htmx） | 整页（标题 + todo-nav + tab 主体 `#wc-domain-card` 默认 arrival + drawer/picker 壳） | 首屏 SSR |
| `?domain=&keyword=&urgency=&source=&page=`（htmx） | tab 主体片段 `#wc-domain-card`（tab 栏 + filter + 表格 + 分页） | tab 切换 / 搜索 / 分页，`hx-target/hx-select=#wc-domain-card` |
| `?drawer={action}&id={id}` | drawer body（标题栏+表单） | 行内按钮 `hx-get + hx-target=#wc-drawer-body` + `_="on 'htmx:afterRequest' add .open to #wc-drawer-overlay"` 打开 |

`#wc-domain-card` 整体可被 `hx-target`/`hx-select` outerHTML 替换；切 tab 由 `status_tabs_with_param` 的 `hx-vals={"domain","page":"1"}` 驱动（覆盖 filter-form 内 hidden domain 并回第 1 页）。

### 5.2 POST：就地操作 + 多区联动（hx-select-oob）
drawer 表单 `hx-post=/admin/wms/work-center`（hidden `action`/`id`；收货/拣货 另带 hidden `items_json`；收货额外带顶层 hidden `idempotency_key` 防双击重复入库）。服务端执行动作后**重渲染「当前 tab 主体（受影响 domain）+ todo-nav」**，一次响应含两区：

- `hx-target=#wc-domain-card` + `hx-select=#wc-domain-card` → 替换 tab 主体（受影响 domain 由 `action_domain(action)` 推断；单据出列、计数下降）
- `hx-select-oob=#todo-nav:outerHTML` → 同时更新摘要带
- `_="on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay"` → 关 drawer

**不用 HX-Trigger 广播**——hx-select / hx-select-oob 从同一响应各取所需，自然协调（避免事件监听 `from:body` 语法坑与「整页重拉」的浪费）。

**直接发 / 批量发**（2026-07）：未拣单就地 `direct_ship`（选仓 drawer → `direct_ship` action）；批量栏 `#wc-batch-bar` `hx-post` 携带 `action=batch_ship` + `ids` + `warehouse_id`（选仓下拉），`dispatch_action` 循环 `direct_ship`，事务整体回滚。`WorkCenterActionForm` 加 `ids` / `warehouse_id`；`action_domain("direct_ship"|"batch_ship") = Outbound`。`ShippingStatus` 加 `Confirmed→Shipped` 转换（migration 078）；`ship()` 抽 `do_ship` 共享 `direct_ship`。响应同单据操作（`#wc-domain-card` + `#wc-total-badge` oob）。

### 5.3 行级明细（收货/拣货）走 JSON hidden input
`serde_urlencoded`（axum::Form）不支持 `items[idx][field]` 嵌套（旧实现因此 422）。改 hidden `items_json`：表单 `onsubmit`（早于 htmx 的 submit 监听）把 `[data-row]` 行的 `[data-k]` 输入收成 JSON 字符串 → handler `serde_json::from_str::<Vec<RowJson>>`（字段统一 String，服务端 parse），对齐 quotation/sales_order 的 `ItemWeb` 范式。

- 拣货：`onsubmit="wcCollectItems(this)"`（`static/app.js`），行字段 `pick_list_item_id/picked_qty/warehouse_id/bin_id`
- **收货入库**：`onsubmit="wcReceiveSubmit(this)"`（校验实收>0 + 仓库必填后调 `wcCollectItems`），行字段 `product_id/received_qty/batch_no/warehouse_id/bin_id`（per-行目标仓库/库位；行内 `warehouse_bin_cell` 按钮 → `#bin-picker-modal` 弹窗：左仓库 + 右库位，inbound 模式排除他物料占用 + 同物料排前推荐）

### 5.4 权限
GET `INVENTORY/read`；POST `INVENTORY/update`（作业中心为仓库工作台域，就地操作统一此权限码；发货从工作中心发出亦用 INVENTORY/update，与 shipping_detail 的 SHIPPING/update 不同——按工作台域收敛）。

## 6. 约束兼容（同 shipping-hub §7）
事务包裹 / TypedPath / `hx-target="closest"` / hyperscript `_=` / UnoCSS 原子类 / fragment 子资源。drawer 直接复用 Doc Hub 组件（`hx-target="this"` 自包含）。

## 7. 与 #93 followup 关系
- **P1 item 4（紧急/临期真实数据）**：★ **消化**。`urgent_summary` + 各 disclosure 红点 + 队列紧急度排序，替掉原假数据墙。
- **P1 item 5（拣货录入页）**：与 Doc Hub 共享 drawer，一并消化。
- P1 item 6（ship 校验 Picked）：Doc Hub 侧。

## 8. 实施阶段
- **Phase A（后端）**：`WorkCenterService` 加 `list_pending` / `urgent_summary` + model；紧急度计算 + 单测。✅
- **Phase B（前端骨架）**：`wms_work_center.rs` 重写为 Hub（摘要带 + 7 disclosure + drawer 骨架），**删卡片网格**。✅
- **Phase C（就地操作）**：拣货 / 收货 / 发货 / 领料 / 调拨 drawer 接各域 service。✅（单端点 + 自洽卡片 + hx-select-oob，收货/拣货行级明细走 items_json 修 422；见 §5）
- **Phase D（配置化）**：临期 N 天 / 拣货超时阈值进 `wms/settings`。⏳ 待办
- **Phase E（tab 化）**：disclosure 折叠卡片 → 锚点条 + tab 主体（`status_tabs_with_param` 6 tab + 计数 badge + `render_domain_filter` + `pagination`，每页 20）；`list_pending` 加 `PendingTaskFilter`（keyword / 紧急度 / 来源）内存 retain 过滤；todo-nav chip 与 drawer 重渲染目标 `#d-{slug}` → `#wc-domain-card`。✅（2026-06）
- **Phase F（出库流合并）**：删「待拣货」tab 并入「待出库」（6→5 tab），消除重复计数（`PickList(Draft)` 是 `Shipping(Picking)` 的严格子集，原待拣货计数 ⊆ 待发货，Picking 段被算两次）+ 对齐主流 ERP；`WorkCenterDomain` 删 `Pick`、`WorkCenterSummary` 删 `picks`、`PendingTask` 加 `outbound_stage: Option<OutboundStage>`；`repo.rs` 删 `use_age`/`PICK_TIMEOUT_HOURS`（urgency 收敛单一日期口径），Outbound 分支加 `LEFT JOIN pick_lists` 算 stage；前端 Outbound tab 按 `OutboundStage` 显示拣货/发货按钮，原"未拣货不能直接 ship"守卫改为阶段驱动。✅（2026-07）
- **Phase G（批量发货 + 默认聚焦 + header 精简）**：待出库 ReadyToShip 批量发货（勾选多行 → `batch_ship`，tx 整体回滚；见 §3/§5.2）；`active_domain` 无指定 tab 时默认聚焦 overdue 最多的环节（进来即最该干的）；去掉 todo-nav 摘要带 + 当班，总数 badge 移 h1，紧急度提示并入筛选行可点击 pill。✅（2026-07；其中"默认聚焦最紧急"实测会落到待出库而非待收货，仓库员不顺手，Phase H 改回固定待收货）
- **Phase H（直接发不拣货）**：拣货设为可选（参考三 ERP——Odoo ship_only / ERPNext / OFBiz quick ship 拣货都非强制，ABT 原强制是过度设计）；未拣（Confirmed）单就地直接发货（选仓 drawer → `direct_ship`，跳过 Picking）；`outbound` `ship()` 抽 `do_ship` + 新增 `direct_ship(id, warehouse_id, bin_id)`；`ShippingStatus` 加 `Confirmed→Shipped` 转换（migration 078，手动 psql）；批量发货改为未拣单批量 `direct_ship`（批量栏选仓）；`active_domain` 默认改回待收货。✅（2026-07）

每阶段独立 PR，远程 `weichen`，feature 分支（`feat/wms-wc-*`）。

## 9. 风险与决议点
1. **紧急度排序性能**：`list_pending` 拉各域 top N 后内存排序；某环节待办量大（>100）时需各域 service 支持按 `expected_date` 排序查询。**建议**：MVP 内存排序 + top N 截断（如 20）。
2. **跨域映射成本**：7 域实体各异，每域一个 `to_pending_task` 适配。可接受。
3. **就地操作 vs 跳 Doc Hub 边界**：见 §3，快操作就地、复杂跳转。
4. **默认展开策略**：有逾期/紧急的环节默认展开（异常驱动），其余折叠。**建议采纳**。
5. **阈值配置**：临期 N 天，硬编码 vs `wms/settings`。**建议** settings。（原"拣货超时阈值"随 2026-07 出库流合并、urgency 统一 `expected_ship_date` 而废弃。）
6. **出库流合并 urgency 取舍**（2026-07）：合并待拣货/待发货后，urgency 统一用 `expected_ship_date`，删去原 Pick 的 `created_at` age 超时口径。取舍：丢失「拣货 created_at 超时」独立预警——但 `expected_ship_date` 还远 = 不急（不红合理），且 4h 阈值本就是临时硬编码。**采纳**：单一日期口径，`repo.rs` 删 `use_age` / `PICK_TIMEOUT_HOURS` 简化。

## 10. 收货入库闭环（待收货 drawer PO 直收 + 工单入库）

取消来料通知后，待收货 drawer 按 `PendingTask.source_kind` 分两路（2026-06 落地）：
- **采购收货**（`receive_po`）：`po_receive_drawer_body` 渲染 PO 明细（待收量 = quantity - received_qty），选仓库/库位提交 → `PurchaseStockInService::receive_and_stock_in`
- **生产入库**（`receive_wo`）：`wo_receive_drawer_body` 渲染工单完工产品（待入库 = completed_qty - 已入库），选仓库/库位提交 → 仅 `inventory_transaction.record`（source=work_order，不立应付、不回写工单完工量——报工已累加）

**采购入库编排**（`abt-core/src/wms/stock_in/PurchaseStockInService::receive_and_stock_in`，事务内 8 步，替代原 `ArrivalAcceptedHandler` 异步事件）：
1. 幂等 `try_claim(idempotency_key)`
2. 超收校验（含容差 `over_delivery_allowance_pct`）
3. 逐行 `inventory_transaction.record`（`source_type="purchase_order"` + `source_id=po_id`）
4. 增量累加 PO `received_qty`（`add_received_qty` 行锁，并发部分收货串行化；`order_item_id=0` 时按 product_id 解析，兼容 stock-in/create 多 PO 前端）
5. PO 状态流转（`>=quantity`→Received；`>0`→PartiallyReceived；乐观锁）
6. 立应付（PO 维度 upsert：`source_type=PurchaseOrder` + `source_id=po_id`，多次部分收货 `rewrite_amount_by_source` 重算金额）
7. 成本分录（`CostType::Material`，source=PO）
8. 审计日志

取消来料通知后不再发 `ArrivalInspected` 事件、不再依赖 `ArrivalAcceptedHandler`（同步事务内编排，比异步事件更可靠，消除窗口期断链）。`ArrivalAcceptedHandler` + `arrival_notice` 模块本步保留（处理历史/手动来料通知），第二步再删。

**与 stock-in/create 的关系**：stock-in/create 采购分支（`handle_purchase_stock_in`）也调同一 `PurchaseStockInService`（多 PO 批量入口），与 work-center 单 PO 就地收货共享引擎。

**库位选择**：统一用 `bin_picker_modal`（`abt-web/src/components/bin_search.rs`，z-[1001] 盖在 drawer 之上）+ `/api/bin-picker` 端点（`picker_bins` 按 `mode` 分语义：inbound 排除他物料占用 + 同物料排前 + 空位可选；outbound 仅该物料有实物存量的库位，按量降序）。行内 `warehouse_bin_cell` 按钮（hidden 同时带 `name` + `data-k`，入库创建页读 name、作业中心 drawer 读 data-k）→ JS `binPickerOpen`/`binPickerSelect`（`static/app.js`）；`binPickerSelect` 填回 warehouse_id + bin_id + 按钮文字，发货 drawer 额外刷新「可用」列。

**幂等**：work-center drawer body 加载时 hyperscript `on load` 生成 `idempotency_key`（顶层 hidden 字段不进 `items_json`，service 内 `try_claim`）；stock-in/create 由上层 `create_stock_in` 的 `try_claim` 保证（service 内传 None）。

## 11. 单据台账页（2026-07 新增 · 全量查询入口）

作业中心回归纯待办后，WMS 单据的全量查询（历史 / 全状态）由独立「单据台账」承载（菜单 1 项 `/admin/wms/ledger`）。对齐 Odoo Transfers + filter / ERPNext 各 DocType 独立 List / OFBiz Find-List 范式——三家 ERP 无一在工作台并列「待办/全部」。

### 11.1 定位（菜单分工）
- **作业中心** = 处理待办（强时效、紧急度排序、就地 drawer）
- **单据台账** = 检索全量（按类型/状态/单号/日期查历史单据）
- 关联：作业中心各 tab 右上「查看全部」→ 跳台账对应类型 tab（`?type=<slug>`）

### 11.2 页面结构（单端点 list，`pages/wms_ledger.rs`）
- Query：`type`(arrival/outbound/transfer/requisition/cycle-count) + `status` + `keyword` + `date_from` + `date_to` + `page`
- **双层 tab**：第一层类型 tab（5）+ 第二层状态 tab（picking 4 态 / cycle_count 6 态），均 `status_tabs_with_oob` 传空 `select_oob`（避免 `#status-tabs` 冲突）
- 过滤表单：单号搜索（防抖 300ms）+ 日期范围；hidden `type`/`status` 携带当前 tab
- 表格列**按类型动态**：收货=单号/来源/仓库 · 出库=单号/客户 · 调拨=单号/来源仓→目标仓 · 领料=单号/工单/仓库 · 盘点=单号/仓库
- 行落点：出库单号跳 `ShippingDetailPath`；其余暂纯文本（独立 detail 后续补）
- 分页（每页 20），复用列表页单端点模式（范本 `pages/wms_stock_in_list.rs`）

### 11.3 数据源（关键：前 5 种同源 `stock_pickings`）
- 收货/出库/调拨/领料：`picking_service.list`，按类型传 `picking_types`（收货合并 `[IncomingPurchase, IncomingWorkOrder]`；其余单值）
- 盘点：`cycle_count_service.list`
- WMS 作业单据统一在 `stock_pickings` 表（5 种 `PickingType`，`enums.rs:223`：IncomingPurchase/IncomingWorkOrder/OutgoingSales/InternalTransfer/InternalIssue），盘点独立 `cycle_counts` 表

### 11.4 接口扩展（abt-core，不 breaking）
- `PickingFilter`（`picking/model.rs:108`）加 `picking_types: Option<Vec<PickingType>>`（多值 OR，repo 用 `IN (...)` 逐占位符 bind）+ `date_from/date_to`（scheduled_date 范围）；`picking/repo.rs::list` 配套
- `CycleCountFilter`（`cycle_count/model.rs:86`）加 `doc_number`（ILIKE）+ `date_from/date_to`（count_date 范围）；`cycle_count/repo.rs::list` 配套
- 两 service trait 签名不变（仅 filter 字段扩展，旧构造方 `..Default::default()` 兼容）
- 不需 migration（纯查询扩展）

### 11.5 不适用 / 保留不动
- `/admin/wms/stock-in`（StockInListPath）查的是 `inventory_transaction` **库存流水**（流水视角，按 TransactionFilter），与单据台账（单据视角）正交，保留不动
- 作业中心 5 环境 tab 待办队列（`PendingTask` 统一表）+ 所有就地 drawer 操作（收货/发货/发料/调拨/盘点）+ 待出库批量发货栏，保留不动
