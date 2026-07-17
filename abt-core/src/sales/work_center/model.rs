//! 销售作业中心聚合视图模型

use rust_decimal::Decimal;

use crate::sales::quotation::model::Quotation;
use crate::sales::reconciliation::model::ReconciliationStatus;
use crate::sales::sales_order::model::SalesOrder;
use crate::sales::sales_return::model::SalesReturn;

/// 销售作业中心待办汇总（锚点条 + 各 card 计数）。
///
/// 各项查询失败按 0 容错，不连累整页（同采购 / MES work_center）。
/// 写操作复用各域既有 Service，此处只做只读聚合。
#[derive(Debug, Clone, Default)]
pub struct SalesWorkCenterSummary {
    // ── 报价单（QuotationStatus: Draft=1 Sent=2 Accepted=3）──
    pub quotation_draft: u64,
    pub quotation_sent: u64,
    pub quotation_accepted: u64,
    // ── 销售订单（Draft=1 Confirmed=2 ReadyToShip=3 ShippingRequested=8 PartiallyShipped=4）──
    pub order_draft: u64,
    /// Confirmed + ReadyToShip 合计（均为「待发货」语义）
    pub order_pending_ship: u64,
    /// ShippingRequested（已申请待仓库拣货）
    pub order_shipping: u64,
    pub order_partial: u64,
    // ── 销售退货（ReturnStatus: Draft=1 Confirmed=2 Received=3）──
    pub return_pending: u64,
    pub return_pending_receive: u64,
    pub return_pending_inspect: u64,
    // ── 月对账单（ReconciliationStatus: Draft=1 Sent=2 Confirmed=3）──
    pub recon_draft: u64,
    pub recon_sent: u64,
    pub recon_confirmed: u64,
    // ── AR 联动（fms ArApService，客户维度，金额口径）──
    /// 未清应收余额（Σ amount_outstanding）
    pub ar_outstanding_amount: Decimal,
    /// 逾期应收金额（due_date < today 且未清）
    pub ar_overdue_amount: Decimal,
    // ── 各业务「全部」计数（tab badge 用，对齐 card 默认全部查询）──
    pub total_quotations: u64,
    pub total_orders: u64,
    pub total_returns: u64,
    pub total_recon: u64,
}

impl SalesWorkCenterSummary {
    /// 待办总数（锚点条左侧大数）。不含 AR 金额（避免与对账/订单语义重复）。
    pub fn total(&self) -> u64 {
        self.quotation_draft
            + self.quotation_sent
            + self.quotation_accepted
            + self.order_draft
            + self.order_pending_ship
            + self.order_shipping
            + self.order_partial
            + self.return_pending
            + self.return_pending_receive
            + self.return_pending_inspect
            + self.recon_draft
            + self.recon_sent
            + self.recon_confirmed
    }
}

// =============================================================================
// 行展开聚合视图（row-detail）
//
// 各聚合方法对子查询失败 best-effort 容错（返回默认/空，log warn），
// 不让单条外键缺失连累整行展开（同 summary 的容错哲学）。
// =============================================================================

/// 销售订单作业中心行展开聚合（订单 card row-detail）。
#[derive(Debug, Clone)]
pub struct SalesOrderHubSummary {
    pub order: SalesOrder,
    pub customer_name: String,
    /// 发货进度（聚合订单明细）
    pub progress: SalesOrderProgress,
    /// 来源单据链（Quotation → SalesOrder 派生，经 DocumentLink）
    pub source_chain: SalesOrderSourceChain,
    /// 应收台账摘要（客户维度）
    pub ar_summary: SalesOrderArSummary,
}

/// 发货进度（聚合 sales_order_items 各数量字段）。
#[derive(Debug, Clone, Default)]
pub struct SalesOrderProgress {
    /// 订购量 SUM(quantity)
    pub ordered_qty: Decimal,
    /// 已发货 SUM(shipped_qty)
    pub shipped_qty: Decimal,
    /// 未交量 SUM(open_qty) = quantity - shipped_qty - cancelled_qty
    pub open_qty: Decimal,
    /// 已退货 SUM(returned_qty)
    pub returned_qty: Decimal,
    /// 发货百分比 = shipped / ordered * 100（ordered = 0 时为 0，上限 100）
    pub shipped_pct: Decimal,
    /// 明细行数
    pub item_count: usize,
}

/// 销售订单上游来源链（经 DocumentLinkService 反查 Quotation→SalesOrder；best-effort）。
#[derive(Debug, Clone, Default)]
pub struct SalesOrderSourceChain {
    /// 上游报价单单号列表
    pub quotation_docs: Vec<String>,
}

/// 销售订单 AR 台账摘要（客户维度，ArApService::get_party_balance；best-effort）。
///
/// 注意：AR 在发货时由 `ShipmentShippedHandler` 立账（source_type=ShippingRequest，非销售订单），
/// 无法精确按 order_id 反查，第一阶段用客户维度余额近似。
#[derive(Debug, Clone, Default)]
pub struct SalesOrderArSummary {
    /// 已立应收（客户 AR 总额 total_ar）
    pub ar_amount: Decimal,
    /// 未清余额
    pub outstanding: Decimal,
}

/// 报价单行展开聚合（报价 card row-detail）。
#[derive(Debug, Clone)]
pub struct QuotationHubSummary {
    pub quotation: Quotation,
    pub customer_name: String,
    pub item_count: usize,
    pub total_amount: Decimal,
    /// 是否可转销售订单（Accepted 状态）
    pub can_convert_to_so: bool,
}

/// 销售退货行展开聚合（退货 card row-detail）。
#[derive(Debug, Clone)]
pub struct SalesReturnHubSummary {
    pub return_order: SalesReturn,
    pub customer_name: String,
    /// 来源销售订单单号
    pub source_so_doc: String,
    pub item_count: usize,
    pub total_qty: Decimal,
    /// 状态文案
    pub status_hint: String,
}

/// 对账收发行展开聚合（按对象类型分发：草稿/待发送对账单、待结算对账单 + 客户 AR 未清）。
#[derive(Debug, Clone)]
pub struct SettlementHubSummary {
    pub recon_type: SettlementReconType,
    pub customer_name: String,
    /// 对账单聚合（两类都填充对账单主体，差异在 status）
    pub recon: ReconciliationAggregate,
    /// 客户 AR 未清余额
    pub ar_outstanding: Decimal,
}

/// 对账单聚合。
#[derive(Debug, Clone)]
pub struct ReconciliationAggregate {
    pub id: i64,
    pub status: ReconciliationStatus,
    pub doc_number: String,
    pub period: String,
    pub total_amount: Decimal,
    pub confirmed_amount: Decimal,
    pub difference: Decimal,
    pub item_count: usize,
}

/// 对账收款 card 的对象类型（对应行展开的 2 类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementReconType {
    /// 草稿/待发送对账单（Draft / Sent）
    DraftRecon,
    /// 待结算对账单（Confirmed）
    PendingSettle,
}

impl SettlementReconType {
    /// 从路径参数解析（"draft" | "settle"）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::DraftRecon),
            "settle" => Some(Self::PendingSettle),
            _ => None,
        }
    }
}
