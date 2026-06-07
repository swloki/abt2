use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{mes_report_list, mes_report_create, mes_report_detail, mes_wage_list};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/reports")]
pub struct ReportListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/reports/table")]
pub struct ReportTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/reports/create")]
pub struct ReportCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/reports/{id}")]
pub struct ReportDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/wages")]
pub struct WageListPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(ReportListPath::PATH, get(mes_report_list::get_report_list))
        .route(ReportTablePath::PATH, get(mes_report_list::get_report_table))
        .route(
            ReportCreatePath::PATH,
            get(mes_report_create::get_report_create).post(mes_report_create::create_report),
        )
        .route(ReportDetailPath::PATH, get(mes_report_detail::get_report_detail))
        .route(WageListPath::PATH, get(mes_wage_list::get_wage_list))
}
