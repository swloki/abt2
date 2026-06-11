use axum::routing::{get, post, delete};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{bom_list, bom_create, bom_detail, bom_edit};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms")]
pub struct BomListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/new")]
pub struct BomCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}")]
pub struct BomDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/edit")]
pub struct BomEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/nodes")]
pub struct BomNodesPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/nodes/{node_id}")]
pub struct BomNodePath {
    pub id: i64,
    pub node_id: i64,
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
#[typed_path("/admin/md/boms/{id}/category")]
pub struct BomUpdateCategoryPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/save-as")]
pub struct BomSaveAsPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/nodes/{node_id}/move")]
pub struct BomNodeMovePath {
    pub id: i64,
    pub node_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/products")]
pub struct BomProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/cost-drawer")]
pub struct BomCostDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/labor-cost-drawer")]
pub struct BomLaborCostDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/cost-drawer/temp-price")]
pub struct BomCostTempPricePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/boms/{id}/cost-drawer/temp-prices")]
pub struct BomCostClearTempPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BomListPath::PATH, get(bom_list::get_bom_list))
.route(BomCreatePath::PATH, get(bom_create::get_bom_create).post(bom_create::post_bom_create))
        .route(BomProductsPath::PATH, get(bom_edit::get_bom_products))
        .route(BomDetailPath::PATH, get(bom_detail::get_bom_detail))
        .route(BomEditPath::PATH, get(bom_edit::get_bom_edit))
        .route(BomNodesPath::PATH, post(bom_edit::add_node))
        .route(BomNodePath::PATH, get(bom_edit::get_node_edit_form).post(bom_edit::update_node).delete(bom_edit::delete_node))
        .route(BomNodeMovePath::PATH, post(bom_edit::move_node))
        .route(BomDeletePath::PATH, post(bom_list::delete_bom))
        .route(BomPublishPath::PATH, post(bom_detail::publish_bom))
        .route(BomUpdateCategoryPath::PATH, post(bom_edit::update_category))
        .route(BomSaveAsPath::PATH, post(bom_edit::save_as))
        .route(BomCostDrawerPath::PATH, get(bom_detail::get_cost_drawer))
        .route(BomLaborCostDrawerPath::PATH, get(bom_detail::get_labor_cost_drawer))
        .route(BomCostTempPricePath::PATH, post(bom_detail::save_temp_price))
        .route(BomCostClearTempPath::PATH, delete(bom_detail::clear_temp_prices))
}
