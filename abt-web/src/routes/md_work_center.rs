use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{md_work_center_create, md_work_center_detail, md_work_center_list};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers")]
pub struct WorkCenterListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/new")]
pub struct WorkCenterCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/{id}")]
pub struct WorkCenterDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers/{id}/edit")]
pub struct WorkCenterEditPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WorkCenterListPath::PATH,
            get(md_work_center_list::get_work_center_list),
        )
        .route(
            WorkCenterCreatePath::PATH,
            get(md_work_center_create::get_work_center_create)
                .post(md_work_center_create::post_work_center_create),
        )
        .route(
            WorkCenterDetailPath::PATH,
            get(md_work_center_detail::get_work_center_detail),
        )
        .route(
            WorkCenterEditPath::PATH,
            get(md_work_center_create::get_work_center_edit)
                .post(md_work_center_create::post_work_center_update),
        )
}
