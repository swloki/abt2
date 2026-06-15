use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::supplier_price_catalog;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices")]
pub struct SupplierPricesPath;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            SupplierPricesPath::PATH,
            get(supplier_price_catalog::get_supplier_prices)
                .post(supplier_price_catalog::create_price),
        )
        .route(
            "/admin/purchase/supplier-prices/{id}/delete",
            post(supplier_price_catalog::delete_price),
        )
}
