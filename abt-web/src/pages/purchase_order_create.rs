use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::enums::PurchaseQuotationStatus;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::*;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::TaxRateService;
use abt_core::purchase::quotation::model::PurchaseQuotationQuery;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
 pub name: Option<String>,
 pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SupplierDetailParams {
 pub supplier_id: i64,
}

// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct POCreateForm {
 pub supplier_id: i64,
 pub order_date: String,
 pub expected_delivery_date: Option<String>,
 pub payment_terms: Option<String>,
 pub currency: Option<String>,
 pub delivery_address: Option<String>,
 pub related_quotation_id: Option<String>,
 pub buyer_id: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
 pub action: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemWeb {
 pub product_id: String,
 pub description: Option<String>,
 pub quantity: String,
 pub unit_price: String,
 pub item_delivery_date: Option<String>,
 pub discount_pct: Option<String>,
 pub tax_rate_id: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_create(
 _path: POCreatePath,
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
 let supplier_svc = state.supplier_service();
 let pq_svc = state.purchase_quotation_service();

 let suppliers = supplier_svc
 .list(
 &service_ctx,
 &mut conn,
 SupplierQuery {
 name: None,
 status: None,
 category: None,
 },
 PageParams::new(1, 200),
 )
 .await?;


 let quotations = pq_svc
 .list(
 &service_ctx,
 &mut conn,
 PurchaseQuotationQuery {
 supplier_id: None,
 status: Some(PurchaseQuotationStatus::Active),
 quotation_date_start: None,
 quotation_date_end: None,
 },
 PageParams::new(1, 200),
 )
 .await?;

 let tax_rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();

let content = po_create_page(&suppliers.items, &quotations.items, &tax_rates, POCreatePath::PATH, "", true, None, None, &[]);
 let page_html = admin_page(
 is_htmx,
 "新建采购订单",
 &claims,
 "purchase",
 POCreatePath::PATH,
 "采购管理",
 Some("新建采购订单"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: return supplier detail fragment (contact/phone/address/info bar)
#[require_permission("SUPPLIER", "read")]
pub async fn get_po_supplier_detail(
 ctx: RequestContext,
 Query(params): Query<SupplierDetailParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.supplier_service();

 let supplier = svc.get(&service_ctx, &mut conn, params.supplier_id).await?;
 let contacts = svc
 .list_contacts(&service_ctx, &mut conn, params.supplier_id)
 .await
 .unwrap_or_default();

 let primary = contacts.iter().find(|c| c.is_primary);
 let contact_name = primary
 .map(|c| c.name.as_str())
 .unwrap_or("—");
 let contact_phone = primary
 .and_then(|c| c.phone.as_deref())
 .unwrap_or("—");

 // Compute cooperation years from created_at
 let coop_years = {
 let created = supplier.created_at;
 let now = chrono::Utc::now();
 let diff = now.signed_duration_since(created);
 diff.num_days() / 365
 };

 Ok(Html(
 supplier_detail_fragment(contact_name, contact_phone, coop_years).into_string(),
 ))
}


/// HTMX/JS: return active tax rates as JSON
#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_tax_rates(ctx: RequestContext) -> Result<axum::Json<serde_json::Value>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
 let json: Vec<serde_json::Value> = rates.iter().map(|r| serde_json::json!({
 "id": r.id, "code": r.code, "name": r.name, "rate": r.rate.to_string()
 })).collect();
 Ok(axum::Json(serde_json::Value::Array(json)))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 product_id: i64,
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_po_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.product_service();
 let product = svc
 .get(&service_ctx, &mut conn, params.product_id)
 .await?;
 let tax_rates = state.tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
 Ok(Html(item_row_fragment(&product, &tax_rates).into_string()))
}

/// PO 创建核心逻辑（解析 POCreateForm → svc.create），创建页与 work_center drawer 共用。
/// 失败时返回字段错误 map + 产品数据（重渲染 form 恢复行，§5.6）。
pub async fn do_create_po(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: POCreateForm,
) -> std::result::Result<i64, (std::collections::HashMap<&'static str, String>, Vec<ItemWeb>)> {
    let svc = state.purchase_order_service();
    let mut errors: std::collections::HashMap<&'static str, String> = std::collections::HashMap::new();
    let order_date = chrono::NaiveDate::parse_from_str(&form.order_date, "%Y-%m-%d")
        .unwrap_or_else(|_| {
            errors.insert("__all__", "无效订单日期格式".to_string());
            chrono::Local::now().date_naive()
        });
    let expected_delivery_date = form
        .expected_delivery_date
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    if form.supplier_id <= 0 {
        errors.entry("supplier_id").or_insert_with(|| "请选择供应商".to_string());
    }
    let web_items: Vec<ItemWeb> = match serde_json::from_str(&form.items_json) {
        Ok(v) => v,
        Err(e) => { errors.insert("__all__", format!("无效产品数据: {e}")); vec![] }
    };
    if web_items.is_empty() && errors.is_empty() {
        errors.insert("__all__", "请至少添加一个采购产品明细".to_string());
    }
    let saved_items = web_items.clone();
    let mut items: Vec<CreateOrderItemRequest> = Vec::with_capacity(web_items.len());
    for (idx, item) in web_items.into_iter().enumerate() {
        let line_no = (idx as i32) + 1;
        let quantity: rust_decimal::Decimal = item.quantity.parse().unwrap_or_else(|_| {
            errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 无效数量"));
            rust_decimal::Decimal::ZERO
        });
        if quantity < rust_decimal::Decimal::ZERO {
            errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 数量不能为负"));
        }
        let unit_price: rust_decimal::Decimal = item.unit_price.parse().unwrap_or_else(|_| {
            errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 无效单价"));
            rust_decimal::Decimal::ZERO
        });
        if unit_price < rust_decimal::Decimal::ZERO {
            errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 单价不能为负"));
        }
        let item_delivery_date = item.item_delivery_date.as_deref().filter(|s| !s.is_empty())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        items.push(CreateOrderItemRequest {
            product_id: item.product_id.parse().unwrap_or(0),
            line_no,
            description: item.description.unwrap_or_default(),
            quantity,
            unit_price,
            quotation_item_id: None,
            expected_delivery_date: item_delivery_date,
            discount_pct: item.discount_pct.and_then(|s| s.parse().ok()).unwrap_or(rust_decimal::Decimal::ZERO),
            tax_rate_id: item.tax_rate_id.and_then(|s| s.parse().ok()).filter(|&v: &i64| v > 0),
        });
    }
    if !errors.is_empty() {
        return Err((errors, saved_items));
    }
    let create_req = CreatePurchaseOrderRequest {
        supplier_id: form.supplier_id,
        order_date,
        expected_delivery_date,
        payment_terms: form.payment_terms,
        delivery_address: form.delivery_address,
        remark: form.remark.unwrap_or_default(),
        currency_code: form.currency.unwrap_or_else(|| String::from("CNY")),
        currency_rate: rust_decimal::Decimal::ONE,
        discount_amount: rust_decimal::Decimal::ZERO,
        items,
    };
    let mut tx = state.pool.begin().await.map_err(|e| {
        let mut m = std::collections::HashMap::new();
        m.insert("__all__", format!("数据库错误: {e}"));
        (m, saved_items.clone())
    })?;
    match svc.create(&service_ctx, &mut tx, create_req, None).await {
        Ok(id) => { tx.commit().await.map_err(|e| {
            let mut m = std::collections::HashMap::new();
            m.insert("__all__", format!("提交失败: {e}"));
            (m, saved_items.clone())
        })?; Ok(id) }
        Err(e) => {
            let mut m = std::collections::HashMap::new();
            m.insert("__all__", format!("{e}"));
            Err((m, saved_items))
        }
    }
}

/// POST: create purchase order from form submission (HTMX)
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn create_po(
    _path: POCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<POCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let action = form.action.clone();
    let submitted = form.clone();
    match do_create_po(&state, &service_ctx, form).await {
        Ok(id) => {
            let redirect = if action.as_deref() == Some("draft") { POListPath::PATH.to_string() } else { PODetailPath { id }.to_string() };
            Ok(([("HX-Redirect", redirect)], Html(String::new())).into_response())
        }
        Err((errors, saved_items)) => {
            let mut preview_rows = Vec::new();
            for item in &saved_items {
                if let Ok(pid) = item.product_id.parse::<i64>() {
                    if pid > 0 {
                        if let Ok(product) = state.product_service().get(&service_ctx, &mut conn, pid).await {
                            let price = item.unit_price.parse().unwrap_or(rust_decimal::Decimal::ZERO);
                            preview_rows.push((product, price));
                        }
                    }
                }
            }
            let html = po_create_page(
                &[],
                &[],
                &[],
                POCreatePath::PATH,
                "",
                true,
                Some(&submitted),
                Some(&errors),
                &preview_rows,
            );
            Ok(html.into_response())
        }
    }
}
pub fn po_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    quotations: &[abt_core::purchase::quotation::model::PurchaseQuotation],
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
    submitted: Option<&POCreateForm>,
    errors: Option<&std::collections::HashMap<&str, String>>,
    preview_rows: &[(abt_core::master_data::product::model::Product, rust_decimal::Decimal)],
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_delivery = chrono::Local::now()
 .checked_add_days(chrono::Days::new(15))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

 html! {
    div id="po-app" {
        @if show_header {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", POListPath::PATH))
                { (icon::arrow_left_icon("w-4 h-4")) "返回采购订单列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购订单" }
            }
        }

        form id="po-form" hx-post=(post_path) hx-target="this" hx-swap="outerHTML" _=(after_request_hs) {
            @if let Some(msg) = errors.and_then(|e| e.get("__all__").map(|s| s.as_str())) {
                div class="text-danger text-xs mb-3 leading-relaxed" { (msg) }
            }
            input type="hidden" id="items-json" name="items_json" value="[]";
            // ── 基本信息（合并供应商 + 订单信息，参考三家 ERP 精简）──
            div class="data-card mb-3 [&_input,&_select]:py-1.5" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-2.5 pb-1.5 border-b border-border-soft"
                { "基本信息" }
                div class="grid grid-cols-3 gap-y-3 gap-x-4 mb-1" {
                    div class="form-field" {
                        label {
                            "供应商"
                            span class="text-danger" { "*" }
                        }
                        select
                            name="supplier_id"
                            required
                            hx-get=(POSupplierDetailPath::PATH)
                            hx-trigger="change"
                            hx-target="#supplier-detail"
                            hx-swap="innerHTML"
                            hx-include="this"
                        {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "订单日期" }
                        input type="date" name="order_date" value=(today) readonly {}
                    }
                    div class="form-field" {
                        label { "预期交货日期" }
                        input type="date" name="expected_delivery_date" value=(default_delivery) {}
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" {
                            option value="CNY" selected { "CNY" }
                            option value="USD" { "USD" }
                            option value="EUR" { "EUR" }
                        }
                    }
                    div class="form-field" {
                        label { "付款条件" }
                        select name="payment_terms" {
                            option value="" { "请选择付款条件" }
                            option value="30天净额" { "30天净额" }
                            option value="60天净额" { "60天净额" }
                            option value="预付30%" { "预付30%" }
                            option value="货到付款" { "货到付款" }
                            option value="月结30天" { "月结30天" }
                        }
                    }
                    div class="form-field" {
                        label { "关联报价" }
                        select name="related_quotation_id" {
                            option value="" { "请选择采购报价" }
                            @for q in quotations {
                                option value=(q.id) { (q.doc_number) }
                            }
                        }
                    }
                }
                // ── Supplier Info Bar ──（保留 handler 联动占位，三家 ERP 不在头部展示联系人/电话/地址）
                div id="supplier-detail" class="mt-2" {}
            }
            // ── Line Items ──
            div class="data-card p-0 overflow-hidden mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg px-5 pt-5 pb-3"
                { "采购产品明细" }
                div class="overflow-x-auto" {
                    table class="data-table min-w-[900px]" {
                        thead {
                            tr {
                                th class="w-9 text-center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="w-[200px]" { "描述" }
                                th class="w-[100px] text-right" { "数量" }
                                th class="w-[120px] text-right" { "单价" }
                                th class="w-[110px] text-right" { "小计" }
                                th class="w-[80px] text-right" { "折扣%" }
                                th class="w-[150px]" { "税率" }
                                th class="w-[120px]" { "预期交货日期" }
                                th class="w-9" {}
                            }
                        }
                        tbody id="po-item-tbody" {
                            @for (product, _price) in preview_rows {
                                (item_row_fragment(product, tax_rates))
                            }
                        }
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                        _="on click add .is-open to #product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品行" }
                }
                div class="flex justify-end p-4 border-t border-border" {
                    div class="flex text-sm gap-6" {
                        div {
                            "不含税: "
                            span id="sum-untaxed" class="font-semibold" { "0.00" }
                        }
                        div {
                            "税额: "
                            span id="sum-tax" class="font-semibold" { "0.00" }
                        }
                        div {
                            "含税总计: "
                            span id="sum-total" class="font-semibold text-accent" { "0.00" }
                        }
                    }
                }
            }
            // ── 备注 ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "备注" }
                textarea
                    name="remark"
                    placeholder="输入订单相关备注信息…"
                    class="w-full resize-y rounded-sm min-h-[80px] border border-border text-sm"
                    style="padding:8px 12px;font-family:inherit" {}
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", POListPath::PATH))
                { "取消" }
                div class="flex gap-3" {
                    button
                        type="submit"
                        name="action" value="draft"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    { "保存草稿" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { "提交订单" }
                }
            }
            script {
                ({
                    maud::PreEscaped(
                        "document.currentScript.parentElement.addEventListener('submit', function(ev){
 var errors=[];
 document.querySelectorAll('#po-item-tbody tr').forEach(function(row, i){
 var q=parseFloat(row.querySelector('[name=quantity]').value)||0;
 var p=parseFloat(row.querySelector('[name=unit_price]').value)||0;
 if(q<=0) errors.push('第'+(i+1)+'行数量必须大于0');
 if(p<=0) errors.push('第'+(i+1)+'行单价必须大于0');
 });
 if(errors.length>0){ alert(errors.join('\\n')); ev.preventDefault(); return; }
 var items=[];
 document.querySelectorAll('#po-item-tbody tr').forEach(function(row){
 var obj={};
 row.querySelectorAll('input,select,textarea').forEach(function(el){
 if(el.name && !obj[el.name]) obj[el.name]=el.value;
 });
 items.push(obj);
 });
 var el=document.querySelector('#items-json'); if(el) el.value=JSON.stringify(items);
 })",
                    )
                })
            }
        }

        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                POItemRowPath::PATH,
                "po-item-tbody",
            )
        })
    }
}
}

