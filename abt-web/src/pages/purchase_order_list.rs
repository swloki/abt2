use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::identity::UserService;
use abt_core::purchase::enums::{InvoiceStatus, PurchaseOrderStatus};
use abt_core::purchase::order::model::*;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct POQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub supplier_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_range: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
 let today = chrono::Local::now().date_naive();
 match range {
 "7d" => (Some(today - chrono::Days::new(7)), None),
 "30d" => (Some(today - chrono::Days::new(30)), None),
 "3m" => (Some(today - chrono::Months::new(3)), None),
 _ => (None, None),
 }
}

fn build_filter(params: &POQueryParams) -> PurchaseOrderQuery {
 let (order_date_start, order_date_end) = params
 .date_range
 .as_deref()
 .map(parse_date_range)
 .unwrap_or((None, None));
 // 筛选值 2 = 「在途订单」= Confirmed(未到货) ∪ PartiallyReceived(部分到货)；其余按单状态
 let (status, statuses) = match params.status {
  Some(2) => (
   None,
   Some(vec![
    PurchaseOrderStatus::Confirmed,
    PurchaseOrderStatus::PartiallyReceived,
   ]),
  ),
  other => (other.and_then(PurchaseOrderStatus::from_i16), None),
 };
 PurchaseOrderQuery {
 supplier_id: params.supplier_id,
 status,
 statuses,
 order_date_start,
 order_date_end,
 ..Default::default()
 }
}

async fn resolve_supplier_names<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 orders: &[PurchaseOrder],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = orders.iter().map(|o| o.supplier_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 let all = svc
 .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 200))
 .await;
 match all {
 Ok(result) => result
 .items
 .into_iter()
 .filter(|s| ids.contains(&s.id))
 .map(|s| (s.id, s.name))
 .collect(),
 Err(_) => HashMap::new(),
 }
}

async fn resolve_buyer_names<S: UserService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 orders: &[PurchaseOrder],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = orders.iter().map(|o| o.operator_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 svc.get_users_by_ids(ctx, db, ids)
 .await
 .map(|users| {
 users.into_iter()
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect()
 })
 .unwrap_or_default()
}

// ── Status Labels ──

fn status_label(s: PurchaseOrderStatus) -> (&'static str, &'static str) {
 match s {
 PurchaseOrderStatus::Draft => ("草稿", "status-draft"),
 PurchaseOrderStatus::Confirmed => ("在途", "status-confirmed"),
 PurchaseOrderStatus::PartiallyReceived => ("部分收货", "status-partial"),
 PurchaseOrderStatus::Received => ("已收货", "status-success"),
 PurchaseOrderStatus::Closed => ("已关闭", "status-cancelled"),
 PurchaseOrderStatus::Cancelled => ("已取消", "status-cancelled"),
 PurchaseOrderStatus::PendingApproval => ("待审批", "status-pending"),
 }
}

fn invoice_status_label(s: InvoiceStatus) -> (&'static str, &'static str) {
 match s {
 InvoiceStatus::NoInvoice => ("未开票", "status-muted"),
 InvoiceStatus::ToInvoice => ("部分开票", "status-warning"),
 InvoiceStatus::FullyInvoiced => ("已开票", "status-success"),
 }
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_list(
 _path: POListPath,
 ctx: RequestContext,
 Query(params): Query<POQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("PURCHASE_ORDER", "create").await;
 let can_delete = ctx.has_permission("PURCHASE_ORDER", "delete").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();
 let user_svc = state.user_service();

 let filter = build_filter(&params);
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

 let supplier_names = resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
 let buyer_names = resolve_buyer_names(&user_svc, &service_ctx, &mut conn, &result.items).await;

 let suppliers = supplier_svc
 .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
 .await?;

 let content = po_list_page(&result, &supplier_names, &buyer_names, &suppliers.items, &params, can_create, can_delete);
 let page_html = admin_page(
 is_htmx, "采购订单", &claims, "purchase", POListPath::PATH, "采购管理", Some("采购订单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn po_list_page(
 result: &abt_core::shared::types::PaginatedResult<PurchaseOrder>,
 supplier_names: &HashMap<i64, String>,
 buyer_names: &HashMap<i64, String>,
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 params: &POQueryParams,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "采购订单" }
            div class="flex gap-3" {
                @if can_create {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        href=(POCreatePath::PATH)
                    { (icon::plus_icon("w-4 h-4")) "新建采购订单" }
                }
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    _="on click call mergeSelectedPOs()"
                { "合并选中" }
            }
        }
        // ── Tabs + Filter + Data Table (HTMX panel) ──
        ({
            po_table_fragment(
                result,
                supplier_names,
                buyer_names,
                suppliers,
                params,
                can_delete,
            )
        })
    }
}
}

