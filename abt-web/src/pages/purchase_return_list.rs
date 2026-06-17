use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReturnStatus;
use abt_core::purchase::return_order::model::*;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_return::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PRQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_range: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let today = chrono::Local::now().date_naive();
    match range {
        "7d" => (Some(today - chrono::Days::new(7)), None),
        "30d" => (Some(today - chrono::Days::new(30)), None),
        "3m" => (Some(today - chrono::Months::new(3)), None),
        _ => (None, None),
    }
}

fn build_filter(params: &PRQueryParams) -> PurchaseReturnQuery {
    let (return_date_start, return_date_end) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    PurchaseReturnQuery {
        order_id: None,
        supplier_id: params.supplier_id,
        status: params.status.and_then(PurchaseReturnStatus::from_i16),
        return_date_start,
        return_date_end,
    }
}

fn build_query_string(params: &PRQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(sid) = params.supplier_id {
        q.push(format!("supplier_id={sid}"));
    }
    if let Some(ref dr) = params.date_range {
        q.push(format!("date_range={dr}"));
    }
    q.join("&")
}

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    returns: &[PurchaseReturn],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = returns.iter().map(|r| r.supplier_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let all = svc
        .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 200))
        .await;
    match all {
        Ok(result) => result
            .items
            .into_iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| (s.id, s.name))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

// ── Status Labels ──

fn status_label(s: PurchaseReturnStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseReturnStatus::Draft => ("草稿", "status-draft"),
        PurchaseReturnStatus::Confirmed => ("已确认", "status-info"),
        PurchaseReturnStatus::Shipped => ("已发货", "status-shipped"),
        PurchaseReturnStatus::Settled => ("已结算", "status-completed"),
        PurchaseReturnStatus::Cancelled => ("已取消", "status-rejected"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_RETURN", "read")]
pub async fn get_pr_list(
    _path: PRListPath,
    ctx: RequestContext,
    Query(params): Query<PRQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("PURCHASE_RETURN", "create").await;
    let can_delete = ctx.has_permission("PURCHASE_RETURN", "delete").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();
    let supplier_svc = state.supplier_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    let content = pr_list_page(&result, &supplier_names, &suppliers.items, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "采购退货", &claims, "purchase", PRListPath::PATH, "采购管理", Some("采购退货"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn pr_list_page(
    result: &abt_core::shared::types::PaginatedResult<PurchaseReturn>,
    supplier_names: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PRQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "采购退货" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(PRCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建采购退货"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (pr_table_fragment(result, supplier_names, suppliers, params, can_delete))
        }
    }
}

fn pr_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<PurchaseReturn>,
    supplier_names: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PRQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "已发货", count: None },
        TabItem { value: "4".into(), label: "已结算", count: None },
        TabItem { value: "5".into(), label: "已取消", count: None },
    ];

    let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");

    html! {
        div class="pr-list-panel" {
            (status_tabs_with_param(PRListPath::PATH, "#pr-data-card", "#pr-filter-form", tabs, &active_value, "status"))

            // ── Filter Bar ──
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="pr-filter-form"
                hx-get=(PRListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#pr-data-card"
                hx-select="#pr-data-card"
                hx-swap="outerHTML"
                hx-select-oob="#status-tabs"
                hx-include="#pr-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索退货单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="supplier_id" {
                    option value="" { "全部供应商" }
                    @for s in suppliers {
                        option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
                    }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="date_range" {
                    option value="" selected[selected_range.is_empty()] { "退货日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
            }

            // ── Data Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="pr-data-card" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "单据编号" }
                                th { "供应商名称" }
                                th { "关联订单" }
                                th { "状态" }
                                th { "退货原因" }
                                th class="text-right text-[13px]" { "退货金额" }
                                th { "退货日期" }
                                th { "创建时间" }
                                th class="text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (pr_row(r, supplier_names, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无退货数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(PRListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn pr_row(
    r: &PurchaseReturn,
    supplier_names: &HashMap<i64, String>,
    can_delete: bool,
) -> Markup {
    let detail_path = PRDetailPath { id: r.id };
    let delete_path = format!("{}/delete", detail_path);
    let (status_text, status_class) = status_label(r.status);
    let supplier_name = supplier_names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—");
    let created = r.created_at.format("%Y-%m-%d").to_string();
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = r.status == PurchaseReturnStatus::Draft;

    html! {
        tr style="cursor:pointer" {
            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) { (r.doc_number) }
            td onclick=(&onclick) { (supplier_name) }
            td class="font-mono tabular-nums" onclick=(&onclick) { (r.order_id) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td onclick=(&onclick) { (r.return_reason) }
            td class="text-right text-[13px]" onclick=(&onclick) { (r.total_amount) }
            td class="font-mono tabular-nums" onclick=(&onclick) { (r.return_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) { (created) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 group-hover:opacity-100 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                        a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" href=(detail_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
                                hx-confirm="确认删除该退货单吗？"
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
