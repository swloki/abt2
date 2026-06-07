use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::WageListPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_wage_list(_path: WageListPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" { h1 class="page-title" { "计件工资" } }
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" { thead { tr {
                    th { "工人" } th { "工序" } th { "日期" }
                    th class="num-right" { "完成" } th class="num-right" { "不良" }
                    th class="num-right" { "单价" } th class="num-right" { "工资" }
                }} tbody {
                    tr { td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无工资数据" } }
                }}
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "计件工资", &claims, "production", WageListPath::PATH, "生产管理", None, content).into_string()))
}
