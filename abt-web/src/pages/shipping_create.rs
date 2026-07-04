use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::*;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::wms::picking::model::*;
use abt_core::wms::picking::PickingService;
use abt_core::wms::enums::PickingStatus;
use abt_core::shared::types::PageParams;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::components::overlay::modal_shell;
use crate::errors::Result;
use abt_core::shared::types::DomainError;
use crate::layout::page::admin_page;
use crate::routes::shipping::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

fn order_status_text(s: SalesOrderStatus) -> &'static str {
 match s {
 SalesOrderStatus::Draft => "草稿",
 SalesOrderStatus::Confirmed => "已确认",
 SalesOrderStatus::ReadyToShip => "待发货",
 SalesOrderStatus::PartiallyShipped => "部分发货",
 SalesOrderStatus::Shipped => "已发货",
 SalesOrderStatus::Completed => "已完成",
 SalesOrderStatus::Cancelled => "已取消",
 SalesOrderStatus::ShippingRequested => "已申请发货",
 }
}

// ── Data Structs ──

#[derive(Debug)]
struct OrderItemRow {
    order_item_id: i64,
    product_code: String,
    product_name: String,
    specification: Option<String>,
    unit: Option<String>,
    ordered_qty: rust_decimal::Decimal,
    shipped_qty: rust_decimal::Decimal,
}

/// 从订单明细 + 产品构造聚合行（get_order_items 端点用）
fn order_item_row(
    item: &SalesOrderItem,
    product: Option<&abt_core::master_data::product::model::Product>,
) -> OrderItemRow {
    OrderItemRow {
        order_item_id: item.id,
        product_code: product.map(|p| p.product_code.clone()).unwrap_or_default(),
        product_name: product.map(|p| p.pdt_name.clone()).unwrap_or_default(),
        specification: product.map(|p| Some(p.meta.specification.clone())).unwrap_or(None),
        unit: product.map(|p| Some(p.unit.clone())).unwrap_or(None),
        ordered_qty: item.quantity,
        shipped_qty: item.shipped_qty,
    }
}

/// 订单详情页「创建发货申请」带入的预填数据：customer_id + order_id。
/// 明细由前端 HTMX 调 get_order_items 端点加载（避免 SSR 阶段重复拉数据 + 拼 JSON）。
#[derive(Default)]
pub struct ShippingPrefill {
    pub customer_id: Option<i64>,
    pub order_id: Option<i64>,
}

// ── Form & Query Structs ──

#[derive(Debug, Deserialize)]
pub struct ShippingCreateForm {
 pub customer_id: i64,
 pub order_id: i64,
 pub expected_ship_date: Option<String>,
 pub shipping_address: Option<String>,
 pub carrier: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
}

