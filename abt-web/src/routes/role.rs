use axum::routing::{get, post};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{permission_config, role_create, role_detail, role_edit, role_list};
use crate::state::AppState;
use axum::Router;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles")]
pub struct RoleListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/table")]
pub struct RoleTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/new")]
pub struct RoleCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/{id}")]
pub struct RoleDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/{id}/delete")]
pub struct RoleDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/{id}/permissions")]
pub struct RolePermissionPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/roles/{id}/edit")]
pub struct RoleEditPath {
    pub id: i64,
}
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/permissions")]
pub struct PermissionConfigPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/permissions/toggle")]
pub struct PermissionTogglePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/permissions/toggle-batch")]
pub struct PermissionToggleBatchPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(RoleListPath::PATH, get(role_list::get_role_list))
        .route(RoleTablePath::PATH, get(role_list::get_role_table))
        .route(RoleCreatePath::PATH, get(role_create::get_role_create).post(role_create::post_role_create))
        .route(RoleDetailPath::PATH, get(role_detail::get_role_detail))
        .route(RoleDeletePath::PATH, post(role_list::delete_role))
        .route(RoleEditPath::PATH, get(role_edit::get_role_edit).post(role_edit::post_role_edit))
        .route(RolePermissionPath::PATH, post(role_detail::post_permission_assign))
        .route(PermissionConfigPath::PATH, get(permission_config::get_permission_config))
        .route(PermissionTogglePath::PATH, post(permission_config::post_permission_toggle))
        .route(PermissionToggleBatchPath::PATH, post(permission_config::post_permission_toggle_batch))
}
