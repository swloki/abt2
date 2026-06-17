use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::om::enums::TrackingNodeType;
use abt_core::om::outsourcing_order::OutsourcingOrderService;
use abt_core::om::outsourcing_tracking::OutsourcingTracking;
use abt_core::om::outsourcing_tracking::OutsourcingTrackingService;
use abt_core::shared::types::pagination::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
    OmOutsourcingDetailPath, OmTrackingListPath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TrackingQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub node_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub overdue_status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

const ALL_NODES: [TrackingNodeType; 7] = [
    TrackingNodeType::SendMaterial,
    TrackingNodeType::CarrierPickup,
    TrackingNodeType::SupplierReceived,
    TrackingNodeType::InProduction,
    TrackingNodeType::Shipped,
    TrackingNodeType::IqcInspected,
    TrackingNodeType::Warehoused,
];

fn node_label(nt: TrackingNodeType) -> &'static str {
    match nt {
        TrackingNodeType::SendMaterial => "发料",
        TrackingNodeType::CarrierPickup => "承运取货",
        TrackingNodeType::SupplierReceived => "供应商收料",
        TrackingNodeType::InProduction => "生产中",
        TrackingNodeType::Shipped => "已发货",
        TrackingNodeType::IqcInspected => "IQC检验",
        TrackingNodeType::Warehoused => "已入库",
    }
}

fn parse_node_type(s: &str) -> Option<TrackingNodeType> {
    match s {
        "SendMaterial" => Some(TrackingNodeType::SendMaterial),
        "CarrierPickup" => Some(TrackingNodeType::CarrierPickup),
        "SupplierReceived" => Some(TrackingNodeType::SupplierReceived),
        "InProduction" => Some(TrackingNodeType::InProduction),
        "Shipped" => Some(TrackingNodeType::Shipped),
        "IqcInspected" => Some(TrackingNodeType::IqcInspected),
        "Warehoused" => Some(TrackingNodeType::Warehoused),
        _ => None,
    }
}

