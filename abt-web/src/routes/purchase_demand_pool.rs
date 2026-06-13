use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::purchase_demand_pool;
use crate::pages::purchase_demand_pool_create;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/demand-pool")]
pub struct PurchaseDemandPoolListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/demand-pool/create")]
pub struct PurchaseDemandPoolCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/demand-pool/create/supplier-detail")]
pub struct PurchaseDemandSupplierDetailPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/demand-pool/demand-rows")]
pub struct PurchaseDemandRowsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PurchaseDemandPoolListPath::PATH,
            get(purchase_demand_pool::get_demand_pool_list),
        )
        .route(
            PurchaseDemandPoolCreatePath::PATH,
            get(purchase_demand_pool_create::get_demand_pool_create)
                .post(purchase_demand_pool_create::create_order_from_demands),
        )
        .route(
            PurchaseDemandSupplierDetailPath::PATH,
            get(purchase_demand_pool_create::get_supplier_detail),
        )
        .route(
            PurchaseDemandRowsPath::PATH,
            get(purchase_demand_pool::get_demand_rows),
        )
}
