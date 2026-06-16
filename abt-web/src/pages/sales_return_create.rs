use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::{SalesOrderQuery, SalesOrderStatus};
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::model::{
    CreateReturnItemReq, CreateReturnReq, ReturnDisposition,
};
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::shipping_request::model::ShippingQuery;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::sales_return::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn order_status_text(s: SalesOrderStatus) -> &'static str {
    match s {
        SalesOrderStatus::Draft => "草稿",
        SalesOrderStatus::Confirmed => "已确认",
        SalesOrderStatus::PartiallyShipped => "部分发货",
        SalesOrderStatus::Shipped => "已发货",
        SalesOrderStatus::Completed => "已完成",
        SalesOrderStatus::Cancelled => "已取消",
    }
}

// ── Form & Query Structs ──

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReturnCreateForm {
    pub order_id: i64,
    pub shipping_request_id: i64,
    pub customer_id: i64,
    pub return_reason: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReturnItemWeb {
    order_item_id: i64,
    product_id: i64,
    returned_qty: String,
    disposition: i16,
}

#[derive(Debug, Deserialize)]
pub struct OrderSearchQuery {
    pub customer_id: Option<i64>,
    pub keyword: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "create")]
pub async fn get_return_create(
    _path: ReturnCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let customer_svc = state.customer_service();
    let customers = customer_svc
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

    let content = return_create_page(&customers.items);
    let page_html = admin_page(
        is_htmx,
        "新建退货单",
        &claims,
        "sales",
        ReturnCreatePath::PATH,
        "销售管理",
        Some("新建退货单"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search orders -> returns HTML fragment with embedded JSON data
#[require_permission("SALES_ORDER", "read")]
pub async fn get_orders(
    ctx: RequestContext,
    Query(params): Query<OrderSearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let customer_id = match params.customer_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(order_search_empty().into_string())),
    };

    // 1. Fetch orders via SalesOrderService::list
    let order_svc = state.sales_order_service();
    let keyword = params.keyword.as_deref().and_then(|k| {
        if k.is_empty() {
            None
        } else {
            Some(k.to_string())
        }
    });
    let orders_result = order_svc
        .list(
            &service_ctx,
            &mut conn,
            SalesOrderQuery {
                customer_id: Some(customer_id),
                keyword,
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;

    // Filter to only active statuses (Confirmed, PartiallyShipped, Shipped)
    let active_statuses = [
        SalesOrderStatus::Confirmed,
        SalesOrderStatus::PartiallyShipped,
        SalesOrderStatus::Shipped,
    ];
    let orders: Vec<_> = orders_result
        .items
        .into_iter()
        .filter(|o| active_statuses.contains(&o.status))
        .collect();

    if orders.is_empty() {
        return Ok(Html(order_search_empty().into_string()));
    }

    let order_ids: Vec<i64> = orders.iter().map(|o| o.id).collect();

    // 2. Fetch order items for each order via SalesOrderService::list_items
    let mut items_map: std::collections::HashMap<i64, Vec<abt_core::sales::sales_order::model::SalesOrderItem>> =
        std::collections::HashMap::new();
    for &oid in &order_ids {
        let items = order_svc
            .list_items(&service_ctx, &mut conn, oid)
            .await?;
        items_map.insert(oid, items);
    }

    // 3. Collect all unique product IDs and batch-fetch product info
    let all_product_ids: Vec<i64> = items_map
        .values()
        .flat_map(|items| items.iter().map(|i| i.product_id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let product_svc = state.product_service();
    let products = if all_product_ids.is_empty() {
        vec![]
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, all_product_ids)
            .await?
    };
    let product_map: std::collections::HashMap<i64, &abt_core::master_data::product::model::Product> =
        products.iter().map(|p| (p.product_id, p)).collect();

    // 4. Resolve shipping IDs for these orders (latest per order)
    let shipping_svc = state.shipping_service();
    let mut shipping_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &oid in &order_ids {
        let shippings = shipping_svc
            .list(
                &service_ctx,
                &mut conn,
                ShippingQuery {
                    order_id: Some(oid),
                    ..Default::default()
                },
                PageParams::new(1, 100),
            )
            .await?;
        // Take the latest shipping (highest ID) as the original DISTINCT ON logic did
        if let Some(latest) = shippings.items.iter().max_by_key(|s| s.id) {
            shipping_map.insert(oid, latest.id);
        }
    }

    Ok(Html(
        order_search_results(&orders, &items_map, &product_map, &shipping_map).into_string(),
    ))
}

/// POST: create return from form submission
#[require_permission("SALES_ORDER", "create")]
pub async fn create_return(
    _path: ReturnCreatePath,
    ctx: RequestContext,
    Form(form): Form<ReturnCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { claims: _, mut conn, state, service_ctx, .. } = ctx;

    if form.customer_id == 0 {
        return Err(DomainError::validation("请选择客户").into());
    }
    if form.order_id == 0 {
        return Err(DomainError::validation("请选择来源订单").into());
    }
    if form.shipping_request_id == 0 {
        return Err(DomainError::validation("该订单没有发货记录，无法创建退货。请先完成发货后再申请退货").into());
    }

    let web_items: Vec<ReturnItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效退货明细数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个退货产品").into());
    }

    // Build CreateReturnReq for the service
    let items: Vec<CreateReturnItemReq> = web_items
        .into_iter()
        .map(|item| {
            let qty: rust_decimal::Decimal = item
                .returned_qty
                .parse()
                .unwrap_or(rust_decimal::Decimal::ONE);
            let disposition = ReturnDisposition::from_i16(item.disposition)
                .unwrap_or(ReturnDisposition::Restock);
            CreateReturnItemReq {
                order_item_id: item.order_item_id,
                returned_qty: qty,
                disposition,
            }
        })
        .collect();

    let req = CreateReturnReq {
        order_id: form.order_id,
        shipping_request_id: form.shipping_request_id,
        customer_id: form.customer_id,
        return_reason: form.return_reason,
        items,
    };

    let svc = state.sales_return_service();
    let return_id = svc.create(&service_ctx, &mut conn, req).await?;

    let redirect = ReturnDetailPath { id: return_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn return_create_page(customers: &[abt_core::master_data::customer::model::Customer]) -> Markup {
    let customers_json = serde_json::to_string(&customers.iter().map(|c| serde_json::json!({"id":c.id,"name":c.name})).collect::<Vec<_>>()).unwrap_or_default();

    html! {
        div id="return-app" class="padded-section" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReturnListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回退货列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建退货单" }
                div class="flex gap-3" {
                    span class="loading-placeholder" {
                        (icon::clock_icon("w-3.5 h-3.5"))
                        "自动保存草稿"
                    }
                }
            }

            form id="return-form"
                  hx-post=(ReturnCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="items_json";
                input type="hidden" name="customer_id" id="f-customer-id";
                input type="hidden" name="order_id" id="f-order-id";
                input type="hidden" name="shipping_request_id" id="f-shipping-id";
                input type="hidden" name="return_reason" id="f-reason";

                // ── 关联单据 ──
                div class="form-section-card" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::clipboard_document_icon("w-[18px] h-[18px]"))
                        "关联单据"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "客户 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="customer-select" {
                                option value="" { "请选择客户" }
                                @for c in customers {
                                    option value=(c.id) { (c.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源订单 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="order-select" disabled {
                                option value="" { "请先选择客户" }
                            }
                        }
                        div class="form-field span-2" {
                            div class="linked-info-bar hidden-initial" id="linked-info" {
                                span { span class="label" { "客户：" } span id="li-customer" { "—" } }
                                span { span class="label" { "订单金额：" } span id="li-amount" { "—" } }
                                span { span class="label" { "订单日期：" } span id="li-date" { "—" } }
                            }
                        }
                    }
                }

                // ── 退货信息 ──
                div class="form-section-card" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::clipboard_document_icon("w-[18px] h-[18px]"))
                        "退货信息"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "退货原因 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="reason-select" {
                                option value="" { "请选择退货原因" }
                                option value="quality" { "质量缺陷" }
                                option value="wrong_spec" { "规格不符" }
                                option value="excess" { "数量多余" }
                                option value="damage" { "运输损坏" }
                                option value="cancel" { "客户取消" }
                                option value="other" { "其他原因" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "处理方式" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="disposition-select" {
                                option value="restock" { "退回入库" }
                                option value="replace" { "换货处理" }
                                option value="scrap" { "报废处理" }
                            }
                        }
                    }
                }

                // ── 退货产品明细 ──
                div class="form-section-card flush hidden-initial" id="items-section" {
                    div class="flush-header" {
                        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                            (icon::package_icon("w-[18px] h-[18px]"))
                            "退货产品明细"
                        }
                    }
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                        table class="line-items-table" {
                            thead {
                                tr {
                                    th class="col-num" { "#" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th class="col-unit" { "单位" }
                                    th class="col-price" { "原单价 (¥)" }
                                    th class="col-qty" { "已发数量" }
                                    th class="col-qty" { "退货数量 " span class="required" { "*" } }
                                    th class="col-subtotal" { "退货金额 (¥)" }
                                    th class="col-action" { }
                                }
                            }
                            tbody id="line-items-body" {
                                // Populated by JS when order is selected
                            }
                        }
                    }
                    div class="add-row-bar" {
                        button type="button" class="btn-add-row" onclick="addReturnRow()" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加产品行"
                        }
                    }
                    div class="totals-bar" {
                        div class="totals-item" {
                            span class="totals-label" { "退货总数量" }
                            span class="totals-value" id="total-qty" { "0" }
                        }
                        div class="totals-item" {
                            span class="totals-label" { "退货总额" }
                            span class="totals-value grand" id="grand-total" { "¥ 0.00" }
                        }
                    }
                }

                // ── 备注 ──
                div class="form-section-card" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::file_text_icon("w-[18px] h-[18px]"))
                        "备注"
                    }
                    textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5" name="remark" placeholder="输入退货相关备注，如质量问题详细描述、客户诉求、处理要求等…" {}
                }

                // ── 附件 ──
                div class="form-section-card" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::upload_icon("w-[18px] h-[18px]"))
                        "附件"
                    }
                    div class="upload-area" id="upload-area" {
                        (icon::upload_icon("w-8 h-8"))
                        p class="upload-title" { "点击或拖拽文件到此处上传" }
                        p class="upload-hint" { "支持 PDF、Word、Excel、图片，单个文件不超过 10MB" }
                        input type="file" id="file-input" multiple name="attachments" class="hidden-initial" {}
                    }
                    div id="file-list" class="mt-3" {}
                }
            }

            // ── Action Bar ──
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn btn-default" href=(format!("{}?restore=true", ReturnListPath::PATH)) { "取消" }
                div class="flex gap-3" {
                    button type="button" class="btn btn-default" onclick="handleSaveDraft()" {
                        (icon::save_icon("w-4 h-4"))
                        "保存草稿"
                    }
                    button type="button" class="btn btn-primary" _="on click call handleSubmit() then if it trigger submit on #return-form" {
                        (icon::send_icon("w-4 h-4"))
                        "提交退货"
                    }
                }
            }

            // ── Inline JS ──
            (maud::PreEscaped(format!(r#"<script>
(function(){{
const customersJson = {customers_json};
let selectedOrder = null;
let orderCache = {{}};

// Customer select → load orders
document.getElementById('customer-select').addEventListener('change', function() {{
    const cid = this.value;
    document.getElementById('f-customer-id').value = cid;
    const oSel = document.getElementById('order-select');
    oSel.innerHTML = '<option value="">加载中...</option>';
    oSel.disabled = true;
    if (!cid) {{ oSel.innerHTML = '<option value="">请先选择客户</option>'; return; }}
    fetch('{orders_path}?customer_id=' + cid)
        .then(r => r.text())
        .then(html => {{
            const div = document.createElement('div');
            div.innerHTML = html;
            const items = div.querySelectorAll('.product-select-item');
            oSel.innerHTML = '<option value="">请选择销售订单</option>';
            items.forEach(el => {{
                const btn = el.querySelector('[data-order]');
                if (btn) {{
                    const data = JSON.parse(btn.dataset.order);
                    const opt = document.createElement('option');
                    opt.value = data.id;
                    opt.textContent = data.doc_number;
                    opt.dataset.order = btn.dataset.order;
                    oSel.appendChild(opt);
                    orderCache[data.id] = data;
                }}
            }});
            oSel.disabled = false;
            if (items.length === 0) oSel.innerHTML = '<option value="">无可用订单</option>';
        }});
}}),

// Order select → show info + populate items
document.getElementById('order-select').addEventListener('change', function() {{
    const opt = this.options[this.selectedIndex];
    const info = document.getElementById('linked-info');
    const section = document.getElementById('items-section');
    if (!opt || !opt.value) {{
        info.classList.add('hidden-initial');
        section.classList.add('hidden-initial');
        document.getElementById('f-order-id').value = '';
        return;
    }}
    const data = JSON.parse(opt.dataset.order);
    selectedOrder = data;
    document.getElementById('f-order-id').value = data.id;
    document.getElementById('f-shipping-id').value = data.shipping_id || 0;
    document.getElementById('li-customer').textContent = document.getElementById('customer-select').selectedOptions[0].text;
    document.getElementById('li-amount').textContent = '¥ ' + data.items.reduce((s,i) => s + parseFloat(i.order_qty||0) * parseFloat(i.unit_price||0), 0).toLocaleString('zh-CN', {{minimumFractionDigits:2}});
    document.getElementById('li-date').textContent = data.doc_number;
    info.classList.remove('hidden-initial');
    section.classList.remove('hidden-initial');
    populateItems(data.items);
}});

function populateItems(items) {{
    const tbody = document.getElementById('line-items-body');
    tbody.innerHTML = '';
    items.forEach((item, i) => {{
        tbody.appendChild(createItemRow(i+1, item));
    }});
    recalcTotals();
}}

function createItemRow(num, item) {{
    const tr = document.createElement('tr');
    tr.innerHTML = `
        <td class="line-num">${{num}}</td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] input-readonly-bg" type="text" value="${{item.product_code||''}}" readonly tabindex="-1"></td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] input-readonly-bg" type="text" value="${{item.product_name||''}}" readonly tabindex="-1"></td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] input-readonly-bg-center" type="text" value="${{item.unit||''}}" readonly tabindex="-1"></td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input input-readonly-bg" type="text" value="${{parseFloat(item.unit_price||0).toLocaleString('zh-CN',{{minimumFractionDigits:2}})}}" readonly tabindex="-1"></td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input input-readonly-bg" type="text" value="${{item.order_qty||''}}" readonly tabindex="-1"></td>
        <td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" data-field="qty" data-order-item-id="${{item.order_item_id}}" data-price="${{item.unit_price||0}}" value="" placeholder="0" min="1" oninput="calcRow(this)"></td>
        <td class="line-total" data-field="subtotal">—</td>
        <td><button type="button" class="btn-remove-row" onclick="removeRow(this)" title="删除行">{chevron}</button></td>
    `;
    return tr;
}}

function calcRow(input) {{
    const row = input.closest('tr');
    const qty = parseFloat(input.value) || 0;
    const price = parseFloat(input.dataset.price) || 0;
    const subtotal = qty * price;
    row.querySelector('[data-field="subtotal"]').textContent = subtotal > 0 ? '¥ ' + subtotal.toLocaleString('zh-CN', {{minimumFractionDigits:2}}) : '—';
    const shipped = parseFloat(row.querySelectorAll('.form-input')[4].value) || 0;
    if (qty > shipped && shipped > 0) {{
        input.style.borderColor = 'var(--danger)';
        input.style.boxShadow = '0 0 0 2px color-mix(in srgb, var(--danger) 12%, transparent)';
    }} else {{
        input.style.borderColor = '';
        input.style.boxShadow = '';
    }}
    recalcTotals();
}}

function recalcTotals() {{
    let total = 0, totalQty = 0;
    document.querySelectorAll('#line-items-body tr').forEach(r => {{
        const qty = parseFloat(r.querySelector('[data-field="qty"]')?.value) || 0;
        const price = parseFloat(r.querySelector('[data-field="qty"]')?.dataset.price) || 0;
        total += qty * price;
        totalQty += qty;
    }});
    document.getElementById('total-qty').textContent = totalQty;
    document.getElementById('grand-total').textContent = '¥ ' + total.toLocaleString('zh-CN', {{minimumFractionDigits:2}});
}}

function addReturnRow() {{
    if (!selectedOrder || !selectedOrder.items) return;
    const tbody = document.getElementById('line-items-body');
    const num = tbody.rows.length + 1;
    tbody.appendChild(createItemRow(num, {{order_item_id:0, product_code:'', product_name:'', unit:'', unit_price:'0', order_qty:''}}));
}}

function removeRow(btn) {{
    const tbody = document.getElementById('line-items-body');
    if (tbody.rows.length <= 1) return;
    btn.closest('tr').remove();
    Array.from(tbody.rows).forEach((r,i) => r.querySelector('.line-num').textContent = i+1);
    recalcTotals();
}}

function handleSubmit() {{
    const order = document.getElementById('f-order-id').value;
    const reason = document.getElementById('reason-select').value;
    if (!order) {{ show_error_toast('请选择来源订单'); return false; }}
    if (!reason) {{ show_error_toast('请选择退货原因'); return false; }}
    const rows = document.querySelectorAll('#line-items-body tr');
    const items = [];
    let hasQty = false;
    rows.forEach(r => {{
        const qtyInput = r.querySelector('[data-field="qty"]');
        const qty = parseFloat(qtyInput?.value) || 0;
        if (qty > 0) {{
            hasQty = true;
            items.push({{
                order_item_id: parseInt(qtyInput.dataset.orderItemId),
                product_id: 0,
                returned_qty: qty.toString(),
                disposition: parseInt(document.getElementById('disposition-select').value === 'restock' ? 1 : document.getElementById('disposition-select').value === 'replace' ? 2 : 3)
            }});
        }}
    }});
    if (!hasQty) {{ show_error_toast('请至少填写一行退货数量'); return false; }}
    document.getElementById('f-reason').value = document.getElementById('reason-select').selectedOptions[0].text;
    document.querySelector('[name="items_json"]').value = JSON.stringify(items);
    return true;
}}

function handleSaveDraft() {{
    show_info_toast('草稿功能开发中');
}}

// Expose to global scope for inline event handlers
window.calcRow = calcRow;
window.recalcTotals = recalcTotals;
window.addReturnRow = addReturnRow;
window.removeRow = removeRow;
window.handleSubmit = handleSubmit;
window.handleSaveDraft = handleSaveDraft;

// File upload
(function(){{
    const area = document.getElementById('upload-area');
    const input = document.getElementById('file-input');
    const list = document.getElementById('file-list');
    if (!area || !input) return;
    area.addEventListener('click', () => input.click());
    area.addEventListener('dragover', e => {{ e.preventDefault(); area.style.borderColor = 'var(--accent)'; }});
    area.addEventListener('dragleave', () => {{ area.style.borderColor = 'var(--border)'; }});
    area.addEventListener('drop', e => {{ e.preventDefault(); area.style.borderColor = 'var(--border)'; input.files = e.dataTransfer.files; onFiles(); }});
    input.addEventListener('change', onFiles);
    function onFiles() {{
        list.innerHTML = '';
        Array.from(input.files).forEach(f => {{
            const d = document.createElement('div');
            d.className = 'file-item';
            d.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--muted)" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg><span class="file-item-name">'+f.name+'</span><span class="file-item-size">'+(f.size/1024).toFixed(0)+' KB</span>';
            list.appendChild(d);
        }});
    }}
}})();
}})();
</script>"#, customers_json = customers_json, orders_path = ReturnOrdersPath::PATH, chevron = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>"#)))
        }
    }
}

