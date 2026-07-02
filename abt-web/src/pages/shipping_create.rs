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
 order_id: i64,
 order_item_id: i64,
 product_id: i64,
 product_code: String,
 product_name: String,
 specification: Option<String>,
 unit: Option<String>,
 ordered_qty: rust_decimal::Decimal,
 shipped_qty: rust_decimal::Decimal,
}

/// 从订单明细 + 产品构造聚合行（order_search 与订单预填共用）
fn order_item_row(
 item: &SalesOrderItem,
 product: Option<&abt_core::master_data::product::model::Product>,
 order_id: i64,
) -> OrderItemRow {
 OrderItemRow {
 order_id,
 order_item_id: item.id,
 product_id: item.product_id,
 product_code: product.map(|p| p.product_code.clone()).unwrap_or_default(),
 product_name: product.map(|p| p.pdt_name.clone()).unwrap_or_default(),
 specification: product.map(|p| Some(p.meta.specification.clone())).unwrap_or(None),
 unit: product.map(|p| Some(p.unit.clone())).unwrap_or(None),
 ordered_qty: item.quantity,
 shipped_qty: item.shipped_qty,
 }
}

/// 聚合行 → 前端 selectOrder() 消费的 item JSON（order_search 与订单预填共用）
fn order_item_to_json(row: &OrderItemRow) -> serde_json::Value {
 serde_json::json!({
 "order_item_id": row.order_item_id,
 "product_id": row.product_id,
 "product_code": &row.product_code,
 "product_name": &row.product_name,
 "specification": row.specification.as_deref().unwrap_or(""),
 "unit": row.unit.as_deref().unwrap_or(""),
 "ordered_qty": row.ordered_qty.to_string(),
 "shipped_qty": row.shipped_qty.to_string(),
 })
}

/// 订单详情页「创建发货申请」带入的预填数据
#[derive(Default)]
struct ShippingPrefill {
 customer_id: Option<i64>,
 /// 完整 orderData JSON，前端 selectOrder() 直接消费
 order_json: Option<String>,
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
 let product_svc = state.product_service();
 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
 .await?;

