use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::purchase::return_order::model::*;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_return::*;
use crate::utils::RequestContext;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct OrderItemsParams {
 pub order_id: i64,
}

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct PRCreateForm {
 pub order_id: i64,
 pub return_date: String,
 pub return_reason: String,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
 order_item_id: i64,
 product_id: i64,
 returned_qty: String,
 unit_price: String,
}

// ── Handlers ──

#[require_permission("PURCHASE_RETURN", "create")]
pub async fn get_pr_create(
 _path: PRCreatePath,
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
 let order_svc = state.purchase_order_service();

 // Fetch orders eligible for return: Confirmed, PartiallyReceived, Received
 let statuses = [
 PurchaseOrderStatus::Confirmed,
 PurchaseOrderStatus::PartiallyReceived,
 PurchaseOrderStatus::Received,
 ];

 let mut all_orders = Vec::new();
 for status in &statuses {
 let result = order_svc
 .list(
 &service_ctx,
 &mut conn,
 PurchaseOrderQuery {
 status: Some(*status),
 ..Default::default()
 },
 PageParams::new(1, 100),
 )
 .await?;
 all_orders.extend(result.items);
 }

 let content = pr_create_page(&all_orders, PRCreatePath::PATH, "", true);
 let page_html = admin_page(
 is_htmx,
 "新建采购退货",
 &claims,
 "purchase",
 PRCreatePath::PATH,
 "采购管理",
 Some("新建采购退货"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: fetch order items + supplier info for a selected purchase order
#[require_permission("PURCHASE_RETURN", "create")]
pub async fn get_pr_order_items(
 ctx: RequestContext,
 Query(params): Query<OrderItemsParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let order_svc = state.purchase_order_service();
 let order = order_svc
 .get(&service_ctx, &mut conn, params.order_id)
 .await?;
 let items = order_svc
 .list_items(&service_ctx, &mut conn, params.order_id)
 .await?;

 // Fetch supplier info
 let supplier_svc = state.supplier_service();
 let supplier = supplier_svc
 .get(&service_ctx, &mut conn, order.supplier_id)
 .await?;
 let contacts = supplier_svc
 .list_contacts(&service_ctx, &mut conn, order.supplier_id)
 .await?;

 // Primary contact info
 let primary = contacts.iter().find(|c| c.is_primary).or_else(|| contacts.first());
 let contact_name = primary.map(|c| c.name.as_str()).unwrap_or("—");
 let contact_phone = primary
 .and_then(|c| c.phone.as_ref())
 .map(|s| s.as_str())
 .unwrap_or("—");

 // Collect product IDs and batch-fetch product info
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let product_svc = state.product_service();
 let products = if product_ids.is_empty() {
 vec![]
 } else {
 product_svc
 .get_by_ids(&service_ctx, &mut conn, product_ids)
 .await?
 };
 let product_map: std::collections::HashMap<i64, &abt_core::master_data::product::model::Product> =
 products.iter().map(|p| (p.product_id, p)).collect();

 let supplier_info = SupplierInfo {
 name: supplier.name,
 contact: contact_name.to_string(),
 phone: contact_phone.to_string(),
 };

 Ok(Html(
 order_items_fragment(&items, &product_map, &supplier_info).into_string(),
 ))
}

/// Supplier info carried from order selection
struct SupplierInfo {
 name: String,
 contact: String,
 phone: String,
}

/// 退货创建核心逻辑（解析 PRCreateForm → svc.create），创建页与 work_center drawer 共用。
pub async fn do_create_pr(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: PRCreateForm,
) -> Result<i64> {
    if form.order_id == 0 {
        return Err(DomainError::validation("请选择采购订单").into());
    }

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let order_svc = state.purchase_order_service();
    let order = order_svc
        .get(service_ctx, &mut tx, form.order_id)
        .await?;

    let return_date = chrono::NaiveDate::parse_from_str(&form.return_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效退货日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效退货明细数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个退货产品").into());
    }

    let items: Vec<CreateReturnItemRequest> = web_items
        .into_iter()
        .map(|item| CreateReturnItemRequest {
            order_item_id: item.order_item_id,
            product_id: item.product_id,
            returned_qty: item
                .returned_qty
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            unit_price: item
                .unit_price
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
        })
        .collect();

    let create_req = CreatePurchaseReturnRequest {
        order_id: form.order_id,
        supplier_id: order.supplier_id,
        return_date,
        return_reason: form.return_reason,
        remark: form.remark.unwrap_or_default(),
        items,
    };

    let svc = state.purchase_return_service();
    let id = svc.create(service_ctx, &mut tx, create_req, None).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(id)
}

/// POST: create purchase return from form submission (HTMX)
#[require_permission("PURCHASE_RETURN", "create")]
pub async fn create_pr(
 _path: PRCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PRCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let id = do_create_pr(&state, &service_ctx, form).await?;
 let redirect = PRDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

pub fn pr_create_page(
 orders: &[abt_core::purchase::order::model::PurchaseOrder],
 post_path: &str,
 after_request_hs: &str,
 show_header: bool,
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();

 html! {
    div id="pr-app" {
        @if show_header {
            div class="flex items-center justify-between mb-6" {
                a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", PRListPath::PATH))
                { (icon::arrow_left_icon("w-4 h-4")) "返回采购退货列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购退货" }
            }
        }

        form
            id="pr-form"
            hx-post=(post_path)
            hx-swap="none"
            _=(after_request_hs)
            onsubmit="PRCreate.collectItems();return true"
        {
            input type="hidden" id="items-json" name="items_json" value="[]";
            // ── 关联单据 ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "关联单据" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "采购订单"
                            span class="text-danger" { "*" }
                        }
                        select
                            id="pr-order-select"
                            name="order_id"
                            hx-get=(PROrderItemsPath::PATH)
                            hx-trigger="change"
                            hx-target="#pr-order-data"
                            hx-swap="innerHTML"
                            hx-include="#pr-order-select"
                        {
                            option value="" { "请选择采购订单" }
                            @for o in orders {
                                @let status_text = order_status_text(o.status);
                                option value=(o.id) { (o.doc_number) " — " (status_text) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "供应商" }
                        input type="text" id="pr-supplier-name" readonly value="—" {}
                    }
                    div class="form-field" {
                        label { "联系人" }
                        input type="text" id="pr-contact" readonly value="—" {}
                    }
                    div class="form-field" {
                        label { "联系电话" }
                        input type="text" id="pr-phone" readonly value="—" {}
                    }
                }
            }
            // ── 退货信息 ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "退货信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "退货日期"
                            span class="text-danger" { "*" }
                        }
                        input type="date" name="return_date" value=(today) required {}
                    }
                    div class="form-field" {
                        label {
                            "退货原因"
                            span class="text-danger" { "*" }
                        }
                        select name="return_reason" required {
                            option value="" { "请选择" }
                            option value="质量不合格" { "质量不合格" }
                            option value="规格不符" { "规格不符" }
                            option value="数量短缺" { "数量短缺" }
                            option value="损坏" { "损坏" }
                            option value="交货延迟" { "交货延迟" }
                            option value="其他" { "其他" }
                        }
                    }
                    div class="form-field" {
                        label { "处理方式" }
                        select name="processing_method" {
                            option value="" { "请选择" }
                            option value="退货退款" { "退货退款" }
                            option value="换货" { "换货" }
                            option value="返工" { "返工" }
                        }
                    }
                    div class="form-field" {
                        label { "物流公司" }
                        input type="text" name="logistics_company" placeholder="输入物流公司名称…" {}
                    }
                    div class="form-field" {
                        label { "物流单号" }
                        input type="text" name="tracking_number" placeholder="输入物流单号…" {}
                    }
                    div class="form-field" {
                        label { "处理人" }
                        select name="handler" {
                            option value="" { "请选择" }
                            option value="current_user" { "当前用户" }
                        }
                    }
                    div class="form-field" {
                        label { "收货仓库" }
                        select name="receiving_warehouse" {
                            option value="" { "请选择仓库" }
                            option value="东莞原料仓" { "东莞原料仓" }
                            option value="深圳成品仓" { "深圳成品仓" }
                            option value="苏州配件仓" { "苏州配件仓" }
                        }
                    }
                    div class="form-field col-span-2" {
                        label { "备注" }
                        textarea
                            name="remark"
                            placeholder="输入退货相关备注信息…"
                            class="w-full resize-y rounded-sm min-h-[80px] border border-border text-sm"
                            style="padding:8px 12px;font-family:inherit" {}
                    }
                }
            }
            // ── 退货产品明细 ──
            div id="pr-items-section"
                class="data-card p-0 overflow-hidden mb-4"
                style="display:none"
            {
                div class="flex justify-between items-center px-5 pt-5 pb-3" {
                    span
                        class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft m-0 p-0 border-none"
                    { "退货产品明细" }
                }
                div class="overflow-x-auto" {
                    table class="data-table" style="min-width:1100px" {
                        thead {
                            tr {
                                th class="w-9 text-center" { "行号" }
                                th { "物料编码" }
                                th { "物料名称" }
                                th { "规格" }
                                th { "单位" }
                                th class="text-right text-[13px]" { "订单数量" }
                                th class="text-right text-[13px]" { "已收货" }
                                th class="w-[120px] text-right" { "退货数量" }
                                th class="text-right text-[13px]" { "单价" }
                                th class="text-right text-[13px]" { "退货金额" }
                                th class="w-9" { "操作" }
                            }
                        }
                        tbody id="pr-item-tbody" {}
                    }
                }
            }
            // Hidden container for HTMX swap of order data
            div id="pr-order-data" style="display:none" {}

            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", PRListPath::PATH))
                { "取消" }
                div class="flex gap-3" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        id="pr-save-draft"
                    { "保存草稿" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { "提交退货" }
                }
            }
        }
    }
    script src="/return-create.js?v=20260612" {}
}
}

