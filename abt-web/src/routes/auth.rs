use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::login;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/login")]
pub struct LoginPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/logout")]
pub struct LogoutPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/auth/refresh")]
pub struct RefreshTokenPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(LoginPath::PATH, get(login::get_login).post(login::post_login))
        .route(LogoutPath::PATH, post(login::post_logout))
        .route(RefreshTokenPath::PATH, post(login::post_refresh_token))
}
