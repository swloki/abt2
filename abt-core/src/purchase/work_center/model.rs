//! 采购作业中心聚合视图模型

/// 采购作业中心待办汇总（锚点条 + 各 card 计数）。
///
/// 各项查询失败按 0 容错，不连累整页（同 MES / WMS work_center）。
/// 写操作复用各域既有 Service，此处只做只读聚合。
#[derive(Debug, Clone, Default)]
pub struct PurchaseWorkCenterSummary {
    /// 待处理外购需求（物料维度，需求池 card 数据源）
    pub pending_demand: u64,
    /// 待审批零星请购（Draft）
    pub pending_misc: u64,
    /// 采购订单待审批（PendingApproval）
    pub po_pending_approval: u64,
    /// 采购订单待收货（Confirmed）
    pub po_pending_receive: u64,
    /// 采购订单部分收货（PartiallyReceived）
    pub po_partial: u64,
    /// 草稿对账单（Draft）
    pub recon_draft: u64,
    /// 付款申请待审批（Draft）
    pub payment_pending_approval: u64,
    /// 采购退货待发货（Confirmed）
    pub return_pending_ship: u64,
    /// 采购退货已发出（Shipped）
    pub return_shipped: u64,
    /// 逾期：待收货订单中期望交货日早于今日（近似，扫描首页 N 条）
    pub overdue_count: u64,
    /// 临期：待收货订单中期望交货日在今日起 N 天内
    pub soon_count: u64,
}

impl PurchaseWorkCenterSummary {
    /// 待办总数（锚点条左侧大数）。不含 overdue/soon，避免与待收货计数重复。
    pub fn total(&self) -> u64 {
        self.pending_demand
            + self.pending_misc
            + self.po_pending_approval
            + self.po_pending_receive
            + self.po_partial
            + self.recon_draft
            + self.payment_pending_approval
            + self.return_pending_ship
            + self.return_shipped
    }
}
