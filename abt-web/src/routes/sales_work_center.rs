use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::sales_work_center;
use crate::state::AppState;

// ── Typed Paths ──
//
// 销售作业中心采用「每个 card 一个端点」的单端点模式（同采购 / MES / WMS work_center）：
// 首页内联渲染 card 外壳，#sc-card 占位 div `hx-trigger="load"` 拉默认 card；
// 各 card 的 tab/筛选/分页走各自 GET 端点 + hx-select="#sc-card" 局部刷新；
// 写操作 POST 广播 HX-Trigger（soChanged / salesQuotationChanged / salesReturnChanged /
// salesReconChanged），相关 card 监听自刷新。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center")]
pub struct SalesWorkCenterPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations")]
pub struct ScQuotationsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/orders")]
pub struct ScOrdersPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns")]
pub struct ScReturnsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/settlement")]
pub struct ScSettlementPath;

// ── 行展开 row-detail GET（HTMX 按需加载，返回单 <tr class="row-detail">）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/row-detail")]
pub struct ScQuotationRowDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/orders/{id}/row-detail")]
pub struct ScOrderRowDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/row-detail")]
pub struct ScReturnRowDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/settlement/{recon_type}/{ref_id}/row-detail")]
pub struct ScSettlementRowDetailPath {
    pub recon_type: String,
    pub ref_id: i64,
}

// ── 详情 drawer GET（就地查看 + 状态操作，对标采购 order-overlay）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/detail-drawer")]
pub struct ScQuotationDetailDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/orders/{id}/detail-drawer")]
pub struct ScOrderDetailDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/detail-drawer")]
pub struct ScReturnDetailDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/settlement/{id}/detail-drawer")]
pub struct ScSettlementDetailDrawerPath {
    pub id: i64,
}

// ── 写操作 POST（事务包裹，HX-Trigger 广播）──
// 报价：submit / accept / to-so（reject / expire 留后续）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/submit")]
pub struct ScQuotationSubmitPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/accept")]
pub struct ScQuotationAcceptPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/to-so")]
pub struct ScQuotationToSoPath {
    pub id: i64,
}

// 销售订单：confirm / cancel（申请发货 request-ship 留后续，复用 sales_order_detail 逻辑）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/orders/{id}/confirm")]
pub struct ScOrderConfirmPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/orders/{id}/cancel")]
pub struct ScOrderCancelPath {
    pub id: i64,
}

// 销售退货：approve / receive / complete（inspect / reject 留后续）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/approve")]
pub struct ScReturnApprovePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/receive")]
pub struct ScReturnReceivePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/complete")]
pub struct ScReturnCompletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/cancel")]
pub struct ScReturnCancelPath {
    pub id: i64,
}

// 月对账单：send / confirm / settle（dispute 留后续）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/send")]
pub struct ScReconSendPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/confirm")]
pub struct ScReconConfirmPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/settle")]
pub struct ScReconSettlePath {
    pub id: i64,
}

// 补全流转（reject / expire / inspect / dispute）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/reject")]
pub struct ScQuotationRejectPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/quotations/{id}/expire")]
pub struct ScQuotationExpirePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/inspect")]
pub struct ScReturnInspectPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/returns/{id}/reject")]
pub struct ScReturnRejectPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/dispute")]
pub struct ScReconDisputePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/reopen")]
pub struct ScReconReopenPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/{id}/force-settle")]
pub struct ScReconForceSettlePath {
    pub id: i64,
}

// 对账创建 drawer（就地新建对账单）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/create-drawer")]
pub struct ScReconCreateDrawerPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/sales/work-center/reconciliations/create")]
pub struct ScReconCreatePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // 首页 + 4 card
        .route(
            SalesWorkCenterPath::PATH,
            get(sales_work_center::get_work_center),
        )
        .route(
            ScQuotationsPath::PATH,
            get(sales_work_center::get_quotations_card),
        )
        .route(ScOrdersPath::PATH, get(sales_work_center::get_orders_card))
        .route(ScReturnsPath::PATH, get(sales_work_center::get_returns_card))
        .route(
            ScSettlementPath::PATH,
            get(sales_work_center::get_settlement_card),
        )
        // 行展开
        .route(
            ScQuotationRowDetailPath::PATH,
            get(sales_work_center::get_quotation_row_detail),
        )
        .route(
            ScOrderRowDetailPath::PATH,
            get(sales_work_center::get_order_row_detail),
        )
        .route(
            ScReturnRowDetailPath::PATH,
            get(sales_work_center::get_return_row_detail),
        )
        .route(
            ScSettlementRowDetailPath::PATH,
            get(sales_work_center::get_settlement_row_detail),
        )
        // 详情 drawer GET
        .route(
            ScQuotationDetailDrawerPath::PATH,
            get(sales_work_center::get_quotation_detail_drawer),
        )
        .route(
            ScOrderDetailDrawerPath::PATH,
            get(sales_work_center::get_order_detail_drawer),
        )
        .route(
            ScReturnDetailDrawerPath::PATH,
            get(sales_work_center::get_return_detail_drawer),
        )
        .route(
            ScSettlementDetailDrawerPath::PATH,
            get(sales_work_center::get_settlement_detail_drawer),
        )
        // 报价写操作
        .route(
            ScQuotationSubmitPath::PATH,
            post(sales_work_center::submit_quotation),
        )
        .route(
            ScQuotationAcceptPath::PATH,
            post(sales_work_center::accept_quotation),
        )
        .route(
            ScQuotationToSoPath::PATH,
            post(sales_work_center::quotation_to_so),
        )
        // 订单写操作
        .route(
            ScOrderConfirmPath::PATH,
            post(sales_work_center::confirm_order),
        )
        .route(
            ScOrderCancelPath::PATH,
            post(sales_work_center::cancel_order),
        )
        // 退货写操作
        .route(
            ScReturnApprovePath::PATH,
            post(sales_work_center::approve_return),
        )
        .route(
            ScReturnReceivePath::PATH,
            post(sales_work_center::receive_return),
        )
        .route(
            ScReturnCompletePath::PATH,
            post(sales_work_center::complete_return),
        )
        .route(
            ScReturnCancelPath::PATH,
            post(sales_work_center::cancel_return),
        )
        // 对账写操作
        .route(
            ScReconSendPath::PATH,
            post(sales_work_center::send_recon),
        )
        .route(
            ScReconConfirmPath::PATH,
            post(sales_work_center::confirm_recon),
        )
        .route(
            ScReconSettlePath::PATH,
            post(sales_work_center::settle_recon),
        )
        // 补全流转
        .route(
            ScQuotationRejectPath::PATH,
            post(sales_work_center::reject_quotation),
        )
        .route(
            ScQuotationExpirePath::PATH,
            post(sales_work_center::expire_quotation),
        )
        .route(
            ScReturnInspectPath::PATH,
            post(sales_work_center::inspect_return),
        )
        .route(
            ScReturnRejectPath::PATH,
            post(sales_work_center::reject_return),
        )
        .route(
            ScReconDisputePath::PATH,
            post(sales_work_center::dispute_recon),
        )
        .route(
            ScReconReopenPath::PATH,
            post(sales_work_center::reopen_recon),
        )
        .route(
            ScReconForceSettlePath::PATH,
            post(sales_work_center::force_settle_recon),
        )
        // 对账创建 drawer
        .route(
            ScReconCreateDrawerPath::PATH,
            get(sales_work_center::get_recon_create_drawer),
        )
        .route(
            ScReconCreatePath::PATH,
            post(sales_work_center::post_recon_create),
        )
}
