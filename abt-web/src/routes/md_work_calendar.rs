use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{md_work_calendar_create, md_work_calendar_detail, md_work_calendar_list};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars")]
pub struct WorkCalendarListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars/new")]
pub struct WorkCalendarCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-calendars/{id}")]
pub struct WorkCalendarDetailPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WorkCalendarListPath::PATH,
            get(md_work_calendar_list::get_work_calendar_list),
        )
        .route(
            WorkCalendarCreatePath::PATH,
            get(md_work_calendar_create::get_work_calendar_create)
                .post(md_work_calendar_create::post_work_calendar_create),
        )
        .route(
            WorkCalendarDetailPath::PATH,
            get(md_work_calendar_detail::get_work_calendar_detail),
        )
}
