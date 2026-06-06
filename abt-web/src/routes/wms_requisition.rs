use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_requisition_list, wms_requisition_create, wms_requisition_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions")]
pub struct RequisitionListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/table")]
pub struct RequisitionTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/create")]
pub struct RequisitionCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/create/products")]
pub struct RequisitionProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/create/item-row")]
pub struct RequisitionItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/requisitions/{id}")]
pub struct RequisitionDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(RequisitionListPath::PATH, get(wms_requisition_list::get_requisition_list))
        .route(RequisitionTablePath::PATH, get(wms_requisition_list::get_requisition_table))
        .route(RequisitionProductsPath::PATH, get(wms_requisition_create::get_products))
        .route(RequisitionItemRowPath::PATH, get(wms_requisition_create::get_item_row))
        .route(RequisitionCreatePath::PATH, get(wms_requisition_create::get_requisition_create).post(wms_requisition_create::create_requisition))
        .route(RequisitionDetailPath::PATH, get(wms_requisition_detail::get_requisition_detail).post(wms_requisition_detail::post_requisition_action))
}
