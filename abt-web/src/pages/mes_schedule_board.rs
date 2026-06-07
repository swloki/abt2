use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::ScheduleBoardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_schedule_board(_path: ScheduleBoardPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" { h1 class="page-title" { "排程看板" } }
        div class="info-card" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
            "排程看板功能开发中"
        }
    }};
    Ok(Html(admin_page(is_htmx, "排程看板", &claims, "production", ScheduleBoardPath::PATH, "生产管理", None, content).into_string()))
}