/// 草稿保存表单（宽松校验）
#[derive(Debug, Deserialize)]
pub struct ShippingDraftForm {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub draft_id: Option<i64>,
 pub customer_id: i64,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub order_id: Option<i64>,
 pub expected_ship_date: Option<String>,
 pub shipping_address: Option<String>,
 pub carrier: Option<String>,
 pub remark: Option<String>,
 pub items_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShippingItemWeb {
 order_item_id: i64,
 warehouse_id: i64,
 requested_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct CustomerContactsQuery {
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub customer_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OrderSearchQuery {
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub customer_id: Option<i64>,
 pub keyword: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ShippingCreateQuery {
 #[serde(default)]
 pub order_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("SHIPPING", "create")]
pub async fn get_shipping_create(
 _path: ShippingCreatePath,
 Query(q): Query<ShippingCreateQuery>,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let customer_svc = state.customer_service();
 let warehouse_svc = state.warehouse_service();
 let order_svc = state.sales_order_service();
 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
 .await?;

 // 从订单详情页「创建发货申请」带入：仅传 customer_id + order_id；明细由前端 HTMX 加载（对齐 stock_in 范式）
 let prefill = if let Some(oid) = q.order_id.filter(|&id| id > 0) {
 match order_svc.find_by_id(&service_ctx, &mut conn, oid).await {
 Ok(order) => ShippingPrefill {
 customer_id: Some(order.customer_id),
 order_id: Some(oid),
 },
 Err(_) => ShippingPrefill::default(),
 }
 } else {
 ShippingPrefill::default()
 };

 let content = shipping_create_page(&customers.items, &warehouses.items, &prefill, ShippingCreatePath::PATH, "", true);
 let page_html = admin_page(
 is_htmx, "新建发货申请", &claims, "sales",
 ShippingCreatePath::PATH, "销售管理", Some("新建发货申请"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

/// 提取的业务逻辑（tx + create_from_order），供独立页 POST 与作业中心 drawer POST 共用。返回新建发货单 id。
pub async fn do_create_shipping(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    form: ShippingCreateForm,
) -> Result<i64> {
    let svc = state.picking_service();
    if form.customer_id == 0 {
        return Err(DomainError::validation("请选择客户").into());
    }
    if form.order_id == 0 {
        return Err(DomainError::validation("请选择来源订单").into());
    }
    let web_items: Vec<ShippingItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效产品数据: {e}")))?;
    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个发货产品").into());
    }
    let items: Vec<CreateShippingItemReq> = web_items.into_iter().map(|item| {
        CreateShippingItemReq {
            order_item_id: item.order_item_id,
            warehouse_id: Some(item.warehouse_id),
            requested_qty: item.requested_qty.parse().unwrap_or(rust_decimal::Decimal::ONE),
        }
    }).collect();
    let expected_ship_date = form.expected_ship_date
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let shipping_address = form.shipping_address.filter(|s| !s.is_empty());
    let req = CreateFromOrderReq {
        order_id: form.order_id,
        expected_ship_date,
        shipping_address,
        items,
    };
    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let id = svc.create_from_order(service_ctx, &mut tx, req).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(id)
}

#[require_permission("SHIPPING", "create")]
pub async fn post_shipping_create(
 _path: ShippingCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ShippingCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let id = do_create_shipping(&state, &service_ctx, form).await?;
 let redirect = ShippingDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Edit Draft ──

#[require_permission("SHIPPING", "create")]
pub async fn get_shipping_edit(
 path: ShippingEditPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let shipping_svc = state.picking_service();
 let customer_svc = state.customer_service();
 let warehouse_svc = state.warehouse_service();
 let order_svc = state.sales_order_service();
 let product_svc = state.product_service();

 let draft = shipping_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 if draft.status != PickingStatus::Draft {
 return Err(DomainError::business_rule("仅草稿状态可以编辑").into());
 }

 let draft_items = shipping_svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 // 草稿来源订单：取订单号（页头展示）+ 原订单明细（按 source_item_id 匹配取 ordered/shipped，供 pending 校验）
 let (order_no, order_item_map): (String, HashMap<i64, SalesOrderItem>) = match draft.source_id {
 Some(oid) => {
 let order_no = order_svc.find_by_id(&service_ctx, &mut conn, oid).await
 .ok().map(|o| o.doc_number).unwrap_or_default();
 let map = order_svc.list_items(&service_ctx, &mut conn, oid).await.unwrap_or_default()
 .into_iter().map(|i| (i.id, i)).collect();
 (order_no, map)
 }
 None => (String::new(), HashMap::new()),
 };

 let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = product_svc
 .get_by_ids(&service_ctx, &mut conn, draft_items.iter().map(|i| i.product_id).collect())
 .await.unwrap_or_default()
 .into_iter().map(|p| (p.product_id, p)).collect();

 // 草稿明细 → OrderItemRow（复用 shipping_item_row 渲染；product 取自 product_map，ordered/shipped 匹配原订单）
 let rows: Vec<OrderItemRow> = draft_items.iter().map(|item| {
 let product = product_map.get(&item.product_id);
 let order_item = item.source_item_id.and_then(|sid| order_item_map.get(&sid));
 OrderItemRow {
 order_item_id: item.source_item_id.unwrap_or(0),
 product_code: product.map(|p| p.product_code.clone()).unwrap_or_default(),
 product_name: product.map(|p| p.pdt_name.clone()).unwrap_or_default(),
 specification: product.map(|p| Some(p.meta.specification.clone())).unwrap_or(None),
 unit: product.map(|p| Some(p.unit.clone())).unwrap_or(None),
 ordered_qty: order_item.map(|oi| oi.quantity).unwrap_or(rust_decimal::Decimal::ZERO),
 shipped_qty: order_item.map(|oi| oi.shipped_qty).unwrap_or(rust_decimal::Decimal::ZERO),
 }
 }).collect();

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
 .await?;

 let content = shipping_edit_page(&draft, &order_no, &rows, &customers.items, &warehouses.items, true);
 let page_html = admin_page(
 is_htmx, "编辑发货申请", &claims, "sales",
 ShippingEditPath { id: path.id }.to_string().as_str(), "销售管理", Some("编辑发货申请"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

// ── Save / Update Draft ──

#[require_permission("SHIPPING", "create")]
pub async fn post_save_draft(
 _path: ShippingSaveDraftPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ShippingDraftForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.picking_service();

 let expected_ship_date = form.expected_ship_date
 .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

 let items: Vec<CreateDraftItemReq> = form.items_json
 .and_then(|json| serde_json::from_str::<Vec<ShippingItemWeb>>(&json).ok())
 .unwrap_or_default()
 .into_iter()
 .map(|item| CreateDraftItemReq {
 order_item_id: if item.order_item_id > 0 { Some(item.order_item_id) } else { None },
 product_id: None,
 warehouse_id: Some(item.warehouse_id),
 requested_qty: item.requested_qty.parse().unwrap_or(rust_decimal::Decimal::ONE),
 description: String::new(),
 })
 .collect();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let id = if let Some(draft_id) = form.draft_id {
 // 更新已有草稿
 svc.update_draft(&service_ctx, &mut tx, draft_id, UpdateDraftReq {
 customer_id: Some(form.customer_id),
 order_id: form.order_id,
 expected_ship_date,
 shipping_address: form.shipping_address.filter(|s| !s.is_empty()),
 carrier: form.carrier.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 items: Some(items),
 }).await?;
 draft_id
 } else {
 // 新建草稿
 svc.save_draft(&service_ctx, &mut tx, CreateDraftReq {
 customer_id: form.customer_id,
 order_id: form.order_id,
 expected_ship_date,
 shipping_address: form.shipping_address.filter(|s| !s.is_empty()),
 carrier: form.carrier.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 items,
 }).await?
 };
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// HTMX: 客户选择变化 → 返回带默认收货地址的 #shipping-address input（outerHTML 替换）。
/// 对齐 stock_in「服务端渲染片段 + HTMX swap」范式，取代旧 customer_info_card 全卡替换。
#[require_permission("SHIPPING", "read")]
pub async fn get_customer_contacts(
    ctx: RequestContext,
    Query(params): Query<CustomerContactsQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let customer_svc = state.customer_service();

    let addresses = match params.customer_id {
        Some(cid) if cid > 0 => customer_svc.list_addresses(&service_ctx, &mut conn, cid).await.unwrap_or_default(),
        _ => vec![],
    };
    let default_addr = addresses.iter()
        .find(|a| a.address_type == "shipping")
        .or_else(|| addresses.first());
    let shipping_address = default_addr.map(|a| {
        let mut parts = vec![a.province.clone(), a.city.clone()];
        if let Some(ref d) = a.district { parts.push(d.clone()); }
        parts.push(a.detail.clone());
        parts.join("")
    }).unwrap_or_default();

    Ok(Html(html! {
        input type="text" name="shipping_address" id="shipping-address"
            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
            placeholder="选择客户后自动填充，可修改"
            value=(shipping_address) {}
    }.into_string()))
}

/// HTMX: 按 customer_id + keyword 搜索销售订单。每行「选择」按钮 hx-get=order-items 端点
/// 直接加载明细到 #shipping-items-tbody（替代旧 selectOrder + JS 拼 DOM）。
#[require_permission("SHIPPING", "read")]
pub async fn get_order_search(
    ctx: RequestContext,
    Query(params): Query<OrderSearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let order_svc = state.sales_order_service();

    let customer_id = match params.customer_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(order_search_empty().into_string())),
    };
    let filter = SalesOrderQuery {
        customer_id: Some(customer_id),
        keyword: params.keyword.clone(),
        ..Default::default()
    };
    let result = order_svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 10)).await?;

    if result.items.is_empty() {
        return Ok(Html(order_search_empty().into_string()));
    }
    Ok(Html(order_search_results(&result.items).into_string()))
}

// ── Components ──

/// 草稿编辑页（/admin/wms/shipping/{id}/edit）。
///
/// 范式对齐 shipping_create_page（新范式）：明细 SSR 渲染（复用 shipping_item_row）+
/// form hx-post 原生提交（onsubmit=wmsShippingCollectItems）。客户+订单锁定（草稿基于特定
/// 订单创建，编辑只调发货量/仓库/承运商/备注/日期）。删旧 JS 拼 DOM / htmx.ajax / onclick。
fn shipping_edit_page(
    draft: &StockPicking,
    order_no: &str,
    rows: &[OrderItemRow],
    customers: &[abt_core::master_data::customer::model::Customer],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    show_header: bool,
) -> Markup {
    let draft_id = draft.id;
    let customer_id = draft.partner_id.unwrap_or(0);
    let order_id = draft.source_id.unwrap_or(0);
    let expected_ship_date = draft.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
    let shipping_address = draft.remark.as_str();
    let carrier = "";
    let remark = "";

    html! {
        div id="shipping-app" class="p-6" {
            @if show_header {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", ShippingListPath::PATH))
                { (icon::arrow_left_icon("w-4 h-4")) "返回发货申请列表" }
                div class="flex items-center justify-between mb-6" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "编辑发货申请（草稿）" }
                }
            }

            form id="shipping-form" class="space-y-5"
                hx-post=(ShippingSaveDraftPath::PATH)
                hx-swap="none"
                hx-disabled-elt="#shipping-submit-btn"
                onsubmit="return wmsShippingCollectItems()"
            {
                input type="hidden" name="draft_id" value=(draft_id);
                input type="hidden" name="order_id" value=(order_id);
                input type="hidden" name="customer_id" value=(customer_id);
                input type="hidden" name="items_json" id="shipping-items-json" {};

                // ── 顶部：客户（锁定）+ 来源订单号 + 发货日期 ──
                div class="flex items-center justify-between gap-4 flex-wrap" {
                    div class="flex items-center gap-2 flex-1 min-w-[260px]" {
                        (icon::user_icon("w-[15px] h-[15px] text-muted shrink-0"))
                        select
                            class="flex-1 px-3 py-[7px] border border-border rounded-sm text-[13px] bg-surface text-fg-2 outline-none cursor-not-allowed"
                            id="shipping-customer-select" disabled
                        {
                            @for c in customers {
                                option value=(c.id) selected[customer_id == c.id] { (c.name) }
                            }
                        }
                    }
                    div class="flex items-center gap-2 text-[13px] text-muted" {
                        (icon::file_text_icon("w-[15px] h-[15px] text-muted"))
                        span { "来源订单 " (order_no) }
                    }
                    div class="flex items-center gap-2" {
                        (icon::calendar_icon("w-[15px] h-[15px] text-muted"))
                        input type="date" name="expected_ship_date" id="ship-date"
                            class="w-[140px] px-3 py-[7px] border border-border rounded-sm text-[13px] bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            value=(expected_ship_date) {}
                    }
                }

                // ── 承运商 + 收货地址 + 备注 ──
                div class="grid grid-cols-1 md:grid-cols-2 gap-4" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "承运商" }
                        select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="carrier"
                        {
                            option value="" { "请选择承运商" }
                            option value="顺丰速运" selected[carrier == "顺丰速运"] { "顺丰速运" }
                            option value="德邦物流" selected[carrier == "德邦物流"] { "德邦物流" }
                            option value="中通快运" selected[carrier == "中通快运"] { "中通快运" }
                            option value="京东物流" selected[carrier == "京东物流"] { "京东物流" }
                            option value="自提" selected[carrier == "自提"] { "自提 / 自送" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "收货地址" }
                        input type="text" name="shipping_address" id="shipping-address"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            value=(shipping_address) placeholder="收货地址" {}
                    }
                    div class="form-field md:col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "备注" }
                        textarea name="remark" rows="2"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[48px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            placeholder="输入发货相关备注…"
                        { (remark) }
                    }
                }

                // 分隔线
                div class="h-px bg-border-soft" {}

                // ── 发货明细（SSR 渲染，复用 shipping_item_row）──
                div {
                    div class="flex items-center gap-2 mb-3" {
                        (icon::clipboard_list_icon("w-4 h-4 text-accent"))
                        span class="text-[13px] font-semibold text-fg" { "发货明细" }
                        span id="shipping-item-count" class="ml-auto text-xs text-muted" { "共 0 项" }
                    }
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th class="w-10" { "序号" }
                                    th { "产品" }
                                    th class="w-[180px]" {
                                        "发货仓库 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-[120px] text-right" {
                                        "本次发货 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-10" {}
                                }
                            }
                            tbody id="shipping-items-tbody" {
                                @for row in rows {
                                    (shipping_item_row(row, warehouses));
                                }
                            }
                        }
                    }
                }

                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 -mx-6 px-6 py-4 bg-bg border-t border-border-soft"
                {
                    @if show_header {
                        a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            href=(format!("{}?restore=true", ShippingListPath::PATH))
                        { "取消" }
                    } @else {
                        button type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    button type="submit" id="shipping-submit-btn"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::save_icon("w-4 h-4")) "保存草稿" }
                }
            }

            script src=(crate::layout::page::cache_url("/shipping-create.js")) {}
            ({
                maud::PreEscaped(
                    r#"<script>wmsShippingRenumber();wmsShippingCalcSummary();</script>"#,
                )
            })
        }
    }
}

