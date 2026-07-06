//! 财务作业中心聚合视图模型

use rust_decimal::Decimal;

/// 财务作业中心待办汇总（顶栏 pill + 各 tab badge 用）。
///
/// 各项查询失败按 0 / Decimal::ZERO 容错，不连累整页（同采购 / MES work_center）。
/// 写操作复用 fms 各域既有 Service，此处只做只读聚合。
#[derive(Debug, Clone, Default)]
pub struct FmsWorkCenterSummary {
    // ── 应收（AR，party_type=Customer）──
    /// 未清金额 ← ledger_summary(Customer).total_outstanding
    pub ar_outstanding_amount: Decimal,
    /// 逾期金额 ← .total_overdue
    pub ar_overdue_amount: Decimal,
    /// 7 天内到期 ← .due_within_7d
    pub ar_due_soon_amount: Decimal,
    /// 未清笔数 ← list_ledger(outstanding_only).total
    pub ar_outstanding_count: u64,
    // ── 应付（AP，party_type=Supplier）── 对称
    pub ap_outstanding_amount: Decimal,
    pub ap_overdue_amount: Decimal,
    pub ap_due_soon_amount: Decimal,
    pub ap_outstanding_count: u64,
    // ── 应收调整（已过账总数，tab badge 用；调整单创建即过账，非待办，不计入 total）──
    pub ar_adjustment_total: u64,
    // ── 应付调整 ──
    pub ap_adjustment_total: u64,
    // ── 核销 ──
    /// 核销记录总数 ← list_settlements(default).total
    pub settlement_total: u64,
}

impl FmsWorkCenterSummary {
    /// 顶栏「待办总数」pill：AR/AP 未清笔数（不含金额/调整/核销，避免与 pill 重复）。
    pub fn total(&self) -> u64 {
        self.ar_outstanding_count + self.ap_outstanding_count
    }
    /// 顶栏红 pill：AR+AP 逾期金额合计。
    pub fn total_overdue(&self) -> Decimal {
        self.ar_overdue_amount + self.ap_overdue_amount
    }
    /// 顶栏黄 pill：AR+AP 7 天内到期金额合计。
    pub fn total_due_soon(&self) -> Decimal {
        self.ar_due_soon_amount + self.ap_due_soon_amount
    }
}
