use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::enums::MiscRequestStatus;
use abt_core::purchase::misc_request::model::*;
use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::shared::types::PageParams;


use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::misc_request::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MiscQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn build_filter(params: &MiscQueryParams) -> MiscRequestQuery {
    MiscRequestQuery {
        department_id: None,
        status: params.status.and_then(MiscRequestStatus::from_i16),
        request_date_start: None,
        request_date_end: None,
    }
}

fn build_query_string(params: &MiscQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    q.join("&")
}

// ── Status Labels ──

fn status_label(s: MiscRequestStatus) -> (&'static str, &'static str) {
    match s {
        MiscRequestStatus::Draft => ("草稿", "status-draft"),
        MiscRequestStatus::Approved => ("已审批", "status-confirmed"),
        MiscRequestStatus::Purchasing => ("采购中", "status-info"),
        MiscRequestStatus::Received => ("已收货", "status-success"),
        MiscRequestStatus::Closed => ("已关闭", "status-cancelled"),
        MiscRequestStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("MISC_REQUEST", "read")]
pub async fn get_misc_list(
    _path: MiscListPath,
    ctx: RequestContext,
    Query(params): Query<MiscQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.misc_request_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let content = misc_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "零星请购",
        &claims,
        "purchase",
        MiscListPath::PATH,
        "采购管理",
        Some("零星请购"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("MISC_REQUEST", "read")]
pub async fn get_misc_table(
    ctx: RequestContext,
    Query(params): Query<MiscQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.misc_request_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(misc_table_fragment(&result, &params).into_string()))
}

// ── Components ──

fn misc_list_page(
    result: &abt_core::shared::types::PaginatedResult<MiscellaneousRequest>,
    params: &MiscQueryParams,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "零星请购" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(MiscCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建零星请购"
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (misc_table_fragment(result, params))
        }
    }
}

fn misc_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<MiscellaneousRequest>,
    params: &MiscQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已审批", count: None },
        TabItem { value: "3".into(), label: "采购中", count: None },
        TabItem { value: "4".into(), label: "已收货", count: None },
        TabItem { value: "5".into(), label: "已关闭", count: None },
        TabItem { value: "6".into(), label: "已取消", count: None },
    ];

    html! {
        div class="misc-list-panel" {
            (status_tabs(MiscTablePath::PATH, "closest .misc-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索单据编号…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(MiscTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .misc-list-panel"
                        hx-swap="outerHTML";
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "单据编号" }
                                th { "用途说明" }
                                th { "状态" }
                                th class="num-right" { "总金额" }
                                th { "申请日期" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (misc_row(r))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无请购数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(MiscListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn misc_row(r: &MiscellaneousRequest) -> Markup {
    let detail_path = MiscDetailPath { id: r.id };
    let (status_text, status_class) = status_label(r.status);
    let created = r.created_at.format("%Y-%m-%d").to_string();
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = r.status == MiscRequestStatus::Draft;

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (r.doc_number) }
            td onclick=(&onclick) { (r.purpose.as_str()) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right" onclick=(&onclick) { (r.total_amount.to_string()) }
            td class="mono" onclick=(&onclick) { (r.request_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) { (created) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" {
                        a class="row-action-btn" href=(detail_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                    }
                }
            }
        }
    }
}
