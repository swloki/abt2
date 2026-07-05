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
use abt_core::shared::identity::UserService;
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
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──


// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct ItemWeb {
 product_id: String,
 unit_price: String,
 min_order_qty: Option<String>,
 lead_time_days: Option<String>,
 is_preferred: Option<String>,
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
 let user_svc = state.user_service();

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

 let users = user_svc
 .list_users(&service_ctx, &mut conn, 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = pq_create_page(&suppliers.items, &users, PQCreatePath::PATH, "", true);
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

/// HTMX: search products → return HTML fragment

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
 // P2 录入前置比价：取该物料其他活跃报价（compare_by_product 已按单价 ASC 排序、过滤过期）
 let quotes = state
 .purchase_quotation_service()
 .compare(&service_ctx, &mut conn, params.product_id)
 .await
 .unwrap_or_default();
 let supplier_ids: Vec<i64> = quotes.iter().map(|q| q.supplier_id).collect();
 let names: std::collections::HashMap<i64, String> = if supplier_ids.is_empty() {
 std::collections::HashMap::new()
 } else {
 state
 .supplier_service()
 .get_by_ids(&service_ctx, &mut conn, &supplier_ids)
 .await
 .map(|r| r.into_iter().map(|s| (s.id, s.name)).collect())
 .unwrap_or_default()
 };
 Ok(Html(
 item_row_fragment(&product, unit_price, &quotes, &names, params.supplier_id).into_string(),
 ))
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 product_id: i64,
 #[serde(default)]
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
pub async fn do_create_pq(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: PQCreateForm,
) -> Result<i64> {
    let svc = state.purchase_quotation_service();
    let quotation_date = chrono::NaiveDate::parse_from_str(&form.quotation_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效报价日期格式: {e}")))?;
    let valid_from = chrono::NaiveDate::parse_from_str(&form.valid_from, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效生效日期格式: {e}")))?;
    let valid_until = chrono::NaiveDate::parse_from_str(&form.valid_until, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效失效日期格式: {e}")))?;
    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;
    // 行级币种统一继承主表（一份报价一个币种，表头 currency 为唯一来源）
    let main_currency = form.currency.clone().unwrap_or_else(|| "CNY".to_string());
    let items: Vec<CreateQuotationItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| CreateQuotationItemRequest {
            product_id: item.product_id.parse().unwrap_or(0),
            line_no: (idx as i32) + 1,
            unit_price: item
                .unit_price
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            min_order_qty: item.min_order_qty.and_then(|s| s.parse().ok()),
            lead_time_days: item.lead_time_days.and_then(|s| s.parse().ok()),
            currency: main_currency.clone(),
            is_preferred: item.is_preferred.is_some(),
        })
        .collect();
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
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let id = svc.create(service_ctx, &mut tx, create_req, None).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(id)
}

#[require_permission("PURCHASE_QUOTATION", "create")]
pub async fn create_pq(
 _path: PQCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PQCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let id = do_create_pq(&state, &service_ctx, form).await?;
 let redirect = PQDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

pub fn pq_create_page(
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 users: &[abt_core::shared::identity::model::User],
 post_path: &str,
 after_request_hs: &str,
 show_header: bool,
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_valid = chrono::Local::now()
 .checked_add_days(chrono::Days::new(30))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

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

        form id="pq-form" hx-post=(post_path) hx-swap="none" _=(after_request_hs) {
            input type="hidden" id="items-json" name="items_json" value="[]";
            input type="hidden" id="form-action" name="action" value="submit";
            // ── Supplier Selection ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "供应商信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "供应商"
                            span class="text-danger" { "*" }
                        }
                        select
                            name="supplier_id"
                            required
                            hx-get=(PQSupplierContactsPath::PATH)
                            hx-trigger="change"
                            hx-target="#supplier-contact-fields"
                            hx-swap="innerHTML"
                            hx-vals="js:{supplier_id: this.value}"
                        {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" id="supplier-contact-fields" {
                    div class="form-field" {
                        label { "联系人" }
                        input type="text" readonly placeholder="—" class="bg-surface" {}
                    }
                    div class="form-field" {
                        label { "联系电话" }
                        input type="text" readonly placeholder="—" class="bg-surface" {}
                    }
                }
            }
            // ── Quote Info ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "报价信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "报价日期" }
                        input type="date" name="quotation_date" value=(today) readonly {}
                    }
                    div class="form-field" {
                        label {
                            "生效日期"
                            span class="text-danger" { "*" }
                        }
                        input type="date" name="valid_from" id="f-valid-from" value=(today) {}
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
                            value=(default_valid) {}
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" id="pq-currency"
                            _="on change call pqRefresh()" {
                            option value="CNY" selected { "CNY (人民币)" }
                            option value="USD" { "USD (美元)" }
                            option value="EUR" { "EUR (欧元)" }
                        }
                    }
                    div class="form-field" {
                        label { "采购员" }
                        select name="buyer_id" {
                            option value="" { "请选择采购员" }
                            @for u in users {
                                @if u.is_active {
                                    option value=(u.user_id) {
                                        (u.display_name.as_deref().unwrap_or(&u.username))
                                    }
                                }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "供应商报价单号" }
                        input type="text" name="supplier_quotation_no"
                            placeholder="供应商自带单号（选填）"
                            maxlength="64" {}
                    }
                }
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
                    table class="data-table min-w-[900px]" {
                        thead {
                            tr {
                                th class="w-9 text-center" { "#" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th class="w-[120px] text-right" { "单价" }
                                th class="w-[100px] text-right" { "最小订购量" }
                                th class="w-[90px] text-right" { "交货天数" }
                                th class="w-[80px] text-center" { "币种" }
                                th class="text-center w-14" { "首选" }
                                th class="w-9" {}
                            }
                        }
                        tbody id="pq-item-tbody"
                            _="on input call pqRefresh()\non 'htmx:afterSettle' call pqRefresh()" {}
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
 // 有效期校验
 var vf = document.querySelector('#f-valid-from').value;
 var vu = document.querySelector('#f-valid-until').value;
 var today = new Date().toISOString().slice(0,10);
 if (!vf || !vu) { show_error_toast('请填写生效日期和失效日期'); return; }
 if (vu <= vf) { show_error_toast('失效日期必须晚于生效日期'); return; }
 if (vu < today) { show_error_toast('失效日期不能早于今天'); return; }
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
 document.querySelector('#items-json').value = JSON.stringify(items);
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
    quotes: &[QuotationComparison],
    supplier_names: &std::collections::HashMap<i64, String>,
    current_supplier_id: Option<i64>,
) -> Markup {
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
                        hx-vals="js:{ supplier_id: document.querySelector('#pq-app select[name=supplier_id]').value, product_id: this.dataset.productId, price: this.closest('tr').querySelector('[name=item_unit_price]').value, min_order_qty: this.closest('tr').querySelector('[name=item_min_order_qty]').value, lead_time_days: this.closest('tr').querySelector('[name=item_lead_time_days]').value, currency_code: this.closest('tr').querySelector('[name=item_currency]').value }"
                    { (icon::currency_icon("w-3.5 h-3.5")) }
                }
            }
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input w-[90px] text-right text-[13px] font-mono rounded-sm px-2 py-[5px] border border-border"
                type="number"
                step="any"
                placeholder="—"
                name="item_min_order_qty" {}
        }
        td {
            input
                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input w-[80px] text-right text-[13px] font-mono rounded-sm px-2 py-[5px] border border-border"
                type="number"
                step="any"
                placeholder="—"
                name="item_lead_time_days" {}
        }
        td {
            input
                class="w-full px-2 py-[5px] border border-border rounded-sm text-center text-[13px] bg-surface text-muted"
                type="text"
                style="width:70px"
                name="item_currency"
                value="CNY"
                readonly
                title="币种继承自表头" {}
        }
        td class="text-center" {
            input
                type="checkbox"
                name="item_is_preferred"
                class="cursor-pointer"
                style="width:16px;height:16px;accent-color:var(--primary)" {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center"
                title="删除行"
                _="on click remove next <tr.pq-compare-row/> then remove closest <tr/> then call pqRefresh()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="item_product_id" value=(product.product_id) {}
    }
    // P2 比价子行：class=pq-compare-row，不参与 collect / stats。始终渲染（无报价时提示「暂无」），
    // 保证删除按钮的 `remove next <tr.pq-compare-row/>` 始终命中、结构稳定。
    (compare_row_fragment(quotes, supplier_names, current_supplier_id))
}
}

/// 比价子行：展示该物料其他活跃报价（compare_by_product 已按单价 ASC 排序、过滤过期）。
/// 第一条为最低价，标绿 + ✓；当前供应商已有的报价标蓝「本供应商」。
fn compare_row_fragment(
    quotes: &[QuotationComparison],
    supplier_names: &std::collections::HashMap<i64, String>,
    current_supplier_id: Option<i64>,
) -> Markup {
    let count = quotes.len();
 html! {
    tr class="pq-compare-row" {
        td colspan="9" class="bg-surface px-4 py-1.5" {
            div class="flex flex-wrap items-center gap-x-4 gap-y-0.5 text-xs" {
                span class="font-medium text-fg-2 mr-1" {
                    @if count == 0 {
                        "暂无其他活跃报价"
                    } @else {
                        (format!("活跃报价 {} 家：", count))
                    }
                }
                @for (i, q) in quotes.iter().take(5).enumerate() {
                    (compare_chip(i, q, supplier_names, current_supplier_id))
                }
                @if count > 5 {
                    span class="text-muted" { (format!("… 共 {} 家", count)) }
                }
            }
        }
    }
}
}

fn compare_chip(
    i: usize,
    q: &QuotationComparison,
    supplier_names: &std::collections::HashMap<i64, String>,
    current_supplier_id: Option<i64>,
) -> Markup {
    let name = supplier_names
        .get(&q.supplier_id)
        .map(|s| s.as_str())
        .unwrap_or("未知供应商");
    let is_lowest = i == 0;
    let is_self = current_supplier_id == Some(q.supplier_id);
    let cls = if is_lowest { "text-success font-semibold" } else { "text-muted" };
 html! {
    span class=(cls) {
        (name) " "
        (q.unit_price) " "
        (q.currency)
        @if is_lowest { " ✓" }
        @if is_self { span class="text-accent" { " · 本供应商" } }
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
