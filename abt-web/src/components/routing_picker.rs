use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::RoutingQuery;
use abt_core::shared::types::PageParams;

use crate::errors::Result;
use crate::state::AppState;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/routings/search")]
pub struct RoutingSearchPath;

#[derive(Debug, Deserialize)]
pub struct RoutingSearchParams {
    pub keyword: Option<String>,
    pub target_id: Option<String>,
    pub display_id: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route(RoutingSearchPath::PATH, get(search_routings))
}

pub async fn search_routings(
    ctx: RequestContext,
    Query(params): Query<RoutingSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.routing_service();
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            RoutingQuery { keyword: params.keyword.filter(|s| !s.is_empty()) },
            PageParams::new(1, 20),
        )
        .await?;
    let target = params.target_id.as_deref().unwrap_or("routing_id");
    let display = params.display_id.as_deref().unwrap_or("routing-display");
    Ok(Html(routing_picker_results(&result.items, target, display).into_string()))
}

/// 工艺路径选择弹窗（fill-input：选路径→填 hidden target_id + 显示名 + 发 routingSelected + 关弹窗）
pub fn routing_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    html! {
        div class="fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            id=(modal_id) _=(close_hs) {
            div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { "选择工艺路径" }
                    button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors" _=(close_hs) { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="routing-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "工艺路径名称" }
                            input class="routing-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text" name="keyword" placeholder="输入工艺路径名称…"
                                hx-get=(RoutingSearchPath::PATH) hx-trigger="keyup changed delay:300ms" hx-sync="this:replace"
                                hx-target="#routing-search-results" hx-swap="innerHTML" hx-include=".routing-search-bar" {}
                        }
                    }
                    div id="routing-search-results" class="max-h-[400px] overflow-y-auto"
                        hx-get=(RoutingSearchPath::PATH) hx-trigger="intersect once" hx-swap="innerHTML"
                        hx-vals=(format!("{{\"target_id\":\"{}\",\"display_id\":\"{}\"}}", target_id, display_id)) {
                        div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                    }
                }
            }
        }
    }
}

fn routing_picker_results(
    routings: &[abt_core::master_data::routing::model::Routing],
    target_id: &str,
    display_id: &str,
) -> Markup {
    let click_hs = format!(
        "on click set #{}'s value to my @data-rid then put my @data-rname into #{} then send routingSelected to body then remove .is-open from closest .is-open",
        target_id, display_id
    );
    html! {
        @if routings.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                p class="mt-2 text-sm" { "未找到匹配的工艺路径" }
            }
        } @else {
            div class="py-2" {
                @for r in routings {
                    div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        data-rid=(r.id) data-rname=(r.name.as_str()) _=(click_hs.clone()) {
                        div class="text-sm font-medium text-fg" { (r.name.as_str()) }
                        span class="text-xs text-accent font-medium shrink-0" { "选择" }
                    }
                }
            }
        }
    }
}
