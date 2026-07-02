use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_cycle_count_create;
use crate::state::AppState;

// ── Typed Paths ──
// 盘点 list / detail 页已收口到作业中心（阶段 3.2b）：作业中心承载待办/全部视图 + 详情 drawer，
// 不再有独立 list / detail 路由。Create 路由保留（新建页暂保留跳转，阶段 3.5 再 drawer 化）。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/create")]
pub struct CycleCountCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/create/products")]
pub struct CycleCountProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/cycle-counts/create/item-row")]
pub struct CycleCountItemRowPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
                .route(CycleCountItemRowPath::PATH, get(wms_cycle_count_create::get_item_row))
        .route(CycleCountCreatePath::PATH, get(wms_cycle_count_create::get_cycle_count_create).post(wms_cycle_count_create::create_cycle_count))
}
