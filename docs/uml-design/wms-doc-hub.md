# WMS 单据 Hub 工作台设计（Doc Hub）

> 关联：[#93](https://github.com/swloki/abt2/issues/93) 之后，流程型单据详情页从「平铺详情」升级为「单对象全生命周期工作台」
> 参照原型：MES `04-order-hub.html`（工单工作台，"少即是多"范式）；本设计配套 `03-shipping-hub.html`（发货单 Hub 样本）
> 首批样本：发货单 `outbound`；后续复刻 `arrival` / `transfer` / `requisition` / `cycle_count` / `conversion`
> 状态：**设计骨架，待评审**（接口与模型先行，评审确认后再实施代码）
> 现行实现：`abt-web/src/pages/shipping_detail.rs`（传统平铺详情页）；`ShippingRequestService` / `PickListService`

## 1. 背景与定位

### 1.1 问题

当前 WMS 流程型单据详情页（`shipping_detail.rs` 为典型）是「传统详情页」：

- **信息平铺**：header + 信息卡片（6 字段一字排开）+ 明细表，首屏无优先级，要读完全表才知道状态
- **写操作整页刷新**：`confirm` / `pick` / `ship` / `cancel` 均为 `hx-post` + `HX-Redirect` 整页跳转——htmx 最浪费的用法
- **关联实体分散**：来源订单仅一行 `href` 链接；#93 新做的 `PickList` 不在详情页内；库存事务要去 `/admin/wms/transaction-logs` 另查；操作日志无入口
- **无关键指标摘要、无异常驱动**：缺货、待拣、临期等关键信号无法一眼可见

### 1.2 目标：单据工作台（Doc Hub）

借鉴 MES 工单 Hub（`04-order-hub.html`），把一张单据的全生命周期聚合到一个工作台页：

- **顶部 Header**：单号 + 状态步骤条 + 来源链 + 摘要带（关键数字 + 异常染色 + 红点）
- **Disclosure 折叠区**：关联子域（明细 / 拣货 / 库存事务 / 日志）按需展开，**默认不查库**
- **Drawer 抽屉**：写操作在当前上下文就地发起，提交后**局部刷新**，不跳页

「少即是多」= 信息密度更高（一页聚合全生命周期），但首屏清爽（默认折叠，需要时展开，异常红点驱动）。

### 1.3 与 WorkCenter 的边界（不混淆）

| | WorkCenter（#93 已上线） | Doc Hub（本设计） |
|---|---|---|
| 维度 | **跨单据**待办池 | **单对象**全生命周期 |
| 回答 | 「今天该做哪些单据」 | 「这张单据的全部」 |
| 形态 | 跨单据折叠队列（见 [wms-work-center-hub](wms-work-center-hub.md)） | 单据工作台 |
| 关系 | WorkCenter 待办行 → 跳 Doc Hub 看单据全貌 | — |

WorkCenter 同样采用 Hub 范式重做（跨单据队列版，详见 [wms-work-center-hub.md](wms-work-center-hub.md)）：其 disclosure 按**环节**聚合多张待办单据，与本设计「按**单据**拆解子域」的 disclosure 互为镜像，两者共享 drawer 组件（拣货 / 收货 / 确认发出）。

## 2. 适用范围

| 适用（Hub 化） | 不适用（保持现状） |
|---|---|
| **流程型单据详情页**：`outbound` / `arrival` / `transfer` / `requisition` / `cycle_count` / `conversion` | 31 个**列表页**（status-tabs 单端点模式已是最佳实践，禁动） |
| | **主数据**：`warehouse` / `bin` / `strategy` / `settings`（列表+表单即可） |
| | **查询/台账**：`stock_list` / `transaction_log` / `cascade` / `low_stock`（列表+筛选即可） |
| | `WorkCenter`（另一维度 Hub，见 1.3） |

**判定标准**：有状态机 + 有上下游关联 + 生命周期够长 → 适合 Hub。短生命周期 / 无关联实体的单据（如简单入库）保持现详情页。

## 3. 页面骨架（四层）

### 3.1 Header 层（固定，首屏必查，整块可替换）
- 单号 + 状态徽章 + 操作按钮（按状态显隐）
- 状态步骤条（statusbar，复用现有 `workflow_steps` 组件）
- 来源链 `source-trace`：`SO → 发货单 → 拣货单 → 库存事务`
- 摘要带 `stat-strip`：关键数字 + 染色 + 红点，可点击直达对应 disclosure

### 3.2 Disclosure 层（折叠，懒加载）
每个区块是一个独立可折叠卡片：
- **默认折叠**：仅渲染 header + 一行摘要文本，**不查子域数据**
- **展开时** `hx-get` 拉取区块 body 片段（懒加载，重查询只在展开时执行）
- icon 角标红点标记异常（如缺货）

### 3.3 Drawer 层（写操作）
- 侧滑抽屉，在当前页面上下文发起（不跳页）
- 提交后局部刷新 Header + 相关 Disclosure（`hx-select-oob` 或 `HX-Trigger` 联动）

### 3.4 整页（仅首次加载）
- 首次 `GET /admin/wms/shipping/{id}` 渲染 Header + 各 Disclosure 的 header（折叠态）
- 后续所有交互（展开/提交/状态推进）均为片段级 htmx，不整页刷新

## 4. 发货单 Hub 具体映射（样本）

### 4.1 来源链
`SO-2026-06-000170` → `SR-2026-06-000043`（发货单）→ `PK-2026-06-001`（拣货单）→ 库存事务流水

### 4.2 摘要带字段口径 ★（对齐 ATP 铁律）

| 指标 | 口径 | 数据源（trait） |
|---|---|---|
| 待拣 | `Σ requested_qty`（Picking 态） | `ShippingRequestService::list_items` |
| 已拣 | `Σ picked_qty` | `PickListService::list_items`（经 `find_by_outbound`） |
| 已发 | `Σ shipped_qty` | `ShippingRequestService::list_items` |
| **缺货红点** | 任一明细 `ATP < requested_qty − shipped_qty` | **`InventoryTransactionService::query_available`** |

> **铁律**：缺货红点的可用量必须用 ATP 口径（`stock_ledger.quantity − Lock − Reservation`），**禁止**读 `stock_ledger.available_qty` / `reserved_qty`（仅含 Lock，不含 Reservation）。详见 `README.md` §ATP 与 [[reference-abt-stock-atp-vs-projected]]。

### 4.3 Disclosure 区块

| # | 区块 | 加载策略 | 内容 |
|---|---|---|---|
| ① | 发货信息 | **首屏直出**（轻量） | 客户/收货地址/承运商/物流单号/预计发货/操作员/来源订单 |
| ② | 发货明细 | **首屏直出**（中量） | 产品表：申请/已发/**可用库存(ATP)**；缺货行红底 |
| ③ | 拣货单 ★消化 #93 P1 | 懒加载 | PickList 状态 + `picked_qty` / `bin`；含「录入拣货」drawer 入口 |
| ④ | 库存事务 | **懒加载**（重查询） | 本单 `inventory_transaction` 流水（时间/类型/产品/数量/库位） |
| ⑤ | 操作日志 | **懒加载**（重查询） | `audit_log` 时间线（经 `AuditLogService::query_logs`） |

### 4.4 Drawer 操作

| Drawer | 动作 | 消化 |
|---|---|---|
| 录入拣货 | 录入 `picked_qty` / `bin_id` → `complete_pick` | ★ #93 P1 item 5（拣货录入页收敛为 drawer） |
| 确认发出 | 确认 `ship`（扣库存 + 释放预留 + 事件立账） | 现有 `ship` 整页刷新 → 局部刷新 |
| 取消 | `cancel` | 现有 |

## 5. HTMX 交互契约 ★（核心：发挥 htmx 威力）

> 本节是「从整页刷新 → 片段刷新 + 事件联动」的升级规范，对照 `abt-web/CLAUDE.md` 组件化三原则。

### 5.1 Disclosure 懒加载
每个 disclosure body 是一个**子资源片段端点**：
```
GET /admin/wms/shipping/{id}/fragments/{block}
  block ∈ {info, items, pick, transactions, log}
  → 返回该区块 body 的 HTML 片段（Maud 渲染）
```
disclosure header 声明 `hx-get` + `hx-target="next .di-body"`，首次展开拉取一次（用 hyperscript 保证只拉一次：`on click toggle .open, if its first time call htmx trigger`，或后端返回时一并带回 header 的「已加载」态）。

### 5.2 Drawer 提交 + 多区局部刷新（HX-Trigger 事件驱动）
drawer 表单 `hx-post` 提交后，后端不返回整页，而是触发事件让相关区自刷新（参照 `abt-web/CLAUDE.md` §事件驱动解耦）：
```
响应头 HX-Trigger: "shippingUpdated"
→ Header 区：       hx-trigger="shippingUpdated from:body" hx-get=.../fragments/header
→ 拣货 disclosure： hx-trigger="shippingUpdated from:body" hx-get=.../fragments/pick
→ 摘要带：          随 Header 片段一并刷新
```
避免写「聚合刷新路由」——每个被动区各自声明监听，解耦。

### 5.3 操作按钮的局部刷新（替代 HX-Redirect）
当前 `confirm` / `pick` / `ship` 是 `HX-Redirect` 整页。Hub 化后：
- 按钮包在 Header 组件内，`hx-target="closest .detail-header"` + `hx-swap="outerHTML"`
- 提交后后端返回**新的 Header 片段**（状态推进后按钮自动切换：如 Confirmed→Picking 后「开始拣货」消失、「确认发出」出现）
- 不再 `HX-Redirect`，用户停留在原视图

### 5.4 与「列表页单端点」约束的关系澄清

`abt-web/CLAUDE.md`：「**禁止为局部刷新单独创建 Handler**」——此约束**针对列表页**（tab 切换/搜索/分页本质是同一数据视图的不同参数，应共用一个 list 端点）。

Doc Hub 的 fragment 子端点（`/shipping/{id}/fragments/{block}`）是**详情页的子资源**，性质不同，**不违反**该约束：

| | 列表页单端点 | Doc Hub fragment |
|---|---|---|
| 本质 | 同一数据视图的不同**参数** | 单据下不同**子域**的独立视图 |
| 端点 | 一个 list handler | 每子域一个 fragment handler（合理） |
| 约束 | 禁止拆分 | 允许拆分，但**同一子域内**的筛选仍遵循单端点 |

## 6. 接口变更（Service trait）

### 6.1 `ShippingRequestService` 新增
```rust
/// Hub 摘要带数据（首屏轻量查询，含缺货判定）
async fn hub_summary(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64,
) -> Result<ShippingHubSummary>;

/// 本单相关的库存事务流水（④ disclosure 懒加载）
async fn list_transactions(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, page: PageParams,
) -> Result<PaginatedResult<InventoryTxnView>>;
```

### 6.2 `ShippingHubSummary` model
```rust
pub struct ShippingHubSummary {
    pub pending_pick_qty: Decimal,        // 待拣 Σ requested_qty
    pub picked_qty: Decimal,              // 已拣 Σ picked_qty
    pub shipped_qty: Decimal,             // 已发 Σ shipped_qty
    pub shortage: Option<ShortageSignal>, // 缺货红点；None = 无缺货
}

pub struct ShortageSignal {
    pub product_id: i64,
    pub product_name: String,
    pub requested_qty: Decimal,
    pub available_qty: Decimal, // ATP 口径（query_available）
}
```

### 6.3 `PickListService` 新增（消化 #93 P1 item 5）
```rust
/// 录入拣货明细（人工拣货：picked_qty / bin_id），完成后调 complete_pick
async fn record_pick_items(
    &self, ctx: &ServiceContext, db: PgExecutor<'_>,
    id: i64, items: Vec<PickItemInput>,
) -> Result<()>;

pub struct PickItemInput {
    pub pick_list_item_id: i64,
    pub picked_qty: Decimal,
    pub bin_id: Option<i64>,   // 库位（可选，FIFO/FEFO 建议未实现时人工指定）
}
```
配合 `outbound::pick()` 调整：MVP 自动满拣 → 改为「只 `generate_from_outbound`（留 Draft），不自动 complete」，等人工录入后 `complete_pick`。保留自动满拣作为「快速拣货」快捷动作。

### 6.4 跨域调用矩阵（新增）

| 调用方 | 被调方 | 方式 | 用途 |
|---|---|---|---|
| shipping Hub | `InventoryTransactionService` | trait | `query_available`（缺货红点 ATP）+ `list_transactions`（事务流水） |
| shipping Hub | `PickListService` | 同域 trait | `find_by_outbound` + `record_pick_items`（拣货 disclosure + drawer） |
| shipping Hub | `AuditLogService` | shared trait | `query_logs`（操作日志 disclosure） |

全部走 trait，**零 repo 直访**（遵循 #93 职责归属约束）。

## 7. 约束兼容性自检（对照 `abt-web/CLAUDE.md`）

| 约束 | Hub 如何遵守 |
|---|---|
| 多步写事务包裹 | drawer 的 `record_pick_items` / `ship` 仍 `pool.begin()+commit()`（范本 `ship_shipping`，事故 SO-2026-06-000170） |
| TypedPath | 所有 fragment / drawer 路径用 TypedPath（如 `ShippingFragmentPath { id, block }`） |
| `hx-target="this"` / `closest` | Header 按钮、disclosure 均用相对 target，禁硬编码 `#id`（组件化三原则 §1） |
| 状态随身 | `hx-vals` 绑定上下文（§2） |
| 禁 `onclick` / 禁 `fetch` | drawer 开关、disclosure toggle 用 hyperscript `_=`；表单用 `hx-post` |
| UnoCSS 原子类 | Maud 实现时 `class=""` 全用原子类（原型 HTML 的语义类如 `.disclosure` 仅设计稿，实现期转原子类） |
| 禁为局部刷新建 handler（列表页） | Doc Hub fragment 是详情页子资源，见 §5.4，不违反 |
| Maud 双 class 陷阱 | 实现期注意 [[reference-abt-web-maud-double-class]] |

## 8. 与 #93 follow-up 的关系

| follow-up 项 | 本设计处置 |
|---|---|
| **P1 item 5**（PickList 前端拣货录入页） | ★ **消化**：收敛为 Hub 内「录入拣货」drawer，不另起 `/admin/wms/shipping/{id}/pick` 独立页 |
| **P1 item 6**（ship 校验 PickList Picked） | **消化**：ship drawer 提交前校验关联 pick_list 已 Picked |
| P1 item 4（WorkCenter 紧急提醒真实数据） | 不消化（属 WorkCenter，非 Doc Hub） |
| P2 items（Handler 迁 fms 等） | 不消化（技术债，独立） |

## 9. 实施阶段

- **Phase A（后端接口）** ✅ 已完成（2026-06-25，cargo clippy 绿）：`ShippingRequestService` 加 `hub_summary` / `list_transactions`；`PickListService` 加 `record_pick_items`（+ `PickListItemRepo::update_picked`）；`outbound::pick()` 改 generate-only（留 Draft）。
  - **ship() 校验 PickList Picked（#93 P1 item 6）延后 Phase B**：因 pick 改 generate-only 后 PickList 留 Draft，若后端单方面 enable 校验会断现有「开始拣货→确认发出」流程；故随 Phase B 前端 Hub drawer 上线一并 enable。
- **Phase B（前端 Hub 骨架）**：`shipping_detail.rs` 重构为 Hub（Header + 折叠 disclosure + drawer 骨架）；操作按钮改局部刷新（替 `HX-Redirect`）。
- **Phase C（懒加载 + 拣货 drawer）**：5 个 fragment 端点；录入拣货 drawer 接 `PickListService::record_pick_items`。
- **Phase D（复刻）**：`arrival` / `transfer` / `requisition` / `cycle_count` / `conversion` 套用同骨架。

每阶段独立 PR，远程 `weichen`，feature 分支（`feat/wms-doc-hub-*`）。

## 10. 风险与决议点（待评审）

1. **片段端点数量膨胀**：6 单据 × 5 区块 = 30 fragment 端点。
   - 决议：每单据各自 fragment handler（清晰）vs 抽通用 `FragmentService`（复用但泛化成本）。**建议前者**，各单据子域语义差异大。
2. **懒加载 vs 首屏直出边界**：哪些区块首屏直出、哪些懒加载。
   - 建议：发货信息 + 发货明细直出（轻/中量）；拣货/库存事务/操作日志懒加载（重查询）。
3. **ATP 查询性能**：摘要带缺货红点 + 明细可用列，若逐明细 `query_available` 是 N 次查询。
   - 决议：需新增**批量 ATP** 接口（`query_available_batch(product_ids, warehouse_id)`），避免 N+1。
4. **拣货 drawer 人工模式 vs 自动满拣默认**：是否默认自动满拣、drawer 仅用于修正？还是默认人工录入？
   - 建议：保留「快速拣货（自动满拣）」按钮 + 「逐行录入」drawer 双入口。
5. **状态推进按钮与 drawer 并存**：confirm/cancel 是轻操作（按钮直接 `hx-post`），pick/ship 是重操作（走 drawer 收集录入/确认）。需明确哪些走按钮、哪些走 drawer。
