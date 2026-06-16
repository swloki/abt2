use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReconStatus;
use abt_core::purchase::reconciliation::model::*;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_reconciliation::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PreconQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn build_filter(params: &PreconQueryParams) -> PurchaseReconciliationQuery {
    PurchaseReconciliationQuery {
        supplier_id: params.supplier_id,
        period: params.period.clone(),
        status: params.status.and_then(PurchaseReconStatus::from_i16),
    }
}

fn build_query_string(params: &PreconQueryParams) -> String {
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
    if let Some(ref p) = params.period {
        q.push(format!("period={p}"));
    }
    q.join("&")
}

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[PurchaseReconciliation],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|r| r.supplier_id).collect();
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

async fn resolve_item_counts<S: PurchaseReconciliationService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[PurchaseReconciliation],
) -> HashMap<i64, usize> {
    let mut counts = HashMap::new();
    for r in items {
        if let Ok(items) = svc.list_items(ctx, db, r.id).await {
            counts.insert(r.id, items.len());
        }
    }
    counts
}

fn generate_periods(count: usize) -> Vec<String> {
    let now = chrono::Local::now();
    let mut periods = Vec::with_capacity(count);
    for i in 0..count {
        if let Some(d) = now.checked_sub_months(chrono::Months::new(i as u32)) {
            periods.push(d.format("%Y-%m").to_string());
        }
    }
    periods
}

// ── Status Labels ──

fn status_label(s: PurchaseReconStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseReconStatus::Draft => ("草稿", "status-draft"),
        PurchaseReconStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseReconStatus::Settled => ("已结算", "status-settled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "read")]
pub async fn get_precon_list(
    _path: PreconListPath,
    ctx: RequestContext,
    Query(params): Query<PreconQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("PURCHASE_RECON", "create").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_reconciliation_service();
    let supplier_svc = state.supplier_service();
    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;
    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
    let item_counts = resolve_item_counts(&svc, &service_ctx, &mut conn, &result.items).await;
    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;
    let content = precon_list_page(&result, &supplier_names, &item_counts, &suppliers.items, &params, can_create);
    let page_html = admin_page(
        is_htmx, "采购对账", &claims, "purchase", PreconListPath::PATH, "采购管理", Some("采购对账"), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn precon_list_page(
    result: &abt_core::shared::types::PaginatedResult<PurchaseReconciliation>,
    supplier_names: &HashMap<i64, String>,
    item_counts: &HashMap<i64, usize>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PreconQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "采购对账" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="btn btn-primary" href=(PreconCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建对账单"
                        }
                    }
                }
            }
            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (precon_table_fragment(result, supplier_names, item_counts, suppliers, params))
        }
    }
}

fn precon_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<PurchaseReconciliation>,
    supplier_names: &HashMap<i64, String>,
    item_counts: &HashMap<i64, usize>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PreconQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "已结算", count: None },
    ];
    let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_period = params.period.as_deref().unwrap_or("");
    let periods = generate_periods(12);
    html! {
        div class="precon-list-panel" {
            (status_tabs_with_param(PreconListPath::PATH, "#precon-data-card", "#precon-filter-form", tabs, &active_value, "status"))
            // ── Filter Bar ──
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="precon-filter-form"
                hx-get=(PreconListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#precon-data-card"
                hx-select="#precon-data-card"
                hx-swap="outerHTML"
                hx-select-oob="#status-tabs"
                hx-include="#precon-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索对账单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="supplier_id" {
                    option value="" { "全部供应商" }
                    @for s in suppliers {
                        option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
                    }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="period" {
                    option value="" { "全部期间" }
                    @for p in &periods {
                        option value=(p) selected[*selected_period == *p] { (p) }
                    }
                }
            }
            // ── Data Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="precon-data-card" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "对账单号" }
                                th { "供应商名称" }
                                th { "对账期间" }
                                th { "状态" }
                                th class="num-right" { "订单笔数" }
                                th class="num-right" { "应付金额" }
                                th class="num-right" { "退货冲减" }
                                th class="num-right" { "实付金额" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (precon_row(r, supplier_names, item_counts))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无对账数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(PreconListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn precon_row(
    r: &PurchaseReconciliation,
    supplier_names: &HashMap<i64, String>,
    item_counts: &HashMap<i64, usize>,
) -> Markup {
    let detail_path = PreconDetailPath { id: r.id };
    let (status_text, status_class) = status_label(r.status);
    let supplier_name = supplier_names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—");
    let count = item_counts.get(&r.id).unwrap_or(&0);
    let return_amount = r.total_amount - r.confirmed_amount;
    let onclick = format!("location.href='{}'", detail_path);
    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (r.doc_number) }
            td onclick=(&onclick) { (supplier_name) }
            td class="mono" onclick=(&onclick) { (&r.period) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right" onclick=(&onclick) { (count) }
            td class="num-right" onclick=(&onclick) { (format_amount(r.total_amount)) }
            td class="num-right" onclick=(&onclick) { (format_amount(return_amount)) }
            td class="num-right" onclick=(&onclick) { (format_amount(r.confirmed_amount)) }
            td onclick="event.stopPropagation()" {
                a class="row-action-btn" href=(detail_path.to_string()) title="查看详情" {
                    (icon::edit_icon("w-4 h-4"))
                }
            }
        }
    }
}

fn format_amount(v: rust_decimal::Decimal) -> String {
    use rust_decimal::prelude::ToPrimitive;
    match v.to_f64() {
        Some(f) => format!("{f:.2}"),
        None => v.to_string(),
    }
}
