//! 采购作业中心聚合视图模型

use rust_decimal::Decimal;

use crate::purchase::order::model::PurchaseOrder;
use crate::purchase::return_order::model::PurchaseReturn;

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
    /// 各业务「全部」计数（tab badge 用，对齐 card 默认全部查询；与 pending_* 待办计数区分）
    pub demand_detail_total: u64,
    pub total_orders: u64,
    pub total_recon: u64,
    pub total_returns: u64,
    pub total_quotations: u64,
    pub total_misc: u64,
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

// =============================================================================
// 行展开聚合视图（row-detail）
//
// 各聚合方法对子查询失败 best-effort 容错（返回默认/空，log warn），
// 不让单条外键缺失连累整行展开（同 summary 的容错哲学）。
// =============================================================================

/// 采购订单作业中心行展开聚合（订单 card row-detail）。
#[derive(Debug, Clone)]
pub struct PoHubSummary {
    /// 订单主实体（复用，避免重复定义）
    pub order: PurchaseOrder,
    pub supplier_name: String,
    /// 收货进度（聚合订单明细）
    pub progress: PoProgress,
    /// 来源单据链（PO → SO 派生，经 DocumentLink）
    pub source_chain: PoSourceChain,
    /// 应付台账立账摘要
    pub ap_summary: PoApSummary,
}

/// 收货进度（聚合 purchase_order_items 各数量字段）。
#[derive(Debug, Clone, Default)]
pub struct PoProgress {
    /// 订购量 SUM(quantity)
    pub ordered_qty: Decimal,
    /// 已收货 SUM(received_qty)
    pub received_qty: Decimal,
    /// 已退货 SUM(returned_qty)
    pub returned_qty: Decimal,
    /// 已检验 SUM(inspected_qty)
    pub inspected_qty: Decimal,
    /// 收货百分比 = received / ordered * 100（ordered = 0 时为 0，上限 100）
    pub received_pct: Decimal,
    /// 明细行数
    pub item_count: usize,
}

/// PO 上游来源链（经 DocumentLinkService 反查）。
#[derive(Debug, Clone, Default)]
pub struct PoSourceChain {
    /// 上游销售订单单号列表（PO → SO 派生）
    pub sales_order_docs: Vec<String>,
}

/// PO 应付台账立账摘要（经 ArApService 反查该 PO 的立账行）。
#[derive(Debug, Clone, Default)]
pub struct PoApSummary {
    /// 已立应付金额 SUM(amount)
    pub ap_amount: Decimal,
    /// 已付（已核销）金额 SUM(amount_applied)
    pub paid_amount: Decimal,
}

/// 三单匹配校验摘要（付款 drawer ci-row）。
///
/// 提炼自 `PaymentRequestServiceImpl::approve` 的私有校验，口径一致（容差 ±0.5%）。
/// 「未提供」的情形（无对账单 / 无发票）按 approve 现有逻辑放行（= true）。
#[derive(Debug, Clone, Default)]
pub struct ThreeWayMatchSummary {
    /// PO/对账侧：付款金额 vs 对账确认金额（容差 0.5%）
    pub po_matched: bool,
    /// 入库侧：对账 received_qty ≤ PO 收货量 且 金额 = 净量×单价（容差 0.5%）
    pub receipt_matched: bool,
    /// 发票侧：发票金额 vs 付款金额（容差 0.5%）
    pub invoice_matched: bool,
    /// 综合可付款（三项全 true）
    pub can_pay: bool,
    /// 差异说明（任一不匹配时填，前端红色提示）
    pub differences: Vec<String>,
}

/// 对账付款行展开聚合（对账付款 card row-detail，按对象类型分发）。
#[derive(Debug, Clone)]
pub struct SettlementHubSummary {
    pub recon_type: SettlementReconType,
    pub supplier_name: String,
    /// 草稿对账单聚合（recon_type == DraftRecon 时填充）
    pub draft_recon: Option<DraftReconAggregate>,
    /// 待审批付款聚合（recon_type == PendingPayment 时填充）
    pub pending_payment: Option<PendingPaymentAggregate>,
}

/// 对账付款 card 的对象类型（对应 settlement card 的 2 个 tab）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementReconType {
    /// 草稿对账单（ref_id = reconciliation_id）
    DraftRecon,
    /// 待审批付款（ref_id = payment_id）
    PendingPayment,
}

impl SettlementReconType {
    /// 从路径参数解析（"draft" | "payment"）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::DraftRecon),
            "payment" => Some(Self::PendingPayment),
            _ => None,
        }
    }
}

/// 草稿对账单聚合（DraftRecon 分支）。
#[derive(Debug, Clone, Default)]
pub struct DraftReconAggregate {
    pub doc_number: String,
    pub period: String,
    pub total_amount: Decimal,
    pub confirmed_amount: Decimal,
    pub difference: Decimal,
    pub item_count: usize,
    /// 该供应商待结算（Shipped）退货笔数
    pub pending_returns_count: u64,
    pub pending_returns_amount: Decimal,
    /// 供应商 AP 未清余额
    pub ap_outstanding: Decimal,
}

/// 待审批付款聚合（PendingPayment 分支）。
#[derive(Debug, Clone)]
pub struct PendingPaymentAggregate {
    pub payment_id: i64,
    pub doc_number: String,
    pub amount: Decimal,
    /// 付款方式（中文）
    pub payment_method: String,
    pub invoice_number: Option<String>,
    pub invoice_amount: Option<Decimal>,
    /// 来源对账单单号
    pub source_recon_doc: Option<String>,
    /// 三单匹配校验
    pub three_way_match: ThreeWayMatchSummary,
    /// 供应商 AP 未清余额
    pub ap_outstanding: Decimal,
}

/// 采购退货行展开聚合（退货 card row-detail）。
#[derive(Debug, Clone)]
pub struct ReturnHubSummary {
    pub return_order: PurchaseReturn,
    pub supplier_name: String,
    /// 来源采购订单单号
    pub source_po_doc: String,
    /// 来源 PO 状态（中文）
    pub source_po_status: String,
    pub item_count: usize,
    pub total_qty: Decimal,
    /// 结算状态文案
    pub settlement_hint: String,
}
