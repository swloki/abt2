use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::CardQueryPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_card_query(_path: CardQueryPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = card_query_page();
    Ok(Html(admin_page(is_htmx, "流转卡查询", &claims, "production", CardQueryPath::PATH, "生产管理", None, content).into_string()))
}

fn card_query_page() -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "流转卡查询" } }
        div class="info-card" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
            "请输入流转卡序列号进行查询"
        }
        div class="form-grid" style="max-width:400px;margin:0 auto" {
            div class="form-field span-2" {
                label class="form-label" { "流转卡序列号" }
                input class="form-input" type="text" placeholder="扫描或输入卡号...";
            }
        }
    }}
}
