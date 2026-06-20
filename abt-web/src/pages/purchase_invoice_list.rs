use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::gl::invoice::InvoiceStatus;
use abt_core::gl::purchase_invoice::model::{PurchaseInvoice, PurchaseInvoiceFilter};
use abt_core::gl::purchase_invoice::PurchaseInvoiceService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::types::PaginatedResult;

use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{PurchaseInvoiceCreatePath, PurchaseInvoiceDetailPath, PurchaseInvoiceListPath};
use crate::utils::{empty_as_none, fmt_amount, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct InvoiceQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_list(
    _path: PurchaseInvoiceListPath,
    ctx: RequestContext,
    Query(params): Query<InvoiceQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let svc = state.purchase_invoice_service();
    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(page_num, 20),
        )
        .await?;

    // 供应商名解析（批量查询，消 N+1）
    let supplier_ids: Vec<i64> = result
        .items
        .iter()
        .map(|i| i.supplier_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let suppliers = state
        .supplier_service()
        .get_by_ids(&service_ctx, &mut conn, &supplier_ids)
        .await
        .unwrap_or_default();
    use std::collections::HashMap;
    let supplier_map: HashMap<i64, String> = suppliers
        .iter()
        .map(|s| (s.id, s.name.clone()))
        .collect();

    let content = invoice_list_page(&result, &params, &supplier_map);
    let page_html = admin_page(
        is_htmx,
        "采购发票",
        &claims,
        "gl",
        PurchaseInvoiceListPath::PATH,
        "总账管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

fn build_filter(params: &InvoiceQueryParams) -> PurchaseInvoiceFilter {
    PurchaseInvoiceFilter {
        supplier_id: params.supplier_id,
        status: params.status.and_then(InvoiceStatus::from_i16),
        period: params.period.clone(),
    }
}

fn build_query_string(params: &InvoiceQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(v) = params.supplier_id {
        parts.push(format!("supplier_id={v}"));
    }
    if let Some(v) = params.status {
        parts.push(format!("status={v}"));
    }
    if let Some(ref v) = params.period {
        parts.push(format!("period={v}"));
    }
    if let Some(v) = params.page
        && v > 1
    {
        parts.push(format!("page={v}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

// ── Components ──

/// 发票状态标签：Draft/Posted/Cancelled（动态色，跟随 fms/gl 模式）
fn status_label(s: &InvoiceStatus) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    match s {
        InvoiceStatus::Draft => ("Draft", "rgba(0,0,0,0.04)", "var(--muted)"),
        InvoiceStatus::Posted => ("Posted", "rgba(22,163,74,0.08)", "#16a34a"),
        InvoiceStatus::Cancelled => ("Cancelled", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

fn invoice_list_page(
    result: &PaginatedResult<PurchaseInvoice>,
    params: &InvoiceQueryParams,
    supplier_map: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "采购发票" }
                a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on text-sm font-medium cursor-pointer hover:bg-accent-hover transition-all duration-150 shadow-xs"
                    href=(PurchaseInvoiceCreatePath::PATH) {
                    (crate::components::icon::plus_icon("w-4 h-4"))
                    "新建发票"
                }
            }
            (invoice_table_fragment(result, params, supplier_map))
        }
    }
}

fn invoice_table_fragment(
    result: &PaginatedResult<PurchaseInvoice>,
    params: &InvoiceQueryParams,
    supplier_map: &std::collections::HashMap<i64, String>,
) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.map(|v| v.to_string()).unwrap_or_default();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "Draft", count: None },
        TabItem { value: "2".into(), label: "Posted", count: None },
        TabItem { value: "3".into(), label: "Cancelled", count: None },
    ];

    html! {
        div {
            (status_tabs_with_param(PurchaseInvoiceListPath::PATH, "#purchase-invoice-data-card", "#purchase-invoice-filter-form", tabs, &selected_status, "status"))

            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="purchase-invoice-filter-form"
                hx-get=(PurchaseInvoiceListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#purchase-invoice-data-card"
                hx-select="#purchase-invoice-data-card"
                hx-swap="outerHTML"
                hx-include="#purchase-invoice-filter-form"
                hx-push-url="true" {
                input class="w-[120px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono tabular-nums" type="text" name="period"
                    placeholder="期间 YYYY-MM"
                    value=(params.period.as_deref().unwrap_or(""));
                input class="w-[140px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" type="number" name="supplier_id"
                    placeholder="供应商 ID"
                    value=(params.supplier_id.map(|v| v.to_string()).unwrap_or_default());
            }

            (invoice_data_card(result, params, supplier_map))
        }
    }
}

fn invoice_data_card(
    result: &PaginatedResult<PurchaseInvoice>,
    params: &InvoiceQueryParams,
    supplier_map: &std::collections::HashMap<i64, String>,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="data-card" id="purchase-invoice-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "发票号" }
                            th { "期间" }
                            th { "开票日期" }
                            th { "供应商" }
                            th class="text-right" { "金额合计" }
                            th { "状态" }
                            th class="w-[80px]" { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (status_text, status_bg, status_color) = status_label(&item.status);
                            @let detail_path = PurchaseInvoiceDetailPath { id: item.id };
                            @let supplier_name = supplier_map.get(&item.supplier_id).cloned().unwrap_or_else(|| format!("#{}", item.supplier_id));
                            tr class="cursor-pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="font-mono tabular-nums text-accent" { (&item.doc_number) }
                                td class="font-mono tabular-nums" { (&item.period) }
                                td class="text-xs text-muted" { (item.issue_date.format("%Y-%m-%d")) }
                                td { (supplier_name) }
                                td class="font-mono tabular-nums text-right text-fg" { (fmt_amount(item.total)) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                                        (status_text)
                                    }
                                }
                                td {
                                    a href=(detail_path.to_string()) class="text-accent text-xs" _="on click halt the event" { "查看" }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="7" class="text-center text-muted py-8" { "暂无采购发票记录" }
                            }
                        }
                    }
                }
            }
            (pagination(PurchaseInvoiceListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
