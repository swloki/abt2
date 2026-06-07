use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::MaterialUsagePath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_material_usage(_path: MaterialUsagePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" { h1 class="page-title" { "物料消耗追踪" } }
        div class="info-card" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
            "物料消耗追踪功能开发中"
        }
    }};
    Ok(Html(admin_page(is_htmx, "物料消耗追踪", &claims, "production", MaterialUsagePath::PATH, "生产管理", None, content).into_string()))
}