fn build_query_string(params: &TrackingQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword {
        q.push(format!("keyword={v}"));
    }
    if let Some(v) = params.supplier_id {
        q.push(format!("supplier_id={v}"));
    }
    if let Some(ref v) = params.node_type {
        q.push(format!("node_type={v}"));
    }
    if let Some(ref v) = params.overdue_status {
        q.push(format!("overdue_status={v}"));
    }
    q.join("&")
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

/// Build dot-indicator row for the 7 standard tracking nodes.
/// `completed_nodes` is a set of node types that have been tracked (tracked_at is not null).
/// `current_node` is the overdue node from the query result.
fn node_progress_dots(completed_nodes: &std::collections::HashSet<TrackingNodeType>, current_node: TrackingNodeType) -> Markup {
    html! {
        div style="display:inline-flex;align-items:center;gap:3px" {
            @for nt in &ALL_NODES {
                @let is_current = *nt == current_node;
                @let is_done = completed_nodes.contains(nt);
                @let (bg, title) = if is_current {
                    ("var(--danger)", node_label(*nt))
                } else if is_done {
                    ("var(--success)", node_label(*nt))
                } else {
                    ("var(--border)", node_label(*nt))
                };
                span title=(title) style=(format!(
                    "width:8px;height:8px;border-radius:50%;background:{bg};display:inline-block;cursor:default"
                )) {}
            }
        }
    }
}

// ── Handlers ──

#[require_permission("OUTSOURCING", "read")]
pub async fn get_list(
    _path: OmTrackingListPath,
    ctx: RequestContext,
    Query(params): Query<TrackingQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let (result, order_map, supplier_map, product_map) =
        fetch_tracking_data(&state, &service_ctx, &mut conn, &params).await?;

    let content = tracking_list_page(&result, &order_map, &supplier_map, &product_map, &params);
    let page_html = admin_page(
        is_htmx,
        "委外追踪",
        &claims,
        "outsourcing",
        OmTrackingListPath::PATH,
        "委外管理",
        Some("/admin/om"),
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Data fetching ──

struct TrackingRow {
    tracking: OutsourcingTracking,
    completed_nodes: std::collections::HashSet<TrackingNodeType>,
}

async fn fetch_tracking_data(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: &mut abt_core::shared::types::PgPoolConn,
    params: &TrackingQueryParams,
) -> Result<(
    abt_core::shared::types::pagination::PaginatedResult<TrackingRow>,
    HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
    HashMap<i64, String>,
    HashMap<i64, String>,
)> {
    use chrono::Utc;

    let tracking_svc = state.outsourcing_tracking_service();
    let order_svc = state.outsourcing_order_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();

    let filter_node_type = params.node_type.as_deref().and_then(parse_node_type);
    let page_num = params.page.unwrap_or(1);
    let page_params = PageParams::new(page_num, 20);

    let tracking_result = tracking_svc
        .list_active_summary(service_ctx, conn, params.supplier_id, filter_node_type, page_params)
        .await?;

    // Collect unique outsourcing_ids
    let outsourcing_ids: Vec<i64> = tracking_result.items.iter().map(|t| t.outsourcing_id).collect();

    // Fetch related orders
    let mut order_map: HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder> = HashMap::new();
    for &oid in &outsourcing_ids {
        if !order_map.contains_key(&oid)
            && let Ok(order) = order_svc.find_by_id(service_ctx, conn, oid).await {
                order_map.insert(oid, order);
            }
    }

    // Keyword filter: if keyword is set, filter orders by doc_number match
    let keyword_filter = params.keyword.as_deref().and_then(|k| {
        let trimmed = k.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    });

    // Post-filter overdue_status since list_active_summary returns all active tracking
    let overdue_status_filter = params.overdue_status.as_deref();

    // For each tracking entry, fetch all nodes for that outsourcing_id to determine completed set
    let mut rows: Vec<TrackingRow> = Vec::new();
    for tracking in &tracking_result.items {
        // Skip if keyword doesn't match order doc_number
        if let Some(ref kw) = keyword_filter
            && let Some(order) = order_map.get(&tracking.outsourcing_id)
                && !order.doc_number.to_lowercase().contains(kw) {
                    continue;
                }

        // Post-filter by overdue_status
        if let Some(status) = overdue_status_filter {
            let now = Utc::now();
            match status {
                "overdue" => {
                    // planned_at < now AND tracked_at IS NULL
                    let is_overdue = tracking.tracked_at.is_none()
                        && tracking.planned_at.is_some_and(|p| p < now);
                    if !is_overdue {
                        continue;
                    }
                }
                "due_soon" => {
                    // planned_at is within 3 days from now
                    let is_due_soon = tracking.tracked_at.is_none()
                        && tracking.planned_at.is_some_and(|p| {
                            let delta = p - now;
                            delta.num_days() >= 0 && delta.num_days() <= 3
                        });
                    if !is_due_soon {
                        continue;
                    }
                }
                _ => {}
            }
        }

        let all_nodes = tracking_svc
            .list_by_outsourcing(service_ctx, conn, tracking.outsourcing_id, PageParams::new(1, 100))
            .await;

        let completed_nodes: std::collections::HashSet<TrackingNodeType> = match all_nodes {
            Ok(nodes) => nodes
                .items
                .iter()
                .filter(|n| n.tracked_at.is_some())
                .map(|n| n.node_type)
                .collect(),
            Err(_) => std::collections::HashSet::new(),
        };

        rows.push(TrackingRow {
            tracking: tracking.clone(),
            completed_nodes,
        });
    }

    // Rebuild paginated result (filtering may have reduced items)
    let total = if keyword_filter.is_some() || overdue_status_filter.is_some() {
        rows.len() as u64
    } else {
        tracking_result.total
    };
    let result = abt_core::shared::types::pagination::PaginatedResult::new(
        rows,
        total,
        tracking_result.page,
        tracking_result.page_size,
    );

    // Resolve supplier names
    let supplier_ids: Vec<i64> = order_map.values().map(|o| o.supplier_id).collect();
    let mut supplier_map: HashMap<i64, String> = HashMap::new();
    for &sid in &supplier_ids {
        if !supplier_map.contains_key(&sid)
            && let Ok(supplier) = supplier_svc.get(service_ctx, conn, sid).await {
                supplier_map.insert(sid, supplier.name);
            }
    }

    // Resolve product names
    let product_ids: Vec<i64> = order_map.values().map(|o| o.product_id).collect();
    let mut product_map: HashMap<i64, String> = HashMap::new();
    if !product_ids.is_empty()
        && let Ok(products) = product_svc.get_by_ids(service_ctx, conn, product_ids).await {
            for p in products {
                product_map.insert(p.product_id, p.pdt_name);
            }
        }

    Ok((result, order_map, supplier_map, product_map))
}

// ── Components ──

fn tracking_list_page(
    result: &abt_core::shared::types::pagination::PaginatedResult<TrackingRow>,
    order_map: &HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
    supplier_map: &HashMap<i64, String>,
    product_map: &HashMap<i64, String>,
    params: &TrackingQueryParams,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "委外追踪" }
                div class="flex gap-3" {
                    button class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-secondary [&_svg]:w-4 [&_svg]:h-4"
                        onclick="location.reload()"
                        style="display:inline-flex;align-items:center;gap:6px" {
                        (icon::refresh_icon("w-4 h-4"))
                        "刷新"
                    }
                }
            }

            // ── Stat Cards ──
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4);margin-bottom:var(--space-4)" {
                (stat_card("追踪中", result.total, "var(--accent)", icon::clock_icon))
                (stat_card("超期节点", result.total, "var(--danger)", icon::circle_alert_icon))
                (stat_card("即将到期", 0, "var(--warning)", icon::bell_icon))
                (stat_card("按时完成", 0, "var(--success)", icon::check_circle_icon))
            }

            // ── Table Fragment ──
            (tracking_table_fragment(result, order_map, supplier_map, product_map, params))
        }
    }
}

