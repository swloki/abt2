use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_transaction_log_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transactions")]
pub struct TransactionListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/transactions/table")]
pub struct TransactionTablePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(TransactionListPath::PATH, get(wms_transaction_log_list::get_transaction_list))
        .route(TransactionTablePath::PATH, get(wms_transaction_log_list::get_transaction_table))
}
