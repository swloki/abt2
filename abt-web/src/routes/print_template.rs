use axum::routing::{get, post};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{print_template_list, print_template_edit};
use crate::state::AppState;
use axum::Router;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates")]
pub struct PrintTemplateListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/new")]
pub struct PrintTemplateCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/{id}")]
pub struct PrintTemplateDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/{id}/edit")]
pub struct PrintTemplateEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/{id}/delete")]
pub struct PrintTemplateDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/{id}/set-default")]
pub struct PrintTemplateSetDefaultPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/print/{template_id}")]
pub struct PrintPreviewPath {
    pub template_id: i64,
}

/// 编辑器实时预览（不入库，POST html_content + document_type → 后端 minijinja 渲染）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/system/print-templates/render-preview")]
pub struct RenderPreviewPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(PrintTemplateListPath::PATH, get(print_template_list::list))
        .route(PrintTemplateCreatePath::PATH, get(print_template_edit::create_page).post(print_template_edit::create))
        .route(PrintTemplateEditPath::PATH, get(print_template_edit::edit_page).post(print_template_edit::update))
        .route(PrintTemplateDeletePath::PATH, post(print_template_list::delete))
        .route(PrintTemplateSetDefaultPath::PATH, post(print_template_list::set_default))
        .route(PrintPreviewPath::PATH, get(print_template_edit::preview))
        .route(RenderPreviewPath::PATH, post(print_template_edit::render_preview))
}
