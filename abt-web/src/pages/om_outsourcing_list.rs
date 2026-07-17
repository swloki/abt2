use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::{Supplier, SupplierQuery};
use abt_core::om::enums::{OutsourcingStatus, OutsourcingType, TrackingNodeType};
use abt_core::om::outsourcing_order::{OutsourcingOrderQuery, OutsourcingOrderService};
use abt_core::om::outsourcing_tracking::OutsourcingTrackingService;
use abt_core::shared::types::pagination::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
 OmOutsourcingCreatePath, OmOutsourcingDetailPath, OmOutsourcingListPath,
};
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OutsourcingQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub outsourcing_type: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub supplier_id: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_from: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub date_to: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
}

// ── Helpers ──

fn status_label(s: &OutsourcingStatus) -> (&'static str, &'static str, &'static str) {
 match s {
 OutsourcingStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
 OutsourcingStatus::Sent => ("已发送", "rgba(22,119,255,0.08)", "var(--accent)"),
 OutsourcingStatus::InProduction => ("生产中", "rgba(250,140,22,0.08)", "#fa8c16"),
 OutsourcingStatus::Delivered => ("已交付", "rgba(114,46,209,0.08)", "#722ed1"),
 OutsourcingStatus::Received => ("已收货", "rgba(82,196,26,0.08)", "var(--success)"),
 OutsourcingStatus::Closed => ("已关闭", "rgba(0,0,0,0.06)", "var(--muted)"),
 OutsourcingStatus::ConvertedToInternal => ("转自制", "rgba(22,119,255,0.08)", "var(--accent)"),
 OutsourcingStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
 }
}

fn type_label(t: &OutsourcingType) -> (&'static str, &'static str, &'static str) {
 match t {
 OutsourcingType::Full => ("整体委外", "rgba(22,119,255,0.08)", "var(--accent)"),
 OutsourcingType::Process => ("工序委外", "rgba(250,140,22,0.08)", "#fa8c16"),
 OutsourcingType::Material => ("材料委外", "rgba(114,46,209,0.08)", "#722ed1"),
 OutsourcingType::Rework => ("委外返工", "rgba(245,63,63,0.06)", "#f53f3f"),
 }
}

fn tracking_node_label(t: &TrackingNodeType) -> &'static str {
 match t {
 TrackingNodeType::SendMaterial => "已发料",
 TrackingNodeType::CarrierPickup => "承运取货",
 TrackingNodeType::SupplierReceived => "供应商收货",
 TrackingNodeType::InProduction => "生产中",
 TrackingNodeType::Shipped => "已发货",
 TrackingNodeType::IqcInspected => "IQC检验",
 TrackingNodeType::Warehoused => "已入库",
 }
}

fn format_amount(d: rust_decimal::Decimal) -> String {
 let f: f64 = d.try_into().unwrap_or(0.0);
 if f == 0.0 { return "0".to_string(); }
 let abs = f.abs();
 if abs >= 1_000_000.0 {
 format!("{:.1}M", f / 1_000_000.0)
 } else {
 let formatted = format!("{:.2}", f);
 let parts: Vec<&str> = formatted.split('.').collect();
 let int_str = parts[0];
 let mut result = String::new();
 for (i, c) in int_str.chars().rev().enumerate() {
 if i > 0 && i % 3 == 0 { result.insert(0, ','); }
 result.insert(0, c);
 }
 let dec = parts[1].trim_end_matches('0');
 if dec.is_empty() { result } else { format!("{result}.{dec}") }
 }
}

fn parse_status(s: &str) -> Option<OutsourcingStatus> {
 match s {
 "Draft" => Some(OutsourcingStatus::Draft),
 "Sent" => Some(OutsourcingStatus::Sent),
 "InProduction" => Some(OutsourcingStatus::InProduction),
 "Delivered" => Some(OutsourcingStatus::Delivered),
 "Received" => Some(OutsourcingStatus::Received),
 "Closed" => Some(OutsourcingStatus::Closed),
 "ConvertedToInternal" => Some(OutsourcingStatus::ConvertedToInternal),
 "Cancelled" => Some(OutsourcingStatus::Cancelled),
 _ => None,
 }
}

