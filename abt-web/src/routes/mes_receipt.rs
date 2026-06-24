use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_receipt_list, mes_receipt_create, mes_receipt_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts")]
pub struct ReceiptListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/create")]
pub struct ReceiptCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/{id}")]
pub struct ReceiptDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/{receipt_id}/confirm")]
pub struct ReceiptConfirmPath {
    pub receipt_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/search-wo")]
pub struct ReceiptSearchWoPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/search-wh")]
pub struct ReceiptSearchWhPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/wo-selected")]
pub struct ReceiptWoSelectedPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/wh-zones")]
pub struct ReceiptWhZonesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/zn-bins")]
pub struct ReceiptZnBinsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ReceiptListPath::PATH, get(mes_receipt_list::get_receipt_list))
        .route(
            ReceiptCreatePath::PATH,
            get(mes_receipt_create::get_receipt_create).post(mes_receipt_create::create_receipt),
        )
        .route(ReceiptDetailPath::PATH, get(mes_receipt_detail::get_receipt_detail))
        .route(ReceiptConfirmPath::PATH, post(mes_receipt_detail::confirm_receipt))
        .route(ReceiptSearchWoPath::PATH, get(mes_receipt_create::search_wo))
        .route(ReceiptSearchWhPath::PATH, get(mes_receipt_create::search_wh))
        .route(ReceiptWoSelectedPath::PATH, get(mes_receipt_create::wo_selected))
        .route(ReceiptWhZonesPath::PATH, get(mes_receipt_create::get_wh_zones))
        .route(ReceiptZnBinsPath::PATH, get(mes_receipt_create::get_zn_bins))
}
