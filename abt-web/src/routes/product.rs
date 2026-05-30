use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{product_list, product_create, product_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products")]
pub struct ProductListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/table")]
pub struct ProductTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/new")]
pub struct ProductCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}")]
pub struct ProductDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}")]
pub struct ProductUpdatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/delete")]
pub struct ProductDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/usage")]
pub struct ProductUsagePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/price")]
pub struct ProductPricePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/price-history")]
pub struct ProductPriceHistoryPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/watch")]
pub struct ProductWatchPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/unwatch")]
pub struct ProductUnwatchPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/products/{id}/copy")]
pub struct ProductCopyPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ProductListPath::PATH, get(product_list::get_product_list))
        .route(ProductTablePath::PATH, get(product_list::get_product_table))
        .route(ProductCreatePath::PATH, get(product_create::get_product_create).post(product_create::post_product_create))
        .route(ProductDetailPath::PATH, get(product_detail::get_product_detail))
        .route(ProductUpdatePath::PATH, post(product_detail::update_product))
        .route(ProductDeletePath::PATH, post(product_list::delete_product))
        .route(ProductUsagePath::PATH, get(product_list::get_product_usage))
        .route(ProductPricePath::PATH, post(product_list::update_product_price))
        .route(ProductPriceHistoryPath::PATH, get(product_list::get_price_history))
        .route(ProductWatchPath::PATH, post(product_list::watch_product))
        .route(ProductUnwatchPath::PATH, post(product_list::unwatch_product))
        .route(ProductCopyPath::PATH, get(product_create::copy_product))
}