fn stat_card(
    label: &str,
    count: u64,
    color: &str,
    icon_fn: fn(&str) -> Markup,
) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="padding:var(--space-4);display:flex;align-items:center;gap:var(--space-3)" {
            div style=(format!("width:40px;height:40px;border-radius:var(--radius-lg);background:color-mix(in srgb, {color} 10%, transparent);display:flex;align-items:center;justify-content:center;color:{color}")) {
                (icon_fn("w-5 h-5"))
            }
            div {
                div style="font-size:var(--text-xs);color:var(--muted)" { (label) }
                div style="font-size:var(--text-2xl);font-weight:600;color:{color};line-height:1.2" { (count) }
            }
        }
    }
}

fn tracking_table_fragment(
    result: &abt_core::shared::types::pagination::PaginatedResult<TrackingRow>,
    order_map: &HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
    supplier_map: &HashMap<i64, String>,
    product_map: &HashMap<i64, String>,
    params: &TrackingQueryParams,
) -> Markup {
    html! {
        div class="tracking-list-panel" {
            // ── Filter Bar ──
            form class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(OmTrackingListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#tracking-data-card"
                hx-select="#tracking-data-card"
                hx-swap="outerHTML"
                hx-include="closest form" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:180px"
                        placeholder="搜索委外单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="supplier_id" {
                    option value="" selected[params.supplier_id.is_none()] { "全部供应商" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="node_type" {
                    option value="" selected[params.node_type.is_none()] { "全部节点" }
                    option value="SendMaterial" selected[params.node_type.as_deref() == Some("SendMaterial")] { "发料" }
                    option value="CarrierPickup" selected[params.node_type.as_deref() == Some("CarrierPickup")] { "承运取货" }
                    option value="SupplierReceived" selected[params.node_type.as_deref() == Some("SupplierReceived")] { "供应商收料" }
                    option value="InProduction" selected[params.node_type.as_deref() == Some("InProduction")] { "生产中" }
                    option value="Shipped" selected[params.node_type.as_deref() == Some("Shipped")] { "已发货" }
                    option value="IqcInspected" selected[params.node_type.as_deref() == Some("IqcInspected")] { "IQC检验" }
                    option value="Warehoused" selected[params.node_type.as_deref() == Some("Warehoused")] { "已入库" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="overdue_status" {
                    option value="" selected[params.overdue_status.is_none()] { "全部状态" }
                    option value="overdue" selected[params.overdue_status.as_deref() == Some("overdue")] { "超期" }
                    option value="due_soon" selected[params.overdue_status.as_deref() == Some("due_soon")] { "即将到期" }
                    option value="ontime" selected[params.overdue_status.as_deref() == Some("ontime")] { "按时" }
                }
            }

            // ── Data Table ──
            (tracking_data_card(result, order_map, supplier_map, product_map, params))
        }
    }
}

fn tracking_data_card(
    result: &abt_core::shared::types::pagination::PaginatedResult<TrackingRow>,
    order_map: &HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
    supplier_map: &HashMap<i64, String>,
    product_map: &HashMap<i64, String>,
    params: &TrackingQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="tracking-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "委外单号" }
                            th { "供应商" }
                            th { "产品" }
                            th { "数量" }
                            th { "金额" }
                            th { "当前进度节点" }
                            th { "最新完成节点" }
                            th { "下一节点" }
                            th { "计划时间" }
                            th { "状态" }
                        }
                    }
                    tbody {
                        @for row in &result.items {
                            @let tracking = &row.tracking;
                            @let order = order_map.get(&tracking.outsourcing_id);
                            @let doc_number = order.map(|o| o.doc_number.as_str()).unwrap_or("—");
                            @let supplier_name = order
                                .and_then(|o| supplier_map.get(&o.supplier_id))
                                .map(|s| s.as_str())
                                .unwrap_or("—");
                            @let product_name = order
                                .and_then(|o| product_map.get(&o.product_id))
                                .map(|s| s.as_str())
                                .unwrap_or("—");

                            // Quantity and amount
                            @let qty_str = order.map(|o| format_amount(o.planned_qty)).unwrap_or_else(|| "—".to_string());
                            @let amount_str = order.map(|o| format_amount(o.planned_qty * o.unit_price)).unwrap_or_else(|| "—".to_string());

                            // Find latest completed node
                            @let latest_completed = row.completed_nodes.iter().max_by_key(|nt| nt.ordinal());
                            @let latest_label = latest_completed.map(|nt| node_label(*nt)).unwrap_or("—");

                            // Next node after current
                            @let next_node = ALL_NODES.iter().find(|nt| nt.ordinal() > tracking.node_type.ordinal());
                            @let next_label = next_node.map(|nt| node_label(*nt)).unwrap_or("—");

                            // Status: overdue if planned_at < now and tracked_at is null
                            @let is_overdue = tracking.planned_at.is_some_and(|p| p < chrono::Utc::now());
                            @let (status_text, status_bg, status_color) = if is_overdue {
                                ("超期", "rgba(245,63,63,0.08)", "var(--danger)")
                            } else {
                                ("待完成", "rgba(250,140,22,0.08)", "var(--warning)")
                            };

                            @let detail_path = OmOutsourcingDetailPath { id: tracking.outsourcing_id };

                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="color:var(--accent)" { (doc_number) }
                                td { (supplier_name) }
                                td { (product_name) }
                                td class="font-mono tabular-nums" style="text-align:right" { (qty_str) }
                                td class="font-mono tabular-nums" style="text-align:right" { (amount_str) }
                                td { (node_progress_dots(&row.completed_nodes, tracking.node_type)) }
                                td { (latest_label) }
                                td { (next_label) }
                                td style="font-size:12px" {
                                    @if let Some(planned) = tracking.planned_at {
                                        (planned.format("%Y-%m-%d"))
                                    } @else {
                                        "—"
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{status_bg};color:{status_color}")) {
                                        (status_text)
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无追踪数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(OmTrackingListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
