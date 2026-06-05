use axum::extract::Query;
use axum::response::Html;
use maud::{html, Markup};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use abt_core::wms::enums::ConversionStatus;
use abt_core::wms::form_conversion::{FormConversion, FormConversionService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_conversion::{ConversionCreatePath, ConversionDetailPath, ConversionListPath, ConversionTablePath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ConversionQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_conversion_list(
    _path: ConversionListPath,
    ctx: RequestContext,
    Query(params): Query<ConversionQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.form_conversion_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    let content = conversion_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "形态转换",
        &claims,
        "inventory",
        ConversionListPath::PATH,
        "库存管理",
        Some("形态转换"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_conversion_table(
    _path: ConversionTablePath,
    ctx: RequestContext,
    Query(params): Query<ConversionQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.form_conversion_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    Ok(Html(conversion_table_fragment(&result, &params).into_string()))
}

// ── Helpers ──

fn build_filter(params: &ConversionQueryParams) -> abt_core::wms::form_conversion::ConversionFilter {
    abt_core::wms::form_conversion::ConversionFilter {
        status: params.status.and_then(|s| ConversionStatus::from_i16(s)),
        warehouse_id: None,
    }
}

// ── Components ──

fn conversion_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<FormConversion>,
    params: &ConversionQueryParams,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "形态转换" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(ConversionCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建转换"
                    }
                }
            }

            (conversion_table_fragment(result, params))
        }
    }
}

fn conversion_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<FormConversion>,
    params: &ConversionQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已完成", count: None },
        TabItem { value: "3".into(), label: "已取消", count: None },
    ];

    html! {
        div class="conversion-list-panel" {
            (status_tabs(ConversionTablePath::PATH, "closest .conversion-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索转换单号…"
                        hx-get=(ConversionTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .conversion-list-panel"
                        hx-swap="outerHTML";
                }
            }

            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "转换单号" }
                                th { "转换仓库" }
                                th { "转换日期" }
                                th { "状态" }
                                th { "消耗项" }
                                th { "产出项" }
                                th { "操作员" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for c in &result.items {
                                (conversion_row(c))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无转换数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(ConversionListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn conversion_row(c: &FormConversion) -> Markup {
    let detail_path = ConversionDetailPath { id: c.id };

    let (status_label, status_class) = match c.status {
        ConversionStatus::Draft => ("草稿", "status-draft"),
        ConversionStatus::Completed => ("已完成", "status-completed"),
        ConversionStatus::Cancelled => ("已取消", "status-cancelled"),
    };

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (c.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) { (c.conversion_date.to_string()) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看" href=(detail_path.to_string()) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &ConversionQueryParams) -> String {
    let mut q = vec![];
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(p) = params.page {
        if p > 1 {
            q.push(format!("page={p}"));
        }
    }
    q.join("&")
}
