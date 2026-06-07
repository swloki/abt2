use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_receipt_list, mes_receipt_create, mes_receipt_detail, mes_material_usage};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts")]
pub struct ReceiptListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/receipts/table")]
pub struct ReceiptTablePath;

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
#[typed_path("/admin/mes/material-usage")]
pub struct MaterialUsagePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ReceiptListPath::PATH, get(mes_receipt_list::get_receipt_list))
        .route(ReceiptTablePath::PATH, get(mes_receipt_list::get_receipt_table))
        .route(
            ReceiptCreatePath::PATH,
            get(mes_receipt_create::get_receipt_create).post(mes_receipt_create::create_receipt),
        )
        .route(ReceiptDetailPath::PATH, get(mes_receipt_detail::get_receipt_detail))
        .route(ReceiptConfirmPath::PATH, post(mes_receipt_detail::confirm_receipt))
        .route(MaterialUsagePath::PATH, get(mes_material_usage::get_material_usage))
}
