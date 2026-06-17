use std::collections::HashMap;

use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
    OmDashboardPath, OmOutsourcingCreatePath, OmOutsourcingListPath, OmTrackingListPath,
};
use crate::utils::{fmt_qty, RequestContext};
use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::om::enums::{OutsourcingStatus, OutsourcingType, TrackingNodeType};
use abt_core::om::outsourcing_order::OutsourcingOrderService;
use abt_core::om::outsourcing_order::{OutsourcingOrder, OutsourcingOrderQuery};
use abt_core::om::outsourcing_tracking::OutsourcingTrackingService;
use abt_core::shared::types::pagination::PageParams;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("OM", "read")]
pub async fn get_dashboard(
    _path: OmDashboardPath,
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
    let svc = state.outsourcing_order_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let tracking_svc = state.outsourcing_tracking_service();
    let db = &mut conn;

    // Stat counts – each query fetches page 1 size 1 to get total only
    let total = svc
        .list(
            &service_ctx,
            db,
            OutsourcingOrderQuery::default(),
            PageParams::new(1, 1),
        )
        .await?
        .total;

    let in_production = svc
        .list(
            &service_ctx,
            db,
            OutsourcingOrderQuery {
                status: Some(OutsourcingStatus::InProduction),
                ..Default::default()
            },
            PageParams::new(1, 1),
        )
        .await?
        .total;

    let delivered = svc
        .list(
            &service_ctx,
            db,
            OutsourcingOrderQuery {
                status: Some(OutsourcingStatus::Delivered),
                ..Default::default()
            },
            PageParams::new(1, 1),
        )
        .await?
        .total;

    // Overdue: no easy filter, placeholder 0
    let overdue: u64 = 0;

    // Fetch all orders for analytics (type distribution, supplier ranking)
    let all_result = svc
        .list(
            &service_ctx,
            db,
            OutsourcingOrderQuery::default(),
            PageParams::new(1, 200),
        )
        .await?;

    // Recent orders – last 5
    let recent_orders: Vec<&OutsourcingOrder> = all_result.items.iter().take(5).collect();

    // Resolve names and tracking for recent orders
    let supplier_names =
        resolve_supplier_names(&supplier_svc, &service_ctx, db, &all_result.items).await;
    let product_names =
        resolve_product_names(&product_svc, &service_ctx, db, &all_result.items).await;
    let latest_tracking =
        resolve_latest_tracking(&tracking_svc, &service_ctx, db, &all_result.items).await;

    // Monthly amount: sum unit_price * planned_qty from all items (best-effort)
    let monthly_amount: Decimal = all_result
        .items
        .iter()
        .fold(Decimal::ZERO, |acc, o| acc + o.unit_price * o.planned_qty);

    // Type distribution
    let type_dist = calc_type_distribution(&all_result.items);

    // Supplier ranking by amount
    let supplier_ranking = calc_supplier_ranking(&all_result.items, &supplier_names);

    let ctx = OmDashboardContext {
        in_production,
        delivered,
        overdue,
        monthly_amount,
        recent: &recent_orders,
        supplier_names: &supplier_names,
        product_names: &product_names,
        latest_tracking: &latest_tracking,
        type_dist: &type_dist,
        supplier_ranking: &supplier_ranking,
    };
    let content = om_dashboard_page(total, &ctx);

    let page_html = admin_page(
        is_htmx,
        "委外管理",
        &claims,
        "outsourcing",
        OmDashboardPath::PATH,
        "委外管理",
        None,
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Resolve helpers ──

async fn resolve_supplier_names<S: SupplierService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[OutsourcingOrder],
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
    items: &[OutsourcingOrder],
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
    items: &[OutsourcingOrder],
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

// ── Analytics ──

#[allow(dead_code)]
struct TypeDistEntry {
    label: &'static str,
    bg: &'static str,
    fg: &'static str,
    count: u64,
    pct: f64,
}

fn calc_type_distribution(items: &[OutsourcingOrder]) -> Vec<TypeDistEntry> {
    let total = items.len() as u64;
    let mut counts: HashMap<OutsourcingType, u64> = HashMap::new();
    for o in items {
        *counts.entry(o.outsourcing_type).or_default() += 1;
    }
    let types = [
        (OutsourcingType::Full, "产品全委外", "rgba(22,119,255,0.08)", "var(--accent)"),
        (OutsourcingType::Process, "工序委外", "rgba(250,140,22,0.08)", "#fa8c16"),
        (OutsourcingType::Material, "材料委外", "rgba(114,46,209,0.08)", "#722ed1"),
        (OutsourcingType::Rework, "委外返工", "rgba(245,63,63,0.06)", "#f53f3f"),
    ];
    types
        .into_iter()
        .map(|(t, label, bg, fg)| {
            let count = *counts.get(&t).unwrap_or(&0);
            let pct = if total > 0 {
                count as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            TypeDistEntry {
                label,
                bg,
                fg,
                count,
                pct,
            }
        })
        .collect()
}

struct SupplierRankEntry {
    name: String,
    amount: Decimal,
    pct: f64,
}

fn calc_supplier_ranking(
    items: &[OutsourcingOrder],
    supplier_names: &HashMap<i64, String>,
) -> Vec<SupplierRankEntry> {
    let mut amounts: HashMap<i64, Decimal> = HashMap::new();
    for o in items {
        *amounts.entry(o.supplier_id).or_default() += o.unit_price * o.planned_qty;
    }
    let max_amount = amounts.values().copied().max().unwrap_or(Decimal::ZERO);
    let mut entries: Vec<SupplierRankEntry> = amounts
        .into_iter()
        .map(|(id, amount)| {
            let name = supplier_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let pct = if max_amount > Decimal::ZERO {
                let ratio: f64 = (amount / max_amount).try_into().unwrap_or(0.0);
                ratio * 100.0
            } else {
                0.0
            };
            SupplierRankEntry {
                name,
                amount,
                pct,
            }
        })
        .collect();
    entries.sort_by(|a, b| b.amount.cmp(&a.amount));
    entries.truncate(5);
    entries
}

// ── Page ──

struct OmDashboardContext<'a> {
    in_production: u64,
    delivered: u64,
    overdue: u64,
    monthly_amount: Decimal,
    recent: &'a [&'a OutsourcingOrder],
    supplier_names: &'a HashMap<i64, String>,
    product_names: &'a HashMap<i64, String>,
    latest_tracking: &'a HashMap<i64, String>,
    type_dist: &'a [TypeDistEntry],
    supplier_ranking: &'a [SupplierRankEntry],
}


fn om_dashboard_page(total: u64, ctx: &OmDashboardContext) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "委外管理总览" }
                div class="flex gap-3" {
                    a href=(OmOutsourcingCreatePath::PATH) class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                        (icon::plus_icon("w-4 h-4"))
                        " 新建委外单"
                    }
                }
            }

            // ── Stat Cards ──
            div style="display:grid;grid-template-columns:repeat(5,1fr);gap:var(--space-5);margin-bottom:var(--space-6)" {
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" { (icon::file_text_icon("w-5 h-5")) }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (total) }
                        div class="text-sm text-text-muted mt-1" { "委外单总数" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" { (icon::tool_icon("w-5 h-5")) }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (ctx.in_production) }
                        div class="text-sm text-text-muted mt-1" { "生产中" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" { (icon::package_icon("w-5 h-5")) }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (ctx.delivered) }
                        div class="text-sm text-text-muted mt-1" { "待收货" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 red" { (icon::alert_triangle_icon("w-5 h-5")) }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (ctx.overdue) }
                        div class="text-sm text-text-muted mt-1" { "超期预警" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 purple" { (icon::currency_icon("w-5 h-5")) }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (format_amount(ctx.monthly_amount)) }
                        div class="text-sm text-text-muted mt-1" { "本月委外金额" }
                    }
                }
            }

            // ── Quick Entry Grid ──
            div class="mb-8" {
                h2 class="text-lg font-semibold text-fg flex items-center gap-2" { "快捷入口" }
                div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4)" {
                    (quick_entry_card(OmOutsourcingCreatePath::PATH, "新建委外单", "创建委外生产订单", "blue", "plus", None))
                    (quick_entry_card(OmOutsourcingListPath::PATH, "委外单管理", "查看和管理所有委外单", "green", "list", Some(total)))
                    (quick_entry_card(OmTrackingListPath::PATH, "追踪管理", "物流节点与进度跟踪", "orange", "track", if ctx.overdue > 0 { Some(ctx.overdue) } else { None }))
                    (quick_entry_card(OmOutsourcingListPath::PATH, "收货登记", "委外到货确认与入库", "cyan", "receive", Some(ctx.delivered)))
                }
            }

            // ── Recent Outsourcing Orders ──
            div class="mb-8" {
                h2 class="text-lg font-semibold text-fg flex items-center gap-2" {
                    (icon::clock_icon("w-4 h-4"))
                    " 最近委外单"
                }
                div class="data-card" style="overflow:hidden" {
                    table class="data-table" style="width:100%;min-width:auto" {
                        thead {
                            tr {
                                th { "委外单号" }
                                th { "供应商" }
                                th { "产品" }
                                th { "类型" }
                                th { "数量(计划/完成)" }
                                th { "金额" }
                                th { "最新追踪" }
                                th { "状态" }
                            }
                        }
                        tbody {
                            @if ctx.recent.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;color:var(--muted);padding:var(--space-8)" { "暂无委外单" }
                                }
                            } @else {
                                @for order in ctx.recent {
                                    @let supplier_name = ctx.supplier_names.get(&order.supplier_id).map(|s| s.as_str()).unwrap_or("—");
                                    @let product_name = ctx.product_names.get(&order.product_id).map(|s| s.as_str()).unwrap_or("—");
                                    @let amount = order.planned_qty * order.unit_price;
                                    @let tracking = ctx.latest_tracking.get(&order.id).map(|s| s.as_str()).unwrap_or("—");
                                    @let (s_label, s_bg, s_color) = status_label(&order.status);
                                    @let (t_label, t_bg, t_color) = type_label(&order.outsourcing_type);
                                    tr {
                                        td {
                                            a href=(format!("/admin/om/outsourcing/{}", order.id)) style="color:var(--accent)" {
                                                (order.doc_number.as_str())
                                            }
                                        }
                                        td { (supplier_name) }
                                        td { (product_name) }
                                        td {
                                            span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", t_bg, t_color)) {
                                                (t_label)
                                            }
                                        }
                                        td style="text-align:center" {
                                            span style="font-weight:500" { (fmt_qty(order.planned_qty)) }
                                            " / "
                                            span style="color:var(--success)" { (fmt_qty(order.completed_qty)) }
                                        }
                                        td style="text-align:right" { (format_amount(amount)) }
                                        td {
                                            span style="font-size:var(--text-xs)" { (tracking) }
                                        }
                                        td {
                                            span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", s_bg, s_color)) {
                                                (s_label)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Analytics Row: Type Distribution + Supplier Ranking ──
            div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-5);margin-top:var(--space-5)" {
                // ── 委外类型分布 ──
                div class="mb-8" style="margin-bottom:0" {
                    h2 class="text-lg font-semibold text-fg flex items-center gap-2" {
                        (icon::grid_icon("w-4 h-4"))
                        " 委外类型分布"
                    }
                    div class="data-card" style="padding:var(--space-5)" {
                        @for entry in ctx.type_dist {
                            div style="margin-bottom:var(--space-4)" {
                                div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:var(--space-2)" {
                                    span style="font-size:13px;font-weight:500" { (entry.label) }
                                    span style="font-size:13px;color:var(--muted)" {
                                        (entry.count) " 单"
                                        " ("
                                        (format!("{:.1}%", entry.pct))
                                        ")"
                                    }
                                }
                                div style="height:8px;background:var(--bg-secondary);border-radius:4px;overflow:hidden" {
                                    div style=(format!("height:100%;width:{}%;background:{};border-radius:4px;transition:width .3s", entry.pct, entry.fg)) {}
                                }
                            }
                        }
                        @if ctx.type_dist.iter().all(|e| e.count == 0) {
                            div style="text-align:center;color:var(--muted);padding:var(--space-6);font-size:13px" { "暂无数据" }
                        }
                    }
                }

                // ── 供应商委外金额排名 ──
                div class="mb-8" style="margin-bottom:0" {
                    h2 class="text-lg font-semibold text-fg flex items-center gap-2" {
                        (icon::trending_up_icon("w-4 h-4"))
                        " 供应商委外金额排名"
                    }
                    div class="data-card" style="padding:var(--space-5)" {
                        @for (i, entry) in ctx.supplier_ranking.iter().enumerate() {
                            @let medal = match i {
                                0 => "🥇",
                                1 => "🥈",
                                2 => "🥉",
                                _ => "",
                            };
                            div style="margin-bottom:var(--space-4)" {
                                div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:var(--space-2)" {
                                    span style="font-size:13px;font-weight:500" {
                                        span style="margin-right:6px" { (medal) }
                                        (entry.name)
                                    }
                                    span style="font-size:13px;color:var(--muted)" {
                                        (format_amount(entry.amount))
                                    }
                                }
                                div style="height:8px;background:var(--bg-secondary);border-radius:4px;overflow:hidden" {
                                    div style=(format!("height:100%;width:{}%;background:linear-gradient(90deg,var(--accent),#7c3aed);border-radius:4px;transition:width .3s", entry.pct)) {}
                                }
                            }
                        }
                        @if ctx.supplier_ranking.is_empty() {
                            div style="text-align:center;color:var(--muted);padding:var(--space-6);font-size:13px" { "暂无数据" }
                        }
                    }
                }
            }
        }
    }
}

