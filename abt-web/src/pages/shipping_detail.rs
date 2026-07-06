use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::print_template::{PrintTemplate, PrintTemplateService};
use abt_core::master_data::product::model::Product;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::wms::picking::model::*;
use abt_core::wms::picking::PickingService;
use abt_core::wms::enums::PickingStatus;
use abt_core::wms::inventory_transaction::model::InventoryTransaction;
use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::components::print_dropdown::{print_dropdown, PrintParam};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::print_template::PrintTemplateListPath;
use crate::routes::shipping::*;
use crate::utils::fmt_qty;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PickingStatus) -> (&'static str, &'static str) {
 match s {
 PickingStatus::Draft => ("草稿", "status-draft"),
 PickingStatus::Confirmed => ("已确认", "status-confirmed"),
 PickingStatus::Done => ("已发货", "status-shipped"),
 PickingStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

struct ProductDetail {
 code: String,
 name: String,
 spec: Option<String>,
 unit: Option<String>,
}

// ── Handlers ──

#[require_permission("SHIPPING", "read")]
pub async fn get_shipping_detail(
 path: ShippingDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let shipping_svc = state.picking_service();
 let customer_svc = state.customer_service();
 let order_svc = state.sales_order_service();
 let product_svc = state.product_service();
 let _warehouse_svc = state.warehouse_service();
 let user_svc = state.user_service();
 let shipping = shipping_svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

 let items = shipping_svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 let customer_name = match shipping.partner_id {
 Some(cid) => customer_svc.get(&service_ctx, &mut conn, cid)
 .await.map(|c| c.name).unwrap_or_else(|_| "未知客户".into()),
 None => "未知客户".into(),
 };
 let order_number = match shipping.source_id {
 Some(oid) => order_svc.find_by_id(&service_ctx, &mut conn, oid)
 .await.map(|o| o.doc_number).unwrap_or_else(|_| "—".into()),
 None => "—".into(),
 };
 let operator_name = user_svc.get_user(&service_ctx, &mut conn, shipping.operator_id)
 .await.map(|u| u.display_name.unwrap_or(u.username)).unwrap_or_else(|_| "—".into());

 // Resolve product details via product service
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let product_details: HashMap<i64, ProductDetail> = if product_ids.is_empty() {
 HashMap::new()
 } else {
 product_svc.get_by_ids(&service_ctx, &mut conn, product_ids)
 .await
 .map(|products| products.into_iter().map(|p| {
 (p.product_id, ProductDetail {
 code: p.product_code,
 name: p.pdt_name,
 spec: Some(p.meta.specification),
 unit: Some(p.unit),
 })
 }).collect())
 .unwrap_or_default()
 };

 // 行级仓库已移除（发货仓在 picking 头 from_warehouse_id）；warehouse_names 留空兼容 item_row 签名
 let warehouse_names: HashMap<i64, String> = HashMap::new();

 let hub_summary = shipping_svc
     .hub_summary(&service_ctx, &mut conn, path.id)
     .await
     .unwrap_or(ShippingHubSummary {
         pending_ship_qty: rust_decimal::Decimal::ZERO,
         shipped_qty: rust_decimal::Decimal::ZERO,
         shortage: None,
     });
 let print_templates = state
     .print_template_service()
     .list_by_document_type(&mut conn, "delivery_note")
     .await
     .unwrap_or_default();
 let content = shipping_detail_page(&shipping, &items, &customer_name, &order_number, &operator_name, &product_details, &warehouse_names, &hub_summary, &print_templates);
 let page_html = admin_page(
 is_htmx, "发货详情", &claims, "sales",
 &format!("{}/{}", ShippingListPath::PATH, path.id),
 "销售管理", Some("发货详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SHIPPING", "update")]
pub async fn confirm_shipping(
 path: ConfirmShippingPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 // confirm 是多步写（状态机 + status + 审计），事务包裹防半失败残留
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.picking_service();
 svc.confirm(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SHIPPING", "update")]
pub async fn cancel_shipping(
 path: CancelShippingPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;

 // cancel 是多步写（状态机 + status + 审计），事务包裹防半失败残留
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.picking_service();
 svc.cancel(&service_ctx, &mut tx, path.id).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = ShippingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// 打印发货单：用默认 delivery_note 模板 + 真实业务数据渲染，返回完整 HTML 供浏览器打印。
#[require_permission("SHIPPING", "read")]
pub async fn print_shipping(
    path: ShippingPrintPath,
    ctx: RequestContext,
    Query(param): Query<PrintParam>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let shipping_svc = state.picking_service();
    let customer_svc = state.customer_service();
    let order_svc = state.sales_order_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();
    let print_svc = state.print_template_service();

    let shipping = shipping_svc
        .find_by_id(&service_ctx, &mut conn, path.id)
        .await?;
    let items = shipping_svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    // ── 客户信息 ──
    let customer_name = match shipping.partner_id {
        Some(cid) => customer_svc
            .get(&service_ctx, &mut conn, cid)
            .await
            .map(|c| c.name)
            .unwrap_or_default(),
        None => String::new(),
    };

    // 联系人 + 地址（取首选）
    let (contact_phone, contact_name, customer_address) = match shipping.partner_id {
        Some(cid) => {
            let contacts = customer_svc
                .list_contacts(&service_ctx, &mut conn, cid)
                .await
                .unwrap_or_default();
            let addresses = customer_svc
                .list_addresses(&service_ctx, &mut conn, cid)
                .await
                .unwrap_or_default();
            let primary_contact = contacts.iter().find(|c| c.is_primary).or(contacts.first());
            let default_addr = addresses.iter().find(|a| a.is_default).or(addresses.first());
            let phone = primary_contact
                .and_then(|c| c.phone.clone())
                .unwrap_or_default();
            let contact = primary_contact
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let addr = default_addr
                .map(|a| {
                    format!(
                        "{}{}{} {}",
                        a.province,
                        a.city,
                        a.district.as_deref().unwrap_or(""),
                        a.detail
                    )
                })
                .unwrap_or_default();
            (phone, contact, addr)
        }
        None => (String::new(), String::new(), String::new()),
    };

    // ── 销售订单信息（金额 / 收货地址 / 销售员）──
    let (order_total, order_delivery_addr, sales_rep_name) = match shipping.source_id {
        Some(oid) => {
            match order_svc.find_by_id(&service_ctx, &mut conn, oid).await {
                Ok(o) => {
                    let rep_name = user_svc
                        .get_user(&service_ctx, &mut conn, o.sales_rep_id)
                        .await
                        .map(|u| u.display_name.unwrap_or(u.username))
                        .unwrap_or_default();
                    (o.total_amount, o.delivery_address, rep_name)
                }
                Err(_) => (rust_decimal::Decimal::ZERO, String::new(), String::new()),
            }
        }
        None => (rust_decimal::Decimal::ZERO, String::new(), String::new()),
    };

    // 收货地址优先用订单上的，其次客户默认地址
    let final_address = if order_delivery_addr.is_empty() {
        customer_address
    } else {
        order_delivery_addr
    };

    // ── 状态文案 ──
    let status_text = match shipping.status {
        PickingStatus::Draft => "草稿",
        PickingStatus::Confirmed => "已确认",
        PickingStatus::Done => "已发货",
        PickingStatus::Cancelled => "已取消",
    };

    // ── 产品明细 ──
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_map: HashMap<i64, Product> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .map(|products| products.into_iter().map(|p| (p.product_id, p)).collect())
            .unwrap_or_default()
    };

    let detail_items: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let product = product_map.get(&item.product_id);
            serde_json::json!({
                "产品编码": product.map(|p| p.product_code.as_str()).unwrap_or(""),
                "产品名称": product.map(|p| p.pdt_name.as_str()).unwrap_or(""),
                "规格型号": product.map(|p| p.meta.specification.as_str()).unwrap_or(""),
                "单位": product.map(|p| p.unit.as_str()).unwrap_or(""),
                "本次出库数量": fmt_qty(item.qty_requested),
                "批次号": item.batch_no.as_deref().unwrap_or(""),
                "单价": "",
                "金额": "",
                "行备注": item.remark.as_str(),
            })
        })
        .collect();

    // ── 渲染 ──
    let vars = serde_json::json!({
        "出库单号": shipping.doc_number,
        "出库日期": shipping.created_at.format("%Y-%m-%d").to_string(),
        "计划日期": shipping.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
        "客户全称": customer_name,
        "收货地址": final_address,
        "联系电话": contact_phone,
        "收货人": contact_name,
        "客户经理": sales_rep_name,
        "订单总金额": order_total.to_string(),
        "大写金额": "",
        "备注": shipping.remark,
        "单据状态": status_text,
        "明细": detail_items,
        "公司名称": "江门市艾伯特照明科技有限公司",
        "打印时间": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });

    let html = match param.template_id {
        Some(tid) => print_svc.render(&mut conn, tid, vars).await?,
        None => print_svc
            .render_default(&mut conn, "delivery_note", vars)
            .await?,
    };

    // 自动弹出打印对话框
    Ok(Html(format!(
        "{html}<script>window.onload=function(){{window.print()}}</script>"
    )))
}

// ── Workflow Steps ──

fn workflow_steps(current: PickingStatus) -> Markup {
 let steps: &[(&str, PickingStatus)] = &[
 ("草稿", PickingStatus::Draft),
 ("已确认", PickingStatus::Confirmed),
 ("已发货", PickingStatus::Done),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == PickingStatus::Cancelled;

 html! {
    div class="flex items-center mt-6 mb-6" {
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

// ── Components ──

fn shipping_detail_page(
 s: &StockPicking,
 items: &[StockPickingItem],
 customer_name: &str,
 order_number: &str,
 operator_name: &str,
 product_details: &HashMap<i64, ProductDetail>,
 warehouse_names: &HashMap<i64, String>,
 hub_summary: &ShippingHubSummary,
 print_templates: &[PrintTemplate],
) -> Markup {
 let (status_text, status_class) = status_label(s.status);

 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", ShippingListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回发货申请列表" }
        // ── Detail Header ──
        div class="block bg-bg border border-border-soft rounded-lg p-6" {
            div {
                div class="flex items-center justify-between" {
                    h1 class="text-2xl font-extrabold font-mono tabular-nums" { (s.doc_number) }
                    span class=({
                        format!(
                            "status-pill {}",
                            crate::utils::status_color(status_class),
                        )
                    }) { (status_text) }
                }
                // 来源链 + 缺货徽章（Doc Hub）
                div class="flex items-center gap-3 mt-3 flex-wrap" {
                    @if let Some(shortage) = &hub_summary.shortage {
                        span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-xs font-semibold" {
                            (icon::circle_alert_icon("w-3.5 h-3.5"))
                            "缺货 · " (shortage.product_name)
                        }
                    }
                    div class="flex items-center gap-2 text-xs text-muted bg-surface border border-border-soft rounded-md px-3 py-1.5 flex-wrap min-w-0" {
                        span class="font-medium" { "来源链" }
                        @if let Some(oid) = s.source_id {
                            a class="text-accent font-mono" href=(format!("/admin/orders/{oid}")) { (order_number) }
                            span class="text-border" { "→" }
                        }
                        span class="text-fg font-mono font-semibold" { (s.doc_number) }
                    }
                }
                // 摘要带 stat-strip（待发 / 已发 / 库存）
                div class="flex items-stretch bg-bg border border-border-soft rounded-lg mt-4 overflow-hidden" {
                    div class="flex-1 px-5 py-3 flex flex-col gap-0.5 border-r border-border-soft" {
                        span class="font-mono text-lg font-bold text-fg tabular-nums" { (fmt_qty(hub_summary.pending_ship_qty)) }
                        span class="text-xs text-muted font-medium" { "待发" }
                    }
                    div class="flex-1 px-5 py-3 flex flex-col gap-0.5 border-r border-border-soft" {
                        span class="font-mono text-lg font-bold text-fg tabular-nums" { (fmt_qty(hub_summary.shipped_qty)) }
                        span class="text-xs text-muted font-medium" { "已发" }
                    }
                    div class="flex-1 px-5 py-3 flex flex-col gap-0.5" {
                        @if hub_summary.shortage.is_some() {
                            span class="font-mono text-lg font-bold text-danger tabular-nums" { "缺货" }
                        } @else {
                            span class="font-mono text-lg font-bold text-success tabular-nums" { "充足" }
                        }
                        span class="text-xs text-muted font-medium" { "库存" }
                    }
                }
            }
            div class="flex gap-3" {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", ShippingListPath::PATH))
                { "返回列表" }
                (print_dropdown(
                    "print-frame",
                    &ShippingPrintPath { id: s.id }.to_string(),
                    print_templates,
                    &format!("{}?document_type=delivery_note", PrintTemplateListPath::PATH),
                    false,
                ))
                @if s.status == PickingStatus::Draft {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=(ConfirmShippingPath { id: s.id }.to_string())
                        hx-confirm="确认审核此发货单？"
                    { "确认发货" }
                }
                @if matches!(s.status, PickingStatus::Draft | PickingStatus::Confirmed) {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                        hx-post=(CancelShippingPath { id: s.id }.to_string())
                        hx-confirm="确认取消此发货单？"
                    { "取消" }
                }
            }
        }
        // ── Workflow Steps ──
        (workflow_steps(s.status))
        // ── 发货信息 disclosure ──
        (doc_disclosure("d-info", "发货信息", &format!("客户 {}", customer_name), html! {
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "客户名称" }
                    span class="text-sm text-fg font-medium" { (customer_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "预计发货日期" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        ({
                            s.scheduled_date
                                .map(|d| d.format("%Y-%m-%d").to_string())
                                .unwrap_or_else(|| "—".into())
                        })
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "操作员" }
                    span class="text-sm text-fg font-medium" { (operator_name) }
                }
            }
        }))
        // ── 发货明细 disclosure ──
        (doc_disclosure("d-items", "发货明细", &format!("{} 行", items.len()), html! {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th { "规格描述" }
                            th { "单位" }
                            th class="text-right text-[13px]" { "申请数量" }
                            th class="text-right text-[13px]" { "已发货" }
                            th { "发货仓库" }
                        }
                    }
                    tbody {
                        @for item in items { (item_row(item, product_details, warehouse_names)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="8" class="text-center p-8 text-muted" { "暂无明细" }
                            }
                        }
                    }
                }
            }
        }))
        // ── 库存事务 disclosure（懒加载）──
        (doc_disclosure_lazy("d-txn", "库存事务", "本单库存流水", &ShippingFragmentPath { id: s.id, block: "transactions".to_string() }.to_string()))
        // ── Remarks ──
        @if !s.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] mt-6"
            {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                    "备注"
                }
                p class="text-muted" { (s.remark.as_str()) }
            }
        }
    }
    // 隐藏 iframe：打印按钮触发时加载打印内容，load 后自动唤起打印对话框
    iframe id="print-frame" class="hidden" {}
}
}