fn order_search_results(
    orders: &[abt_core::sales::sales_order::model::SalesOrder],
    items_map: &std::collections::HashMap<i64, Vec<abt_core::sales::sales_order::model::SalesOrderItem>>,
    product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
    shipping_map: &std::collections::HashMap<i64, i64>,
) -> Markup {
    html! {
        div class="product-select-list" {
            @for order in orders {
                @let status_text = order_status_text(order.status);
                @let order_date = order.order_date.format("%Y-%m-%d").to_string();
                @let total = order.total_amount.to_string();
                @let shipping_id = shipping_map.get(&order.id).copied().unwrap_or(0);
                @let items_json = serde_json::json!({
                    "id": order.id,
                    "doc_number": &order.doc_number,
                    "shipping_id": shipping_id,
                    "items": items_map.get(&order.id).map(|items| items.iter().map(|item| {
                        let product = product_map.get(&item.product_id);
                        serde_json::json!({
                            "order_item_id": item.id,
                            "product_id": item.product_id,
                            "product_code": product.map(|p| p.product_code.as_str()).unwrap_or(""),
                            "product_name": product.map(|p| p.pdt_name.as_str()).unwrap_or_else(|| item.description.as_str()),
                            "unit": product.map(|p| p.unit.as_str()).unwrap_or(""),
                            "order_qty": item.quantity.to_string(),
                            "unit_price": item.unit_price.to_string(),
                        })
                    }).collect::<Vec<_>>()).unwrap_or_default()
                }).to_string();

                div class="product-select-item" {
                    div class="product-select-info" {
                        div class="product-select-name" { (order.doc_number) }
                        div class="product-select-meta" {
                            span { (order_date) }
                            span class="product-select-sep" { "·" }
                            span { (status_text) }
                            span class="product-select-sep" { "·" }
                            span { "¥" (total) }
                        }
                    }
                    button type="button" class="btn btn-sm btn-primary"
                        data-order=(items_json)
                        onclick="selectOrder(JSON.parse(this.dataset.order))" {
                        "选择"
                    }
                }
            }
        }
    }
}

fn order_search_empty() -> Markup {
    html! {
        div class="loading-placeholder" {
            (icon::package_icon("w-8 h-8"))
            p class="mt-2 text-sm" { "请先选择客户，或未找到匹配的订单" }
        }
    }
}
