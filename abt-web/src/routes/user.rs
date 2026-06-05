use axum::routing::{get, post};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{user_list, user_create, user_detail, user_edit};
use crate::state::AppState;
use axum::Router;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users")]
pub struct UserListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/table")]
pub struct UserTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/new")]
pub struct UserCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}")]
pub struct UserDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/delete")]
pub struct UserDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/toggle-status")]
pub struct UserToggleStatusPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/roles")]
pub struct UserRoleAssignPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/departments")]
pub struct UserDeptAssignPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/password")]
pub struct UserChangePasswordPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/users/{id}/edit")]
pub struct UserEditPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(UserListPath::PATH, get(user_list::get_user_list))
        .route(UserTablePath::PATH, get(user_list::get_user_table))
        .route(UserCreatePath::PATH, get(user_create::get_user_create).post(user_create::post_user_create))
        .route(UserDetailPath::PATH, get(user_detail::get_user_detail))
        .route(UserEditPath::PATH, get(user_edit::get_user_edit).post(user_edit::post_user_edit))
        .route(UserDeletePath::PATH, post(user_list::delete_user))
        .route(UserToggleStatusPath::PATH, post(user_list::toggle_user_status))
        .route(UserRoleAssignPath::PATH, post(user_detail::post_role_assign))
        .route(UserDeptAssignPath::PATH, post(user_detail::post_dept_assign))
        .route(UserChangePasswordPath::PATH, post(user_detail::post_change_password))
}
