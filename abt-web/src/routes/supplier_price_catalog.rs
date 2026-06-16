use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::supplier_price_catalog;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices")]
pub struct SupplierPricesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices/create")]
pub struct PriceCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices/{id}")]
pub struct PriceEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices/{id}/delete")]
pub struct PriceDeletePath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            SupplierPricesPath::PATH,
            get(supplier_price_catalog::get_list).post(supplier_price_catalog::create_price),
        )
        .route(
            PriceCreatePath::PATH,
            get(supplier_price_catalog::get_create_modal),
        )
        .route(
            PriceEditPath::PATH,
            get(supplier_price_catalog::get_edit_modal)
                .post(supplier_price_catalog::update_price),
        )
        .route(
            PriceDeletePath::PATH,
            post(supplier_price_catalog::delete_price),
        )
}
