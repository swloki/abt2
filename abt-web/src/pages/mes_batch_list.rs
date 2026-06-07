use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::BatchStatus;
use abt_core::mes::production_batch::{BatchListFilter, BatchListItem, ProductionBatchService};
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::{BatchListPath, BatchTablePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BatchQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

fn batch_status_label(s: &BatchStatus) -> (&'static str, &'static str, &'static str) {
    use BatchStatus::*;
    match s {
        Pending => ("待生产", "rgba(0,0,0,0.04)", "var(--muted)"),
        InProgress => ("进行中", "rgba(250,140,22,0.08)", "#fa8c16"),
        Suspended => ("已暂停", "rgba(245,63,63,0.06)", "#f53f3f"),
        PendingReceipt => ("待入库", "rgba(22,119,255,0.08)", "var(--accent)"),
        Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        Cancelled => ("已取消", "rgba(114,46,209,0.06)", "#722ed1"),
    }
}

fn parse_batch_status(s: &str) -> Option<BatchStatus> {
    use BatchStatus::*;
    match s {
        "Pending" => Some(Pending),
        "InProgress" => Some(InProgress),
        "Suspended" => Some(Suspended),
        "PendingReceipt" => Some(PendingReceipt),
        "Completed" => Some(Completed),
        "Cancelled" => Some(Cancelled),
        _ => None,
    }
}

fn fmt_current_step(current: i32, step_name: Option<&str>, total: Option<i32>) -> Markup {
    if current == 0 {
        return html! { span style="color:var(--muted)" { "未开始" } };
    }
    let total_str = total.map_or(String::new(), |t| format!("/{t}"));
    match step_name {
        Some(name) => html! { (current)(total_str) " " (name) },
        None => html! { "工序 "(current)(total_str) },
    }
}

#[require_permission("MES", "read")]
pub async fn get_batch_list(
    _path: BatchListPath, ctx: RequestContext, Query(params): Query<BatchQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let filter = BatchListFilter {
        status: params.status.as_deref().and_then(parse_batch_status),
        keyword: params.keyword.clone(),
    };
    let page = params.page.unwrap_or(1);
    let result = state.production_batch_service()
        .list_batches(&service_ctx, &mut conn, filter, page, 20).await?;
    let content = batch_list_page(&result, &params);
    Ok(Html(admin_page(is_htmx, "生产批次", &claims, "production", BatchListPath::PATH, "生产管理", None, content).into_string()))
}

#[require_permission("MES", "read")]
pub async fn get_batch_table(
    _path: BatchTablePath, ctx: RequestContext, Query(params): Query<BatchQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let filter = BatchListFilter {
        status: params.status.as_deref().and_then(parse_batch_status),
        keyword: params.keyword.clone(),
    };
    let page = params.page.unwrap_or(1);
    let result = state.production_batch_service()
        .list_batches(&service_ctx, &mut conn, filter, page, 20).await?;
    Ok(Html(batch_data_card(&result, &params).into_string()))
}

fn batch_list_page(
    result: &PaginatedResult<BatchListItem>,
    params: &BatchQueryParams,
) -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "生产批次" } }
        (batch_table_fragment(result, params))
    }}
}

fn batch_table_fragment(
    result: &PaginatedResult<BatchListItem>,
    params: &BatchQueryParams,
) -> Markup {
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(result.total) },
        TabItem { value: "Pending".into(), label: "待生产", count: None },
        TabItem { value: "InProgress".into(), label: "进行中", count: None },
        TabItem { value: "Suspended".into(), label: "已暂停", count: None },
        TabItem { value: "PendingReceipt".into(), label: "待入库", count: None },
        TabItem { value: "Completed".into(), label: "已完成", count: None },
        TabItem { value: "Cancelled".into(), label: "已取消", count: None },
    ];
    let sel = params.status.as_deref().unwrap_or("");

    html! { div {
        (status_tabs_with_param(BatchTablePath::PATH, "#batch-data-card", "closest form", tabs, sel, "status"))
        form class="filter-bar filter-form" hx-get=(BatchTablePath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#batch-data-card" hx-select="#batch-data-card" hx-swap="outerHTML" hx-include="closest form" {
            div class="search-wrap" { (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="keyword" style="width:180px" placeholder="搜索批次号…" value=(params.keyword.as_deref().unwrap_or(""));
            }
        }
        (batch_data_card(result, params))
    }}
}

fn batch_data_card(
    result: &PaginatedResult<BatchListItem>,
    params: &BatchQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(k) = &params.keyword { qs.push(format!("keyword={k}")); }
    if let Some(s) = &params.status { qs.push(format!("status={s}")); }
    let query = qs.join("&");

    html! {
        div class="data-card" id="batch-data-card" {
            div class="data-card-scroll" {
                table class="data-table" { thead { tr {
                    th { "批次号" } th { "工单编号" } th { "产品" } th class="num-right" { "数量" }
                    th class="num-right" { "已完成" } th { "当前工序" } th { "状态" } th { "操作" }
                }} tbody {
                    @for item in &result.items {
                        @let (sl, sb, sc) = batch_status_label(&item.status);
                        @let dp = format!("/admin/mes/batches/{}", item.id);
                        tr style="cursor:pointer" onclick=(format!("location.href='{}'", dp)) {
                            td class="link-cell mono" style="color:var(--accent)" { (item.batch_no) }
                            td class="mono" { (item.wo_doc_number.as_deref().unwrap_or("\u{2014}")) }
                            td { (item.product_name.as_deref().unwrap_or("\u{2014}")) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.batch_qty)) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.completed_qty)) }
                            td { (fmt_current_step(item.current_step, item.current_step_name.as_deref(), item.total_steps)) }
                            td { span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sb, sc)) { (sl) } }
                            td { a href=(dp) style="color:var(--accent);font-size:var(--text-xs)" { "查看" } }
                        }
                    }
                    @if result.items.is_empty() {
                        tr { td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无批次数据" } }
                    }
                }}
            }
            (pagination(BatchTablePath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
