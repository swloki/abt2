pub mod dashboard;
pub mod login;

use axum::Router;
use axum::middleware;

use crate::auth::middleware::auth_middleware;
use crate::state::AppState;

/// Build the full router. Each module owns its TypedPath + handlers + router().
pub fn router(state: AppState) -> Router {
    // Protected routes: apply auth middleware
    let protected = dashboard::router()
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    // Public routes (login/logout) already consume their own state
    login::router(state).merge(protected)
}
