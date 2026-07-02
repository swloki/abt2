use chrono::{DateTime, Utc};

/// 单环节统计（总数 + 逾期/临期计数；均基于该域全量，不含 keyword/urgency filter）
#[derive(Debug, Clone, Copy, Default)]
pub struct DomainStats {
    pub total: u64,
    pub overdue: u64,
    pub soon: u64,
}

/// 仓库作业中心待办汇总（5 个业务环节；每域 total + 紧急度计数）
///
/// 各字段 = 该环节"待处理"状态单据的统计。语义为执行层待办（非计划层需求），
/// 参照 Odoo `stock.picking` Operations 看板。2026-07：原 `picks` 合并入 `outbounds`（待出库）。
#[derive(Debug, Clone, Default)]
pub struct WorkCenterSummary {
    pub arrivals: DomainStats,
    pub outbounds: DomainStats,
    pub requisitions: DomainStats,
    pub transfers: DomainStats,
    pub cycle_counts: DomainStats,
}

impl WorkCenterSummary {
    /// 某 domain 的统计
    pub fn of(&self, d: WorkCenterDomain) -> DomainStats {
        match d {
            WorkCenterDomain::Arrival => self.arrivals,
            WorkCenterDomain::Outbound => self.outbounds,
            WorkCenterDomain::Requisition => self.requisitions,
            WorkCenterDomain::Transfer => self.transfers,
            WorkCenterDomain::CycleCount => self.cycle_counts,
        }
    }

    /// 待办总数（跨环节）
    pub fn total(&self) -> u64 {
        self.arrivals.total
            + self.outbounds.total
            + self.requisitions.total
            + self.transfers.total
            + self.cycle_counts.total
    }

    /// 是否无任何待办
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// 作业环节（对应作业中心的一个 tab / 锚点条 chip）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkCenterDomain {
    Arrival,
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

/// 待收货任务来源（Arrival domain 双来源：采购 PO / 生产工单），驱动 drawer 选 PO/工单表单。
/// 其他 domain 无意义，默认 `PurchaseOrder` 占位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSourceKind {
    PurchaseOrder,
    WorkOrder,
}

/// 跨域统一待办视图
///
/// 各域实体（PurchaseOrder / WorkOrder / ShippingRequest / ...）在 WorkCenterServiceImpl
/// 内映射成此结构，供前端作业中心 disclosure 队列统一渲染。
/// 跳转路径由前端按 `domain` + `doc_id` 拼接（分层：abt-core 不硬编码前端 URL）。
#[derive(Debug, Clone)]
pub struct PendingTask {
    pub doc_id: i64,
    pub doc_number: String,
    pub domain: WorkCenterDomain,
    /// 任务来源（仅 Arrival domain 有意义：PO/工单；其他 domain 占位 PurchaseOrder）
    pub source_kind: TaskSourceKind,
    /// 客户 / 供应商 / 产品名
    pub counterparty: String,
    /// 一行摘要，如 "待收 320 件"
    pub summary: String,
    /// 到期日（驱动 urgency 判逾期/临期）
    pub expected_at: Option<DateTime<Utc>>,
    /// 收到时间（单据 created_at，进入待办的时刻）
    pub received_at: Option<DateTime<Utc>>,
    pub urgency: Urgency,
}

/// 待办队列过滤条件（`list_pending` 用；过滤下推到 `WorkCenterRepo` SQL）。
/// keyword 模糊匹配 doc_number/counterparty；urgency/source_kind 精确筛选。AND 组合。
#[derive(Debug, Clone, Default)]
pub struct PendingTaskFilter {
    /// 关键词：模糊匹配 `doc_number` / `counterparty`（大小写不敏感，去首尾空白）
    pub keyword: Option<String>,
    /// 紧急度筛选
    pub urgency: Option<Urgency>,
    /// 来源类型筛选（仅 Arrival domain 有意义：PO / 工单）
    pub source_kind: Option<TaskSourceKind>,
}