/// Supplier detail fragment returned by HTMX on supplier select change
fn supplier_detail_fragment(contact_name: &str, contact_phone: &str, coop_years: i64) -> Markup {
 html! {
    div class="supplier-info-bar flex bg-surface rounded-sm px-4 py-3 text-sm gap-6 text-fg-2"
    {
        span {
            "联系人: "
            strong { (contact_name) }
        }
        span {
            "电话: "
            strong { (contact_phone) }
        }
        span {
            "地址: "
            strong { "—" }
        }
        span {
            "合作年限: "
            strong { (coop_years) " 年" }
        }
    }
    script {
        ({
            maud::PreEscaped(
                format!(
                    "document.querySelector('#supplier-contact').value = '{}';",
                    contact_name.replace('\'', "\\'"),
                ),
            )
        })
        ({
            maud::PreEscaped(
                format!(
                    "document.querySelector('#supplier-phone').value = '{}';",
                    contact_phone.replace('\'', "\\'"),
                ),
            )
        })
    }
}
}

fn item_row_fragment(
 product: &abt_core::master_data::product::model::Product,
 tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
 let input_style = "width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)";
 html! {
    tr data-item-row="" {
        td class="text-muted text-xs text-center" {}
        td class="font-mono tabular-nums" { (product.product_code) }
        td { (product.pdt_name) }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] text-[13px] rounded-sm px-2 py-[5px] border border-border"
                type="text"
                name="description"
                placeholder="—"
                style="width:190px" {}
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="quantity"
                data-field="qty"
                placeholder="0"
                style=(input_style) {}
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="unit_price"
                data-field="price"
                placeholder="0.00"
                style=(input_style) {}
        }
        td class="line-subtotal font-mono tabular-nums text-right" data-field="subtotal" {
            "0.00"
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input"
                type="number"
                step="any"
                name="discount_pct"
                data-field="discount"
                value="0"
                placeholder="0"
                style=(input_style) {}
        }
        td {
            select
                class="min-w-[150px] w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] text-[13px] rounded-sm px-2 py-[5px] border border-border"
                name="tax_rate_id"
                data-field="tax_rate_id"
            {
                option value="" { "—" }
                @for tr in tax_rates {
                    option value=(tr.id) data-rate=(tr.rate.to_string()) { (tr.name) }
                }
            }
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] w-[110px] text-[13px] rounded-sm px-2 py-[5px] border border-border"
                type="date"
                name="item_delivery_date" {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove closest <tr/> then call updatePurchaseSummary()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(product.product_id) {}
    }
}
}
