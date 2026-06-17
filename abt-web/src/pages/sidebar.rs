use crate::layout::sidebar::sidebar_body_fragment;
use crate::routes::sidebar::SidebarBodyPath;
use crate::utils::RequestContext;

// ── Handler ──

pub async fn get_sidebar_body(
 path: SidebarBodyPath,
 ctx: RequestContext,
) -> axum::response::Html<String> {
 let nav_filter = ctx.nav_filter().await;
 let fragment = sidebar_body_fragment(&ctx.claims, &path.module, &nav_filter);
 axum::response::Html(fragment.into_string())
}