 // 从订单详情页「创建发货申请」带入：预填客户 + 来源订单 + 明细行
 let prefill = if let Some(oid) = q.order_id.filter(|&id| id > 0) {
 match order_svc.find_by_id(&service_ctx, &mut conn, oid).await {
 Ok(order) => {
 let items = order_svc
 .list_items(&service_ctx, &mut conn, oid)
 .await
 .unwrap_or_default();
 let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = product_svc
 .get_by_ids(&service_ctx, &mut conn, items.iter().map(|i| i.product_id).collect())
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|p| (p.product_id, p))
 .collect();
 let rows: Vec<OrderItemRow> = items
 .iter()
 .map(|item| order_item_row(item, product_map.get(&item.product_id), oid))
 .collect();
 ShippingPrefill {
 customer_id: Some(order.customer_id),
 order_json: Some(build_order_prefill_json(&order, &rows)),
 }
 }
 Err(_) => ShippingPrefill::default(),
 }
 } else {
 ShippingPrefill::default()
 };

 let content = shipping_create_page(&customers.items, &warehouses.items, &prefill);
 let page_html = admin_page(
 is_htmx, "新建发货申请", &claims, "sales",
 ShippingCreatePath::PATH, "销售管理", Some("新建发货申请"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SHIPPING", "create")]
pub async fn post_shipping_create(
 _path: ShippingCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ShippingCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

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
 let id = svc.create_from_order(&service_ctx, &mut tx, req).await?;

 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

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

 let draft = shipping_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 if draft.status != PickingStatus::Draft {
 return Err(DomainError::business_rule("仅草稿状态可以编辑").into());
 }

 let items = shipping_svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
 .await?;

 let content = shipping_edit_page(&draft, &items, &customers.items, &warehouses.items);
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

#[require_permission("SHIPPING", "read")]
pub async fn get_customer_contacts(
 ctx: RequestContext,
 Query(params): Query<CustomerContactsQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let customer_svc = state.customer_service();

 let (contacts, addresses) = match params.customer_id {
 Some(cid) if cid > 0 => {
 let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, cid).await.unwrap_or_default();
 let addresses = customer_svc.list_addresses(&service_ctx, &mut conn, cid).await.unwrap_or_default();
 (contacts, addresses)
 }
 _ => (vec![], vec![]),
 };

 let primary_contact = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());
 let contact_name = primary_contact.map(|c| c.name.as_str()).unwrap_or("");
 let contact_phone = primary_contact.and_then(|c| c.phone.as_deref()).unwrap_or("");

 let default_addr = addresses.iter()
 .find(|a| a.address_type == "shipping")
 .or_else(|| addresses.first());
 let shipping_address = default_addr.map(|a| {
 let mut parts = vec![a.province.clone(), a.city.clone()];
 if let Some(ref d) = a.district { parts.push(d.clone()); }
 parts.push(a.detail.clone());
 parts.join("")
 }).unwrap_or_default();

 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 Ok(Html(customer_info_card(&customers.items, params.customer_id, contact_name, contact_phone, &shipping_address).into_string()))
}

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
 let result = order_svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 10))
 .await?;

 if result.items.is_empty() {
 return Ok(Html(order_search_empty().into_string()));
 }

 // 一次批量取所有订单明细（避免逐单 N+1），再收集 product_id
 let product_svc = state.product_service();

 let order_ids: Vec<i64> = result.items.iter().map(|o| o.id).collect();
 let all_items: Vec<(i64, abt_core::sales::sales_order::model::SalesOrderItem)> = order_svc
 .list_items_by_order_ids(&service_ctx, &mut conn, &order_ids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|item| (item.order_id, item))
 .collect();
 let all_product_ids: Vec<i64> = all_items.iter().map(|(_, i)| i.product_id).collect();

 // Fetch product details for all product_ids（repo 层已守卫空 vec）
 let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = product_svc
 .get_by_ids(&service_ctx, &mut conn, all_product_ids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|p| (p.product_id, p))
 .collect();

 let item_rows: Vec<OrderItemRow> = all_items
 .into_iter()
 .map(|(order_id, item)| order_item_row(&item, product_map.get(&item.product_id), order_id))
 .collect();

 let mut items_map: HashMap<i64, Vec<&OrderItemRow>> = HashMap::new();
 for item in &item_rows {
 items_map.entry(item.order_id).or_default().push(item);
 }

 Ok(Html(order_search_results(&result.items, &items_map).into_string()))
}

/// 组装前端 selectOrder() 直接消费的 orderData JSON（用于订单详情页预填）
fn build_order_prefill_json(order: &SalesOrder, rows: &[OrderItemRow]) -> String {
 serde_json::json!({
 "id": order.id,
 "customer_id": order.customer_id,
 "doc_number": order.doc_number,
 "total": order.total_amount.to_string(),
 "order_date": order.order_date.format("%Y-%m-%d").to_string(),
 "status": order_status_text(order.status),
 "items": rows.iter().map(order_item_to_json).collect::<Vec<_>>(),
 }).to_string()
}

// ── Components ──

