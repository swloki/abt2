use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_work_center;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center")]
pub struct WmsWorkCenterPath;

/// disclosure 懒加载：某环节的待办队列片段
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/fragments/{domain}")]
pub struct WmsWorkCenterFragmentPath {
    pub domain: String,
}

/// 拣货 drawer：GET 返回 drawer body（明细录入表单），POST 提交 record_pick_items
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/pick/{id}")]
pub struct WmsWorkCenterPickPath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WmsWorkCenterPath::PATH,
            get(wms_work_center::get_wms_work_center),
        )
        .route(
            WmsWorkCenterFragmentPath::PATH,
            get(wms_work_center::get_domain_fragment),
        )
        .route(
            WmsWorkCenterPickPath::PATH,
            get(wms_work_center::get_pick_drawer).post(wms_work_center::post_pick_items),
        )
}