/// Order items fragment rendered when a PO is selected
fn order_items_fragment(
 items: &[abt_core::purchase::order::model::PurchaseOrderItem],
 product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
 supplier_info: &SupplierInfo,
) -> Markup {
 html! {
    div data-supplier-name=(supplier_info.name)
        data-contact=(supplier_info.contact)
        data-phone=(supplier_info.phone)
    {
        @for item in items {
            @let product = product_map.get(&item.product_id);
            @let product_code = product.map(|p| p.product_code.as_str()).unwrap_or("");
            @let product_name = product
                .map(|p| p.pdt_name.as_str())
                .unwrap_or(item.description.as_str());
            @let specification = product
                .map(|p| p.meta.specification.as_str())
                .unwrap_or("");
            @let unit = product.map(|p| p.unit.as_str()).unwrap_or("");
            @let item_json = serde_json::json!(
                { "order_item_id" : item.id, "product_id" : item.product_id,
                "product_code" : product_code, "product_name" : product_name,
                "specification" : specification, "unit" : unit, "order_qty" : item
                .quantity.to_string(), "received_qty" : item.received_qty.to_string(),
                "unit_price" : item.unit_price.to_string(), "returned_qty" : item
                .quantity.to_string(), }
            )
                .to_string();
            div data-item=(item_json) {}
        }
    }
}
}

fn order_status_text(s: PurchaseOrderStatus) -> &'static str {
 match s {
 PurchaseOrderStatus::Draft => "草稿",
 PurchaseOrderStatus::Confirmed => "已确认",
 PurchaseOrderStatus::PartiallyReceived => "部分收货",
 PurchaseOrderStatus::Received => "已收货",
 PurchaseOrderStatus::Closed => "已关闭",
 PurchaseOrderStatus::Cancelled => "已取消",
 PurchaseOrderStatus::PendingApproval => "待审批",
 }
}
