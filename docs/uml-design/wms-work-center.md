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