/// Doc Hub 折叠区块：header（标题+摘要+chevron）+ 可折叠 body。点击 toggle .hidden。
fn doc_disclosure(id: &str, title: &str, summary: &str, body: Markup) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg mb-3 shadow-xs overflow-hidden" id=(id) {
            div class="flex items-center gap-3 px-5 py-4 cursor-pointer select-none hover:bg-surface-raised transition-colors"
                _="on click toggle .hidden on next .di-body" {
                span class="text-sm font-semibold text-fg shrink-0" { (title) }
                span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" { (summary) }
                (icon::chevron_down_icon("w-4 h-4 text-muted shrink-0"))
            }
            div class="di-body hidden px-5 pb-5 pt-4 border-t border-border-soft" { (body) }
        }
    }
}

/// Doc Hub 懒加载 disclosure：header hx-get 拉片段，body 默认空
fn doc_disclosure_lazy(id: &str, title: &str, summary: &str, frag_url: &str) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg mb-3 shadow-xs overflow-hidden" id=(id) {
            div class="flex items-center gap-3 px-5 py-4 cursor-pointer select-none hover:bg-surface-raised transition-colors"
                hx-get=(frag_url)
                hx-target="next .di-body"
                hx-swap="innerHTML"
                _="on click toggle .hidden on next .di-body" {
                span class="text-sm font-semibold text-fg shrink-0" { (title) }
                span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" { (summary) }
                (icon::chevron_down_icon("w-4 h-4 text-muted shrink-0"))
            }
            div class="di-body hidden px-5 pb-5 pt-4 border-t border-border-soft" {}
        }
    }
}

