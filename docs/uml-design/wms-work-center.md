# WMS 作业中心（WorkCenter）设计

> 关联：#93 / 拣货单 PickList 之后，仓库需要一个"接下来做什么"的聚合页。
> 参照：Odoo `stock.picking` Operations 看板、ERPNext 仓库待办单据聚合。
> 现状：`wms_dashboard` 只有「快捷入口卡片」（纯导航），无待办聚合。

## 1. 定位

WMS 是**执行层**（区别于采购/生产需求池的计划层 demand）。仓库岗需要一个**作业中心**，一进系统就知道先做什么：聚合各域单据的"待处理"状态，按业务环节分区，点进去是该状态的筛选列表。

语义上**不是「需求池」**（仓库不做 MRP 计划），是「**作业待办看板 / Operations**」。

## 2. WorkCenterService 接口

```rust
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 聚合各域待办计数（作业中心首页用）
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<WorkCenterSummary>;
}
```

**设计原则**：WorkCenterService 只做**聚合计数**；各域列表**复用现有 service**（前端按状态筛选跳转），不重复实现列表查询。

## 3. WorkCenterSummary model

```rust
/// 仓库作业中心待办汇总
pub struct WorkCenterSummary {
    pub arrivals_pending: u64,        // 待收货
    pub inspections_pending: u64,     // 待质检
    pub picks_pending: u64,           // 待拣货
    pub outbounds_pending: u64,       // 待发货
    pub requisitions_pending: u64,    // 待领料
    pub transfers_pending: u64,       // 待调拨
    pub cycle_counts_pending: u64,    // 待盘点
}
```

## 4. 各域「待办」状态边界

| 待办分区 | 来源域 | pending 状态（枚举值） |
|---|---|---|
| 待收货 | `arrival_notice` | `ArrivalStatus::Draft / Received` |
| 待质检 | `arrival_notice` / QMS | `ArrivalStatus::Inspecting`（来料待检）；成品 OQC 由 QMS 卡控 |
| **待拣货** | `pick_list`（#93 Phase 3） | `PickListStatus::Draft` |
| **待发货** | `outbound`（shipping_request） | `ShippingStatus::Confirmed / Picking` |
| 待领料 | `material_requisition` | `RequisitionStatus::Confirmed / PartiallyIssued` |
| 待调拨 | `transfer` | `TransferStatus::Draft / InTransit` |
| 待盘点 | `cycle_count` | `CycleCountStatus::Draft / Counting / PendingReview` |

## 5. 实现策略

`WorkCenterServiceImpl::summary` 调各域 service 的 `list`（对应 pending 状态过滤，`PageParams { page: 1, page_size: 1 }`）取 `total`：

- 各 wms 子域 service（arrival / pick_list / outbound / requisition / transfer / cycle_count）—— **同域 trait 调用**，合规
- QMS 待检若需纳入，经 `InspectionResultService`（跨域 trait，合规）

**不直访任何 repo**（遵循 #93 职责归属约束）。性能：7 次轻量 count 查询（page_size=1 只取 total），可接受。

## 6. 前端

新增路由 `/admin/wms/work-center`（仓库作业中心），展示 `WorkCenterSummary` 各分区卡片（数量徽章 + 跳转该域状态筛选列表）。作为 wms 模块首页（替代纯导航的快捷入口）。

## 7. 侧边栏菜单收口（渐进式 / 分阶段）

作业中心已集成 5 个业务的待办操作（收货 / 发货 / 发料 / 调拨 / 盘点入口），inventory 侧边栏对应菜单与之功能重叠。决策：**收口到作业中心为唯一菜单入口**，采用渐进式 + 分阶段试点。

**渐进式原则**：5 个业务的 `list / create / detail` 页面**保留为路由**（不物理删除）。硬约束：
- 出库 `detail` 被销售对账 / 销售退货跨模块链接（`ShippingDetailPath`，4 处），不可删；
- 作业中心库位选择弹窗复用入库 `create` 的 `suggest_bins` 端点，不可删；
- 各 `create / detail` 的「返回」按钮指向各自 `list`（`?restore=true`），不可删。

**入口承载**：作业中心每个 domain tab 增「新建 / 查看全部」入口按钮，跳转保留的 `CreatePath / ListPath`（list 页本身已是成熟全量视图——状态 tab + 搜索 + 分页 + 新建入口，不在作业中心内重做，DRY）。作业中心待办队列 + 就地 drawer 操作不变。

