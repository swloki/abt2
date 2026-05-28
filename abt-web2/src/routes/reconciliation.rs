use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::reconciliation_list;
use crate::pages::reconciliation_detail;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations")]
pub struct ReconciliationListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/table")]
pub struct ReconciliationTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/{id}")]
pub struct ReconciliationDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/{id}/send")]
pub struct SendReconciliationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/{id}/confirm")]
pub struct ConfirmReconciliationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/{id}/dispute")]
pub struct DisputeReconciliationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/reconciliations/{id}/settle")]
pub struct SettleReconciliationPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ReconciliationListPath::PATH, get(reconciliation_list::get_reconciliation_list))
        .route(ReconciliationTablePath::PATH, get(reconciliation_list::get_reconciliation_table))
        .route(ReconciliationDetailPath::PATH, get(reconciliation_detail::get_reconciliation_detail))
        .route(SendReconciliationPath::PATH, post(reconciliation_detail::send_reconciliation))
        .route(ConfirmReconciliationPath::PATH, post(reconciliation_detail::confirm_reconciliation))
        .route(DisputeReconciliationPath::PATH, post(reconciliation_detail::dispute_reconciliation))
        .route(SettleReconciliationPath::PATH, post(reconciliation_detail::settle_reconciliation))
}
