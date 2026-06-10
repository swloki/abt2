use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::{PaymentMethod, PaymentStatus};
use abt_core::purchase::payment::model::*;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::payment_request::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PayQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_range: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub payment_method: Option<i16>,
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

fn build_filter(params: &PayQueryParams) -> PaymentRequestQuery {
    let (payment_date_start, payment_date_end) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    PaymentRequestQuery {
        supplier_id: params.supplier_id,
        status: params.status.and_then(PaymentStatus::from_i16),
        payment_date_start,
        payment_date_end,
        keyword: params.keyword.clone(),
        payment_method: params.payment_method.and_then(PaymentMethod::from_i16),
    }
}

fn build_query_string(params: &PayQueryParams) -> String {
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
    if let Some(pm) = params.payment_method {
        q.push(format!("payment_method={pm}"));
    }
    q.join("&")
}

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[PaymentRequest],
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

async fn resolve_recon_doc_numbers<S: PurchaseReconciliationService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[PaymentRequest],
) -> HashMap<i64, String> {
    let recon_ids: Vec<i64> = items.iter().filter_map(|r| r.reconciliation_id).collect();
    if recon_ids.is_empty() {
        return HashMap::new();
    }
    let mut map = HashMap::new();
    for rid in recon_ids {
        if let Ok(recon) = svc.get(ctx, db, rid).await {
            map.insert(rid, recon.doc_number);
        }
    }
    map
}

// ── Status Labels ──

fn status_label(s: PaymentStatus) -> (&'static str, &'static str) {
    match s {
        PaymentStatus::Draft => ("草稿", "status-draft"),
        PaymentStatus::Approved => ("已审批", "status-approved"),
        PaymentStatus::Paid => ("已支付", "status-paid"),
        PaymentStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

fn payment_method_label(m: PaymentMethod) -> &'static str {
    match m {
        PaymentMethod::BankTransfer => "银行转账",
        PaymentMethod::Cash => "现金",
        PaymentMethod::Note => "票据",
    }
}

// ── Handlers ──

#[require_permission("PAYMENT_REQUEST", "read")]
pub async fn get_pay_list(
    _path: PayListPath,
    ctx: RequestContext,
    Query(params): Query<PayQueryParams>,
) -> Result<Html<String>> {
    let can_create = ctx.has_permission("PURCHASE_ORDER", "create").await;
    let can_delete = ctx.has_permission("PURCHASE_ORDER", "delete").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.payment_request_service();
    let supplier_svc = state.supplier_service();
    let recon_svc = state.purchase_reconciliation_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
    let recon_doc_numbers = resolve_recon_doc_numbers(&recon_svc, &service_ctx, &mut conn, &result.items).await;

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    let content = pay_list_page(&result, &supplier_names, &recon_doc_numbers, &suppliers.items, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "付款申请", &claims, "purchase", PayListPath::PATH, "采购管理", Some("付款申请"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PAYMENT_REQUEST", "read")]
pub async fn get_pay_table(
    ctx: RequestContext,
    Query(params): Query<PayQueryParams>,
) -> Result<Html<String>> {
    let can_delete = ctx.has_permission("PURCHASE_ORDER", "delete").await;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.payment_request_service();
    let supplier_svc = state.supplier_service();
    let recon_svc = state.purchase_reconciliation_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
    let recon_doc_numbers = resolve_recon_doc_numbers(&recon_svc, &service_ctx, &mut conn, &result.items).await;

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(pay_table_fragment(&result, &supplier_names, &recon_doc_numbers, &suppliers.items, &params, can_delete).into_string()))
}

// ── Components ──

fn pay_list_page(
    result: &abt_core::shared::types::PaginatedResult<PaymentRequest>,
    supplier_names: &HashMap<i64, String>,
    recon_doc_numbers: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PayQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "付款申请" }
                div class="page-actions" {
                    @if can_create {
                        a class="btn btn-primary" href=(PayCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建付款申请"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (pay_table_fragment(result, supplier_names, recon_doc_numbers, suppliers, params, can_delete))
        }
    }
}

fn pay_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<PaymentRequest>,
    supplier_names: &HashMap<i64, String>,
    recon_doc_numbers: &HashMap<i64, String>,
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    params: &PayQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已审批", count: None },
        TabItem { value: "3".into(), label: "已支付", count: None },
        TabItem { value: "4".into(), label: "已取消", count: None },
    ];

    let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");
    let selected_method = params.payment_method.map(|m| m.to_string()).unwrap_or_default();

    html! {
        div class="pay-list-panel" {
            (status_tabs(PayTablePath::PATH, "closest .pay-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索申请单号、供应商名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(PayTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .pay-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="supplier_id"
                    hx-get=(PayTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .pay-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" { "全部供应商" }
                    @for s in suppliers {
                        option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
                    }
                }
                select class="filter-select" name="date_range"
                    hx-get=(PayTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .pay-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" selected[selected_range.is_empty()] { "付款日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
                select class="filter-select" name="payment_method"
                    hx-get=(PayTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .pay-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" selected[selected_method.is_empty()] { "全部付款方式" }
                    option value="1" selected[selected_method == "1"] { "银行转账" }
                    option value="2" selected[selected_method == "2"] { "现金" }
                    option value="3" selected[selected_method == "3"] { "票据" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "单据编号" }
                                th { "供应商名称" }
                                th { "关联对账单" }
                                th { "状态" }
                                th class="num-right" { "金额" }
                                th { "付款日期" }
                                th { "付款方式" }
                                th { "发票号" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for r in &result.items {
                                (pay_row(r, supplier_names, recon_doc_numbers, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无付款申请数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(PayListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn pay_row(
    r: &PaymentRequest,
    supplier_names: &HashMap<i64, String>,
    recon_doc_numbers: &HashMap<i64, String>,
    can_delete: bool,
) -> Markup {
    let detail_path = PayDetailPath { id: r.id };
    let cancel_path = PayCancelPath { id: r.id };
    let (status_text, status_class) = status_label(r.status);
    let supplier_name = supplier_names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—");
    let recon_doc_number = r.reconciliation_id
        .and_then(|rid| recon_doc_numbers.get(&rid).map(|s| s.as_str()))
        .unwrap_or("—");
    let created = r.created_at.format("%Y-%m-%d").to_string();
    let onclick = format!("location.href='{}'", detail_path);
    let is_draft = r.status == PaymentStatus::Draft;
    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(&onclick) { (r.doc_number) }
            td onclick=(&onclick) { (supplier_name) }
            td class="mono" onclick=(&onclick) { (recon_doc_number) }
            td onclick=(&onclick) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right" onclick=(&onclick) { (r.amount) }
            td class="mono" onclick=(&onclick) { (r.payment_date.format("%Y-%m-%d")) }
            td onclick=(&onclick) { (payment_method_label(r.payment_method)) }
            td class="mono" onclick=(&onclick) { (r.invoice_number.as_deref().unwrap_or("—")) }
            td onclick=(&onclick) { (created) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" {
                        a class="row-action-btn" href=(detail_path.to_string()) title="编辑" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        @if can_delete {
                            button type="button" class="row-action-btn text-danger" title="取消"
                                hx-confirm="确认取消该付款申请吗？"
                                hx-post=(cancel_path)
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