fn po_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<PurchaseOrder>,
 supplier_names: &HashMap<i64, String>,
 buyer_names: &HashMap<i64, String>,
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
 params: &POQueryParams,
 can_delete: bool,
) -> Markup {
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: None },
 TabItem { value: "2".into(), label: "在途订单", count: None },
 TabItem { value: "4".into(), label: "已收货", count: None },
 TabItem { value: "5".into(), label: "已关闭", count: None },
 TabItem { value: "6".into(), label: "已取消", count: None },
 ];

 let selected_supplier = params.supplier_id.map(|id| id.to_string()).unwrap_or_default();
 let selected_range = params.date_range.as_deref().unwrap_or("");

 html! {
    div class="po-list-panel" {
        ({
            status_tabs_with_param(
                POListPath::PATH,
                "#po-data-card",
                "#po-filter-form",
                tabs,
                &active_value,
                "status",
            )
        })
        // ── Filter Bar ──
        form
            class="flex items-center gap-3 mb-5 flex-wrap filter-form"
            id="po-filter-form"
            hx-get=(POListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#po-data-card"
            hx-select="#po-data-card"
            hx-swap="outerHTML"
            hx-select-oob="#status-tabs"
            hx-include="#po-filter-form"
           
        {
            div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
            {
                (icon::search_icon(""))
                input
                    class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text"
                    name="keyword"
                    placeholder="搜索采购单号…"
                    value=(params.keyword.as_deref().unwrap_or(""));
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="supplier_id"
            {
                option value="" { "全部供应商" }
                @for s in suppliers {
                    option value=(s.id) selected[selected_supplier == s.id.to_string()] { (s.name) }
                }
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="date_range"
            {
                option value="" selected[selected_range.is_empty()] { "订单日期" }
                option value="7d" selected[selected_range == "7d"] { "最近7天" }
                option value="30d" selected[selected_range == "30d"] { "最近30天" }
                option value="3m" selected[selected_range == "3m"] { "最近3个月" }
            }
        }
        // ── Data Table ──
        div class="data-card" id="po-data-card" {
            table class="data-table" {
                thead {
                    tr {
                        th class="w-9" {
                            input
                                type="checkbox"
                                class="po-select-all"
                                _="on click toggle @checked on .po-checkbox" {}
                        }
                        th { "订单编号" }
                        th { "供应商名称" }
                        th { "订单日期" }
                        th { "预计到货" }
                        th { "状态" }
                        th { "开票" }
                        th class="text-right text-[13px]" { "总金额" }
                        th { "业务员" }
                        th class="!text-right" { "操作" }
                    }
                }
                tbody {
                    @for o in &result.items { (po_row(o, supplier_names, buyer_names, can_delete)) }
                    @if result.items.is_empty() {
                        tr {
                            td colspan="10" class="text-center text-muted py-8" { "暂无订单数据" }
                        }
                    }
                }
            }
            ({
                pagination(
                    POListPath::PATH,
                    "#po-data-card",
                    "#po-filter-form",
                    result.total,
                    result.page,
                    result.total_pages,
                )
            })
        }
    }
}
}

fn po_row(
 o: &PurchaseOrder,
 supplier_names: &HashMap<i64, String>,
 buyer_names: &HashMap<i64, String>,
 can_delete: bool,
) -> Markup {
 let detail_path = PODetailPath { id: o.id };
 let delete_path = PODeletePath { id: o.id };
 let (status_text, status_class) = status_label(o.status);
 let supplier_name = supplier_names.get(&o.supplier_id).map(|s| s.as_str()).unwrap_or("—");
 let buyer_name = buyer_names.get(&o.operator_id).map(|s| s.as_str()).unwrap_or("—");
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = o.status == PurchaseOrderStatus::Draft;

 html! {
    tr class="cursor-pointer" {
        td style="cursor:default" {
            @if is_draft {
                input type="checkbox" class="po-checkbox" value=(o.id) {}
            }
        }
        td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) {
            (o.doc_number)
        }
        td onclick=(&onclick) { (supplier_name) }
        td class="font-mono tabular-nums" onclick=(&onclick) { (o.order_date.format("%Y-%m-%d")) }
        td class="font-mono tabular-nums" onclick=(&onclick) {
            ({
                o.expected_delivery_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "—".into())
            })
        }
        td onclick=(&onclick) {
            span class=(format!("status-pill {}", crate::utils::status_color(status_class))) {
                (status_text)
            }
        }
        td onclick=(&onclick) {
            @let (inv_text, inv_class) = invoice_status_label(o.invoice_status);
            span class=(format!("status-pill {}", crate::utils::status_color(inv_class))) {
                (inv_text)
            }
        }
        td class="text-right text-[13px] font-mono tabular-nums" onclick=(&onclick) {
            (format!("{:.2}", o.total_amount))
        }
        td onclick=(&onclick) { (buyer_name) }
        td _="on click halt the event" {
            @if is_draft {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg icon:w-3.5 icon:h-3.5"
                {
                    a   class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer"
                        href=(POEditPath { id: o.id }.to_string())
                        title="编辑"
                    { (icon::edit_icon("w-4 h-4")) }
                    @if can_delete {
                        button
                            type="button"
                            class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger"
                            title="删除"
                            hx-confirm="确认删除该采购订单吗？"
                            hx-post=(delete_path)
                            hx-target="closest tr"
                            hx-swap="outerHTML swap:0.5s"
                        { (icon::trash_icon("w-4 h-4")) }
                    }
                }
            }
        }
    }
}
}