pub fn shipping_create_page(
    customers: &[abt_core::master_data::customer::model::Customer],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    prefill: &ShippingPrefill,
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
) -> Markup {
    let warehouses_json = serde_json::to_string(
        &warehouses.iter().map(|w| serde_json::json!({
            "id": w.id, "name": &w.name,
        })).collect::<Vec<_>>()
    ).unwrap_or_default();
    let prefill_customer_id = prefill.customer_id;
    // 仅传 id；明细由 get_order_items 端点 HTMX 加载（避免在 SSR 阶段重复拉数据）
    let prefill_order_id = prefill.order_id.unwrap_or(0);

    html! {
        div id="shipping-app"
            class="p-6"
            data-warehouses=(warehouses_json)
        {
            @if show_header {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", ShippingListPath::PATH))
                { (icon::chevron_left_icon("w-4 h-4")) "返回发货申请列表" }
                div class="flex items-center justify-between mb-6" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "新建发货申请" }
                }
            }

            form id="shipping-form"
                class="space-y-5"
                hx-post=(post_path)
                hx-swap="none"
                hx-disabled-elt="#shipping-submit-btn"
                onsubmit="return wmsShippingCollectItems()"
                _=(after_request_hs)
            {
                input type="hidden" name="items_json" id="shipping-items-json" {};
                input type="hidden" name="order_id" id="shipping-order-id-input" {};

                // ── 顶部：客户 + 发货日期 ──
                div class="flex items-center justify-between gap-4 flex-wrap" {
                    div class="flex items-center gap-2 flex-1 min-w-[260px]" {
                        (icon::user_icon("w-[15px] h-[15px] text-muted shrink-0"))
                        select
                            class="flex-1 px-3 py-[7px] border border-border rounded-sm text-[13px] bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="customer_id"
                            id="shipping-customer-select"
                            hx-get=(ShippingCustomerContactsPath::PATH)
                            hx-trigger="change"
                            hx-target="#shipping-address"
                            hx-swap="outerHTML"
                            hx-include="this"
                            _="on change call wmsShippingOnCustomerChange()"
                        {
                            option value="" { "请选择客户" }
                            @for c in customers {
                                option value=(c.id) selected[prefill_customer_id == Some(c.id)] { (c.name) }
                            }
                        }
                    }
                    div class="flex items-center gap-2" {
                        (icon::calendar_icon("w-[15px] h-[15px] text-muted"))
                        input type="date" name="expected_ship_date" id="ship-date"
                            class="w-[140px] px-3 py-[7px] border border-border rounded-sm text-[13px] bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            value=(chrono::Local::now().format("%Y-%m-%d").to_string()) {}
                    }
                }


                // ── 来源订单选择 ──
                div {
                    button type="button" id="order-picker-btn"
                        disabled[prefill_customer_id.unwrap_or(0) == 0]
                        class="flex items-center justify-center gap-2 w-full px-4 py-2.5 rounded-md border border-dashed border-border text-muted text-[13px] font-medium bg-surface transition-all duration-150 \
                               enabled:border-accent enabled:text-accent enabled:bg-accent-bg enabled:cursor-pointer enabled:hover:bg-[rgba(37,99,235,0.1)] \
                               disabled:opacity-60 disabled:cursor-not-allowed"
                        _="on click if not my[@disabled] add .is-open to #order-modal then trigger orderPickerOpened on body end"
                    { (icon::plus_icon("w-4 h-4")) "选择销售订单" }
                    div class="mt-2 text-xs text-muted text-center" id="order-selected-hint" {
                        (if prefill_order_id > 0 {
                            "加载中…"
                        } else if prefill_customer_id.unwrap_or(0) > 0 {
                            "未选择销售订单"
                        } else {
                            "请先选择客户"
                        })
                    }
                }

                // 分隔线
                div class="h-px bg-border-soft" {}

                // ── 发货明细 ──
                div {
                    div class="flex items-center gap-2 mb-3" {
                        (icon::clipboard_list_icon("w-4 h-4 text-accent"))
                        span class="text-[13px] font-semibold text-fg" { "发货明细" }
                        span id="shipping-item-count" class="ml-auto text-xs text-muted" { "共 0 项" }
                    }
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th class="w-10" { "序号" }
                                    th { "产品" }
                                    th class="w-[180px]" {
                                        "发货仓库 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-[120px] text-right" {
                                        "本次发货 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-10" {}
                                }
                            }
                            @if prefill_order_id > 0 {
                                tbody id="shipping-items-tbody"
                                    hx-get=(format!("{}?order_id={}", ShippingOrderItemsPath::PATH, prefill_order_id))
                                    hx-trigger="load"
                                    hx-swap="innerHTML" {}
                            } @else {
                                tbody id="shipping-items-tbody" {}
                            }
                        }
                    }
                    div id="shipping-empty-hint" class="text-center py-8 text-muted text-sm" {
                        (icon::package_icon("w-8 h-8"))
                        p class="mt-2" { "选择销售订单后添加发货明细" }
                    }
                }

                // ── 收货地址 + 备注 ──
                div class="grid grid-cols-1 md:grid-cols-2 gap-4" {
                    div {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "收货地址" }
                        @if let Some(cid) = prefill_customer_id.filter(|&c| c > 0) {
                            input type="text" name="shipping_address" id="shipping-address"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                placeholder="选择客户后自动填充，可修改"
                                hx-get=(format!("{}?customer_id={}", ShippingCustomerContactsPath::PATH, cid))
                                hx-trigger="load"
                                hx-swap="outerHTML" {};
                        } @else {
                            input type="text" name="shipping_address" id="shipping-address"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                placeholder="选择客户后自动填充，可修改" {};
                        }
                    }
                    div {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "备注" }
                        textarea name="remark" rows="2"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[48px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            placeholder="输入发货相关备注…" {}
                    }
                }

                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 -mx-6 px-6 py-4 bg-bg border-t border-border-soft"
                {
                    @if show_header {
                        a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            href=(format!("{}?restore=true", ShippingListPath::PATH))
                        { "取消" }
                    } @else {
                        button type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    button type="submit" id="shipping-submit-btn"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::truck_icon("w-4 h-4")) "确认发货" }
                }
            }

            // ── Order Picker Modal ──
            (modal_shell("order-modal", "z-[1000]", html! {
                div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                    _="on click halt the event"
                {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 class="text-base font-semibold text-fg" { "选择销售订单" }
                        button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                            _="on click remove .is-open from #order-modal"
                        { "×" }
                    }
                    div class="overflow-y-auto flex-1 min-h-0 p-6 pt-4" {
                        div class="flex gap-4 border-b border-border-soft mb-3 pb-3" {
                            div class="flex-1" {
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                    type="text" name="keyword" placeholder="输入订单号搜索…"
                                    hx-get=(ShippingOrderSearchPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-sync="this:replace"
                                    hx-target="#shipping-order-results"
                                    hx-swap="innerHTML"
                                    hx-include="#shipping-customer-select" {};
                            }
                        }
                        div id="shipping-order-results"
                            class="max-h-[420px] overflow-y-auto"
                            hx-get=(ShippingOrderSearchPath::PATH)
                            hx-trigger="intersect, orderPickerOpened from:body"
                            hx-include="#shipping-customer-select"
                            hx-swap="innerHTML"
                        {
                            div class="flex items-center justify-center p-8 text-muted text-sm" { "加载中…" }
                        }
                    }
                }
            }))

            script src=(crate::layout::page::cache_url("/shipping-create.js")) {}
        }
    }
}