**分阶段**：
1. **阶段 1（试点）**：领料单 requisition —— 删侧边栏「领料单」菜单 + 作业中心「待领料」tab 加入口按钮；requisition 路由全保留。
2. **阶段 2（推广）**：调拨 / 盘点 / 入库 / 出库，模式同阶段 1（机械复制）；路由保留使 suggest_bins、销售依赖均无破坏。
3. **阶段 3（可选深化）**：作业中心内原生「全部单据」表格 / 详情 drawer 化，视评审而定。

> **实施状态（2026-07-02）**：阶段 1 + 阶段 2 已完成——inventory 侧边栏 5 个菜单（入库管理 / 出库管理 / 领料单 / 库存调拨 / 循环盘点）全部移除；作业中心 5 个 domain tab 均渲染「新建 / 查看全部」入口（`domain_entries`），跳转各业务保留的 Create / List 路由。各业务 list / create / detail 路由与页面保留不动，销售对账 / 退货跨模块链接、作业中心 `suggest_bins` 复用均无破坏。阶段 3 已启动，见下文第 8 节。

## 8. 阶段 3：作业中心彻底收口（少即是多，不跳转）

阶段 1+2 仅收口菜单入口（侧边栏 → 作业中心 tab 的「新建 / 查看全部」跳转），各业务 list / detail 页仍独立存在。用户核心诉求升级为**少即是多——在作业中心完成全部工作，不跳转其他页面**。阶段 3 把 list / detail 功能并入作业中心（待办 + 全部 + 详情 drawer + 就地操作），然后**删除独立 list / detail 页**。

### 删页硬约束（决定哪些页能删）

- **出库 `detail` 保留**：被销售对账 / 销售退货跨模块链接（`ShippingDetailPath`，4 处），不可删；作业中心可同时 drawer 展示。
- **入库 `create` 保留**：其 `suggest_bins` 端点被作业中心库位选择弹窗复用。
- **领料单 / 调拨 / 盘点 的 list / detail 无跨模块依赖**，可安全删。

### 阶段 3.1（领料单试点）实施

**作业中心 Requisition 域扩展**（`wms_work_center.rs`）：
- `WorkCenterQuery` 加 `view: Option<String>`（pending / all）；`render_work_center_page` 在 `Requisition + view=all` 时调 `material_requisition_service.list` 渲染全状态表格（keyword 搜索 + 分页）。
- 二级「待办 / 全部」视图切换（`view_toggle`，仅 Requisition）。
- **详情 drawer（`req_detail`）替代 detail 页**：`req_detail_drawer_body` 渲染单据头 + 行项目 + 状态 + 就地操作（`req_detail_actions`：Draft → 取消 / 确认；Confirmed → 取消 / 发料；PartiallyIssued → 提示）。待办队列与全部视图点单据均触发 drawer（`req_detail_trigger`），不再跳 detail。
- `dispatch_action` 扩展 `confirm` / `cancel`（`issue` 已有）；`WorkCenterActionForm` 加 `view`，`post_work_center_action` 据此重渲染对应 card（全部视图下操作不跳回待办）。
- `domain_detail_url` 的 Requisition 分支改 `None`；`domain_entries` 的 Requisition 不再渲染「查看全部」（`view_toggle` 替代）。

**删除独立页**：
- 删 `pages/wms_requisition_list.rs`、`pages/wms_requisition_detail.rs`
- `routes/wms_requisition.rs`：删 `RequisitionListPath` / `RequisitionDetailPath` 及其路由（保留 `RequisitionCreatePath` + `ProductsPath` + `ItemRowPath`）
- `pages/mod.rs`：移除对应 mod 声明

**改引用**：`wms_requisition_create.rs` 创建后 redirect + 返回链接从 `RequisitionListPath` 改为 `WmsWorkCenterPath?domain=requisition&view=all`。

### 阶段 3.2a（调拨）实施

调拨与领料单同构（4 状态、cancel/dispatch/complete 操作、行项目仅 `quantity`），机械复制 3.1 模式：

**作业中心 Transfer 域扩展**（`wms_work_center.rs`）：
- `view_toggle` / `render_work_center_page` / `post_work_center_action` 通用化：支持 Requisition + Transfer 两域的「待办/全部」视图切换（`is_all_view` + 按 domain 选 `render_X_all_card`）。
- `doc_detail_trigger` 通用化（原 `req_detail_trigger`，加 `drawer` 参数）：领料单 `req_detail`、调拨 `transfer_detail` 共用。
- 新增 `render_transfer_all_card` / `_table` / `_row` / `transfer_status_label`：调 `transfer_service.list` 渲染全状态调拨单表格（来源仓 / 目标仓 / 日期 / 状态）。
- 新增 `transfer_detail_drawer_body` + `transfer_detail_actions`（Draft→取消 / 调出，InTransit→完成）：替代独立 detail 页。
- `dispatch_action` 加 `transfer_cancel`（dispatch/complete 已有）；`action_domain` 加映射。
- `domain_detail_url` Transfer→`None`；`domain_entries` Transfer 去掉「查看全部」；`render_task_row` Transfer 单号→drawer trigger。

