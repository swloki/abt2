use axum::routing::get;
use axum::Router;

use crate::pages::purchase_settings;
use crate::state::AppState;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/settings")]
pub struct PurchaseSettingsPath;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PurchaseSettingsPath::PATH,
            get(purchase_settings::get_purchase_settings)
                .post(purchase_settings::update_purchase_settings),
        )
}
