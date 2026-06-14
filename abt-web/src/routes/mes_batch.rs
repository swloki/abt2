use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_batch_detail, mes_card_query, mes_schedule_board};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{id}")]
pub struct BatchDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{batch_id}/confirm-step")]
pub struct BatchConfirmStepPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{batch_id}/advance")]
pub struct BatchAdvancePath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{batch_id}/suspend")]
pub struct BatchSuspendPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{batch_id}/resume")]
pub struct BatchResumePath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/batches/{batch_id}/scrap")]
pub struct BatchScrapPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/cards")]
pub struct CardQueryPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/cards/search")]
pub struct CardQuerySearchPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/schedule")]
pub struct ScheduleBoardPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BatchDetailPath::PATH, get(mes_batch_detail::get_batch_detail))
        .route(BatchConfirmStepPath::PATH, post(mes_batch_detail::confirm_step))
        .route(BatchAdvancePath::PATH, post(mes_batch_detail::advance_to_receipt))
        .route(BatchSuspendPath::PATH, post(mes_batch_detail::suspend_batch))
        .route(BatchResumePath::PATH, post(mes_batch_detail::resume_batch))
        .route(BatchScrapPath::PATH, post(mes_batch_detail::scrap_batch))
        .route(CardQueryPath::PATH, get(mes_card_query::get_card_query))
        .route(CardQuerySearchPath::PATH, get(mes_card_query::search_card))
        .route(ScheduleBoardPath::PATH, get(mes_schedule_board::get_schedule_board))
}
