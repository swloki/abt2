use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::ShiftType;
use abt_core::mes::work_report::{ReportListItem, ReportListFilter, WorkReportService};
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::{ReportCreatePath, ReportListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReportQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_to: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

fn shift_label(s: &ShiftType) -> &'static str {
    match s {
        ShiftType::Day => "白班",
        ShiftType::Night => "夜班",
    }
}

fn build_filter(params: &ReportQueryParams) -> ReportListFilter {
    ReportListFilter {
        keyword: params.keyword.clone(),
        work_order_id: None,
        shift: None,
        date_from: params.date_from.as_ref().and_then(|d| d.parse().ok()),
        date_to: params.date_to.as_ref().and_then(|d| d.parse().ok()),
    }
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_report_list(
    _path: ReportListPath, ctx: RequestContext, Query(params): Query<ReportQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("WORK_ORDER", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.work_report_service();
    let filter = build_filter(&params);
    let page = params.page.unwrap_or(1).max(1);
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    let content = report_list_page(&result, &params, can_create);
    Ok(Html(admin_page(is_htmx, "报工记录", &claims, "production", ReportListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

fn report_list_page(
    result: &PaginatedResult<ReportListItem>,
    params: &ReportQueryParams,
    can_create: bool,
) -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "报工记录" } div class="page-actions" {
            @if can_create {
                a class="btn btn-primary" href=(ReportCreatePath::PATH) { (icon::plus_icon("w-4 h-4")) "新建报工" }
            }
        }}
        (report_table_fragment(result, params))
    }}
}

fn report_table_fragment(
    result: &PaginatedResult<ReportListItem>,
    params: &ReportQueryParams,
) -> Markup {
    html! { div {
        form id="filter-form" class="filter-bar filter-form" hx-get=(ReportListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#report-data-card" hx-select="#report-data-card" hx-swap="outerHTML" hx-include="#filter-form"
                hx-push-url="true" {
            div class="search-wrap" { (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="keyword" style="width:180px" placeholder="搜索单号…" value=(params.keyword.as_deref().unwrap_or(""));
            }
            input type="date" name="date_from" style="width:140px" value=(params.date_from.as_deref().unwrap_or(""));
            span style="color:var(--muted);font-size:var(--text-sm)" { "至" }
            input type="date" name="date_to" style="width:140px" value=(params.date_to.as_deref().unwrap_or(""));
        }
        (report_data_card(result, params))
    }}
}

fn report_data_card(
    result: &PaginatedResult<ReportListItem>,
    params: &ReportQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(k) = &params.keyword { qs.push(format!("keyword={k}")); }
    if let Some(d) = &params.date_from { qs.push(format!("date_from={d}")); }
    if let Some(d) = &params.date_to { qs.push(format!("date_to={d}")); }
    let query = qs.join("&");

    html! {
        div class="data-card" id="report-data-card" {
            div class="data-card-scroll" {
                table class="data-table" { thead { tr {
                    th { "单号" } th { "产品" } th { "工序" } th { "日期" } th { "工人" }
                    th class="num-right" { "完成" } th class="num-right" { "不良" } th { "班次" } th { "操作" }
                }} tbody {
                    @for item in &result.items {
                        @let dp = format!("/admin/mes/reports/{}", item.id);
                        @let wn = item.worker_name.as_deref().unwrap_or("\u{2014}");
                        @let sl = shift_label(&item.shift);
                        tr style="cursor:pointer" onclick=(format!("location.href='{}'", dp)) {
                            td class="link-cell mono" style="color:var(--accent)" { (item.doc_number) }
                            td { (item.product_name.as_deref().unwrap_or("\u{2014}")) }
                            td { (item.process_name) }
                            td { (item.report_date) }
                            td { (wn) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.completed_qty)) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.defect_qty)) }
                            td { (sl) }
                            td { a href=(dp) style="color:var(--accent);font-size:var(--text-xs)" { "查看" } }
                        }
                    }
                    @if result.items.is_empty() {
                        tr { td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无报工记录" } }
                    }
                }}
            }
            (pagination(ReportListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
