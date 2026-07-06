use axum::response::Redirect;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::shipping_list;
use crate::pages::shipping_detail;
use crate::pages::shipping_create;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping")]
pub struct ShippingListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/create")]
pub struct ShippingCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/edit")]
pub struct ShippingEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/draft")]
pub struct ShippingSaveDraftPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}")]
pub struct ShippingDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/delete")]
pub struct ShippingDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/confirm")]
pub struct ConfirmShippingPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/cancel")]
pub struct CancelShippingPath {
    pub id: i64,
}

/// Doc Hub disclosure 懒加载：拣货单 / 库存事务片段
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/fragments/{block}")]
pub struct ShippingFragmentPath {
    pub id: i64,
    pub block: String,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/customer-contacts")]
pub struct ShippingCustomerContactsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/order-search")]
pub struct ShippingOrderSearchPath;

/// HTMX: 选中订单后加载发货明细行（替代旧 selectOrder JS 拼 DOM），对齐 stock_in confirm 端点范式。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/order-items")]
pub struct ShippingOrderItemsPath;

/// 打印发货单：用默认 delivery_note 模板渲染真实数据
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/shipping/{id}/print")]
pub struct ShippingPrintPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ShippingListPath::PATH, get(shipping_list::get_shipping_list))
        .route(ShippingDetailPath::PATH, get(shipping_detail::get_shipping_detail))
        .route(ShippingCreatePath::PATH, get(shipping_create::get_shipping_create).post(shipping_create::post_shipping_create))
        .route(ShippingEditPath::PATH, get(shipping_create::get_shipping_edit))
        .route(ShippingSaveDraftPath::PATH, post(shipping_create::post_save_draft))
        .route(ShippingDeletePath::PATH, post(shipping_list::delete_shipping))
        .route(ShippingCustomerContactsPath::PATH, get(shipping_create::get_customer_contacts))
        .route(ShippingOrderSearchPath::PATH, get(shipping_create::get_order_search))
        .route(ShippingOrderItemsPath::PATH, get(shipping_create::get_order_items))
        .route(ConfirmShippingPath::PATH, post(shipping_detail::confirm_shipping))
        .route(CancelShippingPath::PATH, post(shipping_detail::cancel_shipping))
        .route(ShippingFragmentPath::PATH, get(shipping_detail::get_shipping_fragment))
        .route(ShippingPrintPath::PATH, get(shipping_detail::print_shipping))
        // 旧路径 /admin/shipping/* → /admin/wms/shipping/* 重定向（服务旧书签）
        .route(
            "/admin/shipping",
            axum::routing::any(|| async { Redirect::permanent("/admin/wms/shipping") }),
        )
        .route(
            "/admin/shipping/{*rest}",
            axum::routing::any(legacy_shipping_redirect),
        )
}

/// 旧发货路径 → wms 出库管理重定向（308 保留 method，POST 旧链接也安全）
async fn legacy_shipping_redirect(axum::extract::Path(rest): axum::extract::Path<String>) -> Redirect {
    Redirect::permanent(&format!("/admin/wms/shipping/{rest}"))
}
