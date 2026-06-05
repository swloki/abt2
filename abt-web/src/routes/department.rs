use axum::routing::{get, post};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::department_list;
use crate::state::AppState;
use axum::Router;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/departments")]
pub struct DepartmentListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/departments/create-drawer")]
pub struct DepartmentCreateDrawerPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/departments/{id}")]
pub struct DepartmentDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/departments/{id}/edit")]
pub struct DepartmentEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/departments/{id}/delete")]
pub struct DepartmentDeletePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(DepartmentListPath::PATH, get(department_list::get_department_list))
        .route(DepartmentCreateDrawerPath::PATH, get(department_list::get_department_create_drawer).post(department_list::post_department_create))
        .route(DepartmentDetailPath::PATH, get(department_list::get_department_detail_fragment))
        .route(DepartmentEditPath::PATH, get(department_list::get_department_edit_drawer).post(department_list::post_department_update))
        .route(DepartmentDeletePath::PATH, post(department_list::delete_department))
}
