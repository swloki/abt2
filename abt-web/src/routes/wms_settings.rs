use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_settings;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/settings")]
pub struct WmsSettingsPath;

pub fn router() -> Router<AppState> {
    Router::new().route(
        WmsSettingsPath::PATH,
        get(wms_settings::get_wms_settings).post(wms_settings::update_wms_settings),
    )
}
