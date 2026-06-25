//! MES 生产作业中心聚合视图模型

/// 作业中心待办汇总（锚点条计数）。
///
/// 各状态工单计数：查询失败按 0 容错，不连累整页（同 WMS work_center）。
#[derive(Debug, Clone, Default)]
pub struct MesWorkCenterSummary {
    /// 待下达：Draft + Planned 工单（订单排期 card 数据源）
    pub pending_release: u64,
    /// 生产中：Released + InProduction 工单（工单 card 数据源）
    pub in_production: u64,
}

impl MesWorkCenterSummary {
    /// 待办总数（锚点条左侧大数）
    pub fn total(&self) -> u64 {
        self.pending_release + self.in_production
    }
}