fn parse_type(s: &str) -> Option<OutsourcingType> {
 match s {
 "Full" => Some(OutsourcingType::Full),
 "Process" => Some(OutsourcingType::Process),
 "Material" => Some(OutsourcingType::Material),
 "Rework" => Some(OutsourcingType::Rework),
 _ => None,
 }
}

fn build_filter(params: &OutsourcingQueryParams) -> OutsourcingOrderQuery {
 OutsourcingOrderQuery {
 status: params.status.as_deref().and_then(parse_status),
 supplier_id: params.supplier_id.as_deref().and_then(|s| s.parse::<i64>().ok()),
 outsourcing_type: params.outsourcing_type.as_deref().and_then(parse_type),
 work_order_id: None,
 routing_id: None,
 batch_id: None,
 date_range: match (params.date_from.as_deref(), params.date_to.as_deref()) {
 (Some(from), Some(to)) => {
 let f = from.parse().ok();
 let t = to.parse().ok();
 match (f, t) {
 (Some(f), Some(t)) => Some((f, t)),
 _ => None,
 }
 }
 _ => None,
 },
 keyword: params.keyword.clone(),
 }
}

async fn resolve_supplier_names<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::om::outsourcing_order::OutsourcingOrder],
) -> HashMap<i64, String> {
 let mut map = HashMap::new();
 for item in items {
 if !map.contains_key(&item.supplier_id)
 && let Ok(supplier) = svc.get(ctx, db, item.supplier_id).await {
 map.insert(item.supplier_id, supplier.name);
 }
 }
 map
}

async fn resolve_product_names<S: ProductService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::om::outsourcing_order::OutsourcingOrder],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let mut map = HashMap::new();
 if let Ok(products) = svc.get_by_ids(ctx, db, ids).await {
 for p in products {
 map.insert(p.product_id, p.pdt_name);
 }
 }
 map
}

async fn resolve_latest_tracking<S: OutsourcingTrackingService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[abt_core::om::outsourcing_order::OutsourcingOrder],
) -> HashMap<i64, String> {
 let mut map = HashMap::new();
 for item in items {
 if let Ok(result) = svc
 .list_by_outsourcing(
 ctx,
 db,
 item.id,
 PageParams {
 page: 1,
 page_size: 100,
 },
 )
 .await
 {
 // Find the tracking node with the highest node_type that has been tracked
 let latest = result
 .items
 .into_iter()
 .filter(|t| t.tracked_at.is_some())
 .max_by_key(|t| t.node_type.as_i16());
 if let Some(t) = latest {
 map.insert(item.id, tracking_node_label(&t.node_type).to_string());
 }
 }
 }
 map
}

// ── Handlers ──

