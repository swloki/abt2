use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
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
        OutsourcingType::Full => ("产品全委外", "rgba(22,119,255,0.08)", "var(--accent)"),
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

fn build_query_string(params: &OutsourcingQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword {
        q.push(format!("keyword={v}"));
    }
    if let Some(ref v) = params.status {
        q.push(format!("status={v}"));
    }
    if let Some(ref v) = params.outsourcing_type {
        q.push(format!("outsourcing_type={v}"));
    }
    if let Some(ref v) = params.supplier_id {
        q.push(format!("supplier_id={v}"));
    }
    if let Some(ref v) = params.date_from {
        q.push(format!("date_from={v}"));
    }
    if let Some(ref v) = params.date_to {
        q.push(format!("date_to={v}"));
    }
    q.join("&")
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

    let content = list_page(
        &result,
        &supplier_names,
        &product_names,
        &latest_tracking,
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
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::om::outsourcing_order::OutsourcingOrder>,
    supplier_names: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    latest_tracking: &HashMap<i64, String>,
    params: &OutsourcingQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="page-header" {
                div class="page-header-left" {
                    h1 class="page-title" { "委外单管理" }
                    @if can_create {
                        a class="btn btn-primary" href=(OmOutsourcingCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建委外单"
                        }
                    }
                }
            }
            (table_fragment(result, supplier_names, product_names, latest_tracking, params))
        }
    }
}

fn table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::om::outsourcing_order::OutsourcingOrder>,
    supplier_names: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    latest_tracking: &HashMap<i64, String>,
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
        div class="plan-list-panel" {
            (status_tabs_with_param(OmOutsourcingListPath::PATH, "#outsourcing-data-card", "#outsourcing-filter-form", tabs, selected_status, "status"))

            // ── Filter Bar ──
            form class="filter-bar filter-form" id="outsourcing-filter-form"
                hx-get=(OmOutsourcingListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#outsourcing-data-card"
                hx-select="#outsourcing-data-card"
                hx-swap="outerHTML"
                hx-include="#outsourcing-filter-form"
                hx-push-url="true" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        style="width:180px"
                        placeholder="搜索委外单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="outsourcing_type" {
                    option value="" selected[params.outsourcing_type.is_none()] { "全部类型" }
                    option value="Full" selected[params.outsourcing_type.as_deref() == Some("Full")] { "产品全委外" }
                    option value="Process" selected[params.outsourcing_type.as_deref() == Some("Process")] { "工序委外" }
                    option value="Material" selected[params.outsourcing_type.as_deref() == Some("Material")] { "材料委外" }
                    option value="Rework" selected[params.outsourcing_type.as_deref() == Some("Rework")] { "委外返工" }
                }
                input class="search-input" type="text" name="supplier_id"
                    style="max-width:120px"
                    placeholder="供应商ID"
                    value=(params.supplier_id.as_deref().unwrap_or(""));
                input class="search-input" type="date" name="date_from"
                    style="max-width:160px"
                    value=(params.date_from.as_deref().unwrap_or(""));
                span style="color:var(--muted);font-size:13px" { "至" }
                input class="search-input" type="date" name="date_to"
                    style="max-width:160px"
                    value=(params.date_to.as_deref().unwrap_or(""));
                a href=(OmOutsourcingListPath::PATH) class="btn btn-default" style="height:36px;text-decoration:none" { "重置" }
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
    params: &OutsourcingQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="data-card" id="outsourcing-data-card" {
            div class="data-card-scroll" {
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
                            @let (t_label, t_bg, t_color) = type_label(&item.outsourcing_type);
                            @let supplier_name = supplier_names.get(&item.supplier_id).map(|s| s.as_str()).unwrap_or("—");
                            @let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                            @let total_amount = format_amount(item.planned_qty * item.unit_price);
                            @let detail_path = OmOutsourcingDetailPath { id: item.id };
                            @let tracking_info = latest_tracking.get(&item.id);
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="link-cell mono" style="color:var(--accent)" { (item.doc_number) }
                                td { (supplier_name) }
                                td { (product_name) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", t_bg, t_color)) {
                                        (t_label)
                                    }
                                }
                                td style="text-align:center" {
                                    span style="font-weight:500" { (fmt_qty(item.planned_qty)) }
                                    " / "
                                    span style="color:var(--success)" { (fmt_qty(item.completed_qty)) }
                                }
                                td style="text-align:right" { (fmt_qty(item.unit_price)) }
                                td style="text-align:right" { (total_amount) }
                                td {
                                    @if let Some(label) = tracking_info {
                                        span style="font-size:var(--text-xs)" { (label) }
                                    } @else {
                                        span style="color:var(--muted);font-size:var(--text-xs)" { "—" }
                                    }
                                }
                                td {
                                    @if let Some(date) = &item.scheduled_date {
                                        (date)
                                    } @else {
                                        span style="color:var(--muted)" { "—" }
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", s_bg, s_color)) {
                                        (s_label)
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无委外单"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(OmOutsourcingListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
