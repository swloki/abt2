use std::collections::HashMap;

use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use rust_decimal::Decimal;

use abt_core::fms::cost_accounting::{
    CostAccountingService, MarginRow, ProductCostRow, ProfitCenterPLRow, WorkOrderCostRow,
};
use abt_core::shared::types::PgExecutor;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::CostAnalysisPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_page(
    _path: CostAnalysisPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx: _,
        claims,
        ..
    } = ctx;

    let svc = state.cost_accounting_service();
    let db: PgExecutor<'_> = &mut *conn;

    // 查询当期数据
    let period = "2026-06";
    let product_rows = svc.list_product_costs(db, period).await?;
    let wo_rows = svc.list_work_order_costs(db).await?;
    let pc_rows = svc.list_profit_center_pl(db, period).await?;
    let margin_rows = svc.list_margin_analysis(db).await?;

    // 聚合产品成本
    let products = aggregate_products(&product_rows);
    let work_orders = aggregate_work_orders(&wo_rows);
    let profit_centers = aggregate_profit_centers(&pc_rows);
    let margins = aggregate_margins(&margin_rows);

    // 统计卡片
    let total_product_cost: Decimal = products.iter().map(|p| p.total_cost).sum();
    let total_wo_cost: Decimal = work_orders.iter().map(|w| w.total_cost).sum();
    let total_margin_pct = if margins.is_empty() {
        Decimal::ZERO
    } else {
        let sum_rate: Decimal = margins.iter().map(|m| m.margin_rate).sum();
        (sum_rate / Decimal::from(margins.len() as i32)).round_dp(1)
    };
    let pc_count = profit_centers.len();

    let stats = PageStats { total_product_cost, total_wo_cost, avg_margin_rate: total_margin_pct, pc_count };

    let content = cost_analysis_page(
        &products,
        &work_orders,
        &profit_centers,
        &margins,
        &stats,
    );
    let page_html = admin_page(
        is_htmx,
        "成本核算分析",
        &claims,
        "finance",
        CostAnalysisPath::PATH,
        "财务管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

struct PageStats {
    total_product_cost: Decimal,
    total_wo_cost: Decimal,
    avg_margin_rate: Decimal,
    pc_count: usize,
}

// ── Aggregated data structures ──

struct ProductCostView {
    product_id: i64,
    product_code: String,
    product_name: String,
    material_cost: Decimal,
    labor_cost: Decimal,
    overhead_cost: Decimal,
    total_cost: Decimal,
}

struct WorkOrderCostView {
    work_order_id: i64,
    doc_number: String,
    product_name: String,
    planned_qty: Decimal,
    completed_qty: Decimal,
    wo_status: i16,
    material_cost: Decimal,
    labor_cost: Decimal,
    outsource_cost: Decimal,
    total_cost: Decimal,
}

struct ProfitCenterView {
    profit_center_id: i64,
    label: String,
    income: Decimal,
    material_cost: Decimal,
    labor_cost: Decimal,
    overhead_cost: Decimal,
    admin_cost: Decimal,
    profit: Decimal,
    profit_rate: Decimal,
}

#[allow(dead_code)]
struct MarginView {
    order_id: i64,
    doc_number: String,
    customer_name: String,
    order_amount: Decimal,
    actual_cost: Decimal,
    margin_amount: Decimal,
    margin_rate: Decimal,
    deviation: Decimal,
}

// ── Aggregation helpers ──

fn aggregate_products(rows: &[ProductCostRow]) -> Vec<ProductCostView> {
    let mut map: HashMap<i64, ProductCostView> = HashMap::new();
    for r in rows {
        let v = map.entry(r.product_id).or_insert_with(|| ProductCostView {
            product_id: r.product_id,
            product_code: r.product_code.clone(),
            product_name: r.product_name.clone(),
            material_cost: Decimal::ZERO,
            labor_cost: Decimal::ZERO,
            overhead_cost: Decimal::ZERO,
            total_cost: Decimal::ZERO,
        });
        match r.cost_type {
            1 => v.material_cost = r.total,
            2 => v.labor_cost = r.total,
            3 => v.overhead_cost = r.total,
            _ => {}
        }
        v.total_cost = v.material_cost + v.labor_cost + v.overhead_cost;
    }
    let mut list: Vec<_> = map.into_values().collect();
    list.sort_by_key(|p| p.product_id);
    list
}

fn aggregate_work_orders(rows: &[WorkOrderCostRow]) -> Vec<WorkOrderCostView> {
    let mut map: HashMap<i64, WorkOrderCostView> = HashMap::new();
    for r in rows {
        let v = map.entry(r.work_order_id).or_insert_with(|| WorkOrderCostView {
            work_order_id: r.work_order_id,
            doc_number: r.doc_number.clone(),
            product_name: r.product_name.clone(),
            planned_qty: r.planned_qty,
            completed_qty: r.completed_qty.unwrap_or(Decimal::ZERO),
            wo_status: r.wo_status,
            material_cost: Decimal::ZERO,
            labor_cost: Decimal::ZERO,
            outsource_cost: Decimal::ZERO,
            total_cost: Decimal::ZERO,
        });
        match r.cost_type {
            1 => v.material_cost = r.total,
            2 => v.labor_cost = r.total,
            3 => v.outsource_cost = r.total,
            _ => {}
        }
        v.total_cost = v.material_cost + v.labor_cost + v.outsource_cost;
    }
    let mut list: Vec<_> = map.into_values().collect();
    list.sort_by_key(|w| w.work_order_id);
    list
}

fn aggregate_profit_centers(rows: &[ProfitCenterPLRow]) -> Vec<ProfitCenterView> {
    let mut map: HashMap<i64, ProfitCenterView> = HashMap::new();
    let labels = HashMap::from([
        (1i64, "华南区"),
        (2i64, "华东区"),
        (3i64, "华北区"),
        (4i64, "西南区"),
        (5i64, "西北区"),
        (6i64, "东北区"),
    ]);
    for r in rows {
        let v = map.entry(r.profit_center).or_insert_with(|| ProfitCenterView {
            profit_center_id: r.profit_center,
            label: labels.get(&r.profit_center).map(|s| s.to_string()).unwrap_or_else(|| format!("利润中心-{}", r.profit_center)),
            income: Decimal::ZERO,
            material_cost: Decimal::ZERO,
            labor_cost: Decimal::ZERO,
            overhead_cost: Decimal::ZERO,
            admin_cost: Decimal::ZERO,
            profit: Decimal::ZERO,
            profit_rate: Decimal::ZERO,
        });
        // cost_type=4 是收入（credit 侧）
        match r.cost_type {
            4 => v.income = r.total_credit,
            1 => v.material_cost += r.total_debit,
            2 => v.labor_cost += r.total_debit,
            3 => v.overhead_cost += r.total_debit,
            _ => v.admin_cost += r.total_debit, // 其他类型归入管理费用
        }
    }
    for v in map.values_mut() {
        // admin_cost 里包含了 cost_type 非 1/2/3/4 的 debit
        v.profit = v.income - v.material_cost - v.labor_cost - v.overhead_cost - v.admin_cost;
        v.profit_rate = if v.income > Decimal::ZERO {
            (v.profit / v.income * Decimal::from(100)).round_dp(1)
        } else {
            Decimal::ZERO
        };
    }
    let mut list: Vec<_> = map.into_values().collect();
    list.sort_by_key(|p| p.profit_center_id);
    list
}

fn aggregate_margins(rows: &[MarginRow]) -> Vec<MarginView> {
    let mut map: HashMap<i64, MarginView> = HashMap::new();
    for r in rows {
        let v = map.entry(r.order_id).or_insert_with(|| MarginView {
            order_id: r.order_id,
            doc_number: r.doc_number.clone(),
            customer_name: r.customer_name.clone(),
            order_amount: r.order_amount,
            actual_cost: Decimal::ZERO,
            margin_amount: Decimal::ZERO,
            margin_rate: Decimal::ZERO,
            deviation: Decimal::ZERO,
        });
        v.actual_cost += r.total_cost;
    }
    for v in map.values_mut() {
        v.margin_amount = v.order_amount - v.actual_cost;
        v.margin_rate = if v.order_amount > Decimal::ZERO {
            (v.margin_amount / v.order_amount * Decimal::from(100)).round_dp(1)
        } else {
            Decimal::ZERO
        };
    }
    let mut list: Vec<_> = map.into_values().collect();
    list.sort_by_key(|m| m.order_id);
    list
}

// ── Formatting helpers ──

fn fmt_money(d: Decimal) -> String {
    let val = d.round_dp(2);
    if val >= Decimal::from(10000) {
        let wan = val / Decimal::from(10000);
        format!("¥{}万", wan.round_dp(1))
    } else {
        format!("¥{}", val)
    }
}

fn fmt_money_full(d: Decimal) -> String {
    format!("¥{}", d.round_dp(2))
}

fn wo_status_label(s: i16) -> (&'static str, &'static str) {
    match s {
        1 => ("草稿", "status-draft"),
        2 => ("已计划", "status-planned"),
        3 => ("已下达", "status-progress"),
        4 => ("已完工", "status-completed"),
        5 => ("已取消", "status-cancelled"),
        _ => ("未知", ""),
    }
}

fn margin_class(rate: Decimal) -> &'static str {
    if rate > Decimal::from(25) {
        "margin-positive"
    } else if rate > Decimal::from(10) {
        "margin-warn"
    } else {
        "margin-negative"
    }
}

