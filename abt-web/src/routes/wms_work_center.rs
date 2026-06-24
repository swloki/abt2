use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_work_center;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center")]
pub struct WmsWorkCenterPath;

pub fn router() -> Router<AppState> {
    Router::new().route(WmsWorkCenterPath::PATH, get(wms_work_center::get_wms_work_center))
}
