use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, serde::Deserialize, axum_extra::routing::TypedPath)]
#[typed_path("/admin/mes/exceptions")]
pub struct ExceptionListPath;

#[require_permission("MES", "read")]
pub async fn get_exception_list(_path: ExceptionListPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" { h1 class="page-title" { "生产异常" } }
        div class="info-card" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
            "生产异常记录功能开发中"
        }
    }};
    Ok(Html(admin_page(is_htmx, "生产异常", &claims, "production", ExceptionListPath::PATH, "生产管理", None, content).into_string()))
}
