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

> **实施状态（2026-07-02）**：阶段 1 + 阶段 2 已完成——inventory 侧边栏 5 个菜单（入库管理 / 出库管理 / 领料单 / 库存调拨 / 循环盘点）全部移除；作业中心 5 个 domain tab 均渲染「新建 / 查看全部」入口（`domain_entries`），跳转各业务保留的 Create / List 路由。各业务 list / create / detail 路由与页面保留不动，销售对账 / 退货跨模块链接、作业中心 `suggest_bins` 复用均无破坏。阶段 3 视评审再定。
