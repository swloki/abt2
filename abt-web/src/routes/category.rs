use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::category_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories")]
pub struct CategoryListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories/tree")]
pub struct CategoryTreePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories/{id}/panel")]
pub struct CategoryDetailPanelPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories")]
pub struct CategoryCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories/{id}")]
pub struct CategoryUpdatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/categories/{id}/delete")]
pub struct CategoryDeletePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(CategoryListPath::PATH, get(category_list::get_category_list))
        .route(CategoryTreePath::PATH, get(category_list::get_category_tree))
        .route(
            CategoryDetailPanelPath::PATH,
            get(category_list::get_category_detail_panel),
        )
        .route(
            CategoryCreatePath::PATH,
            post(category_list::create_category),
        )
        .route(
            CategoryUpdatePath::PATH,
            post(category_list::update_category),
        )
        .route(
            CategoryDeletePath::PATH,
            post(category_list::delete_category),
        )
}
