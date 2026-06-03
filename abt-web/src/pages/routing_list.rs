use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::routing::model::*;
use abt_core::master_data::routing::RoutingService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::components::pagination::pagination;
use crate::routes::routing::{
    RoutingDeletePath, RoutingDetailPath, RoutingListPath, RoutingTablePath,
    RoutingCreatePath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RoutingQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("ROUTING", "read")]
pub async fn get_routing_list(
    _path: RoutingListPath,
    ctx: RequestContext,
    Query(params): Query<RoutingQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.routing_service();

    let filter = RoutingQuery {
        keyword: params.keyword.clone(),
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;
    let content = routing_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "工艺路线管理",
        &claims,
        "md",
        RoutingListPath::PATH,
        "主数据管理",
        Some("工艺路线管理"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "read")]
pub async fn get_routing_table(
    ctx: RequestContext,
    Query(params): Query<RoutingQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.routing_service();

    let filter = RoutingQuery {
        keyword: params.keyword.clone(),
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(routing_table_fragment(&result, &params).into_string()))
}

#[require_permission("ROUTING", "delete")]
pub async fn delete_routing(
    path: RoutingDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.routing_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok((
        [("HX-Redirect", RoutingListPath::PATH)],
        Html(String::new()),
    ))
}

// ── Components ──

fn routing_list_page(
    result: &abt_core::shared::types::PaginatedResult<Routing>,
    params: &RoutingQueryParams,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "工艺路线管理" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(RoutingCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建工艺路线"
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (routing_table_fragment(result, params))
        }
    }
}

fn routing_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Routing>,
    params: &RoutingQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let total_count = result.total;

    html! {
        div class="customer-list-panel" {
            // ── Filter Bar ──
            div class="filter-bar" {
                div class="stat-chip" { "全部 " span class="chip-count" { (total_count) } }
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索工艺路线名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(RoutingTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .customer-list-panel"
                        hx-swap="outerHTML";
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "路线名称" }
                                th { "描述" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (routing_row(r))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="4" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无工艺路线数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(RoutingListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn routing_row(r: &Routing) -> Markup {
    let detail_path = RoutingDetailPath { id: r.id };
    let delete_path = RoutingDeletePath { id: r.id };

    html! {
        tr style="cursor:pointer" {
            td onclick=(format!("location.href='{}'", detail_path)) {
                strong { (r.name) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(ref desc) = r.description {
                    (desc)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(ref created) = r.created_at {
                    (created.format("%Y-%m-%d"))
                } @else {
                    "—"
                }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn text-danger" title="删除"
                        hx-confirm=(format!("确认删除工艺路线 {}？", r.name))
                        hx-post=(delete_path)
                        hx-target="closest tr"
                        hx-swap="outerHTML swap:0.5s" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

// ── Helpers ──

fn build_query_string(params: &RoutingQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    q.join("&")
}
