use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::labor_process_dict_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/process-dicts")]
pub struct ProcessDictListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/process-dicts/table")]
pub struct ProcessDictTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/process-dicts/new")]
pub struct ProcessDictCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/process-dicts/{id}")]
pub struct ProcessDictDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/process-dicts/{id}/delete")]
pub struct ProcessDictDeletePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            ProcessDictListPath::PATH,
            get(labor_process_dict_list::get_process_dict_list),
        )
        .route(
            ProcessDictTablePath::PATH,
            get(labor_process_dict_list::get_process_dict_table),
        )
        .route(
            ProcessDictCreatePath::PATH,
            get(labor_process_dict_list::get_process_dict_create)
                .post(labor_process_dict_list::post_process_dict_create),
        )
        .route(
            ProcessDictDeletePath::PATH,
            post(labor_process_dict_list::delete_process_dict),
        )
}
