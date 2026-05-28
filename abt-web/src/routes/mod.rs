pub mod auth;

pub mod customer;
pub mod dashboard;
pub mod order;
pub mod quotation;
pub mod reconciliation;

pub mod sales_return;
pub mod shipping;
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
                .merge(quotation::router())
                .merge(order::router())
                .merge(shipping::router())
                .merge(sales_return::router())
                .merge(reconciliation::router())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                )),
        )
        .with_state(state)
}
