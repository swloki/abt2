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
pub mod wms_dashboard;
pub mod wms_warehouse;
pub mod wms_bin;
pub mod wms_stock;
pub mod wms_stock_in;
pub mod wms_stock_out;
pub mod wms_arrival;
pub mod wms_transfer;
pub mod wms_requisition;
pub mod wms_conversion;
pub mod wms_backflush;
pub mod wms_cycle_count;
pub mod wms_inventory_lock;
pub mod wms_strategy;
pub mod wms_transaction_log;
pub mod wms_cascade;
pub mod mes_dashboard;
pub mod mes_plan;
pub mod mes_order;
pub mod mes_batch;
pub mod mes_report;
pub mod mes_inspection;
pub mod mes_receipt;
pub mod mes_exception;
pub mod om;
pub mod qms;
pub mod fms;
pub mod excel;
use axum::{Router, routing::get, middleware};

use crate::auth::middleware::auth_middleware;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(auth::router())
        .route("/api/toast", get(crate::toast::get_toasts).post(crate::toast::post_client_toast))
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
                // ── WMS (Inventory) ──
                .merge(wms_dashboard::router())
                .merge(wms_warehouse::router())
                .merge(wms_bin::router())
                .merge(wms_stock::router())
                .merge(wms_stock_in::router())
                .merge(wms_stock_out::router())
                .merge(wms_arrival::router())
                .merge(wms_transfer::router())
                .merge(wms_requisition::router())
                .merge(wms_conversion::router())
                .merge(wms_backflush::router())
                .merge(wms_cycle_count::router())
                .merge(wms_inventory_lock::router())
                .merge(wms_strategy::router())
                .merge(wms_transaction_log::router())
                .merge(wms_cascade::router())
                // ── MES (Production) ──
                .merge(mes_dashboard::router())
                .merge(mes_plan::router())
                .merge(mes_order::router())
                .merge(mes_batch::router())
                .merge(mes_report::router())
                .merge(mes_inspection::router())
                .merge(mes_receipt::router())
                .merge(mes_exception::router())
                // ── OM (Outsourcing) ──
                .merge(om::router())
                // ── QMS (Quality Management) ──
                .merge(qms::router())
                // ── FMS (Financial Management) ──
                .merge(fms::router())
                // ── System Management ──
                .merge(user::router())
                .merge(role::router())
                .merge(department::router())
                // ── Excel Import/Export ──
                .merge(excel::router())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                )),
        )
        .with_state(state)
}
