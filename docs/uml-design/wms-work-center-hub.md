# WMS 作业中心 Hub 设计（WorkCenter Hub）

> 关联：#93 已上线 WorkCenter（dashboard 式计数墙）→ 升级为「就地可执行工作台」
> 参照：`03-shipping-hub.html`（Doc Hub 范式，本设计同款）；[`wms-doc-hub.md`](wms-doc-hub.md)（Doc Hub 契约，互为镜像）
> 配套原型：`03-work-center-hub.html`
> 状态：**已实现**（单端点 + 自洽卡片 + hx-select-oob，2026-06 落地，见 §5）
> 现行实现：`abt-web/src/pages/wms_work_center.rs`（单端点 `/admin/wms/work-center`：GET 分支 page/expand/drawer，POST 分支 action；共享 `#wc-drawer-overlay`）；`WorkCenterService::{summary,list_pending,urgent_summary}`

## 1. 背景与定位

### 1.1 问题：当前 WorkCenter 是 dashboard，不是工作台

`wms_work_center.rs` 现状是「计数墙 + 跳转」：

- 7 张卡片只显示待办计数，点击 → 跳各域列表页 → 再进详情 → 才能操作，**两段断点**
- 底部「紧急/临期」区是**示意假数据**（#93 followup P1 item 4 至今未接）
- htmx 几乎没用：整页一次 `summary()` SSR；原型里 `setInterval` 局部刷新注释掉没做
- 仓库员「一个页面完成更多工作」做不到——看一眼 7 个数字就跳走

### 1.2 目标：就地可执行工作台

采用 `03-shipping-hub.html` 同款「少即是多」范式（摘要带 + disclosure 折叠 + drawer 就地操作）重做，**摒弃卡片网格**：

- **摘要带**：总待办 / 紧急 / 逾期，替代「您有 N 项待办」
- **7 个 disclosure 分区**（替代卡片）：展开是该环节的待办单据队列（按紧急度排序），**不是跳转**
- **队列行就地 drawer 操作**：拣货 / 收货 / 确认发出，复用 Doc Hub 的 drawer 组件，不离开本页
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
- **无状态步骤条、无来源链**（WorkCenter 不是单据，是聚合视图）

### 2.2 Disclosure 层（7 环节，懒加载队列）
每个环节一个折叠卡片（替代原卡片网格的一格）：
- header：图标（角标红点 if 该环节有逾期/紧急）+ 环节名 + 摘要（"3 笔 · 1 逾期 · 最久 2h"）+ chevron
- **默认折叠**：仅 header + 计数摘要，不查明细
- 展开时 `hx-get` 拉该环节 top N 待办队列（懒加载）
- 有逾期/紧急的环节默认展开 + 红点驱动

### 2.3 Drawer 层（就地操作，复用 Doc Hub drawer）
队列每行带快操作按钮，点击弹出 drawer（与 shipping-hub 同套 drawer 组件）。

### 2.4 整页
- 首屏 `GET /admin/wms/work-center` 只查 `summary()` + `urgent_summary()`，渲染 Header + 7 个 disclosure header（折叠态）
- 展开某环节才 `list_pending` 查明细；操作后 htmx 片段级刷新

## 3. 7 环节映射 + 就地操作范围

| 环节 | pending 状态（对齐 summary） | 就地操作（drawer） | 复杂操作（跳 Doc Hub） |
|---|---|---|---|
| 待收货 | 采购 PO(Confirmed/PartiallyReceived) + 工单(完工未入库)，`PendingTask.source_kind` 区分 | **收货入库**（PO 调 `PurchaseStockInService` / 工单 record 库存，见 §10） | — |
| 待拣货 | `PickListStatus::Draft` | **拣货录入**（复用） | 部分拣 / 多库位 |
| 待发货 | `ShippingStatus::Confirmed` / `Picking` | **确认发出**（明细核对 + 拣货仓库/库位） | — |
| 待领料 | `RequisitionStatus::Confirmed` / `PartiallyIssued` | 发料 | 退料 |
| 待调拨 | `TransferStatus::Draft` / `InTransit` | 确认收货 | — |
| 待盘点 | `CycleCountStatus::Draft` / `Counting` / `PendingReview` | — | 盘点录入（跳 Doc Hub） |

