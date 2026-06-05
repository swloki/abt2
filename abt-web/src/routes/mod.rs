pub mod auth;
pub mod product;
pub mod category;
pub mod bom;
pub mod routing;
pub mod supplier;
pub mod labor_process_dict;
pub mod md_dashboard;
pub mod customer;
pub mod dashboard;
pub mod misc_request;
pub mod order;
pub mod payment_request;
pub mod purchase_dashboard;
pub mod purchase_order;
pub mod purchase_quotation;
pub mod purchase_reconciliation;
pub mod purchase_return;
pub mod quotation;
pub mod reconciliation;

pub mod sales_return;
pub mod shipping;
pub mod sidebar;
pub mod user;
pub mod role;
pub mod department;
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
                // ── Master Data (MD) ──
                .merge(md_dashboard::router())
                .merge(product::router())
                .merge(category::router())
                .merge(bom::router())
                .merge(routing::router())
                .merge(supplier::router())
                .merge(labor_process_dict::router())
                // ── Purchase (SRM) ──
                .merge(purchase_dashboard::router())
                .merge(purchase_quotation::router())
                .merge(purchase_order::router())
                .merge(purchase_return::router())
                .merge(purchase_reconciliation::router())
                .merge(payment_request::router())
                .merge(misc_request::router())
                // ── System Management ──
                .merge(user::router())
                .merge(role::router())
                .merge(department::router())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                )),
        )
        .with_state(state)
}
