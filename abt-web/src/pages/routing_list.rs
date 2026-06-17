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
use crate::components::import_modal::{self, ImportModalConfig};
use crate::components::export_button;
use crate::components::pagination::pagination;
use crate::routes::routing::{
    RoutingDeletePath, RoutingDetailPath, RoutingListPath,
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
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("ROUTING", "create").await;
    let can_delete = ctx.has_permission("ROUTING", "delete").await;
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
    let content = routing_list_page(&result, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx,
        "工艺路线管理",
        &claims,
        "md",
        RoutingListPath::PATH,
        "主数据管理",
        Some("工艺路线管理"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
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
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "工艺路线管理" }
                div class="flex gap-3" {
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface"
                        _=(import_modal::import_modal_onclick(&ImportModalConfig { import_type: "labor-process", title: "", template_columns: "" })) {
                        (icon::upload_icon("w-4 h-4"))
                        "导入"
                    }
                    (export_button::export_button("导出工艺路线", "labor-process"))
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(RoutingCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建工艺路线"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (routing_table_fragment(result, params, can_delete))

            // ── Import Modal ──
            (import_modal::import_modal(&ImportModalConfig {
                import_type: "labor-process",
                title: "导入工艺路线",
                template_columns: "产品编码, 工序编码, 工序名称, 单价, 数量, 排序, 备注",
            }))

        }
    }
}

fn routing_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Routing>,
    params: &RoutingQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let total_count = result.total;

    html! {
        div class="customer-list-panel" {
            // ── Filter Bar ──
            div class="flex items-center gap-3 mb-5 flex-wrap" {
                div class="inline-flex items-center gap-1 px-3 py-1 bg-surface rounded-full text-xs text-text-muted font-medium" { "全部 " span class="font-bold text-fg" { (total_count) } }
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索工艺路线名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(RoutingListPath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-sync="this:replace"
                        hx-target="closest .customer-list-panel"
                        hx-swap="outerHTML";
                }
            }

            // ── Data Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "路线名称" }
                                th { "描述" }
                                th { "创建时间" }
                                th class="text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (routing_row(r, can_delete))
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

fn routing_row(r: &Routing, can_delete: bool) -> Markup {
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
                div class="flex items-center gap-1 justify-end [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer" {
                    a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    @if can_delete {
                        button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
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
}

// ── Helpers ──

fn build_query_string(params: &RoutingQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    q.join("&")
}
