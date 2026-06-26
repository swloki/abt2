use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_work_center;
use crate::state::AppState;

// ── Typed Paths ──
//
// 工作中心采用「每个 card 一个端点」的单端点模式（同 MES / WMS work_center）：
// 首页内联渲染 4 个 card 外壳，每个 card 的 tab/筛选/分页走各自 GET 端点
// + hx-select="#pc-xxx-card" 局部刷新；写操作 POST 广播 HX-Trigger
// （poChanged / reconChanged / returnChanged），相关 card 监听自刷新。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center")]
pub struct PurchaseWorkCenterPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/demand")]
pub struct PcDemandPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/orders")]
pub struct PcOrdersPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/settlement")]
pub struct PcSettlementPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/returns")]
pub struct PcReturnsPath;

// ── 行展开 row-detail GET（HTMX 按需加载，返回单 <tr class="row-detail">）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/demand-rows")]
pub struct PcDemandRowsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/orders/{id}/row-detail")]
pub struct PcOrderRowDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/settlement/{recon_type}/{ref_id}/row-detail")]
pub struct PcSettlementRowDetailPath {
    pub recon_type: String,
    pub ref_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/returns/{id}/row-detail")]
pub struct PcReturnRowDetailPath {
    pub id: i64,
}

// ── 转采购单 drawer（就地转单，复用 create_order_from_demands）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/products/{product_id}/convert-po-drawer")]
pub struct PcConvertPoDrawerPath {
    pub product_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/products/{product_id}/convert-po")]
pub struct PcConvertPoPath {
    pub product_id: i64,
}

// ── Drawer GET（就地操作表单）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/orders/{id}/approve-drawer")]
pub struct PcOrderApproveDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/payments/{id}/approve-drawer")]
pub struct PcPaymentApproveDrawerPath {
    pub id: i64,
}

// ── 写操作 POST（事务包裹，HX-Trigger 广播）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/orders/{id}/approve")]
pub struct PcOrderApprovePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/orders/{id}/reject")]
pub struct PcOrderRejectPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/reconciliations/{id}/confirm")]
pub struct PcReconConfirmPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/work-center/payments/{id}/approve")]
pub struct PcPaymentApprovePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PurchaseWorkCenterPath::PATH,
            get(purchase_work_center::get_work_center),
        )
        .route(PcDemandPath::PATH, get(purchase_work_center::get_demand_card))
        .route(PcOrdersPath::PATH, get(purchase_work_center::get_orders_card))
        .route(
            PcSettlementPath::PATH,
            get(purchase_work_center::get_settlement_card),
        )
        .route(
            PcReturnsPath::PATH,
            get(purchase_work_center::get_returns_card),
        )
        .route(
            PcDemandRowsPath::PATH,
            get(purchase_work_center::get_demand_rows),
        )
        .route(
            PcOrderRowDetailPath::PATH,
            get(purchase_work_center::get_order_row_detail),
        )
        .route(
            PcSettlementRowDetailPath::PATH,
            get(purchase_work_center::get_settlement_row_detail),
        )
        .route(
            PcReturnRowDetailPath::PATH,
            get(purchase_work_center::get_return_row_detail),
        )
        .route(
            PcConvertPoDrawerPath::PATH,
            get(purchase_work_center::get_convert_po_drawer),
        )
        .route(
            PcConvertPoPath::PATH,
            post(purchase_work_center::post_convert_po),
        )
        .route(
            PcOrderApproveDrawerPath::PATH,
            get(purchase_work_center::get_order_approve_drawer),
        )
        .route(
            PcPaymentApproveDrawerPath::PATH,
            get(purchase_work_center::get_payment_approve_drawer),
        )
        .route(
            PcOrderApprovePath::PATH,
            post(purchase_work_center::approve_order),
        )
        .route(
            PcOrderRejectPath::PATH,
            post(purchase_work_center::reject_order),
        )
        .route(
            PcReconConfirmPath::PATH,
            post(purchase_work_center::confirm_recon),
        )
        .route(
            PcPaymentApprovePath::PATH,
            post(purchase_work_center::approve_payment),
        )
}