fn shipping_edit_page(
 draft: &StockPicking,
 items: &[StockPickingItem],
 customers: &[abt_core::master_data::customer::model::Customer],
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 let warehouses_json = serde_json::to_string(
 &warehouses.iter().map(|w| serde_json::json!({
 "id": w.id,
 "name": &w.name,
 })).collect::<Vec<_>>()
 ).unwrap_or_default();

 let draft_id = draft.id;
 let customer_id_str = draft.partner_id.map(|id| id.to_string()).unwrap_or_default();
 let order_id_str = draft.source_id.map(|id| id.to_string()).unwrap_or_default();
 let expected_ship_date = draft.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
 let shipping_address = draft.remark.as_str();
 let carrier = "";
 let remark = "";

 // 将已有明细行序列化为 JSON，供 JS 前端恢复表格
 let items_json = serde_json::to_string(
 &items.iter().map(|item| serde_json::json!({
 "order_item_id": item.source_item_id,
 "product_id": item.product_id,
 "warehouse_id": 0,
 "requested_qty": item.qty_requested.to_string(),
 "description": "",
 })).collect::<Vec<_>>()
 ).unwrap_or_default();

 html! {
    div id="shipping-app"
        class="p-6"
        data-warehouses=(warehouses_json)
        data-draft-id=(draft_id)
        data-customer-id=(customer_id_str)
        data-order-id=(order_id_str)
        data-items=(items_json)
    {
        // ── Page Header ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", ShippingListPath::PATH))
        { (icon::arrow_left_icon("w-4 h-4")) "返回发货申请列表" }
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "编辑发货申请（草稿）" }
        }

        form id="shipping-form" hx-post=(ShippingSaveDraftPath::PATH) hx-swap="none" {
            input type="hidden" name="draft_id" value=(draft_id);
            input type="hidden" name="items_json";
            input type="hidden" name="order_id" value=(order_id_str);
            // ── 客户信息 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "客户信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "客户名称 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="customer_id"
                            id="shipping-customer-select"
                            onchange="onCustomerChange()"
                        {
                            option value="" { "请选择客户" }
                            @for c in customers {
                                option value=(c.id) selected[draft.partner_id == Some(c.id)] { (c.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "联系人"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            id="shipping-contact"
                            readonly
                            tabindex="-1"
                            placeholder="自动填充";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "联系电话"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            id="shipping-phone"
                            readonly
                            tabindex="-1"
                            placeholder="自动填充";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "来源订单"
                        }
                        div class="bg-surface rounded-lg flex flex-col-wrap" id="orderPickerWrap" {
                            input
                                class="bg-surface rounded-lg flex flex-col-input"
                                id="orderPickerInput"
                                type="text"
                                readonly
                                placeholder="点击选择来源订单"
                                onclick="openOrderModal()" {}
                            span class="bg-surface rounded-lg flex flex-col-suffix" {
                                button
                                    type="button"
                                    class="clear-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative"
                                    onclick="clearOrder(event)"
                                    title="清除"
                                { "×" }
                                (icon::grid_icon("w-3.5 h-3.5"))
                            }
                        }
                    }
                    div class="form-field col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "收货地址"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            name="shipping_address"
                            id="shipping-address"
                            value=(shipping_address)
                            placeholder="请输入收货地址";
                    }
                }
            }
            // ── 发货信息 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::truck_icon("w-[18px] h-[18px]")) "发货信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "预计发货日期"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="date"
                            name="expected_ship_date"
                            id="ship-date"
                            value=(expected_ship_date);
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "承运商"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="carrier"
                            id="carrier-select"
                        {
                            option value="" { "请选择承运商" }
                            option value="顺丰速运" selected[carrier == "顺丰速运"] {
                                "顺丰速运"
                            }
                            option value="德邦物流" selected[carrier == "德邦物流"] {
                                "德邦物流"
                            }
                            option value="中通快运" selected[carrier == "中通快运"] {
                                "中通快运"
                            }
                            option value="京东物流" selected[carrier == "京东物流"] {
                                "京东物流"
                            }
                            option value="自提" selected[carrier == "自提"] { "自提 / 自送" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "默认发货仓库"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            id="warehouse-default"
                        {
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "优先级"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            id="priority-select"
                        {
                            option value="normal" { "普通" }
                            option value="urgent" { "紧急" }
                            option value="critical" { "特急" }
                        }
                    }
                }
            }
            // ── 备注 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::file_text_icon("w-[18px] h-[18px]")) "备注" }
                textarea
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5"
                    name="remark"
                    placeholder="输入发货相关备注…"
                { (remark) }
            }
            // ── 附件 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::upload_icon("w-[18px] h-[18px]")) "附件" }
                div class="rounded p-8 text-center cursor-pointer" {
                    (icon::upload_icon("w-8 h-8"))
                    p class="upload-title" { "点击或拖拽文件到此处上传" }
                    p class="upload-hint" { "支持 PDF、Word、Excel、图片，单个文件不超过 10MB" }
                }
            }
            // ── 发货产品明细 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::package_icon("w-[18px] h-[18px]")) "发货产品明细" }
                div class="overflow-x-auto" {
                    table class="data-table" id="lineItemsTable" {
                        thead {
                            tr {
                                th class="w-12" { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th class="w-20" { "单位" }
                                th class="w-24" { "订单数量" }
                                th class="w-24" { "已发货" }
                                th class="w-24" { "本次发货" }
                                th class="w-16" { "发货仓库" }
                                th class="w-16" {}
                            }
                        }
                        tbody id="lineItemsBody" {}
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                        onclick="addRow()"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品" }
                }
                div class="flex justify-end p-4 bg-surface border-t border-border-soft gap-8" {
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "发货项目" }
                        span class="text-lg font-bold text-fg" id="totalItems" { "0 项" }
                    }
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "本次发货合计" }
                        span class="text-lg font-bold text-fg grand" id="totalQty" { "0" }
                    }
                }
            }
        }
        // ── Action Bar ──
        div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
        {
            a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                href=(format!("{}?restore=true", ShippingListPath::PATH))
            { "取消" }
            button
                type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                _="on click call handleSave()"
            { (icon::save_icon("w-4 h-4")) "保存" }
        }
        // ── Order Picker Modal ──
        (modal_shell("order-modal", "z-[1000]", html! {
            div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event"
            {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 { "选择来源订单" }
                    button
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                        _="on click remove .is-open from #order-modal"
                    { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6 p-0" {
                    div class="flex gap-4 border-b border-border-soft" {
                        input type="hidden" id="order-search-customer-id" name="customer_id" {}
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-xs font-medium text-fg-2" { "搜索订单" }
                            input
                                class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="keyword"
                                placeholder="输入订单号…"
                                hx-get=(ShippingOrderSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#shipping-order-results"
                                hx-swap="innerHTML"
                                hx-include="#order-search-customer-id" {}
                        }
                    }
                    div id="shipping-order-results"
                        class="max-h-[400px] overflow-y-auto"
                        hx-get=(ShippingOrderSearchPath::PATH)
                        hx-trigger="intersect"
                        hx-include="#order-search-customer-id"
                        hx-swap="innerHTML"
                    {
                        div class="flex items-center justify-center p-8 text-muted" { "加载中…" }
                    }
                }
            }
        }))
        // ── External script + draft restore ──
        script src="/shipping-create.js" {}
        ({
            maud::PreEscaped(
                r#"<script>
 (function(){
 var app = document.getElementById('shipping-app');
 if (!app) return;
 var orderId = app.getAttribute('data-order-id');
 var itemsJson = app.getAttribute('data-items');
 if (orderId && orderId !== '') {
 selectedCustomer = document.getElementById('shipping-customer-select').value;
 var orderInput = document.getElementById('orderPickerInput');
 if (orderInput) {
 orderInput.disabled = false;
 orderInput.placeholder = '点击选择来源订单';
 }
 }
 if (itemsJson && itemsJson !== '[]' && itemsJson !== 'null') {
 try {
 var items = JSON.parse(itemsJson);
 var tbody = document.getElementById('lineItemsBody');
 if (tbody && items.length > 0) {
 fillItemsFromDraft(items);
 }
 } catch(e) {}
 }
 updateTotals();
 })();
 </script>"#,
            )
        })
    }
}
}

