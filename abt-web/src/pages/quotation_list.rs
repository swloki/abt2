use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::quotation::model::*;
use abt_core::sales::quotation::QuotationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::quotation::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QuotationQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub customer_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_range: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let today = chrono::Local::now().date_naive();
    match range {
        "7d" => (Some(today - chrono::Days::new(7)), None),
        "30d" => (Some(today - chrono::Days::new(30)), None),
        "3m" => (Some(today - chrono::Months::new(3)), None),
        _ => (None, None),
    }
}

fn build_filter(params: &QuotationQueryParams) -> QuotationQuery {
    let (date_from, date_to) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    QuotationQuery {
        keyword: params.keyword.clone(),
        status: params.status.and_then(QuotationStatus::from_i16),
        customer_id: params.customer_id,
        date_from,
        date_to,
    }
}

fn build_query_string(params: &QuotationQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.customer_id {
        q.push(format!("customer_id={c}"));
    }
    if let Some(ref dr) = params.date_range {
        q.push(format!("date_range={dr}"));
    }
    q.join("&")
}

// ── Status Labels ──

fn status_label(s: QuotationStatus) -> (&'static str, &'static str) {
    match s {
        QuotationStatus::Draft => ("草稿", "status-draft"),
        QuotationStatus::Sent => ("已发送", "status-sent"),
        QuotationStatus::Accepted => ("已接受", "status-accepted"),
        QuotationStatus::Rejected => ("已拒绝", "status-rejected"),
        QuotationStatus::Expired => ("已过期", "status-expired"),
    }
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotation_list(
    _path: QuotationListPath,
    ctx: RequestContext,
    Query(params): Query<QuotationQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("SALES_ORDER", "create").await;
    let can_delete = ctx.has_permission("SALES_ORDER", "delete").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let mut names = HashMap::new();
    let mut seen_customers = HashSet::new();
    for q in &result.items {
        if seen_customers.insert(q.customer_id)
            && let Ok(c) = customer_svc.get(&service_ctx, &mut conn, q.customer_id).await {
                names.insert(q.customer_id, c.name);
            }
    }

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = quotation_list_page(&claims, &result, &names, &customers.items, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "报价单", &claims, "sales", QuotationListPath::PATH, "销售管理", Some("报价单"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "delete")]
pub async fn delete_quotation(
    path: DeleteQuotationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", QuotationListPath::PATH)], Html(String::new())))
}

// ── Components ──

fn quotation_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Quotation>,
    names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &QuotationQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "报价单" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(QuotationCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建报价单"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (quotation_table_fragment(result, names, customers, params, can_delete))
        }
    }
}

fn quotation_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Quotation>,
    names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &QuotationQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已发送", count: None },
        TabItem { value: "3".into(), label: "已接受", count: None },
        TabItem { value: "4".into(), label: "已拒绝", count: None },
        TabItem { value: "5".into(), label: "已过期", count: None },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");

    html! {
        div class="quotation-list-panel" {
            (status_tabs_with_param(QuotationListPath::PATH, "#quotation-data-card", "#quotation-filter-form", tabs, &active_value, "status"))

            // ── Filter Bar ──
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="quotation-filter-form"
                hx-get=(QuotationListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#quotation-data-card"
                hx-select="#quotation-data-card"
                hx-swap="outerHTML"
                hx-select-oob="#status-tabs"
                hx-include="#quotation-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索报价单号、客户名称…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="customer_id" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="date_range" {
                    option value="" selected[selected_range.is_empty()] { "报价日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
            }

            // ── Data Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="quotation-data-card" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "报价单号" }
                                th { "客户名称" }
                                th { "状态" }
                                th class="text-right text-[13px]" { "总金额" }
                                th { "报价日期" }
                                th { "有效期至" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for q in &result.items {
                                (quotation_row(q, names, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" class="text-center p-8 text-text-muted" {
                                        "暂无报价单数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(QuotationListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn quotation_row(q: &Quotation, names: &HashMap<i64, String>, can_delete: bool) -> Markup {
    let detail_path = QuotationDetailPath { id: q.id };
    let (status_text, status_class) = status_label(q.status);
    let edit_form_path = EditQuotationFormPath { id: q.id };
    let delete_path = DeleteQuotationPath { id: q.id };
    let is_draft = q.status == QuotationStatus::Draft;
    let customer_name = names.get(&q.customer_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr {
            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(format!("location.href='{}'", detail_path)) { (q.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { (customer_name) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="text-right text-[13px]" onclick=(format!("location.href='{}'", detail_path)) {
                span class="font-mono tabular-nums" { (crate::utils::fmt_amount(q.total_amount)) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { (q.quotation_date.format("%Y-%m-%d")) }
            td onclick=(format!("location.href='{}'", detail_path)) { (q.valid_until.format("%Y-%m-%d")) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" {
                        a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑" href=(edit_form_path) {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
                                hx-confirm="确认删除该报价单吗？"
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
}