**就地操作范围克制**：只有「快操作」（单一动作或一条闭环）就地；复杂操作（盘点录多库位、形态转换）跳 Doc Hub。点单据号始终可跳 Doc Hub 看全貌。**待收货 drawer** 按 `source_kind` 分 PO 收货（调 `PurchaseStockInService` 直收入库 + 回写 PO/立应付）与工单入库（record 库存），一次提交闭环（见 §10），仍属就地快操作。

> 「待质检」环节已随来料通知取消而移除（工厂不做来料质检）。来料通知模块/`ArrivalAcceptedHandler` 本步保留（处理历史/手动来料通知），第二步再删。

## 4. 接口设计（WorkCenterService 扩展）

### 4.1 trait 扩展
```rust
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 聚合各环节待办计数（保留；摘要带 + disclosure header 用）
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary>;

    /// 新增：某环节的待办单据队列（disclosure 展开懒加载）
    async fn list_pending(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        domain: WorkCenterDomain, page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>>;

    /// 新增：紧急/逾期汇总（摘要带的紧急/逾期数字；消化 #93 P1 item 4）
    async fn urgent_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<UrgentSummary>;
}
```

### 4.2 model 新增
```rust
pub enum WorkCenterDomain {
    Arrival, Inspection, Pick, Outbound, Requisition, Transfer, CycleCount,
}

pub enum Urgency { Normal, Soon, Overdue }

/// 跨域统一待办视图（各域实体在 WorkCenterService 内映射成此结构）
pub struct PendingTask {
    pub doc_id: i64,
    pub doc_number: String,
    pub domain: WorkCenterDomain,
    pub counterparty: String,           // 客户 / 供应商
    pub summary: String,                // "3 项 · 320 件"
    pub expected_at: Option<DateTime<Utc>>,
    pub urgency: Urgency,
    pub doc_hub_path: String,           // 跳 Doc Hub 看单据全貌
}

pub struct UrgentSummary {
    pub overdue_count: u64,             // 逾期
    pub soon_count: u64,                // today+N 内临期
}
```

### 4.3 紧急度口径（消化 #93 followup P1 item 4）
| 环节 | 逾期 `Overdue` | 临期 `Soon` |
|---|---|---|
| 待收货 | `arrival_notice.expected_date < today` 且 Draft | `≤ today+N` |
| 待发货 | `outbound.expected_ship_date < today` 且 Confirmed/Picking | `≤ today+N` |
| 待拣货 | `pick_list.created_at < now − 阈值` 且 Draft（无到期日，用创建时长判超时） | — |
| 其他 | 按各自 `expected_date` | — |

### 4.4 `list_pending` 实现策略
复用 `implt.rs` 现有 `cnt` 容错模式（各域 `service.list(Filter{status}, page)`），**放大 `page_size`** 取真实 items（而非仅 total），映射成 `PendingTask`：

- 各域 list 返回实体类型不同（`ArrivalNotice` / `PickList` / `ShippingRequest` …）→ 在 WorkCenterService 内各自 `to_pending_task` 适配成统一结构
- **紧急度排序**：MVP 在 WorkCenterService 内存按 `urgency` → `expected_at` 排序，**避免改 6 个域 service 的排序参数**；top N 截断
- 查询失败容错沿用 `cnt` 模式（`tracing::warn`，该环节记空队列，不连累整页）

## 5. HTMX 契约（单端点 + hx-select / hx-select-oob）

**单一端点** `/admin/wms/work-center`（GET + POST），所有交互收敛到一个地址，对齐「列表页单端点」约束。