**删除独立页**：`pages/wms_transfer_list.rs`、`pages/wms_transfer_detail.rs` + `TransferListPath` / `TransferDetailPath` 路由 + mod 声明。

**改引用**：`wms_transfer_create.rs` redirect 改 `WmsWorkCenterPath?domain=transfer&view=all`。

### 阶段 3.2b（盘点）实施

盘点状态机更丰富（6 状态、6 操作），但 `count`（录入实盘量）UI 原详情页未实现（`service.count` 存在但无 UI 调用），drawer 沿用只读明细——故收口复杂度与调拨同档。

**作业中心 CycleCount 域扩展**（`wms_work_center.rs`）：
- `view_toggle` / `render_work_center_page` / `post_work_center_action` 的 `is_all_view` 扩展 CycleCount（req/transfer/cc 三域统一）。
- 新增 `render_cycle_count_all_card` / `_table` / `_row` / `cc_status_label`：调 `cycle_count_service.list` 渲染全状态盘点单表格（`CycleCountFilter` 无 `doc_number` 字段，全部视图暂不提供单号搜索，待 abt-core 补）。
- 新增 `cc_detail_drawer_body` + `cc_detail_actions`：单据头 + 行项目（系 / 盘 / 差三量）+ 就地操作（Draft→开始 / 取消，Counting→完成，Completed→调整 / 取消，PendingReview→批准 / 驳回）。操作用 `cc_` 前缀 action（`cc_start` / `cc_complete` / `cc_cancel` / `cc_adjust` / `cc_approve` / `cc_reject`），避免与调拨 `complete` / `cancel` 冲突。
- `dispatch_action` 加 6 个 `cc_` 分支；`action_domain` 加映射。
- `domain_detail_url` CycleCount→`None`；`domain_entries` 去查看全部；`render_task_row` + `render_row_action` CycleCount→`cc_detail` drawer trigger。

**删除独立页**：`pages/wms_cycle_count_list.rs`、`pages/wms_cycle_count_detail.rs` + `CycleCountListPath` / `CycleCountDetailPath` 路由 + mod 声明。

**改引用**：`wms_cycle_count_create.rs` redirect / 返回链接改 `WmsWorkCenterPath?domain=cycle-count&view=all`。

> 盘点 `count`（录入实盘量）UI 缺失是既有问题（非本阶段引入），建议后续在 `cc_detail` drawer 内补行级录入（仿 pick drawer）。

### 后续阶段（待 3.1 试点验证后推广）

| 阶段 | 范围 | 删除页面 |
|---|---|---|
| 3.2a | 调拨 transfer | list + detail（✅ 已完成，模式同 3.1） |
| 3.2b | 盘点 cycle_count | list + detail（✅ 已完成；`count` 录入 UI 原未实现，drawer 沿用只读明细 + start/complete/cancel/adjust/approve/reject 操作） |
| 3.3 | 入库 stock_in | list + detail（create 保留 suggest_bins） |
| 3.4 | 出库 shipping | list（detail 保留销售依赖） |
| 3.5（可选） | 新建 drawer 化 | 各 create 页 |

> **实施状态（2026-07-02）**：
> - **阶段 3.1 领料单**收口完成——作业中心承载待办 / 全部视图 + 详情 drawer + 确认 / 取消 / 发料就地操作，独立 list / detail 页与路由已删除，create 页入口收口到作业中心。
> - **阶段 3.2a 调拨**收口完成——模式同领料单（通用化 `view_toggle` / `doc_detail_trigger` / `is_all_view`），调拨待办 / 全部视图 + 详情 drawer + 取消 / 调出 / 完成就地操作，独立 list / detail 页与路由已删除。
> - **阶段 3.2b 盘点**收口完成——6 状态 / 6 操作（`cc_` 前缀避免与调拨冲突），盘点待办 / 全部视图 + 详情 drawer（系 / 盘 / 差三量 + start/complete/cancel/adjust/approve/reject），独立 list / detail 页与路由已删除。`count` 录入 UI 既有缺失，drawer 沿用只读明细。
> - `cargo clippy -p abt-web` 通过。
