use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{bom_list, bom_create, bom_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms")]
pub struct BomListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/table")]
pub struct BomTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/new")]
pub struct BomCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}")]
pub struct BomDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}")]
pub struct BomUpdatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/delete")]
pub struct BomDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/publish")]
pub struct BomPublishPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/products")]
pub struct BomProductsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BomListPath::PATH, get(bom_list::get_bom_list))
        .route(BomTablePath::PATH, get(bom_list::get_bom_table))
        .route(BomCreatePath::PATH, get(bom_create::get_bom_create).post(bom_create::post_bom_create))
        .route(BomProductsPath::PATH, get(bom_create::get_bom_products))
        .route(BomDetailPath::PATH, get(bom_detail::get_bom_detail))
        .route(BomUpdatePath::PATH, post(bom_detail::update_bom))
        .route(BomDeletePath::PATH, post(bom_list::delete_bom))
        .route(BomPublishPath::PATH, post(bom_detail::publish_bom))
}