### 5.1 GET 分支（query 参数三选一）
| query | 返回 | 客户端用法 |
|---|---|---|
| 无 | 整页（todo-nav + 7 卡片 header + drawer overlay 壳） | 首屏 SSR |
| `?expand={slug}` | 该环节待办队列片段（task table） | 卡片 head `hx-get + hx-target=#d-{slug}-body` 懒加载 |
| `?drawer={action}&id={id}` | drawer body（标题栏+表单） | 行内按钮 `hx-get + hx-target=#wc-drawer-body` + `_="on 'htmx:afterRequest' add .open to #wc-drawer-overlay"` 打开 |

卡片自洽：`#d-{slug}` 整体可被 `hx-target`/`hx-select` outerHTML 替换。

### 5.2 POST：就地操作 + 多区联动（hx-select-oob）
drawer 表单 `hx-post=/admin/wms/work-center`（hidden `action`/`id`；收货/拣货 另带 hidden `items_json`；收货额外带顶层 hidden `idempotency_key` 防双击重复入库）。服务端执行动作后**重渲染「受影响卡片 + todo-nav」**，一次响应含两区：

- `hx-target=#d-{slug}` + `hx-select=#d-{slug}` → 替换该卡片（单据出列、计数下降）
- `hx-select-oob=#todo-nav:outerHTML` → 同时更新摘要带
- `_="on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay"` → 关 drawer

**不用 HX-Trigger 广播**——hx-select / hx-select-oob 从同一响应各取所需，自然协调（避免事件监听 `from:body` 语法坑与"所有 disclosure 重拉"的浪费）。

### 5.3 行级明细（收货/拣货）走 JSON hidden input
`serde_urlencoded`（axum::Form）不支持 `items[idx][field]` 嵌套（旧实现因此 422）。改 hidden `items_json`：表单 `onsubmit`（早于 htmx 的 submit 监听）把 `[data-row]` 行的 `[data-k]` 输入收成 JSON 字符串 → handler `serde_json::from_str::<Vec<RowJson>>`（字段统一 String，服务端 parse），对齐 quotation/sales_order 的 `ItemWeb` 范式。

- 拣货：`onsubmit="wcCollectItems(this)"`（`static/app.js`），行字段 `pick_list_item_id/picked_qty/warehouse_id/bin_id`
- **收货入库**：`onsubmit="wcReceiveSubmit(this)"`（校验实收>0 + 仓库必填后调 `wcCollectItems`），行字段 `product_id/received_qty/batch_no/warehouse_id/bin_id`（per-行目标仓库/库位；库位走 `#bin-picker` 弹窗 SameMerge 推荐，复用 `suggest_bins` 端点）

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

每阶段独立 PR，远程 `weichen`，feature 分支（`feat/wms-wc-*`）。

## 9. 风险与决议点
1. **紧急度排序性能**：`list_pending` 拉各域 top N 后内存排序；某环节待办量大（>100）时需各域 service 支持按 `expected_date` 排序查询。**建议**：MVP 内存排序 + top N 截断（如 20）。
2. **跨域映射成本**：7 域实体各异，每域一个 `to_pending_task` 适配。可接受。
3. **就地操作 vs 跳 Doc Hub 边界**：见 §3，快操作就地、复杂跳转。
4. **默认展开策略**：有逾期/紧急的环节默认展开（异常驱动），其余折叠。**建议采纳**。
5. **阈值配置**：临期 N 天 / 拣货超时阈值，硬编码 vs `wms/settings`。**建议** settings。

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

**库位选择**：复用 `suggest_bins` 端点 + `#bin-picker` overlay（`wc_bin_picker_shell`，z-[1001] 盖在 drawer overlay 之上）；JS `wcOpenBinPicker`/`wmsPickBin`/`wcResetBin`（`static/app.js`）。`wmsPickBin` 兼容 work-center（`data-k="bin_id"`）与 stock-in（`input[name="bin_id"]`）两种行结构。

**幂等**：work-center drawer body 加载时 hyperscript `on load` 生成 `idempotency_key`（顶层 hidden 字段不进 `items_json`，service 内 `try_claim`）；stock-in/create 由上层 `create_stock_in` 的 `try_claim` 保证（service 内传 None）。