/// Doc Hub disclosure 懒加载端点：按 block 返回拣货单 / 库存事务片段
#[require_permission("SHIPPING", "read")]
pub async fn get_shipping_fragment(path: ShippingFragmentPath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let body = match path.block.as_str() {
        "transactions" => {
            let res = state.picking_service()
                .list_transactions(&service_ctx, &mut conn, path.id, PageParams::new(1, 50))
                .await.unwrap_or_else(|_| PaginatedResult::empty(1, 50));
            render_txn_body(&res.items)
        }
        other => return Err(abt_core::shared::types::error::DomainError::validation(format!("未知区块: {other}")).into()),
    };
    Ok(Html(body.into_string()))
}

fn render_txn_body(txns: &[InventoryTransaction]) -> Markup {
    if txns.is_empty() {
        return html! { div class="text-sm text-muted py-2" { "暂无库存事务（发货后此处显示扣减流水）" } };
    }
    html! {
        table class="w-full border-collapse" {
            thead {
                tr {
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "时间" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "类型" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "产品" }
                    th class="text-right text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "数量" }
                }
            }
            tbody {
                @for t in txns {
                    tr class="border-b border-border-soft last:border-b-0" {
                        td class="py-2 px-2 text-sm font-mono text-muted" { (t.created_at.format("%m-%d %H:%M").to_string()) }
                        td class="py-2 px-2 text-sm text-fg-2" { (format!("{:?}", t.transaction_type)) }
                        td class="py-2 px-2 text-sm font-mono" { "#" (t.product_id) }
                        td class=(format!("py-2 px-2 text-sm font-mono text-right {}", if t.quantity >= rust_decimal::Decimal::ZERO { "text-success" } else { "text-danger" })) { (fmt_qty(t.quantity)) }
                    }
                }
            }
        }
    }
}

fn item_row(
 item: &StockPickingItem,
 details: &HashMap<i64, ProductDetail>,
 _warehouses: &HashMap<i64, String>,
) -> Markup {
 let detail = details.get(&item.product_id);
 let product_code = detail.map(|d| d.code.as_str()).unwrap_or("—");
 let product_name = detail.map(|d| d.name.as_str()).unwrap_or("—");
 let spec = detail.and_then(|d| d.spec.as_deref()).unwrap_or("—");
 let unit = detail.and_then(|d| d.unit.as_deref()).unwrap_or("—");

 html! {
    tr {
        td class="font-mono tabular-nums" { (product_code) }
        td { (product_name) }
        td { (spec) }
        td { (unit) }
        td class="text-right text-[13px]" { (fmt_qty(item.qty_requested)) }
        td class="text-right text-[13px]" { (fmt_qty(item.qty_done)) }
    }
}
}