/// 订单搜索结果：每行「选择」按钮 hx-get=order-items 端点直接加载明细，
/// 关弹窗由 hyperscript 处理；orderItemsLoaded 事件由 get_order_items 响应头触发更新提示。
fn order_search_results(orders: &[abt_core::sales::sales_order::model::SalesOrder]) -> Markup {
    html! {
        div class="py-2" {
            @for order in orders {
                @let status_text = order_status_text(order.status);
                @let order_date = order.order_date.format("%Y-%m-%d").to_string();
                @let total = order.total_amount.to_string();
                @let items_path = format!("{}?order_id={}", ShippingOrderItemsPath::PATH, order.id);

                div class="flex items-center justify-between p-3 border-b border-border-soft" {
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-medium text-fg" { (order.doc_number) }
                        div class="text-xs text-muted flex items-center gap-[6px] flex-wrap mt-0.5" {
                            span { (order_date) }
                            span class="text-border" { "·" }
                            span { (status_text) }
                            span class="text-border" { "·" }
                            span { "¥" (total) }
                        }
                    }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[7px] px-[14px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-[13px] font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-get=(items_path)
                        hx-target="#shipping-items-tbody"
                        hx-swap="innerHTML"
                        _="on click remove .is-open from #order-modal"
                    { "选择" }
                }
            }
        }
    }
}

