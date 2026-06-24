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
