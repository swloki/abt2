use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::print_template::{PrintTemplate, PrintTemplateService};
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::{DemandService, SalesOrderService};
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::identity::UserService;
use abt_core::wms::stock_ledger::StockLedgerService;

use crate::components::fulfillment_workbench::{fulfillment_progress, fulfillment_workbench};
use crate::components::icon;
use crate::components::print_dropdown::{print_dropdown, PrintParam};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::routes::print_template::PrintTemplateListPath;
use abt_core::wms::picking::PickingService;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
 match s {
 SalesOrderStatus::Draft => ("草稿", "status-draft"),
 SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
 SalesOrderStatus::ReadyToShip => ("待发货", "status-ready"),
 SalesOrderStatus::PartiallyShipped => ("部分发货", "status-progress"),
 SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
 SalesOrderStatus::Cancelled => ("已取消", "status-cancelled"),
 SalesOrderStatus::ShippingRequested => ("已申请发货", "status-ready"),
 }
}

fn line_status_pill(s: SalesOrderLineStatus) -> (&'static str, &'static str) {
 match s {
 SalesOrderLineStatus::Pending => ("待处理", "status-pending"),
 SalesOrderLineStatus::Allocated => ("已分配", "status-confirmed"),
 SalesOrderLineStatus::Producing => ("生产中", "status-warn"),
 SalesOrderLineStatus::Purchasing => ("采购中", "status-purple"),
 SalesOrderLineStatus::Shipped => ("已发货", "status-success"),
 SalesOrderLineStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

struct ContactInfo {
 name: String,
 phone: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_detail(
 path: OrderDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();
 let customer_svc = state.customer_service();
 let product_svc = state.product_service();
 let user_svc = state.user_service();

 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();
 // 履行计划
 let plan_lines = svc.list_fulfillment_plan(
 &service_ctx, &mut conn,
 FulfillmentPlanQuery { order_id: Some(path.id), status: None },
 ).await.unwrap_or_default();

 // 查询各产品当前可用库存（ATP），用于实时计算满足率
 let stock_svc = state.stock_ledger_service();
 let mut atp_map: HashMap<i64, Decimal> = HashMap::new();
 for pl in &plan_lines {
 if !atp_map.contains_key(&pl.product_id)
 && let Ok(atp) = stock_svc.query_available(&service_ctx, &mut conn, pl.product_id, None).await {
 atp_map.insert(pl.product_id, atp);
 }
 }

 // 各产品的预留量（用于缺口表「被占用」徽标：reserved>0 才显示）
 let reserved_map: HashMap<i64, Decimal> = {
 let product_ids: Vec<i64> = plan_lines.iter().map(|p| p.product_id).collect();
 if product_ids.is_empty() {
 HashMap::new()
 } else {
 stock_svc
 .query_projected_qty_batch(&service_ctx, &mut conn, &product_ids, None)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|(k, v)| (k, v.reserved))
 .collect()
 }
 };
 // 查询该订单关联的需求池（demand）真实状态，用于「需求状态」列
 // 无 demand → 已满足（库存已锁定，无需补货）；有 demand → 按 demand.status 显示
 let demand_svc = state.sales_demand_service();
 let demands = demand_svc
 .find_by_source(&service_ctx, &mut conn, DocumentType::SalesOrder as i16, path.id)
 .await.unwrap_or_default();

 // Smart Button 统计（参考 Odoo oe_button_box）
 let producing_count = demands.iter().filter(|d| d.acquire_channel == 1).count();
 let purchasing_count = demands.iter().filter(|d| d.acquire_channel == 2).count();

 let demand_map: HashMap<i64, DemandStatus> = demands
 .into_iter()
 .map(|d| (d.source_line_id, d.status))
 .collect();

 let customer_name = customer_svc
 .get(&service_ctx, &mut conn, order.customer_id)
 .await
 .map(|c| c.name)
 .unwrap_or_else(|_| "未知客户".into());

 let contact = {
 let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, order.customer_id).await.unwrap_or_default();
 contacts.into_iter().find(|c| c.id == order.contact_id).map(|c| ContactInfo {
 name: c.name,
 phone: c.phone,
 })
 };

 let sales_rep = user_svc
 .get_user(&service_ctx, &mut conn, order.sales_rep_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 // 产品信息
 let (product_names, product_codes) = {
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 if product_ids.is_empty() {
 (HashMap::new(), HashMap::new())
 } else {
 let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
 let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
 let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
 (names, codes)
 }
 };

 let print_templates = state
     .print_template_service()
     .list_by_document_type(&mut conn, "sales_order")
     .await
     .unwrap_or_default();
 let content = order_detail_page(
 &order, &items, &plan_lines,
 &customer_name, &contact, &sales_rep,
 &product_names, &product_codes, &atp_map, &demand_map, &reserved_map,
 producing_count, purchasing_count, path.id, &print_templates,
 );
 let page_html = admin_page(
 is_htmx, "订单详情", &claims, "sales",
 &format!("{}/{}", OrderListPath::PATH, path.id),
 "销售管理", Some("订单详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

/// 打印销售订单：用 sales_order 模板 + 真实业务数据渲染，返回完整 HTML 供浏览器打印。
/// template_id 可选：None 用 sales_order 类型默认模板，Some(id) 用指定模板。
#[require_permission("SALES_ORDER", "read")]
pub async fn print_order(
    path: OrderPrintPath,
    ctx: RequestContext,
    Query(param): Query<PrintParam>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let svc = state.sales_order_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();
    let print_svc = state.print_template_service();

    let o = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let customer_name = customer_svc
        .get(&service_ctx, &mut conn, o.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_default();

    let sales_rep = user_svc
        .get_user(&service_ctx, &mut conn, o.sales_rep_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_default();

    let status_text = match o.status {
        SalesOrderStatus::Draft => "草稿",
        SalesOrderStatus::Confirmed => "已确认",
        SalesOrderStatus::ReadyToShip => "待发货",
        SalesOrderStatus::PartiallyShipped => "部分发货",
        SalesOrderStatus::Shipped => "已发货",
        SalesOrderStatus::Cancelled => "已取消",
        SalesOrderStatus::ShippingRequested => "已申请发货",
    };

    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_map: HashMap<i64, String> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .map(|ps| ps.into_iter().map(|p| (p.product_id, p.pdt_name)).collect())
            .unwrap_or_default()
    };

    let detail_items: Vec<serde_json::Value> = items
        .iter()
        .map(|it| {
            let line_status_text = match it.line_status {
                SalesOrderLineStatus::Pending => "待处理",
                SalesOrderLineStatus::Allocated => "已分配",
                SalesOrderLineStatus::Producing => "生产中",
                SalesOrderLineStatus::Purchasing => "采购中",
                SalesOrderLineStatus::Shipped => "已发货",
                SalesOrderLineStatus::Cancelled => "已取消",
            };
            serde_json::json!({
                "行号": it.line_no.to_string(),
                "产品名称": product_map.get(&it.product_id).cloned().unwrap_or_default(),
                "数量": fmt_qty(it.quantity),
                "单位": it.unit.as_str(),
                "单价": it.unit_price.to_string(),
                "金额": it.amount.to_string(),
                "已发数量": fmt_qty(it.shipped_qty),
                "未交数量": fmt_qty(it.open_qty()),
                "行状态": line_status_text,
                "备注": it.remark.clone(),
            })
        })
        .collect();

    let vars = serde_json::json!({
        "订单号": o.doc_number,
        "订单日期": o.order_date.format("%Y-%m-%d").to_string(),
        "客户全称": customer_name,
        "订单总金额": o.total_amount.to_string(),
        "交货地址": o.delivery_address,
        "付款条款": o.payment_terms,
        "交货条款": o.delivery_terms,
        "订单状态": status_text,
        "销售员": sales_rep,
        "公司名称": "江门市艾伯特照明科技有限公司",
        "打印时间": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "明细": detail_items,
    });

    let html = match param.template_id {
        Some(tid) => print_svc.render(&mut conn, tid, vars).await?,
        None => print_svc.render_default(&mut conn, "sales_order", vars).await?,
    };

    Ok(Html(format!(
        "{html}<script>window.onload=function(){{window.print()}}</script>"
    )))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_order(
 path: ConfirmOrderPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 svc.confirm(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn cancel_order(
 path: CancelOrderPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 svc.cancel(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── 一键申请发货（订单详情页弹窗，销售不选仓库）──

#[derive(Debug, serde::Deserialize)]
pub struct RequestShipForm {
 pub items_json: String,
 #[serde(default)]
 pub shipping_requirements: String,
}

/// HTMX: 返回「申请发货」modal（append 到 body，含 is-open 直接显示）。
#[require_permission("SHIPPING", "create")]
pub async fn get_request_ship_modal(
 path: RequestShipPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.sales_order_service();
 let product_svc = state.product_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();
 let (codes, names) = if items.is_empty() {
  (HashMap::new(), HashMap::new())
 } else {
  let products = product_svc
   .get_by_ids(&service_ctx, &mut conn, items.iter().map(|i| i.product_id).collect())
   .await.unwrap_or_default();
  let c: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
  let n: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
  (c, n)
 };
 Ok(Html(request_ship_modal_body(&order, &items, &codes, &names).into_string()))
}

/// POST: 一键申请发货 → request_from_order（建发货单 Confirmed + 订单 ShippingRequested）。
#[require_permission("SHIPPING", "create")]
pub async fn request_shipment(
 path: RequestShipPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<RequestShipForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let items: Vec<abt_core::wms::picking::model::RequestShippingItemReq> = serde_json::from_str(&form.items_json)
  .map_err(|e| abt_core::shared::types::error::DomainError::validation(format!("无效申请数据: {e}")))?;
 if items.is_empty() {
  return Err(abt_core::shared::types::error::DomainError::validation("请至少填写一行数量").into());
 }
 let mut tx = state.pool.begin().await
  .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 state.picking_service().request_from_order(&service_ctx, &mut tx, path.id, items, form.shipping_requirements).await?;
 tx.commit().await
  .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let redirect = OrderDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn request_ship_modal_body(
 order: &SalesOrder,
 items: &[SalesOrderItem],
 codes: &HashMap<i64, String>,
 names: &HashMap<i64, String>,
) -> Markup {
 html! {
    div id="request-ship-modal"
        class="modal-overlay fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-100 pointer-events-auto"
        _="on click[me is event.target] remove me"
    {
        div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
            div class="px-6 py-4 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="text-base font-semibold text-fg" { "申请发货 · " (order.doc_number) }
                button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                    _="on click remove closest .modal-overlay" { "×" }
            }
            form id="request-ship-form" class="flex flex-col flex-1 min-h-0"
                hx-post=(RequestShipPath { id: order.id }.to_string())
                hx-swap="none"
                onsubmit="return collectRequestShipItems()" {
                div class="overflow-auto flex-1 px-5 py-3" {
                    table class="data-table" {
                        thead tr {
                            th { "产品" }
                            th class="text-right" { "订单数" }
                            th class="text-right" { "已发" }
                            th class="text-right" { "未发" }
                            th class="text-right" { "本次申请" }
                        }
                        tbody {
                            @for it in items {
                                @let open = it.open_qty();
                                tr {
                                    td {
                                        div class="font-mono text-xs text-fg-2" { (codes.get(&it.product_id).cloned().unwrap_or_default()) }
                                        div class="text-sm text-fg truncate max-w-[220px]" {
                                            (names.get(&it.product_id).cloned().unwrap_or_else(|| format!("#{}", it.product_id)))
                                        }
                                    }
                                    td class="text-right text-sm font-mono" { (fmt_qty(it.quantity)) }
                                    td class="text-right text-sm font-mono" { (fmt_qty(it.shipped_qty)) }
                                    td class="text-right text-sm font-mono" { (fmt_qty(open)) }
                                    td {
                                        input type="number" name="qty" data-order-item-id=(it.id)
                                            value=(open.to_string()) max=(open.to_string()) min="0" step="any"
                                            class="w-[110px] px-2 py-1 border border-border rounded-sm text-sm text-right font-mono";
                                    }
                                }
                            }
                            @if items.is_empty() {
                                tr { td colspan="5" class="text-center text-muted py-6" { "无可申请的订单行" } }
                            }
                        }
                    }
                }
                div class="px-5 py-3 border-t border-border-soft" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" for="shipping-requirements" { "发货要求（选填）" }
                    textarea name="shipping_requirements" id="shipping-requirements"
                        maxlength="200" rows="2"
                        placeholder="请输入发货要求，如指定快递或包装需求（选填，上限 200 字）"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg focus:border-accent focus:shadow-[var(--shadow-focus)] resize-y min-h-[72px]" {};
                }
                input type="hidden" name="items_json" id="request-ship-items-json" value="[]" {};
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                    button type="button" class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm cursor-pointer"
                        _="on click remove closest .modal-overlay" { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-accent text-accent-on text-sm font-medium cursor-pointer hover:bg-accent-hover"
                        { (icon::truck_icon("w-4 h-4")) "提交申请" }
                }
            }
        }
    }
    (maud::PreEscaped(r#"<script>
function collectRequestShipItems() {
  var rows = document.querySelectorAll('#request-ship-form input[name="qty"]');
  var items = [];
  rows.forEach(function(r) {
    var qty = parseFloat(r.value);
    if (qty > 0) items.push({order_item_id: parseInt(r.dataset.orderItemId), requested_qty: String(qty)});
  });
  document.getElementById('request-ship-items-json').value = JSON.stringify(items);
  if (items.length === 0) { alert('请至少填写一行数量（大于0）'); return false; }
  return true;
}
</script>"#))
 }
}

// ── Workflow Steps ──

fn workflow_steps(current: SalesOrderStatus) -> Markup {
 let steps: &[(&str, SalesOrderStatus)] = &[
 ("草稿", SalesOrderStatus::Draft),
 ("已确认", SalesOrderStatus::Confirmed),
 ("待发货", SalesOrderStatus::ReadyToShip),
 ("部分发货", SalesOrderStatus::PartiallyShipped),
 ("已发货", SalesOrderStatus::Shipped),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == SalesOrderStatus::Cancelled;

 html! {
    div class="flex items-center mb-6" {
        @for (i, (label, _)) in steps.iter().enumerate() {
            @if i > 0 {
                div class=({
                        format!(
                            "w-[48px] h-[2px] {}",
                            if i <= current_idx && !is_cancelled {
                                "bg-success"
                            } else {
                                "bg-border"
                            },
                        )
                    }) {}
            }
            @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
                ("bg-border-soft", "text-muted", "")
            } else if i < current_idx {
                ("bg-success", "text-success", "")
            } else if i == current_idx {
                (
                    "bg-accent",
                    "text-accent font-semibold",
                    "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]",
                )
            } else {
                ("bg-slate-300", "text-slate-400", "")
            };
            div class="flex items-center gap-2 shrink-0" {
                span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
                span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
            }
        }
        @if is_cancelled {
            div class="w-[48px] h-[2px] bg-border" {}
            div class="flex items-center gap-2 shrink-0" {
                span class="w-2.5 h-2.5 rounded-full shrink-0 bg-danger-500" {}
                span class="text-xs text-danger-500 font-semibold whitespace-nowrap" { "已取消" }
            }
        }
    }
}
}

fn order_detail_page(
 o: &SalesOrder,
 items: &[SalesOrderItem],
 plan_lines: &[FulfillmentPlanLine],
 customer_name: &str,
 contact: &Option<ContactInfo>,
 sales_rep: &str,
 product_names: &HashMap<i64, String>,
 product_codes: &HashMap<i64, String>,
 atp_map: &HashMap<i64, Decimal>,
 demand_map: &HashMap<i64, DemandStatus>,
 reserved_map: &HashMap<i64, Decimal>,
 producing_count: usize,
 purchasing_count: usize,
 order_id: i64,
 print_templates: &[PrintTemplate],
) -> Markup {
 let (status_text, status_class) = status_label(o.status);
 let contact_name = contact.as_ref().map(|c| c.name.as_str()).unwrap_or("—");
 let contact_phone = contact.as_ref().and_then(|c| c.phone.as_deref()).unwrap_or("—");
 html! {
    div {
        // 隐藏 iframe：打印按钮 set src 后，print 响应自带 window.print()
        iframe id="print-frame" class="hidden" {}
        // ── Back Link ──
        a   class="inline-flex items-center gap-1 text-sm text-muted hover:text-accent transition-colors mb-4 icon:w-4 icon:h-4"
            href=(format!("{}?restore=true", OrderListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回销售订单列表" }
        // ── Detail Header (flex layout, matching prototype) ──
        div class="flex items-start justify-between mb-6" {
            div class="flex items-center gap-3" {
                h1 class="text-xl font-bold font-mono tabular-nums text-fg" { (o.doc_number) }
                span class=(format!("status-pill {}", crate::utils::status_color(status_class))) {
                    (status_text)
                }
            }
            div class="flex gap-2" {
                (print_dropdown(
                    "print-frame",
                    &OrderPrintPath { id: o.id }.to_string(),
                    print_templates,
                    &format!("{}?document_type=sales_order", PrintTemplateListPath::PATH),
                    false,
                ))
                @if {
                    matches!(
                        o.status,
                        SalesOrderStatus::Confirmed
                            | SalesOrderStatus::ReadyToShip
                            | SalesOrderStatus::PartiallyShipped
                            | SalesOrderStatus::ShippingRequested
                    )
                } {
                    button
                        class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-get=(RequestShipPath { id: o.id }.to_string())
                        hx-target="body"
                        hx-swap="beforeend"
                    { (icon::truck_icon("w-4 h-4")) "申请发货" }
                }
                @if o.status == SalesOrderStatus::Draft {
                    button
                        class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=(ConfirmOrderPath { id: o.id }.to_string())
                        hx-confirm="确认审核此订单？"
                    { "确认订单" }
                }
                @if {
                    matches!(
                        o.status,
                        SalesOrderStatus::Draft
                            | SalesOrderStatus::Confirmed
                            | SalesOrderStatus::ReadyToShip
                            | SalesOrderStatus::ShippingRequested
                    )
                } {
                    button
                        class="inline-flex items-center gap-2 py-[6px] px-3 text-[13px] rounded-sm bg-danger-bg text-danger border border-[rgba(207,19,34,0.2)] hover:bg-danger-100 font-medium cursor-pointer transition-all duration-150"
                        hx-post=(CancelOrderPath { id: o.id }.to_string())
                        hx-confirm="确认取消此订单？取消后不可恢复。"
                    { "取消订单" }
                }
            }
        }
        // ── Smart Buttons ──
        @if producing_count > 0 || purchasing_count > 0 {
            div class="flex gap-3 mb-6" {
                @if producing_count > 0 {
                    a   class="inline-flex items-center gap-2 px-4 py-2 rounded-md border border-border-soft bg-bg shadow-xs hover:shadow-md transition-shadow text-sm"
                        href=(format!("/admin/mes/work-center?order_id={}", order_id))
                    {
                        span class="text-lg font-bold text-accent font-mono tabular-nums" {
                            (producing_count)
                        }
                        span class="text-muted" { "自制需求" }
                    }
                }
                @if purchasing_count > 0 {
                    a   class="inline-flex items-center gap-2 px-4 py-2 rounded-md border border-border-soft bg-bg shadow-xs hover:shadow-md transition-shadow text-sm"
                        href=(format!("/admin/purchase/work-center?order_id={}", order_id))
                    {
                        span class="text-lg font-bold text-warn font-mono tabular-nums" {
                            (purchasing_count)
                        }
                        span class="text-muted" { "采购需求" }
                    }
                }
            }
        }
        // ── Workflow Steps ──
        (workflow_steps(o.status))
        // ── Fulfillment Progress ──
        (fulfillment_progress(items, plan_lines))
        // ── Order Info ──
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-5 shadow-[var(--shadow-card)]"
        {
            div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "订单信息"
            }
            div class="grid grid-cols-2 md:grid-cols-3 gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "客户名称" }
                    span class="text-sm text-fg font-medium" { (customer_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "联系人" }
                    span class="text-sm text-fg font-medium" { (contact_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "联系电话" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (contact_phone)
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "业务员" }
                    span class="text-sm text-fg font-medium" { (sales_rep) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "交货日期" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (o.order_date.format("%Y-%m-%d"))
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "付款条款" }
                    span class="text-sm text-fg font-medium" { (o.payment_terms.as_str()) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "交货条款" }
                    span class="text-sm text-fg font-medium" { (o.delivery_terms.as_str()) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "交货地址" }
                    span class="text-sm text-fg font-medium" { (o.delivery_address.as_str()) }
                }
            }
        }
        // ── Items Table (四量模型) ──
        div class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品信息" }
                            th { "单位" }
                            th class="text-right text-[13px]" { "订单量" }
                            th class="text-right text-[13px]" { "已发货" }
                            th class="text-right text-[13px]" { "已取消" }
                            th class="text-right text-[13px]" { "未交量" }
                            th class="text-right text-[13px]" { "单价" }
                            th class="text-right text-[13px]" { "小计" }
                            th { "行状态" }
                            th { "交货日期" }
                            th { "备注" }
                        }
                    }
                    tbody {
                        @for item in items { (item_row(item, product_names, product_codes)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="12" class="text-center p-8 text-muted" { "暂无明细" }
                            }
                        }
                    }
                }
            }
            div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised" {
                div class="flex gap-3" {
                    span class="text-[11px] text-muted font-medium uppercase" { "成本合计" }
                    span class="text-[20px] font-bold text-fg" {
                        (crate::utils::fmt_amount(o.total_cost))
                    }
                }
                div class="flex gap-3" {
                    span class="text-[11px] text-muted font-medium uppercase" { "订单总额" }
                    span class="text-[20px] font-bold text-fg accent" {
                        (crate::utils::fmt_amount(o.total_amount))
                    }
                }
            }
        }
        // ── Fulfillment Workbench ──
        ({
            fulfillment_workbench(
                plan_lines,
                product_names,
                product_codes,
                atp_map,
                demand_map,
                reserved_map,
                order_id,
            )
        })
        // ── 预留明细 Drawer（共享；缺口表「被占用」徽标触发）──
        (crate::components::reservation_detail::reservation_detail_drawer())
        // ── Remarks ──
        @if !o.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6"
            {
                div class="text-sm font-semibold text-fg mb-4" { "备注" }
                p class="text-muted" { (o.remark.as_str()) }
            }
        }
    }
}
}

fn item_row(
 item: &SalesOrderItem,
 names: &HashMap<i64, String>,
 codes: &HashMap<i64, String>,
) -> Markup {
 let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
 let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
 let open_qty = item.open_qty();
 let (ls_label, ls_class) = line_status_pill(item.line_status);

 html! {
    tr {
        td class="font-mono tabular-nums" { (item.line_no) }
        td {
            div class="flex flex-col gap-0.5" {
                span class="text-sm text-fg font-medium" { (product_name) }
                span class="text-xs text-muted font-mono" { (product_code) }
            }
        }
        td { (item.unit.as_str()) }
        td class="text-right text-[13px]" { (fmt_qty(item.quantity)) }
        td class="text-right text-[13px]" { (fmt_qty(item.shipped_qty)) }
        td class="text-right text-[13px]" { (fmt_qty(item.cancelled_qty)) }
        td class="text-right text-[13px]" {
            @if open_qty > Decimal::ZERO {
                span class="text-danger" { (fmt_qty(open_qty)) }
            } @else { (fmt_qty(open_qty)) }
        }
        td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.unit_price)) }
        td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.amount)) }
        td {
            span class=(format!("status-pill {}", crate::utils::status_color(ls_class))) {
                (ls_label)
            }
        }
        td class="font-mono tabular-nums" { (delivery) }
        td class="text-muted text-xs" { (item.remark.as_str()) }
    }
}
}