fn shipping_create_page(
 customers: &[abt_core::master_data::customer::model::Customer],
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 prefill: &ShippingPrefill,
) -> Markup {
 let warehouses_json = serde_json::to_string(
 &warehouses.iter().map(|w| serde_json::json!({
 "id": w.id,
 "name": &w.name,
 })).collect::<Vec<_>>()
 ).unwrap_or_default();

 let prefill_customer_id = prefill.customer_id;
 let prefill_order_json = prefill.order_json.as_deref().unwrap_or("");

 html! {
    div id="shipping-app"
        class="p-6"
        data-warehouses=(warehouses_json)
        data-order-prefill=(prefill_order_json)
    {
        // ── Page Header ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", ShippingListPath::PATH))
        { (icon::arrow_left_icon("w-4 h-4")) "返回发货申请列表" }
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建发货申请" }
        }

        form id="shipping-form" hx-post=(ShippingCreatePath::PATH) hx-swap="none" {
            input type="hidden" name="items_json";
            input type="hidden" name="order_id";
            // ── 客户信息 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "客户信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "客户名称 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="customer_id"
                            id="shipping-customer-select"
                            onchange="onCustomerChange()"
                        {
                            option value="" { "请选择客户" }
                            @for c in customers {
                                option value=(c.id) selected[prefill_customer_id == Some(c.id)] {
                                    (c.name)
                                }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "联系人"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            id="shipping-contact"
                            readonly
                            tabindex="-1"
                            placeholder="自动填充";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "联系电话"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            id="shipping-phone"
                            readonly
                            tabindex="-1"
                            placeholder="自动填充";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "来源订单 "
                            span class="required" { "*" }
                        }
                        div class="bg-surface rounded-lg flex flex-col-wrap" id="orderPickerWrap" {
                            input
                                class="bg-surface rounded-lg flex flex-col-input"
                                id="orderPickerInput"
                                type="text"
                                readonly
                                placeholder="请先选择客户"
                                onclick="openOrderModal()"
                                disabled;
                            span class="bg-surface rounded-lg flex flex-col-suffix" {
                                button
                                    type="button"
                                    class="clear-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative"
                                    onclick="clearOrder(event)"
                                    title="清除"
                                { "×" }
                                (icon::grid_icon("w-3.5 h-3.5"))
                            }
                        }
                    }
                    div class="form-field col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "收货地址 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="text"
                            name="shipping_address"
                            id="shipping-address"
                            placeholder="请输入收货地址";
                    }
                }
                // Customer info bar
                div class="flex items-center gap-6 p-3 rounded-sm text-xs text-fg-2 hidden"
                    id="customerInfoBar"
                {
                    span {
                        span class="label" { "联系人：" }
                        span id="infoContact" { "—" }
                    }
                    span {
                        span class="label" { "电话：" }
                        span id="infoPhone" { "—" }
                    }
                    span {
                        span class="label" { "地址：" }
                        span id="infoAddress" { "—" }
                    }
                }
                // Selected order detail
                div class="flex items-center gap-6 p-3 rounded-sm text-xs text-fg-2 hidden mt-2 bg-accent-bg border-accent/12"
                    id="selectedOrderDetail"
                {
                    span {
                        span class="label" { "订单日期：" }
                        span id="detailOrderDate" { "—" }
                    }
                    span {
                        span class="label" { "状态：" }
                        span id="detailOrderStatus" { "—" }
                    }
                    span {
                        span class="label" { "订单金额：" }
                        span id="detailOrderAmount" { "—" }
                    }
                    span {
                        span class="label" { "产品数量：" }
                        span id="detailOrderProducts" { "—" }
                    }
                }
            }
            // ── 发货信息 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::truck_icon("w-[18px] h-[18px]")) "发货信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "预计发货日期 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="date"
                            name="expected_ship_date"
                            id="ship-date";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "承运商"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="carrier"
                            id="carrier-select"
                        {
                            option value="" { "请选择承运商" }
                            option value="顺丰速运" { "顺丰速运" }
                            option value="德邦物流" { "德邦物流" }
                            option value="中通快运" { "中通快运" }
                            option value="京东物流" { "京东物流" }
                            option value="自提" { "自提 / 自送" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "默认发货仓库"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            id="warehouse-default"
                        {
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "优先级"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            id="priority-select"
                        {
                            option value="normal" { "普通" }
                            option value="urgent" { "紧急" }
                            option value="critical" { "特急" }
                        }
                    }
                }
            }
            // ── 备注 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::file_text_icon("w-[18px] h-[18px]")) "备注" }
                textarea
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5"
                    name="remark"
                    placeholder="输入发货相关备注，如包装要求、送货时间偏好、特殊说明等…" {}
            }
            // ── 附件 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::upload_icon("w-[18px] h-[18px]")) "附件" }
                div class="rounded p-8 text-center cursor-pointer" {
                    (icon::upload_icon("w-8 h-8"))
                    p class="upload-title" { "点击或拖拽文件到此处上传" }
                    p class="upload-hint" { "支持 PDF、Word、Excel、图片，单个文件不超过 10MB" }
                }
            }
            // ── 发货产品明细 ──
            div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden"
            {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { (icon::package_icon("w-[18px] h-[18px]")) "发货产品明细" }
                div class="overflow-x-auto" {
                    table class="data-table" id="lineItemsTable" {
                        thead {
                            tr {
                                th class="w-12" { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格描述" }
                                th class="w-20" { "单位" }
                                th class="w-24" { "订单数量" }
                                th class="w-24" { "已发货" }
                                th class="w-24" {
                                    "本次发货 "
                                    span class="required" { "*" }
                                }
                                th class="w-16" { "发货仓库" }
                                th class="w-16" {}
                            }
                        }
                        tbody id="lineItemsBody" {
                            // Populated by JS when order is selected
                        }
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                        onclick="addRow()"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加产品" }
                }
                div class="flex justify-end p-4 bg-surface border-t border-border-soft gap-8" {
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "发货项目" }
                        span class="text-lg font-bold text-fg" id="totalItems" { "0 项" }
                    }
                    div class="flex gap-3" {
                        span class="text-sm text-muted" { "本次发货合计" }
                        span class="text-lg font-bold text-fg grand" id="totalQty" { "0" }
                    }
                }
            }
        }
        // ── Action Bar ──
        div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
        {
            a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                href=(format!("{}?restore=true", ShippingListPath::PATH))
            { "取消" }
            button
                type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                _="on click call handleSave()"
            { (icon::save_icon("w-4 h-4")) "保存" }
        }
        // ── Order Picker Modal ──
        (modal_shell("order-modal", "z-[1000]", html! {
            div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event"
            {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 { "选择来源订单" }
                    button
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                        _="on click remove .is-open from #order-modal"
                    { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6 p-0" {
                    div class="flex gap-4 border-b border-border-soft" {
                        input type="hidden" id="order-search-customer-id" name="customer_id" {}
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-xs font-medium text-fg-2" { "搜索订单" }
                            input
                                class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="keyword"
                                placeholder="输入订单号…"
                                hx-get=(ShippingOrderSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#shipping-order-results"
                                hx-swap="innerHTML"
                                hx-include="#order-search-customer-id" {}
                        }
                    }
                    div id="shipping-order-results"
                        class="max-h-[400px] overflow-y-auto"
                        hx-get=(ShippingOrderSearchPath::PATH)
                        hx-trigger="intersect"
                        hx-include="#order-search-customer-id"
                        hx-swap="innerHTML"
                    {
                        div class="flex items-center justify-center p-8 text-muted" { "加载中…" }
                    }
                }
            }
        }))
        // ── External script ──
        script src="/shipping-create.js" {}
        ({
            maud::PreEscaped(
                r#"<script>
 (function(){
 var app = document.getElementById('shipping-app');
 if (!app) return;
 var orderJson = app.getAttribute('data-order-prefill');
 if (!orderJson || orderJson === '') return;
 try {
 var orderData = JSON.parse(orderJson);
 selectedCustomer = String(orderData.customer_id || '');
 var hiddenCid = document.querySelector('#order-modal input[name="customer_id"]');
 if (hiddenCid) hiddenCid.value = selectedCustomer;
 var orderInput = document.getElementById('orderPickerInput');
 if (orderInput) { orderInput.disabled = false; orderInput.placeholder = '点击选择来源订单'; }
 var bar = document.getElementById('customerInfoBar');
 if (bar) bar.classList.remove('hidden-initial');
 selectOrder(orderData);
 var dateEl = document.getElementById('detailOrderDate');
 var statusEl = document.getElementById('detailOrderStatus');
 if (dateEl && orderData.order_date) dateEl.textContent = orderData.order_date;
 if (statusEl && orderData.status) statusEl.textContent = orderData.status;
 } catch(e) { if (window.console) console.error('shipping prefill failed', e); }
 })();
 </script>"#,
            )
        })
    }
}
}

