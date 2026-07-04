use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_requisition_create;
use crate::state::AppState;

// ── Typed Paths ──
// 领料单 list / detail 页已收口到作业中心（阶段 3.1）：作业中心承载待办/全部视图 + 详情 drawer，
// 不再有独立 list / detail 路由。Create 路由保留（新建页暂保留跳转，阶段 3.5 再 drawer 化）。

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
#[typed_path("/admin/wms/requisitions/create/wo-items")]
pub struct RequisitionWoItemsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
                .route(RequisitionItemRowPath::PATH, get(wms_requisition_create::get_item_row))
        .route(RequisitionWoItemsPath::PATH, get(wms_requisition_create::get_requisition_wo_items))
        .route(RequisitionCreatePath::PATH, get(wms_requisition_create::get_requisition_create).post(wms_requisition_create::create_requisition))
}