#[require_permission("OM", "read")]
pub async fn get_list(
 _path: OmOutsourcingListPath,
 ctx: RequestContext,
 Query(params): Query<OutsourcingQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("PURCHASE_ORDER", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.outsourcing_order_service();
 let supplier_svc = state.supplier_service();
 let product_svc = state.product_service();
 let tracking_svc = state.outsourcing_tracking_service();

 let filter = build_filter(&params);
 let page_num = params.page.unwrap_or(1);
 let result = svc
 .list(&service_ctx, &mut conn, filter, PageParams { page: page_num, page_size: 20 })
 .await?;
 let supplier_names =
 resolve_supplier_names(&supplier_svc, &service_ctx, &mut conn, &result.items).await;
 let product_names =
 resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
 let latest_tracking =
 resolve_latest_tracking(&tracking_svc, &service_ctx, &mut conn, &result.items).await;

 let suppliers = supplier_svc
 .list(
 &service_ctx,
 &mut conn,
 SupplierQuery { name: None, status: None, category: None },
 PageParams::new(1, 200),
 )
 .await?
 .items;

 let content = list_page(
 &result,
 &supplier_names,
 &product_names,
 &latest_tracking,
 &suppliers,
 &params,
 can_create,
 );
 let page_html = admin_page(
 is_htmx,
 "委外单管理",
 &claims,
 "outsourcing",
 OmOutsourcingListPath::PATH,
 "委外管理",
 None,
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn list_page(
 result: &abt_core::shared::types::PaginatedResult<abt_core::om::outsourcing_order::OutsourcingOrder>,
 supplier_names: &HashMap<i64, String>,
 product_names: &HashMap<i64, String>,
 latest_tracking: &HashMap<i64, String>,
 suppliers: &[Supplier],
 params: &OutsourcingQueryParams,
 can_create: bool,
) -> Markup {
 html! {
    div {
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "委外单管理" }
            div class="flex gap-3" {
                @if can_create {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        href=(OmOutsourcingCreatePath::PATH)
                    { (icon::plus_icon("w-4 h-4")) "新建委外单" }
                }
            }
        }
        ({
            table_fragment(
                result,
                supplier_names,
                product_names,
                latest_tracking,
                suppliers,
                params,
            )
        })
    }
}
}

fn table_fragment(
 result: &abt_core::shared::types::PaginatedResult<abt_core::om::outsourcing_order::OutsourcingOrder>,
 supplier_names: &HashMap<i64, String>,
 product_names: &HashMap<i64, String>,
 latest_tracking: &HashMap<i64, String>,
 suppliers: &[Supplier],
 params: &OutsourcingQueryParams,
) -> Markup {
 let total_count = result.total;
 let selected_status = params.status.as_deref().unwrap_or("");

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "Draft".into(), label: "草稿", count: None },
 TabItem { value: "Sent".into(), label: "已发送", count: None },
 TabItem { value: "InProduction".into(), label: "生产中", count: None },
 TabItem { value: "Delivered".into(), label: "已交付", count: None },
 TabItem { value: "Received".into(), label: "已收货", count: None },
 TabItem { value: "Closed".into(), label: "已关闭", count: None },
 TabItem { value: "ConvertedToInternal".into(), label: "转自制", count: None },
 TabItem { value: "Cancelled".into(), label: "已取消", count: None },
 ];

 html! {
    div {
        ({
            status_tabs_with_param(
                OmOutsourcingListPath::PATH,
                "#outsourcing-data-card",
                "#outsourcing-filter-form",
                tabs,
                selected_status,
                "status",
            )
        })
        // ── Filter Bar ──
        form
            class="flex items-center gap-3 mb-5 flex-wrap filter-form"
            id="outsourcing-filter-form"
            hx-get=(OmOutsourcingListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#outsourcing-data-card"
            hx-select="#outsourcing-data-card"
            hx-swap="outerHTML"
            hx-include="#outsourcing-filter-form"
           
        {
            div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
            {
                (icon::search_icon(""))
                input
                    class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text"
                    name="keyword"
                    placeholder="搜索委外单号…"
                    value=(params.keyword.as_deref().unwrap_or(""));
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="outsourcing_type"
            {
                option value="" selected[params.outsourcing_type.is_none()] { "全部类型" }
                option value="Full" selected[params.outsourcing_type.as_deref() == Some("Full")] {
                    "整体委外"
                }
                option
                    value="Process"
                    selected[params.outsourcing_type.as_deref() == Some("Process")]
                { "工序委外" }
                option
                    value="Material"
                    selected[params.outsourcing_type.as_deref() == Some("Material")]
                { "材料委外" }
                option
                    value="Rework"
                    selected[params.outsourcing_type.as_deref() == Some("Rework")]
                { "委外返工" }
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="supplier_id"
            {
                option value="" selected[params.supplier_id.is_none()] { "全部供应商" }
                @for s in suppliers {
                    option
                        value=(s.id)
                        selected[params.supplier_id.as_deref() == Some(&s.id.to_string())]
                    { (s.name) }
                }
            }
            input
                class="max-w-[160px] w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                type="date"
                name="date_from"
                value=(params.date_from.as_deref().unwrap_or(""));
            span class="text-muted text-[13px]" { "至" }
            input
                class="max-w-[160px] w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                type="date"
                name="date_to"
                value=(params.date_to.as_deref().unwrap_or(""));
            a   href=(OmOutsourcingListPath::PATH)
                class="inline-flex items-center gap-2 h-9 no-underline py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
            { "重置" }
        }
        // ── Data Table ──
        (data_card(result, supplier_names, product_names, latest_tracking, params))
    }
}
}

fn data_card(
 result: &abt_core::shared::types::PaginatedResult<abt_core::om::outsourcing_order::OutsourcingOrder>,
 supplier_names: &HashMap<i64, String>,
 product_names: &HashMap<i64, String>,
 latest_tracking: &HashMap<i64, String>,
 _params: &OutsourcingQueryParams,
) -> Markup {
 html! {
    div class="data-card" id="outsourcing-data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "委外单号" }
                        th { "供应商" }
                        th { "产品" }
                        th { "类型" }
                        th { "计划/完成" }
                        th { "单价" }
                        th { "总金额" }
                        th { "最新追踪" }
                        th { "预计交期" }
                        th { "状态" }
                    }
                }
                tbody {
                    @for item in &result.items {
                        @let (s_label, s_bg, s_color) = status_label(&item.status);
                        @let (t_label, t_bg, t_color) = type_label(
                            &item.outsourcing_type,
                        );
                        @let supplier_name = supplier_names
                            .get(&item.supplier_id)
                            .map(|s| s.as_str())
                            .unwrap_or("—");
                        @let product_name = product_names
                            .get(&item.product_id)
                            .map(|s| s.as_str())
                            .unwrap_or("—");
                        @let total_amount = format_amount(
                            item.planned_qty * item.unit_price,
                        );
                        @let detail_path = OmOutsourcingDetailPath {
                            id: item.id,
                        };
                        @let tracking_info = latest_tracking.get(&item.id);
                        tr  class="cursor-pointer"
                            onclick=(format!("location.href='{}'", detail_path.to_string()))
                        {
                            td  class="text-accent font-medium cursor-pointer font-mono tabular-nums"
                            { (item.doc_number) }
                            td { (supplier_name) }
                            td { (product_name) }
                            td {
                                span
                                    style=({
                                        format!(
                                            "display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}",
                                            t_bg,
                                            t_color,
                                        )
                                    })
                                { (t_label) }
                            }
                            td class="text-center" {
                                span class="font-medium" { (fmt_qty(item.planned_qty)) }
                                " / "
                                span class="text-success" { (fmt_qty(item.completed_qty)) }
                            }
                            td class="text-right" { (fmt_qty(item.unit_price)) }
                            td class="text-right" { (total_amount) }
                            td {
                                @if let Some(label) = tracking_info {
                                    span class="text-xs" { (label) }
                                } @else {
                                    span class="text-muted text-xs" { "—" }
                                }
                            }
                            td {
                                @if let Some(date) = &item.scheduled_date { (date) } @else {
                                    span class="text-muted" { "—" }
                                }
                            }
                            td {
                                span
                                    style=({
                                        format!(
                                            "display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}",
                                            s_bg,
                                            s_color,
                                        )
                                    })
                                { (s_label) }
                            }
                        }
                    }
                    @if result.items.is_empty() {
                        tr {
                            td colspan="10" class="text-center text-muted py-8" { "暂无委外单" }
                        }
                    }
                }
            }
        }
        ({
            pagination(
                OmOutsourcingListPath::PATH,
                "#outsourcing-data-card",
                "#outsourcing-filter-form",
                result.total,
                result.page,
                result.total_pages,
            )
        })
    }
}
}