// ── Helpers ──

fn format_amount(d: Decimal) -> String {
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

fn quick_entry_card(
    href: &str,
    title: &str,
    desc: &str,
    color: &str,
    icon_key: &str,
    badge: Option<u64>,
) -> Markup {
    let (bg, fg) = match color {
        "blue" => ("linear-gradient(135deg,#e6f4ff,#d6e8ff)", "var(--accent)"),
        "green" => ("linear-gradient(135deg,#f0fff0,#e0ffe0)", "var(--success)"),
        "orange" => ("linear-gradient(135deg,#fff8eb,#fff0d6)", "#fa8c16"),
        "purple" => ("linear-gradient(135deg,#f3e8ff,#e9d5ff)", "#7c3aed"),
        "cyan" => ("linear-gradient(135deg,#e6fffb,#b5f5ec)", "#13c2c2"),
        "amber" => ("linear-gradient(135deg,#fffbe6,#fff1b8)", "#d4a017"),
        "red" => ("linear-gradient(135deg,#fff2f0,#ffe8e6)", "var(--danger)"),
        _ => ("rgba(0,0,0,0.04)", "var(--muted)"),
    };
    let icon_svg = match icon_key {
        "plus" => icon::plus_icon("w-full h-full"),
        "list" => icon::clipboard_list_icon("w-full h-full"),
        "track" => icon::truck_icon("w-full h-full"),
        "receive" => icon::download_icon("w-full h-full"),
        _ => icon::grid_icon("w-full h-full"),
    };
    html! {
        a href=(href) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" style="text-decoration:none;position:relative" {
            div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style=(format!("background:{}", bg)) {
                div style=(format!("width:22px;height:22px;color:{}", fg)) {
                    (icon_svg)
                }
            }
            span class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { (title) }
            span class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { (desc) }
            @if let Some(count) = badge {
                span style="position:absolute;top:8px;right:8px;display:inline-flex;align-items:center;justify-content:center;min-width:20px;height:20px;padding:0 6px;border-radius:10px;font-size:11px;font-weight:600;background:var(--danger);color:#fff;line-height:1" {
                    (count)
                }
            }
        }
    }
}

fn status_label(s: &OutsourcingStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        OutsourcingStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        OutsourcingStatus::Sent => ("已发送", "rgba(22,119,255,0.08)", "var(--accent)"),
        OutsourcingStatus::InProduction => ("生产中", "rgba(250,140,22,0.08)", "#fa8c16"),
        OutsourcingStatus::Delivered => ("已交付", "rgba(114,46,209,0.08)", "#722ed1"),
        OutsourcingStatus::Received => ("已收货", "rgba(82,196,26,0.08)", "var(--success)"),
        OutsourcingStatus::Closed => ("已关闭", "rgba(0,0,0,0.06)", "var(--muted)"),
        OutsourcingStatus::ConvertedToInternal => {
            ("转自制", "rgba(22,119,255,0.08)", "var(--accent)")
        }
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
