use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_transfer_create;
use crate::state::AppState;

// ── Typed Paths ──
// 调拨 list / detail 页已收口到作业中心（阶段 3.2）：作业中心承载待办/全部视图 + 详情 drawer，
// 不再有独立 list / detail 路由。Create 路由保留（新建页暂保留跳转，阶段 3.5 再 drawer 化）。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create")]
pub struct TransferCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create/products")]
pub struct TransferProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transfers/create/item-row")]
pub struct TransferItemRowPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
                .route(TransferItemRowPath::PATH, get(wms_transfer_create::get_item_row))
        .route(TransferCreatePath::PATH, get(wms_transfer_create::get_transfer_create).post(wms_transfer_create::create_transfer))
}