fn customer_info_card(
 customers: &[abt_core::master_data::customer::model::Customer],
 selected_customer_id: Option<i64>,
 contact_name: &str,
 contact_phone: &str,
 shipping_address: &str,
) -> Markup {
 let selected = selected_customer_id.map(|id| id.to_string()).unwrap_or_default();

 html! {
    div id="customer-info-card"
        class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden mb-4"
    {
        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
        { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "客户信息" }
        div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
            div class="form-field" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                    "客户名称 "
                    span class="required" { "*" }
                }
                select
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    name="customer_id"
                    hx-get=(ShippingCustomerContactsPath::PATH)
                    hx-trigger="change"
                    hx-target="#customer-info-card"
                    hx-swap="outerHTML"
                    hx-include="this"
                {
                    option value="" { "请选择客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected == c.id.to_string()] { (c.name) }
                    }
                }
            }
            div class="form-field" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系人" }
                input
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="text"
                    value=(contact_name)
                    placeholder="自动填充"
                    readonly {}
            }
            div class="form-field" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系电话" }
                input
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="text"
                    value=(contact_phone)
                    placeholder="自动填充"
                    readonly {}
            }
        }
        div class="grid grid-cols-2 gap-4 gap-x-6 mb-6 mt-3" {
            div class="form-field col-span-2" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "收货地址" }
                input
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="text"
                    name="shipping_address"
                    value=(shipping_address)
                    placeholder="选择客户后自动填充" {}
            }
        }
    }
}
}

