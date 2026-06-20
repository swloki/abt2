use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::gl::invoice::InvoiceStatus;
use abt_core::gl::sales_invoice::{SalesInvoice, SalesInvoiceItem, SalesInvoiceService};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{
    GlEntryDetailPath, SalesInvoiceDetailPath, SalesInvoiceListPath, SalesInvoicePostPath,
};
use crate::routes::gl::SalesInvoiceCancelPath;
use crate::utils::{fmt_amount, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: &InvoiceStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        InvoiceStatus::Draft => ("Draft", "rgba(0,0,0,0.04)", "var(--muted)"),
        InvoiceStatus::Posted => ("Posted", "rgba(22,163,74,0.08)", "#16a34a"),
        InvoiceStatus::Cancelled => ("Cancelled", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

// ── Handler ──

#[require_permission("GL", "read")]
pub async fn get_detail(path: SalesInvoiceDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let svc = state.sales_invoice_service();
    let (invoice, items) = svc.get(&service_ctx, &mut conn, path.id).await?;

    // 客户名
    let customer_name = state
        .customer_service()
        .get(&service_ctx, &mut conn, invoice.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| format!("#{}", invoice.customer_id));

    // 产品名/编码
    let product_ids: Vec<i64> = items
        .iter()
        .map(|i| i.product_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let products = state
        .product_service()
        .get_by_ids(&service_ctx, &mut conn, product_ids)
        .await
        .unwrap_or_default();
    let product_map: std::collections::HashMap<i64, (String, String)> = products
        .iter()
        .map(|p| (p.product_id, (p.product_code.clone(), p.pdt_name.clone())))
        .collect();

    let content = detail_page(&invoice, &items, &customer_name, &product_map);
    let current_path = SalesInvoiceDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        "销售发票详情",
        &claims,
        "gl",
        &current_path,
        "总账管理",
        Some(SalesInvoiceListPath::PATH),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── post / cancel ──

#[require_permission("GL", "update")]
pub async fn post(path: SalesInvoicePostPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    state
        .sales_invoice_service()
        .post(&service_ctx, &mut conn, path.id)
        .await?;
    let redirect = SalesInvoiceDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("GL", "update")]
pub async fn cancel(path: SalesInvoiceCancelPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    state
        .sales_invoice_service()
        .cancel(&service_ctx, &mut conn, path.id)
        .await?;
    let redirect = SalesInvoiceDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page ──

fn detail_page(
    invoice: &SalesInvoice,
    items: &[SalesInvoiceItem],
    customer_name: &str,
    product_map: &std::collections::HashMap<i64, (String, String)>,
) -> Markup {
    let (status_text, status_bg, status_color) = status_label(&invoice.status);
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", SalesInvoiceListPath::PATH)) {
                    (crate::components::icon::arrow_left_icon("w-4 h-4"))
                    "返回列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" {
                    "销售发票 " (invoice.doc_number) " "
                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                        (status_text)
                    }
                }
            }

            // ── 基本信息 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                h3 class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "基本信息" }
                div class="grid gap-4 grid-cols-2 md:grid-cols-3" {
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "发票号" }
                        span class="font-mono tabular-nums" { (&invoice.doc_number) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "客户" }
                        span { (customer_name) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "期间" }
                        span class="font-mono tabular-nums" { (&invoice.period) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "开票日期" }
                        span { (invoice.issue_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "金额小计" }
                        span class="font-mono tabular-nums font-semibold" { (fmt_amount(invoice.subtotal)) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "税额" }
                        span class="font-mono tabular-nums font-semibold" { (fmt_amount(invoice.tax_amount)) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "价税合计" }
                        span class="font-mono tabular-nums font-bold text-accent" { (fmt_amount(invoice.total)) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "状态" }
                        span { (status_text) }
                    }
                    @if let Some(gl_id) = invoice.gl_entry_id {
                        div class="flex flex-col gap-1" {
                            label class="text-xs text-fg-2" { "GL 凭证" }
                            a class="text-accent font-mono tabular-nums text-sm hover:underline"
                                href=(GlEntryDetailPath { id: gl_id }.to_string()) {
                                "# " (gl_id)
                            }
                        }
                    }
                }
            }

            // ── 行项目 ──
            div class="data-card" {
                h3 class="text-base font-semibold text-fg mb-3 px-1" { "发票明细" }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="text-right" { "数量" }
                                th class="text-right" { "单价" }
                                th class="text-right" { "行小计" }
                                th class="text-right" { "行税额" }
                                th class="text-right" { "行价税合计" }
                            }
                        }
                        tbody {
                            @for line in items {
                                @let (code, name) = product_map.get(&line.product_id).cloned().unwrap_or_else(|| ("—".to_string(), format!("#{}", line.product_id)));
                                tr {
                                    td class="font-mono tabular-nums text-accent" { (code) }
                                    td { (name) }
                                    td class="font-mono tabular-nums text-right" { (line.qty.to_string()) }
                                    td class="font-mono tabular-nums text-right" { (fmt_amount(line.unit_price)) }
                                    td class="font-mono tabular-nums text-right" { (fmt_amount(line.line_subtotal)) }
                                    td class="font-mono tabular-nums text-right" { (fmt_amount(line.line_tax)) }
                                    td class="font-mono tabular-nums text-right font-semibold" { (fmt_amount(line.line_total)) }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="7" class="text-center text-muted py-8" { "暂无发票明细" }
                                }
                            }
                        }
                    }
                }
            }

            // ── 状态流转按钮 ──
            @if invoice.status == InvoiceStatus::Draft {
                div class="flex gap-3 mt-5" {
                    button class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-accent text-accent-on text-sm font-medium hover:bg-accent-hover cursor-pointer transition-all duration-150 shadow-xs"
                        hx-post=(SalesInvoicePostPath { id: invoice.id }.to_string())
                        hx-swap="none"
                        _="on click if not confirm('确认过账此发票？将生成 GL 凭证。') halt the event" {
                        (crate::components::icon::check_circle_icon("w-4 h-4"))
                        "过账"
                    }
                }
            }
            @if invoice.status == InvoiceStatus::Posted {
                div class="flex gap-3 mt-5" {
                    button class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-white text-danger border border-border hover:bg-danger-bg text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        hx-post=(SalesInvoiceCancelPath { id: invoice.id }.to_string())
                        hx-swap="none"
                        _="on click if not confirm('确认取消此发票？将同步取消关联 GL 凭证。') halt the event" {
                        (crate::components::icon::x_icon("w-4 h-4"))
                        "取消发票"
                    }
                }
            }
        }
    }
}
