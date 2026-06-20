use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::gl::sales_invoice::model::{
    CreateSalesInvoiceReq, SalesInvoiceItemInput,
};
use abt_core::gl::sales_invoice::SalesInvoiceService;
use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::product::ProductService;
use abt_core::purchase::tax::TaxRateService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{SalesInvoiceCreatePath, SalesInvoiceListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct InvoiceCreateForm {
    customer_id: String,
    issue_date: String,
    items_json: String,
}

/// 行项目 JSON（前端 lineItemCalc.collectItems 产出）
/// 字段：product_id / quantity / unit_price / discount_rate（发票忽略折扣）/ tax_rate_id
#[derive(Debug, Deserialize)]
struct ItemJson {
    product_id: i64,
    quantity: String,
    unit_price: String,
    #[allow(dead_code)]
    discount_rate: Option<String>,
    tax_rate_id: Option<String>,
}

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_create(
    _path: SalesInvoiceCreatePath,
    ctx: RequestContext,
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

    let customers = state
        .customer_service()
        .list(
            &service_ctx,
            &mut conn,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let products = state
        .product_service()
        .list(
            &service_ctx,
            &mut conn,
            ProductQuery {
                name: None,
                code: None,
                status: None,
                owner_department_id: None,
                category_id: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let tax_rates = state
        .tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await?;

    let content = invoice_create_page(&customers.items, &products.items, &tax_rates);
    let page_html = admin_page(
        is_htmx,
        "新建销售发票",
        &claims,
        "gl",
        SalesInvoiceCreatePath::PATH,
        "总账管理",
        Some("新建销售发票"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("GL", "create")]
pub async fn create(
    _path: SalesInvoiceCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<InvoiceCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let items: Vec<ItemJson> = match serde_json::from_str(&form.items_json) {
        Ok(v) => v,
        Err(_) => {
            return Ok(axum::response::Redirect::to(SalesInvoiceCreatePath::PATH).into_response())
        }
    };

    let parsed_items: Vec<SalesInvoiceItemInput> = items
        .into_iter()
        .filter(|i| i.product_id > 0)
        .map(|i| SalesInvoiceItemInput {
            product_id: i.product_id,
            qty: parse_decimal(&i.quantity),
            unit_price: parse_decimal(&i.unit_price),
            tax_rate_id: i.tax_rate_id.as_deref().and_then(|s| {
                let t = s.trim();
                if t.is_empty() { None } else { t.parse::<i64>().ok() }
            }),
        })
        .collect();

    let customer_id: i64 = match form.customer_id.trim().parse() {
        Ok(v) if v > 0 => v,
        _ => return Ok(axum::response::Redirect::to(SalesInvoiceCreatePath::PATH).into_response()),
    };

    if parsed_items.is_empty() {
        return Ok(axum::response::Redirect::to(SalesInvoiceCreatePath::PATH).into_response());
    }

    let issue_date = chrono::NaiveDate::parse_from_str(&form.issue_date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    let req = CreateSalesInvoiceReq {
        customer_id,
        issue_date,
        items: parsed_items,
        source_shipping_id: None,
    };

    let svc = state.sales_invoice_service();
    match svc.create(&service_ctx, &mut conn, req).await {
        Ok(_) => Ok(axum::response::Redirect::to(SalesInvoiceListPath::PATH).into_response()),
        Err(_) => Ok(axum::response::Redirect::to(SalesInvoiceCreatePath::PATH).into_response()),
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    use std::str::FromStr;
    rust_decimal::Decimal::from_str(s.trim())
        .or_else(|_| rust_decimal::Decimal::from_str(&s.trim().replace(',', ".")))
        .unwrap_or(rust_decimal::Decimal::ZERO)
}

// ── Page ──

fn invoice_create_page(
    customers: &[abt_core::master_data::customer::model::Customer],
    products: &[abt_core::master_data::product::model::Product],
    tax_rates: &[abt_core::purchase::tax::TaxRate],
) -> Markup {
    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    html! {
        div {
            a href=(format!("{}?restore=true", SalesInvoiceListPath::PATH))
                class="inline-flex items-center gap-1 text-sm text-muted hover:text-accent transition-colors duration-150 mb-6" {
                (icon::arrow_left_icon("w-4 h-4"))
                "返回列表"
            }

            div class="mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建销售发票" }
            }

            form id="sales-invoice-form"
                hx-post=(SalesInvoiceCreatePath::PATH)
                hx-swap="none"
                onsubmit="lineItemCalc('#sales-invoice-item-tbody').collectItems()" {
                input type="hidden" id="items-json" name="items_json" value="[]" {};

                // ── Section 1: 发票头 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::clipboard_document_icon("w-4 h-4"))
                        " 发票信息"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "客户 " span class="text-danger" { "*" }
                            }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" name="customer_id" required {
                                option value="" { "请选择客户" }
                                @for c in customers {
                                    option value=(c.id) { (c.code) " — " (c.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "开票日期 " span class="text-danger" { "*" }
                            }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" type="date" name="issue_date" value=(today) required;
                        }
                    }
                }

                // ── Section 2: 行项目 ──
                div class="form-section" {
                    div class="flex items-center justify-between text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        span class="flex items-center gap-2" {
                            (icon::dollar_icon("w-4 h-4"))
                            " 发票明细"
                        }
                        span class="text-sm font-normal text-muted" {
                            "合计："
                            strong class="text-accent font-mono" id="grand-value" { "¥ 0.00" }
                        }
                    }
                    div class="overflow-x-auto" {
                        table class="w-full border-separate border-spacing-0 min-w-[870px]" {
                            thead {
                                tr {
                                    th class="w-[140px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "产品 " span class="text-danger" { "*" } }
                                    th class="text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "产品编码" }
                                    th class="w-[100px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "单位" }
                                    th class="w-[100px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "数量 " span class="text-danger" { "*" } }
                                    th class="w-[120px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "单价 " span class="text-danger" { "*" } }
                                    th class="w-[110px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "税率" }
                                    th class="w-[120px] text-right text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "小计 (¥)" }
                                    th class="w-[44px] px-3 py-2 border-b border-border-soft" {}
                                }
                            }
                            tbody id="sales-invoice-item-tbody" {}
                        }
                    }
                    div class="py-4" {
                        button type="button"
                            class="flex items-center justify-center gap-2 w-full py-3 border-1.5 border-dashed border-border text-accent text-sm font-medium cursor-pointer rounded-md hover:border-accent hover:bg-[rgba(37,99,235,0.04)] transition-all duration-200"
                            _="on click call addInvoiceLine()" {
                            (icon::plus_icon("w-4 h-4"))
                            "添加产品行"
                        }
                    }
                    // lineItemCalc.recalcTotals() 会写入 #subtotal-value / #discount-value / #grand-value，
                    // 缺一即报错；发票无折扣概念，前两者隐藏即可
                    span class="hidden" id="subtotal-value" { "¥ 0.00" }
                    span class="hidden" id="discount-value" { "- ¥ 0.00" }
                }

                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", SalesInvoiceListPath::PATH)) {
                        "取消"
                    }
                    button type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "创建发票"
                    }
                }
            }
        }

        (PreEscaped(format!(r#"<script>
// 产品下拉数据（注入给 addInvoiceLine）
window.__INVOICE_PRODUCTS__ = [{}];
// 税率下拉数据（注入给 addInvoiceLine）
window.__INVOICE_TAX_RATES__ = [{}];
function invoiceTaxOptions() {{
    var html = '<option value="">不征税</option>';
    (window.__INVOICE_TAX_RATES__ || []).forEach(function(t) {{
        // 默认预选 VAT13
        var sel = t.code === 'VAT13' ? ' selected' : '';
        html += '<option value="' + t.id + '"' + sel + '>' + t.name + ' (' + t.rate + '%)</option>';
    }});
    return html;
}}
function addInvoiceLine() {{
    var tbody = document.getElementById('sales-invoice-item-tbody');
    var row = document.createElement('tr');
    var opts = '<option value="">选择产品</option>';
    (window.__INVOICE_PRODUCTS__ || []).forEach(function(p) {{
        opts += '<option value="' + p.id + '" data-code="' + p.code + '" data-unit="' + p.unit + '">' + p.name + '</option>';
    }});
    row.innerHTML =
        '<td class="px-3 py-2 border-b border-border-soft"><select class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" name="product_id" onchange="onInvoiceProductChange(this)">' + opts + '</select></td>'
        + '<td class="px-3 py-2 border-b border-border-soft font-mono text-xs text-fg-2 prod-code">—</td>'
        + '<td class="px-3 py-2 border-b border-border-soft text-xs text-fg-2 prod-unit">—</td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><input class="w-[80px] text-right px-2.5 py-1.5 border border-border rounded-sm text-sm font-mono outline-none focus:border-accent" type="number" step="any" name="quantity" placeholder="0" oninput="lineItemCalc(\'#sales-invoice-item-tbody\').calcRow(this.closest(\'tr\'))"></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><input class="w-[100px] text-right px-2.5 py-1.5 border border-border rounded-sm text-sm font-mono outline-none focus:border-accent" type="number" step="any" name="unit_price" placeholder="0.00" oninput="lineItemCalc(\'#sales-invoice-item-tbody\').calcRow(this.closest(\'tr\'))"></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><select class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" name="tax_rate_id">' + invoiceTaxOptions() + '</select></td>'
        // discount_rate 隐藏为 0，满足 lineItemCalc.collectItems 字段约定
        + '<input type="hidden" name="discount_rate" value="0">'
        + '<td class="px-3 py-2 border-b border-border-soft text-right font-mono text-sm line-total">0.00</td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-[rgba(220,38,38,0.08)] hover:text-danger transition-all" title="删除行" onclick="this.closest(\'tr\').remove();lineItemCalc(\'#sales-invoice-item-tbody\').recalcTotals()"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M18 6L6 18M6 6l12 12"/></svg></button></td>';
    tbody.appendChild(row);
    lineItemCalc('#sales-invoice-item-tbody').recalcTotals();
}}
function onInvoiceProductChange(sel) {{
    var row = sel.closest('tr');
    var opt = sel.options[sel.selectedIndex];
    if (!opt) return;
    row.querySelector('.prod-code').textContent = opt.getAttribute('data-code') || '—';
    row.querySelector('.prod-unit').textContent = opt.getAttribute('data-unit') || '—';
}}
document.addEventListener('DOMContentLoaded', function() {{
    addInvoiceLine();
}});
</script>"#,
            products
                .iter()
                .map(|p| format!(
                    "{{\"id\":{},\"code\":{:?},\"name\":{:?},\"unit\":{:?}}}",
                    p.product_id, p.product_code, p.pdt_name, p.unit
                ))
                .collect::<Vec<_>>()
                .join(","),
            tax_rates
                .iter()
                .map(|t| format!(
                    "{{\"id\":{},\"code\":{:?},\"name\":{:?},\"rate\":{}}}",
                    t.id, t.code, t.name, t.rate
                ))
                .collect::<Vec<_>>()
                .join(",")
        )))
    }
}
