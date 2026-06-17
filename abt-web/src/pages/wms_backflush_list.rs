use axum::extract::Query;
use axum::response::Html;
use maud::{html, Markup};
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use abt_core::wms::backflush::{BackflushRecord, BackflushService};
use abt_core::wms::enums::BackflushStatus;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::wms_backflush::{BackflushDetailPath, BackflushListPath};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BackflushQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub doc_number: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub work_order: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_backflush_list(
    _path: BackflushListPath,
    ctx: RequestContext,
    Query(params): Query<BackflushQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.backflush_service();

    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1);
    let page_size = 20u32;

    let result = svc.list(&service_ctx, &mut conn, filter, page, page_size).await?;

    let content = backflush_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "倒冲记录",
        &claims,
        "inventory",
        BackflushListPath::PATH,
        "库存管理",
        Some("倒冲记录"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn build_filter(params: &BackflushQueryParams) -> abt_core::wms::backflush::BackflushFilter {
    abt_core::wms::backflush::BackflushFilter {
        status: params.status.and_then(BackflushStatus::from_i16),
        work_order_id: None,
    }
}

// ── Components ──

fn backflush_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<BackflushRecord>,
    params: &BackflushQueryParams,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "倒冲记录" }
            }

            (backflush_table_fragment(result, params))
        }
    }
}

fn backflush_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<BackflushRecord>,
    params: &BackflushQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div class="backflush-list-panel" {
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="filter-form"
                hx-get=(BackflushListPath::PATH)
                hx-trigger="change,keyup changed delay:300ms from:.search-input"
                hx-target="#backflush-data-card"
                hx-select="#backflush-data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="doc_number"
                        style="width:180px"
                        placeholder="单据编号"
                        value=(params.doc_number.as_deref().unwrap_or(""));
                }
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="work_order"
                        placeholder="工单号"
                        value=(params.work_order.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="status" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "草稿" }
                    option value="2" selected[params.status == Some(2)] { "已执行" }
                    option value="3" selected[params.status == Some(3)] { "已调整" }
                }
            }

            div id="backflush-data-card" class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                        thead {
                            tr {
                                th { "单据编号" }
                                th { "关联工单" }
                                th { "完工产品" }
                                th class="text-right text-[13px]" { "完工数量" }
                                th { "倒冲日期" }
                                th { "状态" }
                                th { "差异预警" }
                                th { "操作员" }
                                th class="!text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (backflush_row(r))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无倒冲记录"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(BackflushListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn backflush_data_card(
    result: &abt_core::shared::types::pagination::PaginatedResult<BackflushRecord>,
    params: &BackflushQueryParams,
) -> Markup {
    let query = build_query_string(params);

    html! {
        div id="backflush-data-card" class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                    thead {
                        tr {
                            th { "单据编号" }
                            th { "关联工单" }
                            th { "完工产品" }
                            th class="text-right text-[13px]" { "完工数量" }
                            th { "倒冲日期" }
                            th { "状态" }
                            th { "差异预警" }
                            th { "操作员" }
                            th class="!text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for r in &result.items {
                            (backflush_row(r))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无倒冲记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(BackflushListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}

fn backflush_row(r: &BackflushRecord) -> Markup {
    let detail_path = BackflushDetailPath { id: r.id };

    let (status_label, status_class) = match r.status {
        BackflushStatus::Draft => ("草稿", "status-draft"),
        BackflushStatus::Executed => ("已执行", "status-completed"),
        BackflushStatus::Adjusted => ("已调整", "status-confirmed"),
    };

    html! {
        tr style="cursor:pointer" {
            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (r.doc_number) }
            td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) { (format!("{:.2}", r.completed_qty)) }
            td class="font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (r.backflush_date.to_string()) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick=(format!("location.href='{}'", detail_path)) { "—" }
            td onclick="event.stopPropagation()" {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="查看详情" href=(detail_path.to_string()) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &BackflushQueryParams) -> String {
    let mut q = vec![];
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(ref v) = params.doc_number
        && !v.is_empty() { q.push(format!("doc_number={v}")); }
    if let Some(ref v) = params.work_order
        && !v.is_empty() { q.push(format!("work_order={v}")); }
    if let Some(p) = params.page {
        q.push(format!("page={p}"));
    }
    q.join("&")
}
