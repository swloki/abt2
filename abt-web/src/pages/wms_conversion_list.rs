use axum::extract::Query;
use axum::response::Html;
use maud::{html, Markup};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use abt_core::wms::enums::ConversionStatus;
use abt_core::wms::form_conversion::{FormConversion, FormConversionService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::wms_conversion::{ConversionCreatePath, ConversionDetailPath, ConversionListPath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ConversionQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub doc_number: Option<String>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_conversion_list(
    _path: ConversionListPath,
    ctx: RequestContext,
    Query(params): Query<ConversionQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_create = ctx.has_permission("INVENTORY", "create").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.form_conversion_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    let content = conversion_list_page(&result, &params, can_create);
    let page_html = admin_page(
        is_htmx,
        "形态转换",
        &claims,
        "inventory",
        ConversionListPath::PATH,
        "库存管理",
        Some("形态转换"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &ConversionQueryParams) -> abt_core::wms::form_conversion::ConversionFilter {
    abt_core::wms::form_conversion::ConversionFilter {
        status: params.status.and_then(ConversionStatus::from_i16),
        warehouse_id: None,
    }
}

fn conversion_data_card(
    result: &abt_core::shared::types::pagination::PaginatedResult<FormConversion>,
    query: &str,
) -> Markup {
    html! {
        div id="conversion-data-card" class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "转换单号" }
                            th { "转换仓库" }
                            th { "转换日期" }
                            th { "状态" }
                            th { "消耗项" }
                            th { "产出项" }
                            th { "操作员" }
                            th class="text-right" { "操作" }
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
            (pagination(ConversionListPath::PATH, query, result.total, result.page, result.total_pages))
        }
    }
}

// ── Components ──

fn conversion_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<FormConversion>,
    params: &ConversionQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "形态转换" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ConversionCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建转换"
                        }
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
            (status_tabs_with_param(ConversionListPath::PATH, "#conversion-data-card", "#conversion-filter-form", tabs, &active_value, "status"))

            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="conversion-filter-form"
                hx-get=(ConversionListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#conversion-data-card"
                hx-select="#conversion-data-card"
                hx-swap="outerHTML"
                hx-include="#conversion-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="doc_number"
                        placeholder="转换单号";
                }
            }

            (conversion_data_card(result, &query))
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
            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (c.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (c.conversion_date.to_string()) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick="event.stopPropagation()" {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看" href=(detail_path.to_string()) {
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
    if let Some(ref s) = params.doc_number {
        q.push(format!("doc_number={s}"));
    }
    q.join("&")
}