fn order_search_empty() -> Markup {
    html! {
        div class="flex flex-col items-center justify-center p-8 text-muted" {
            (icon::package_icon("w-8 h-8"))
            p class="mt-2 text-sm" { "未找到匹配的销售订单" }
        }
    }
}

// ── 订单明细 HTMX 端点（选中订单后服务端渲染行片段，替代旧 JS 拼 DOM）──

#[derive(Debug, Deserialize)]
pub struct OrderItemsQuery {
    pub order_id: i64,
}

/// HTMX: 选中销售订单后加载发货明细行（替代旧 selectOrder JS 拼 DOM），对齐 stock_in confirm 端点范式。
/// 响应头 HX-Trigger-After-Settle: orderItemsLoaded 携带 {doc_number, count} 供前端更新提示。
#[require_permission("SHIPPING", "read")]
pub async fn get_order_items(
    ctx: RequestContext,
    Query(params): Query<OrderItemsQuery>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let order_svc = state.sales_order_service();
    let product_svc = state.product_service();
    let warehouse_svc = state.warehouse_service();

    let order = order_svc.find_by_id(&service_ctx, &mut conn, params.order_id).await?;
    let items = order_svc.list_items(&service_ctx, &mut conn, params.order_id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = product_svc
        .get_by_ids(&service_ctx, &mut conn, items.iter().map(|i| i.product_id).collect())
        .await.unwrap_or_default()
        .into_iter().map(|p| (p.product_id, p)).collect();
    let rows: Vec<OrderItemRow> = items.iter()
        .map(|item| order_item_row(item, product_map.get(&item.product_id)))
        .collect();
    let warehouses = warehouse_svc.list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await.map(|r| r.items).unwrap_or_default();

    let rows_html = shipping_item_rows(&rows, &warehouses).into_string();
    let trigger = serde_json::json!({
        "orderItemsLoaded": {
            "order_id": params.order_id,
            "doc_number": &order.doc_number,
            "count": rows.len()
        }
    }).to_string();
    // 动态 header 值不能直接用 (&str, T) tuple（要求 'static），手动构造 HeaderMap 持有 owned 值。
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "HX-Trigger-After-Settle",
        axum::http::HeaderValue::from_str(&trigger)
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("{}")),
    );
    Ok((headers, Html(rows_html)))
}

