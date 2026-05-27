pub mod auth;
pub mod customer;
pub mod dashboard;
pub mod sidebar;

use axum::{Router, middleware};

use crate::auth::middleware::auth_middleware;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::router())
        .merge(
            dashboard::router()
                .merge(sidebar::router())
                .merge(customer::router())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
        )
        .with_state(state)
}
