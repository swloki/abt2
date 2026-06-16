use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{RMAStatus, Severity};
use abt_core::qms::rma::RmaService;
use abt_core::qms::rma::model::RmaFilter;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{RmaCreatePath, RmaDetailPath, RmaListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RmaQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub severity: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_to: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn severity_label(s: &Severity) -> (&'static str, &'static str, &'static str) {
    match s {
        Severity::Minor => ("Minor", "rgba(22,163,74,0.08)", "var(--success)"),
        Severity::Major => ("Major", "rgba(217,119,6,0.08)", "#b45309"),
        Severity::Critical => ("Critical", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

fn rma_status_label(s: &RMAStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        RMAStatus::Reported => ("已报告", "rgba(22,119,255,0.08)", "var(--accent)"),
        RMAStatus::Investigating => ("调查中", "rgba(217,119,6,0.08)", "#b45309"),
        RMAStatus::ActionTaken => ("已采取措施", "rgba(82,196,26,0.08)", "var(--success)"),
        RMAStatus::Closed => ("已关闭", "rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

fn parse_status(s: &str) -> Option<RMAStatus> {
    match s {
        "Reported" => Some(RMAStatus::Reported),
        "Investigating" => Some(RMAStatus::Investigating),
        "ActionTaken" => Some(RMAStatus::ActionTaken),
        "Closed" => Some(RMAStatus::Closed),
        _ => None,
    }
}

fn parse_severity(s: &str) -> Option<Severity> {
    match s {
        "Minor" => Some(Severity::Minor),
        "Major" => Some(Severity::Major),
        "Critical" => Some(Severity::Critical),
        _ => None,
    }
}

fn build_query_string(params: &RmaQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword {
        q.push(format!("keyword={v}"));
    }
    if let Some(ref v) = params.severity {
        q.push(format!("severity={v}"));
    }
    if let Some(ref v) = params.status {
        q.push(format!("status={v}"));
    }
    if let Some(ref v) = params.date_from {
        q.push(format!("date_from={v}"));
    }
    if let Some(ref v) = params.date_to {
        q.push(format!("date_to={v}"));
    }
    q.join("&")
}

fn build_filter(params: &RmaQueryParams) -> RmaFilter {
    RmaFilter {
        customer_id: None,
        product_id: None,
        severity: params.severity.as_deref().and_then(parse_severity),
        status: params.status.as_deref().and_then(parse_status),
        date_from: params.date_from.as_deref().and_then(|d| d.parse().ok()),
        date_to: params.date_to.as_deref().and_then(|d| d.parse().ok()),
    }
}

async fn resolve_names(
    customer_svc: &impl CustomerService,
    product_svc: &impl ProductService,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[abt_core::qms::rma::model::Rma],
) -> (HashMap<i64, String>, HashMap<i64, String>) {
    let mut customer_names = HashMap::new();
    let mut product_names = HashMap::new();
    for item in items {
        if !customer_names.contains_key(&item.customer_id)
            && let Ok(c) = customer_svc.get(ctx, db, item.customer_id).await {
                customer_names.insert(item.customer_id, c.name);
            }
        if !product_names.contains_key(&item.product_id)
            && let Ok(p) = product_svc.get(ctx, db, item.product_id).await {
                product_names.insert(item.product_id, p.pdt_name);
            }
    }
    (customer_names, product_names)
}

// ── Handlers ──

#[require_permission("QMS", "read")]
pub async fn get_list(
    _path: RmaListPath,
    ctx: RequestContext,
    Query(params): Query<RmaQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("INSPECTION", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.rma_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc.list(&service_ctx, &mut conn, filter, PageParams::new(page_num, 20)).await?;
    let (customer_names, product_names) =
        resolve_names(&customer_svc, &product_svc, &service_ctx, &mut conn, &result.items).await;

    let content = rma_list_page(&result, &customer_names, &product_names, &params, can_create);
    let page_html = admin_page(
        is_htmx, "RMA 客诉追溯", &claims, "quality", RmaListPath::PATH, "质量管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn rma_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::qms::rma::model::Rma>,
    customer_names: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    params: &RmaQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "RMA 客诉追溯" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(RmaCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建RMA"
                        }
                    }
                }
            }
            (rma_table_fragment(result, customer_names, product_names, params))
        }
    }
}

fn rma_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::qms::rma::model::Rma>,
    customer_names: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    params: &RmaQueryParams,
) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.as_deref().unwrap_or("");

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "Reported".into(), label: "已报告", count: None },
        TabItem { value: "Investigating".into(), label: "调查中", count: None },
        TabItem { value: "ActionTaken".into(), label: "已采取措施", count: None },
        TabItem { value: "Closed".into(), label: "已关闭", count: None },
    ];

    html! {
        div class="plan-list-panel" {
            (status_tabs_with_param(RmaListPath::PATH, "#rma-data-card", "#filter-form", tabs, selected_status, "status"))

            // ── Filter Bar ──
            form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(RmaListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#rma-data-card"
                hx-select="#rma-data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true" {
                input type="hidden" name="status" value=(selected_status);
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:180px"
                        placeholder="搜索单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="severity" {
                    option value="" selected[params.severity.is_none()] { "全部严重程度" }
                    option value="Minor" selected[params.severity.as_deref() == Some("Minor")] { "Minor" }
                    option value="Major" selected[params.severity.as_deref() == Some("Major")] { "Major" }
                    option value="Critical" selected[params.severity.as_deref() == Some("Critical")] { "Critical" }
                }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_from"
                    style="max-width:160px"
                    value=(params.date_from.as_deref().unwrap_or(""));
                span style="color:var(--muted);font-size:13px" { "至" }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_to"
                    style="max-width:160px"
                    value=(params.date_to.as_deref().unwrap_or(""));
            }

            // ── Data Table ──
            (rma_data_card(result, customer_names, product_names, params))
        }
    }
}

fn rma_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::qms::rma::model::Rma>,
    customer_names: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    params: &RmaQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="rma-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "单号" }
                            th { "客户" }
                            th { "产品" }
                            th { "缺陷描述" }
                            th { "严重程度" }
                            th { "状态" }
                            th { "创建时间" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (sev_label, sev_bg, sev_color) = severity_label(&item.severity);
                            @let (st_label, st_bg, st_color) = rma_status_label(&item.status);
                            @let customer_name = customer_names.get(&item.customer_id).map(|s| s.as_str()).unwrap_or("—");
                            @let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                            @let detail_path = RmaDetailPath { id: item.id };
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number) }
                                td { (customer_name) }
                                td { (product_name) }
                                td {
                                    span style="display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden" {
                                        (item.defect_description)
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sev_bg, sev_color)) {
                                        (sev_label)
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", st_bg, st_color)) {
                                        (st_label)
                                    }
                                }
                                td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无RMA记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(RmaListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