// ── Page ──

fn cost_analysis_page(
    products: &[ProductCostView],
    work_orders: &[WorkOrderCostView],
    profit_centers: &[ProfitCenterView],
    margins: &[MarginView],
    stats: &PageStats,
) -> Markup {
    html! {
        div class="p-6 relative" {
            // ── 页面标题栏 ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "成本核算分析" }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
                        (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
                        "导出报表"
                    }
                }
            }
            // ── 统计概要 ──
            div class="stat-grid grid gap-5" style="margin-bottom:var(--space-6)" {
                (stat_card("本月产品成本", &format!("{}<span style=\"font-size:14px;color:var(--muted)\">万</span>", fmt_money(stats.total_product_cost).trim_start_matches('¥').replace("万", "")), "var(--accent)", "linear-gradient(135deg,#dbeafe,#bfdbfe)", "#2563eb", r#"M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"#))
                (stat_card("本月工单成本", &format!("{}<span style=\"font-size:14px;color:var(--muted)\">万</span>", fmt_money(stats.total_wo_cost).trim_start_matches('¥').replace("万", "")), "var(--warn)", "linear-gradient(135deg,#fef3c7,#fde68a)", "#d97706", r#"M14.7 6.3a1 1 0 000 1.4l1.6 1.6a1 1 0 001.4 0l3.77-3.77a6 6 0 01-7.94 7.94l-6.91 6.91a2.12 2.12 0 01-3-3l6.91-6.91a6 6 0 017.94-7.94l-3.76 3.76z"#))
                (stat_card("综合毛利率", &format!("<span style=\"color:var(--success)\">{}%</span>", stats.avg_margin_rate), "var(--success)", "linear-gradient(135deg,#dcfce7,#bbf7d0)", "#16a34a", r#"M23 6l-9.5 9.5-5-5L1 18M17 6h6v6"#))
                (stat_card("利润中心数", &stats.pc_count.to_string(), "#7c3aed", "linear-gradient(135deg,#ede9fe,#ddd6fe)", "#7c3aed", r#"M18 20V10M12 20V4M6 20v-6"#))
            }

            // ── 分析Tab ──
            div class="flex gap-1 rounded-lg p-1" {
                button class="flex-1 p-3 rounded text-sm font-medium text-text-muted cursor-pointer text-center border-none active" onclick="switchTab('product')" { "产品成本" }
                button class="flex-1 p-3 rounded text-sm font-medium text-text-muted cursor-pointer text-center border-none" onclick="switchTab('order')" { "工单成本" }
                button class="flex-1 p-3 rounded text-sm font-medium text-text-muted cursor-pointer text-center border-none" onclick="switchTab('profit')" { "利润中心 P&L" }
                button class="flex-1 p-3 rounded text-sm font-medium text-text-muted cursor-pointer text-center border-none" onclick="switchTab('margin')" { "毛利分析" }
            }

            // ── 产品成本面板 ──
            div id="panel-product" class="hidden active" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-bottom:0" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" style="display:flex;justify-content:space-between;align-items:center" {
                        span { "产品成本汇总 · 2026-06" }
                    }
                    @if products.is_empty() {
                        div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无产品成本数据" }
                    } @else {
                        div class="overflow-x-auto" {
                            table class="data-table" style="min-width:900px" {
                                thead {
                                    tr {
                                        th { "产品编码" }
                                        th { "产品名称" }
                                        th style="text-align:right" { "材料成本" }
                                        th style="text-align:right" { "人工成本" }
                                        th style="text-align:right" { "制造费用" }
                                        th style="text-align:right" { "总成本" }
                                        th { "成本构成" }
                                    }
                                }
                                tbody {
                                    @for p in products {
                                        tr {
                                            td class="font-mono tabular-nums" { (p.product_code) }
                                            td style="font-weight:600" { (p.product_name) }
                                            td class="text-right text-[13px]" { (fmt_money_full(p.material_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(p.labor_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(p.overhead_cost)) }
                                            td class="text-right text-[13px]" style="font-weight:700;color:var(--accent)" { (fmt_money_full(p.total_cost)) }
                                            td style="min-width:160px" {
                                                (cost_breakdown_bar(p.material_cost, p.labor_cost, p.overhead_cost, p.total_cost))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div class="flex gap-5 text-[12px] text-text-muted" {
                            span style="display:flex;align-items:center;gap:6px" {
                                span class="cost-w-[10px] h-[3px]" style="background:linear-gradient(135deg,var(--accent),#60a5fa)" {}
                                "材料成本"
                            }
                            span style="display:flex;align-items:center;gap:6px" {
                                span class="cost-w-[10px] h-[3px]" style="background:linear-gradient(135deg,var(--warn),#fbbf24)" {}
                                "人工成本"
                            }
                            span style="display:flex;align-items:center;gap:6px" {
                                span class="cost-w-[10px] h-[3px]" style="background:linear-gradient(135deg,#7c3aed,#a78bfa)" {}
                                "制造费用"
                            }
                        }
                    }
                }
            }

            // ── 工单成本面板 ──
            div id="panel-order" class="hidden" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-bottom:0" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" style="display:flex;justify-content:space-between;align-items:center" {
                        span { "工单成本归集" }
                    }
                    @if work_orders.is_empty() {
                        div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无工单成本数据" }
                    } @else {
                        div class="overflow-x-auto" {
                            table class="data-table" style="min-width:950px" {
                                thead {
                                    tr {
                                        th { "工单号" }
                                        th { "产品" }
                                        th style="text-align:right" { "计划数量" }
                                        th style="text-align:right" { "完工数量" }
                                        th style="text-align:right" { "材料成本" }
                                        th style="text-align:right" { "人工成本" }
                                        th style="text-align:right" { "外协成本" }
                                        th style="text-align:right" { "总成本" }
                                        th { "状态" }
                                    }
                                }
                                tbody {
                                    @for w in work_orders {
                                        tr {
                                            td class="text-accent font-medium cursor-pointer" { (w.doc_number) }
                                            td { (w.product_name) }
                                            td class="text-right text-[13px]" { (w.planned_qty.round_dp(0)) }
                                            td class="text-right text-[13px]" { (w.completed_qty.round_dp(0)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(w.material_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(w.labor_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(w.outsource_cost)) }
                                            td class="text-right text-[13px]" style="font-weight:700;color:var(--accent)" { (fmt_money_full(w.total_cost)) }
                                            @let (label, cls) = wo_status_label(w.wo_status);
                                            td { span class=(format!("status-pill {cls}")) { (label) } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 利润中心 P&L 面板 ──
            div id="panel-profit" class="hidden" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-bottom:0" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" style="display:flex;justify-content:space-between;align-items:center" {
                        span { "利润中心 P&L · 2026-06" }
                    }
                    @if profit_centers.is_empty() {
                        div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无利润中心数据" }
                    } @else {
                        div class="overflow-x-auto" {
                            table class="data-table" style="min-width:900px" {
                                thead {
                                    tr {
                                        th { "利润中心" }
                                        th style="text-align:right" { "收入" }
                                        th style="text-align:right" { "材料成本" }
                                        th style="text-align:right" { "人工成本" }
                                        th style="text-align:right" { "制造费用" }
                                        th style="text-align:right" { "管理费用" }
                                        th style="text-align:right" { "利润" }
                                        th { "利润率" }
                                    }
                                }
                                tbody {
                                    @for pc in profit_centers {
                                        tr {
                                            td style="font-weight:600" { (pc.label) }
                                            td class="text-right text-[13px]" style="font-weight:600" { (fmt_money(pc.income)) }
                                            td class="text-right text-[13px]" { (fmt_money(pc.material_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money(pc.labor_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money(pc.overhead_cost)) }
                                            td class="text-right text-[13px]" { (fmt_money(pc.admin_cost)) }
                                            td class="text-right text-[13px]" style="font-weight:700;color:var(--success)" { (fmt_money(pc.profit)) }
                                            td { span class=(margin_class(pc.profit_rate)) { (format!("{}%", pc.profit_rate)) } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 毛利分析面板 ──
            div id="panel-margin" class="hidden" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-bottom:0" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" style="display:flex;justify-content:space-between;align-items:center" {
                        span { "订单毛利分析" }
                    }
                    @if margins.is_empty() {
                        div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无毛利数据" }
                    } @else {
                        div class="overflow-x-auto" {
                            table class="data-table" style="min-width:1000px" {
                                thead {
                                    tr {
                                        th { "订单号" }
                                        th { "客户" }
                                        th style="text-align:right" { "订单金额" }
                                        th style="text-align:right" { "实际成本" }
                                        th style="text-align:right" { "毛利" }
                                        th { "毛利率" }
                                    }
                                }
                                tbody {
                                    @for m in margins {
                                        tr {
                                            td class="text-accent font-medium cursor-pointer" { (m.doc_number) }
                                            td { (m.customer_name) }
                                            td class="text-right text-[13px]" style="font-weight:600" { (fmt_money_full(m.order_amount)) }
                                            td class="text-right text-[13px]" { (fmt_money_full(m.actual_cost)) }
                                            td class="text-right text-[13px]" style="font-weight:700" {
                                                @if m.margin_amount >= Decimal::ZERO {
                                                    span style="color:var(--success)" { (fmt_money_full(m.margin_amount)) }
                                                } @else {
                                                    span style="color:var(--danger)" { (fmt_money_full(m.margin_amount)) }
                                                }
                                            }
                                            td { span class=(margin_class(m.margin_rate)) { (format!("{}%", m.margin_rate)) } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        (PreEscaped(r#"<script>
        function switchTab(name) {
            var tabs = document.querySelectorAll('.analysis-tab');
            tabs.forEach(function(t) { t.classList.remove('active'); });
            var panels = document.querySelectorAll('.analysis-panel');
            panels.forEach(function(p) { p.classList.remove('active'); });
            event.target.classList.add('active');
            var panel = document.getElementById('panel-' + name);
            if (panel) panel.classList.add('active');
        }
        </script>"#))
    }
}

// ── Components ──

fn cost_breakdown_bar(material: Decimal, labor: Decimal, overhead: Decimal, total: Decimal) -> Markup {
    if total == Decimal::ZERO {
        return html! { "—" };
    }
    let mat_pct = (material / total * Decimal::from(100)).round_dp(0).to_string();
    let lab_pct = (labor / total * Decimal::from(100)).round_dp(0).to_string();
    let ovh_pct = (overhead / total * Decimal::from(100)).round_dp(0).to_string();
    html! {
        div class="flex h-[9px] overflow-hidden gap-[2px] relative" {
            div class="cb-material" style=(format!("width:{}%", mat_pct)) {}
            div class="cb-labor" style=(format!("width:{}%", lab_pct)) {}
            div class="cb-overhead" style=(format!("width:{}%", ovh_pct)) {}
        }
    }
}

fn stat_card(
    title: &str,
    value: &str,
    border_color: &str,
    icon_bg: &str,
    icon_color: &str,
    icon_path: &str,
) -> Markup {
    html! {
        div class="mes-flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" style=(format!("border-left:3px solid {border_color}")) {
            div class="mes-w-[44px] h-[44px] rounded grid place-items-center shrink-0" style=(format!("background:{icon_bg};color:{icon_color}")) {
                (PreEscaped(format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24"><path d="{icon_path}"/></svg>"#)))
            }
            div class="mes-stat-body" {
                div class="mes-text-sm text-text-muted mt-1" { (title) }
                div class="mes-text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (PreEscaped(value)) }
            }
        }
    }
}