/// 发货明细行集合（get_order_items 端点返回 tbody 内容）。
fn shipping_item_rows(
    rows: &[OrderItemRow],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        @for row in rows {
            (shipping_item_row(row, warehouses));
        }
    }
}

/// 单条发货明细行（服务端渲染，替代旧 selectOrder JS 拼 DOM）。
/// 对齐 stock_in::po_detail_row：产品信息集中一格 + 待发余量校验 + 每行独立选仓库。
fn shipping_item_row(
    row: &OrderItemRow,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    use rust_decimal::Decimal;
    let pending = (row.ordered_qty - row.shipped_qty).max(Decimal::ZERO);
    let pending_str = pending.to_string();
    html! {
        tr class="shipping-item-row" oninput="wmsShippingCalcSummary()" {
            td class="line-num text-muted text-xs text-center" {}
            td {
                div class="font-mono tabular-nums text-sm text-fg" { (row.product_code) }
                div class="text-sm text-fg truncate max-w-[260px]" title=(row.product_name) {
                    (row.product_name)
                }
                @if let Some(spec) = row.specification.as_ref().filter(|s| !s.is_empty()) {
                    div class="text-[11px] text-muted truncate max-w-[260px]" { (spec) }
                }
                div class="text-[11px] text-muted pending-hint" data-pending=(pending_str) {
                    "订单 " (crate::utils::fmt_qty(row.ordered_qty))
                    " · 已发 " (crate::utils::fmt_qty(row.shipped_qty))
                    " · 待发 " (crate::utils::fmt_qty(pending))
                    @if let Some(unit) = row.unit.as_deref() {
                        " " (unit)
                    }
                }
            }
            td {
                select name="warehouse_id"
                    class="w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                {
                    option value="" { "选择仓库" }
                    @for w in warehouses {
                        option value=(w.id) { (w.name) }
                    }
                }
            }
            td {
                input type="number" step="any" name="requested_qty"
                    class="w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg text-right font-mono outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    placeholder="0" value=(pending_str) data-pending=(pending_str)
                    oninput="wmsShippingValidateRow(this)" {}
            }
            td {
                button type="button"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    title="删除行"
                    _="on click remove closest <tr/> then call wmsShippingCalcSummary()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="order_item_id" value=(row.order_item_id) {}
        }
    }
}
