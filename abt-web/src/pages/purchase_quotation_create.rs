use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::quotation::model::*;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::overlay::modal_shell;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_quotation::{
    PQCreatePath, PQDetailPath, PQItemRowPath, PQListPath, PQPriceRecordPath,
    PQSupplierContactsPath,
};
use crate::routes::supplier_price_catalog::SupplierPricesPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query Params ──


// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PQCreateForm {
 pub supplier_id: i64,
 pub quotation_date: String,
 pub valid_from: String,
 pub valid_until: String,
 pub currency: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub buyer_id: Option<i64>,
 pub supplier_quotation_no: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
 pub action: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemWeb {
 pub product_id: String,
 pub unit_price: String,
 pub min_order_qty: Option<String>,
 pub lead_time_days: Option<String>,
 pub is_preferred: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_create(
 _path: PQCreatePath,
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

let content = pq_create_page(&suppliers.items, PQCreatePath::PATH, "", true, None, None, &[]);
 let page_html = admin_page(
 is_htmx,
 "新建采购报价",
 &claims,
 "purchase",
 PQCreatePath::PATH,
 "采购管理",
 Some("新建采购报价"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 use abt_core::purchase::supplier_price::SupplierPriceService;
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
 // 选了供应商时，按 match_best_price(supplier, product, qty=1) 自动带单价
 let unit_price = if let Some(sid) = params.supplier_id.filter(|&s| s > 0) {
 state
 .supplier_price_service()
 .match_best_price(&service_ctx, &mut conn, sid, params.product_id, rust_decimal::Decimal::ONE)
 .await?
 .map(|p| p.price)
 } else {
 None
 };
 Ok(Html(item_row_fragment(&product, unit_price, None).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    product_id: i64,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    supplier_id: Option<i64>,
}

/// HTMX: return supplier contact info fragment (contact, phone, address)
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_supplier_contacts(
 ctx: RequestContext,
 Query(params): Query<SupplierContactParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let supplier_svc = state.supplier_service();

 let contacts = if params.supplier_id > 0 {
 supplier_svc
 .list_contacts(&service_ctx, &mut conn, params.supplier_id)
 .await
 .unwrap_or_default()
 } else {
 vec![]
 };

 // Find primary contact, or fall back to first
 let primary = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());

 let contact_name = primary.map(|c| c.name.as_str()).unwrap_or("");
 let contact_phone = primary
 .and_then(|c| c.phone.as_deref())
 .unwrap_or("");
 // 取供应商默认币种（联动币种 select OOB）
 let currency = if params.supplier_id > 0 {
 supplier_svc
 .get(&service_ctx, &mut conn, params.supplier_id)
 .await
 .ok()
 .map(|s| s.currency)
 } else {
 None
 };

 Ok(Html(
 supplier_contact_fields_fragment(contact_name, contact_phone, currency.as_deref()).into_string(),
 ))
}

#[derive(Debug, Deserialize)]
pub struct SupplierContactParams {
 pub supplier_id: i64,
}

/// POST: create purchase quotation from form submission (HTMX)
/// 报价创建核心逻辑（解析 PQCreateForm → svc.create），创建页与 work_center drawer 共用。
/// 失败时返回字段错误 map（key 为字段名或 "__all__" 行级错误），create_pq 据此重渲染 form（§5.6）。
pub async fn do_create_pq(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: PQCreateForm,
) -> std::result::Result<i64, (std::collections::HashMap<&'static str, String>, Vec<ItemWeb>)> {
    let svc = state.purchase_quotation_service();
    let mut errors: std::collections::HashMap<&'static str, String> = std::collections::HashMap::new();
    let quotation_date = chrono::NaiveDate::parse_from_str(&form.quotation_date, "%Y-%m-%d")
        .unwrap_or_else(|_| {
            errors.insert("__all__", "无效报价日期格式".to_string());
            chrono::Local::now().date_naive()
        });
    let valid_from = chrono::NaiveDate::parse_from_str(&form.valid_from, "%Y-%m-%d")
        .unwrap_or_else(|_| {
            errors.entry("__all__").or_insert_with(|| "无效生效日期格式".to_string());
            chrono::Local::now().date_naive()
        });
    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .unwrap_or_else(|_| {
            errors.entry("__all__").or_insert_with(|| "无效失效日期格式".to_string());
            chrono::Local::now().date_naive()
        });
    if form.supplier_id <= 0 {
        errors.entry("supplier_id").or_insert_with(|| "请选择供应商".to_string());
    }
    if valid_until <= valid_from {
        errors.entry("valid_until").or_insert_with(|| "失效日期必须晚于生效日期".to_string());
    }
    let web_items: Vec<ItemWeb> = match serde_json::from_str(&form.items_json) {
        Ok(v) => v,
        Err(e) => {
            errors.insert("__all__", format!("无效产品数据: {e}"));
            vec![]
        }
    };
    if web_items.is_empty() && errors.is_empty() {
        errors.insert("__all__", "请至少添加一个报价产品明细".to_string());
    }
    let main_currency = form.currency.clone().unwrap_or_else(|| "CNY".to_string());
    let saved_items = web_items.clone(); // 失败时带回重渲染
    let mut items: Vec<CreateQuotationItemRequest> = Vec::with_capacity(web_items.len());
    for (idx, item) in web_items.into_iter().enumerate() {
        let line_no = (idx as i32) + 1;
        let unit_price: rust_decimal::Decimal = match item.unit_price.parse() {
            Ok(p) => p,
            Err(_) => {
                errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 无效单价"));
                rust_decimal::Decimal::ZERO
            }
        };
        if unit_price < rust_decimal::Decimal::ZERO {
            errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 单价不能为负"));
        }
        let lead_time_days: Option<i32> = item.lead_time_days.and_then(|s| s.parse().ok());
        if let Some(d) = lead_time_days
            && d < 0 {
                errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 交货天数不能为负"));
            }
        let min_order_qty: Option<rust_decimal::Decimal> =
            item.min_order_qty.and_then(|s| s.parse().ok());
        if let Some(q) = min_order_qty
            && q < rust_decimal::Decimal::ZERO {
                errors.entry("__all__").or_insert_with(|| format!("第{line_no}行: 起订量不能为负"));
            }
        items.push(CreateQuotationItemRequest {
            product_id: item.product_id.parse().unwrap_or(0),
            line_no,
            unit_price,
            min_order_qty,
            lead_time_days,
            currency: main_currency.clone(),
            is_preferred: item.is_preferred.is_some(),
        });
    }
    if !errors.is_empty() {
        return Err((errors, saved_items));
    }
    let create_req = CreatePurchaseQuotationRequest {
        supplier_id: form.supplier_id,
        quotation_date,
        valid_from,
        valid_until,
        remark: form.remark.unwrap_or_default(),
        currency: main_currency,
        buyer_id: form.buyer_id,
        supplier_quotation_no: form.supplier_quotation_no.unwrap_or_default(),
        items,
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| {
            let mut m: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
            m.insert("__all__", format!("数据库错误: {e}"));
            (m, saved_items.clone())
        })?;
    match svc.create(service_ctx, &mut tx, create_req, None).await {
        Ok(id) => {
            tx.commit().await.map_err(|e| {
                let mut m: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
                m.insert("__all__", format!("提交失败: {e}"));
                (m, saved_items.clone())
            })?;
            Ok(id)
        }
        Err(e) => {
            let mut m: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
            m.insert("__all__", format!("{e}"));
            Err((m, saved_items))
        }
    }
}
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn create_pq(
    _path: PQCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PQCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    // 失败 → 重渲染 form（自替换 + 字段标红 + 用户输入回填，§5.6）
    // 成功 → 空 body + HX-Redirect（form 的 after_request_hs 凭 responseText 空跳转）
    let submitted = form.clone();
    match crate::pages::purchase_quotation_create::do_create_pq(&state, &service_ctx, form).await {
        Ok(id) => Ok((
            [("HX-Redirect", PQDetailPath { id }.to_string())],
            Html(String::new()),
        )
            .into_response()),
        Err((errors, saved_items)) => {
            // 重新查询产品以恢复用户已添加的行（产品明细不丢）
            let mut preview_rows: Vec<(abt_core::master_data::product::model::Product, rust_decimal::Decimal, Option<i32>)> = Vec::new();
            for item in &saved_items {
                if let Ok(pid) = item.product_id.parse::<i64>()
                    && pid > 0
                        && let Ok(product) = state.product_service().get(&service_ctx, &mut conn, pid).await {
                            let price: rust_decimal::Decimal = item.unit_price.parse().unwrap_or(rust_decimal::Decimal::ZERO);
                            let ldt: Option<i32> = item.lead_time_days.as_deref().and_then(|s| s.parse().ok());
                            preview_rows.push((product, price, ldt));
                        }
            }
            let suppliers = state
                .supplier_service()
                .list(
                    &service_ctx,
                    &mut conn,
                    abt_core::master_data::supplier::model::SupplierQuery {
                        name: None,
                        status: None,
                        category: None,
                    },
                    PageParams::new(1, 200),
                )
                .await
                .map(|r| r.items)
                .unwrap_or_default();
            let html = crate::pages::purchase_quotation_create::pq_create_page(
                &suppliers,
                PQCreatePath::PATH,
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

// ── Components ──

pub fn pq_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
    submitted: Option<&PQCreateForm>,
    errors: Option<&std::collections::HashMap<&str, String>>,
    preview_rows: &[(abt_core::master_data::product::model::Product, rust_decimal::Decimal, Option<i32>)],
) -> Markup {
 // 字段值：优先用 submitted 回填，否则用默认
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_valid = chrono::Local::now()
 .checked_add_days(chrono::Days::new(30))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();
 let supplier_id_val = submitted.map(|f| f.supplier_id.to_string()).unwrap_or_default();
 let quotation_date_val = submitted.map(|f| f.quotation_date.clone()).unwrap_or_else(|| today.clone());
 let valid_until_val = submitted.map(|f| f.valid_until.clone()).unwrap_or_else(|| default_valid.clone());
 let currency_val = submitted.and_then(|f| f.currency.clone()).unwrap_or_else(|| "CNY".to_string());
 let valid_from_val = submitted.map(|f| f.valid_from.clone()).unwrap_or_else(|| today.clone());

 html! {
    div id="pq-app" {
        @if show_header {
            div class="flex items-center justify-between mb-6" {
                a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", PQListPath::PATH))
                { (icon::arrow_left_icon("w-4 h-4")) "返回采购报价列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购报价" }
            }
        }
        form id="pq-form" hx-post=(post_path) hx-target="this" hx-swap="outerHTML" _=(after_request_hs) {
            @if let Some(msg) = errors.and_then(|e| e.get("__all__").map(|s| s.as_str())) {
                div class="text-danger text-xs mb-3 leading-relaxed" { (msg) }
            }
            input type="hidden" id="items-json" name="items_json" value="[]";
            input type="hidden" id="form-action" name="action" value="submit";
            // ── 基本信息（参考三家 ERP 精简到 4 字段）──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-2" {
                    div class="form-field" {
                        label {
                            "供应商"
                            span class="text-danger" { "*" }
                        }
                        select
                            name="supplier_id"
                            required
                            class=(if errors.and_then(|e| e.get("supplier_id")).is_some() { "w-full px-3 py-2 border border-danger rounded-sm text-sm bg-white text-fg outline-none" } else { "" })
                            hx-get=(PQSupplierContactsPath::PATH)
                            hx-trigger="change"
                            hx-target="#supplier-contact-fields"
                            hx-swap="innerHTML"
                            hx-vals="js:{supplier_id: this.value}"
                        {
                            option value="" disabled selected[submitted.is_none()] { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) selected[submitted.is_some() && s.id == supplier_id_val.parse::<i64>().unwrap_or(-1)] { (s.name) }
                            }
                        }
                        @if let Some(m) = errors.and_then(|e| e.get("supplier_id")) {
                            p class="text-danger text-xs mt-1" { (m) }
                        }
                    }
                    div class="form-field" {
                        label { "报价日期" }
                        input type="date" name="quotation_date" value=(quotation_date_val) readonly {}
                    }
                    div class="form-field" {
                        label {
                            "失效日期"
                            span class="text-danger" { "*" }
                        }
                        input
                            type="date"
                            name="valid_until"
                            id="f-valid-until"
                            class=(if errors.and_then(|e| e.get("valid_until")).is_some() { "border-danger" } else { "" })
                            value=(valid_until_val) {}
                        @if let Some(m) = errors.and_then(|e| e.get("valid_until")) {
                            p class="text-danger text-xs mt-1" { (m) }
                        }
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" id="pq-currency"
                            _="on change call pqRefresh()" {
                            option value="CNY" selected[currency_val == "CNY"] { "CNY (人民币)" }
                            option value="USD" selected[currency_val == "USD"] { "USD (美元)" }
                            option value="EUR" selected[currency_val == "EUR"] { "EUR (欧元)" }
                        }
                    }
                }
                // hidden 占位：保留 supplier_contact_fields_fragment 的 OOB 币种 select 替换链路
                div id="supplier-contact-fields" class="hidden" {}
                // 生效日期默认 = 报价日期（三家 ERP 均不暴露 validFrom，仅作区间起点存库）
                input type="hidden" name="valid_from" value=(valid_from_val);
            }
            // ── Line Items ──
            div class="data-card p-0 overflow-hidden mb-4" {
                div class="flex justify-between items-center px-5 pt-5 pb-3" {
                    span
                        class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft m-0 p-0 border-none"
                    { "报价产品明细" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                        _="on click add .is-open to #product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品" }
                }
                div class="overflow-x-auto" {
                    table class="data-table min-w-[640px]" {
                        thead {
                            tr {
                                th class="w-9 text-center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="w-[140px] text-right" { "单价" }
                                th class="w-[110px] text-right" { "交货天数" }
                                th class="w-12" {}
                            }
                        }
                        tbody id="pq-item-tbody"
                            _="on input call pqRefresh()\non 'htmx:afterSettle' call pqRefresh()" {
                            @for (product, _price, ldt) in preview_rows {
                                (item_row_fragment(product, None, *ldt))
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
            }
            // ── Remark ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "备注" }
                textarea
                    name="remark"
                    placeholder="输入报价相关备注信息…"
                    class="w-full resize-y rounded-sm min-h-[80px] border border-border text-sm"
                    style="padding:8px 12px;font-family:inherit" {}
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                // 左：实时统计带（项数 / 单价区间 / 首选数）
                div class="flex items-center gap-3 text-xs text-muted" {
                    span { "已添 " span id="pq-stat-count" class="font-semibold text-fg" { "0" } " 项" }
                    span class="text-border" { "·" }
                    span { "单价 " span id="pq-stat-range" class="font-mono text-fg" { "—" } }
                    span class="text-border" { "·" }
                    span { "首选 " span id="pq-stat-preferred" class="font-semibold text-fg" { "0" } " 项" }
                }
                // 右：操作按钮
                div class="flex items-center gap-3" {
                    @if show_header {
                        a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            href=(format!("{}?restore=true", PQListPath::PATH))
                        { "取消" }
                    } @else {
                        button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _="on click set #form-action's value to 'draft' then call document.querySelector('#pq-form').requestSubmit()"
                    { "保存草稿" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    {
                        "提交报价"
                        ({
                            maud::PreEscaped(
                                r#"<script>document.currentScript.parentElement.addEventListener('click', function() {
 // 后端 §5.6 字段级校验已覆盖有效期 + 字段非空，前端只收集 items
 var items = [];
 document.querySelectorAll('#pq-item-tbody tr:not(.pq-compare-row)').forEach(function(row) {
 var vals = {};
 row.querySelectorAll('input,select').forEach(function(el) {
 if (el.name && el.name.startsWith('item_')) vals[el.name.replace('item_','')] = el.value;
 });
 items.push(vals);
 });
 if (items.length === 0) {
 show_error_toast('请至少添加一个报价产品明细');
 return;
 }
 var el=document.querySelector('#items-json'); if(el) el.value = JSON.stringify(items);
 document.querySelector('#pq-form').requestSubmit();
})</script>"#,
                            )
                        })
                    }
                }
            }
        }

        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                PQItemRowPath::PATH,
                "pq-item-tbody",
            )
        })
        // 补录价格 modal —— 行内「补录」按钮 hx-get 加载表单进来，afterSettle 唤醒显示
        (modal_shell("price-modal-pq", "z-[1100]", html! {}))
    }
}
}

/// Fragment returned by HTMX for supplier contact fields
fn supplier_contact_fields_fragment(contact_name: &str, contact_phone: &str, currency: Option<&str>) -> Markup {
    let cur = currency.unwrap_or("CNY");
    let opts = [("CNY", "CNY (人民币)"), ("USD", "USD (美元)"), ("EUR", "EUR (欧元)")];
 html! {
    div class="form-field" {
        label { "联系人" }
        input type="text" readonly value=(contact_name) placeholder="—" class="bg-surface" {}
    }
    div class="form-field" {
        label { "联系电话" }
        input type="text" readonly value=(contact_phone) placeholder="—" class="bg-surface" {}
    }
    // OOB 更新币种 select（选 supplier 联动其默认币种）。须带 pqRefresh 绑定：
    // OOB 会整体替换 #pq-currency，hyperscript 不会从原节点迁移，否则切币种不再触发行级跟随/统计。
    // on load：换入后同步一次（已有行的币种跟随新默认值）；on change：后续手动切币种时跟随。
    select name="currency" id="pq-currency" hx-swap-oob="true"
        _="on change call pqRefresh()\non load call pqRefresh()" {
        @for (code, label) in opts {
            option value=(code) selected[code == cur] { (label) }
        }
    }
}
}

fn item_row_fragment(
    product: &abt_core::master_data::product::model::Product,
    unit_price: Option<rust_decimal::Decimal>,
    prefilled_lead_time: Option<i32>,
) -> Markup {
    let ldt_val = prefilled_lead_time.map(|d| d.to_string()).unwrap_or_default();
    let price_val = unit_price.map(|p| p.to_string()).unwrap_or_default();
    let show_record_btn = unit_price.is_none();
 html! {
    tr {
        td class="text-muted text-xs text-center" {}
        td class="font-mono tabular-nums" { (product.product_code) }
        td { (product.pdt_name) }
        td {
            div class="flex items-center gap-1 justify-end" {
                input
                    class="w-[110px] text-right text-[13px] font-mono rounded-sm px-2 py-[5px] border border-border bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="number"
                    step="any"
                    placeholder="0.00"
                    name="item_unit_price"
                    value=(price_val) {};
                @if show_record_btn {
                    button
                        type="button"
                        title="无匹配价格 — 点击补录到供应商价格目录"
                        class="shrink-0 w-[24px] h-[24px] grid place-items-center rounded-sm text-accent hover:bg-accent-bg border border-border"
                        data-product-id=(product.product_id)
                        hx-get=(PQPriceRecordPath::PATH)
                        hx-target="#price-modal-pq"
                        hx-swap="innerHTML"
                        hx-vals="js:{ supplier_id: document.querySelector('#pq-app select[name=supplier_id]').value, product_id: this.dataset.productId, price: this.closest('tr').querySelector('[name=item_unit_price]').value, lead_time_days: this.closest('tr').querySelector('[name=item_lead_time_days]').value }"
                    { (icon::currency_icon("w-3.5 h-3.5")) }
                }
            }
        }
        td {
            input
                class="w-[100px] text-right text-[13px] font-mono rounded-sm px-2 py-[5px] border border-border bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                type="number"
                step="1" min="0"
                placeholder="—"
                name="item_lead_time_days"
                value=(ldt_val) {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove closest <tr/> then call pqRefresh()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="item_product_id" value=(product.product_id) {}
    }
}
}

// ── 补录价格（无匹配供应商价格时的快捷入口）──

#[derive(Debug, Deserialize)]
pub struct PriceRecordParams {
    pub supplier_id: i64,
    pub product_id: i64,
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub min_order_qty: Option<String>,
    #[serde(default)]
    pub lead_time_days: Option<String>,
    #[serde(default)]
    pub currency_code: Option<String>,
}

/// HTMX: 返回补录价格 modal 表单 fragment（含外层 .modal 容器，整体 innerHTML 进 #price-modal-pq）。
/// 复用 `SupplierPriceService.create_price`（提交到 `SupplierPricesPath`），仅预填字段、不做写操作。
#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn get_pq_price_record_drawer(
    _path: PQPriceRecordPath,
    ctx: RequestContext,
    Query(params): Query<PriceRecordParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let supplier = state
        .supplier_service()
        .get(&service_ctx, &mut conn, params.supplier_id)
        .await?;
    let product = state
        .product_service()
        .get(&service_ctx, &mut conn, params.product_id)
        .await?;
    Ok(Html(
        price_record_modal_form(&supplier, &product, &params).into_string(),
    ))
}

/// 渲染补录价格 modal 表单。
/// 提交端点复用价格目录维护页的 `SupplierPricesPath::PATH`（即 `create_price` handler）。
/// 成功后 hyperscript 调用 `fillPriceRowFromForm(me)` 把新价回填到原报价行。
fn price_record_modal_form(
    supplier: &abt_core::master_data::supplier::model::Supplier,
    product: &abt_core::master_data::product::model::Product,
    params: &PriceRecordParams,
) -> Markup {
    let price_val = params.price.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
    let moq_val = params
        .min_order_qty
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("1");
    let ldt_val = params
        .lead_time_days
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("0");
    let cc_val = params
        .currency_code
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("CNY");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_valid = chrono::Local::now()
        .checked_add_days(chrono::Days::new(365))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    // price_form 的字段命名是 create_price handler 期望的（price/min_order_qty/...）
    html! {
        div class="modal bg-bg rounded-xl w-[560px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="text-base font-semibold text-fg" { "补录供应商价格" }
                button
                    class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                    _="on click remove .is-open from #price-modal-pq"
                    type="button"
                { "×" }
            }
            form
                hx-post=(SupplierPricesPath::PATH)
                hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400 and detail.elt is me] call fillPriceRowFromForm(me) then remove .is-open from #price-modal-pq"
            {
                div class="px-6 py-4 overflow-y-auto flex-1 min-h-0" {
                    // 上下文信息（只读）
                    div class="mb-4 p-3 bg-surface rounded-sm text-xs text-fg-2 flex flex-col gap-1" {
                        div {
                            span class="text-muted" { "供应商: " }
                            span class="font-medium text-fg" { (supplier.name) " (" (supplier.code) ")" }
                        }
                        div {
                            span class="text-muted" { "产品: " }
                            span class="font-medium text-fg" { (product.pdt_name) " (" (product.product_code) ")" }
                        }
                    }
                    input type="hidden" name="supplier_id" value=(supplier.id) {};
                    input type="hidden" name="product_id" value=(product.product_id) {};
                    input type="hidden" name="discount_pct" value="0" {};
                    input type="hidden" name="sequence" value="0" {};
                    input type="hidden" name="is_active" value="on" {};

                    div class="grid grid-cols-2 gap-4" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" {
                                "单价"
                                span class="text-danger" { "*" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="number"
                                step="any"
                                placeholder="0.00"
                                name="price"
                                required
                                value=(price_val) {};
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "币种" }
                            select
                                name="currency_code"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            {
                                @for c in &["CNY", "USD", "EUR"] {
                                    option value=(*c) selected[cc_val == *c] {
                                        (match *c {
                                            "CNY" => "CNY 人民币",
                                            "USD" => "USD 美元",
                                            _ => "EUR 欧元",
                                        })
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "起订量" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="number"
                                step="any"
                                name="min_order_qty"
                                value=(moq_val) {};
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "交货天数" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="number"
                                step="any"
                                name="lead_time_days"
                                value=(ldt_val) {};
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "生效日期" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="date"
                                name="valid_from"
                                value=(today) {};
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "失效日期" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="date"
                                name="valid_until"
                                value=(default_valid) {};
                        }
                    }
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _="on click remove .is-open from #price-modal-pq"
                    { "取消" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { "保存到价格目录" }
                }
            }
        }
    }
}