fn order_search_results(
 orders: &[abt_core::sales::sales_order::model::SalesOrder],
 items_map: &HashMap<i64, Vec<&OrderItemRow>>,
) -> Markup {
 html! {
    div class="py-2" {
        @for order in orders {
            @let status_text = order_status_text(order.status);
            @let order_date = order.order_date.format("%Y-%m-%d").to_string();
            @let total = order.total_amount.to_string();
            @let items_json = serde_json::json!(
                { "id" : order.id, "doc_number" : & order.doc_number, "items" : items_map
                .get(& order.id).map(| items | items.iter().map(| item |
                order_item_to_json(item)).collect::< Vec < _ >> ()).unwrap_or_default() }
            )
                .to_string();

            div class="flex items-center justify-between p-3 border-b border-border-soft" {
                div class="product-select-info" {
                    div class="text-sm font-medium text-fg" { (order.doc_number) }
                    div class="text-xs text-muted flex items-center gap-[6px] flex-wrap" {
                        span { (order_date) }
                        span class="text-border" { "·" }
                        span { (status_text) }
                        span class="text-border" { "·" }
                        span { "¥" (total) }
                    }
                }
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                    data-order=(items_json)
                    onclick="selectOrder(JSON.parse(this.dataset.order))"
                { "选择" }
            }
        }
    }
}
}

fn order_search_empty() -> Markup {
 html! {
    div class="flex items-center justify-center p-8 text-muted" {
        (icon::package_icon("w-8 h-8"))
        p class="mt-2 text-sm" { "请先选择客户，或未找到匹配的订单" }
    }
}
}
