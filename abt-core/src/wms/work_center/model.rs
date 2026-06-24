use chrono::{DateTime, Utc};

/// 仓库作业中心待办汇总（7 个业务环节）
///
/// 各字段 = 该环节"待处理"状态的单据数。语义为执行层待办（非计划层需求），
/// 参照 Odoo `stock.picking` Operations 看板。
#[derive(Debug, Clone, Default)]
pub struct WorkCenterSummary {
    pub arrivals_pending: u64, // 待收货（ArrivalStatus::Draft）
    pub inspections_pending: u64, // 待质检（ArrivalStatus::Inspecting）
    pub picks_pending: u64, // 待拣货（PickListStatus::Draft）
    pub outbounds_pending: u64, // 待发货（ShippingStatus::Confirmed + Picking）
    pub requisitions_pending: u64, // 待领料（RequisitionStatus::Confirmed + PartiallyIssued）
    pub transfers_pending: u64, // 待调拨（TransferStatus::Draft + InTransit）
    pub cycle_counts_pending: u64, // 待盘点（CycleCountStatus::Draft + Counting + PendingReview）
}

impl WorkCenterSummary {
    /// 待办总数
    pub fn total(&self) -> u64 {
        self.arrivals_pending
            + self.inspections_pending
            + self.picks_pending
            + self.outbounds_pending
            + self.requisitions_pending
            + self.transfers_pending
            + self.cycle_counts_pending
    }

    /// 是否无任何待办
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// 作业环节（对应作业中心的一个 disclosure 分区 / 锚点条 chip）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkCenterDomain {
    Arrival,
    Inspection,
    Pick,
    Outbound,
    Requisition,
    Transfer,
    CycleCount,
}

/// 待办紧急度（驱动锚点条染色 + 队列排序）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// 正常
    Normal,
    /// 临期（today + N 内到期）
    Soon,
    /// 逾期 / 超时
    Overdue,
}

/// 跨域统一待办视图
///
/// 各域实体（ArrivalNotice / PickList / ShippingRequest / ...）在 WorkCenterServiceImpl
/// 内映射成此结构，供前端作业中心 disclosure 队列统一渲染。
/// 跳转路径由前端按 `domain` + `doc_id` 拼接（分层：abt-core 不硬编码前端 URL）。
#[derive(Debug, Clone)]
pub struct PendingTask {
    pub doc_id: i64,
    pub doc_number: String,
    pub domain: WorkCenterDomain,
    /// 客户 / 供应商名（MVP 可填单据号或简单标识，名字解析后续补）
    pub counterparty: String,
    /// 一行摘要，如 "3 项 · 320 件"
    pub summary: String,
    /// 到期日（拣货等无到期日的环节用 created_at 判超时）
    pub expected_at: Option<DateTime<Utc>>,
    pub urgency: Urgency,
}

/// 紧急 / 临期汇总（摘要带 + 锚点条染色；消化 #93 followup P1 item 4）
#[derive(Debug, Clone, Default)]
pub struct UrgentSummary {
    pub overdue_count: u64,
    pub soon_count: u64,
}
