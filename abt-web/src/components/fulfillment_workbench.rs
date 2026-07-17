use std::collections::HashMap;

use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::master_data::product::model::AcquireChannel;
use abt_core::sales::sales_order::model::{
    DemandStatus, FulfillmentLineStatus, FulfillmentPlanLine, SalesOrderItem,
};

use crate::components::icon;
use crate::components::reservation_detail::reservation_detail_badge;
use crate::utils::fmt_qty;

const DECIMAL_100: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

fn fulfill_status_pill(s: FulfillmentLineStatus) -> (&'static str, &'static str) {
    match s {
        FulfillmentLineStatus::Pending => ("待处理", "status-pending"),
        FulfillmentLineStatus::Allocated => ("已分配", "status-confirmed"),
        FulfillmentLineStatus::Producing => ("生产中", "status-warn"),
        FulfillmentLineStatus::Purchasing => ("采购中", "status-purple"),
        FulfillmentLineStatus::Fulfilled => ("已履约", "status-success"),
    }
}

fn acquire_tag(ch: AcquireChannel) -> (&'static str, &'static str) {
    match ch {
        AcquireChannel::SelfProduced | AcquireChannel::Legacy => ("自制", "status-confirmed"),
        AcquireChannel::Purchased => ("外购", "status-purple"),
        AcquireChannel::Outsourced => ("委外", "status-warn"),
        AcquireChannel::NonInventory => ("非库存", "status-muted"),
    }
}

/// 履约进度条（按数量加权）：已发货 / 已分配 / 补货中 / 未交量。
pub fn fulfillment_progress(items: &[SalesOrderItem], plan_lines: &[FulfillmentPlanLine]) -> Markup {
    // 加权进度：基于数量（quantity），而非行数（line count）
    let total_qty: Decimal = items.iter().map(|i| i.quantity).sum();
    if total_qty <= Decimal::ZERO {
        return html! {};
    }

    let shipped_qty: Decimal = items.iter().map(|i| i.shipped_qty).sum();
    let allocated_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Allocated)
        .map(|p| p.reserved_qty)
        .sum();
    let producing_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Producing)
        .map(|p| p.shortage_qty)
        .sum();
    let purchasing_qty: Decimal = plan_lines
        .iter()
        .filter(|p| p.status == FulfillmentLineStatus::Purchasing)
        .map(|p| p.shortage_qty)
        .sum();
    let pending_qty = total_qty - shipped_qty - allocated_qty - producing_qty - purchasing_qty;
    let restock_qty = producing_qty + purchasing_qty; // 补货中 = 生产中 + 采购中

    // 百分比辅助（trim .0 后缀，如 35.0% → 35%）
    let pct_str = |qty: Decimal| -> String {
        let v = (qty / total_qty * DECIMAL_100).round_dp(1);
        let s = v.to_string();
        if s.ends_with(".0") {
            format!("{}%", &s[..s.len() - 2])
        } else {
            format!("{}%", s)
        }
    };
    let pct_style = |qty: Decimal| -> String {
        let v = (qty / total_qty * DECIMAL_100).round_dp(1);
        format!("width:{}%", v)
    };

    html! {
        div class="bg-bg border border-border rounded-md py-5 px-6 mb-5" {
            // Header: 标题 + 4 个统计箱
            div class="flex items-center justify-between mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg" {
                    (icon::chart_bar_icon("w-4 h-4 text-accent"))
                    "履约进度"
                }
                div class="flex gap-6" {
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-success" {
                            (crate::utils::fmt_qty(shipped_qty))
                        }
                        div class="text-[11px] text-muted mt-0.5" { "已发货" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-accent" {
                            (crate::utils::fmt_qty(allocated_qty))
                        }
                        div class="text-[11px] text-muted mt-0.5" { "已分配" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-warn" {
                            (crate::utils::fmt_qty(restock_qty))
                        }
                        div class="text-[11px] text-muted mt-0.5" { "补货中" }
                    }
                    div class="text-center" {
                        div class="text-lg font-bold font-mono tabular-nums text-fg" {
                            (crate::utils::fmt_qty(pending_qty))
                        }
                        div class="text-[11px] text-muted mt-0.5" { "未交量" }
                    }
                }
            }
            // 细进度条（8px 高，无文字）
            div class="flex h-2 rounded overflow-hidden bg-border-soft" {
                @if shipped_qty > Decimal::ZERO {
                    div class="bg-success [transition:width_600ms_cubic-bezier(0.2,0,0,1)]"
                        style=(pct_style(shipped_qty)) {}
                }
                @if allocated_qty > Decimal::ZERO {
                    div class="bg-accent [transition:width_600ms_cubic-bezier(0.2,0,0,1)]"
                        style=(pct_style(allocated_qty)) {}
                }
                @if producing_qty > Decimal::ZERO {
                    div class="bg-warn [transition:width_600ms_cubic-bezier(0.2,0,0,1)]"
                        style=(pct_style(producing_qty)) {}
                }
                @if purchasing_qty > Decimal::ZERO {
                    div class="bg-purple-500 [transition:width_600ms_cubic-bezier(0.2,0,0,1)]"
                        style=(pct_style(purchasing_qty)) {}
                }
                @if pending_qty > Decimal::ZERO {
                    div class="bg-border [transition:width_600ms_cubic-bezier(0.2,0,0,1)]"
                        style=(pct_style(pending_qty)) {}
                }
            }
            // 图例
            div class="flex gap-5 mt-3 flex-wrap" {
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-success" {}
                    (format!("已发货 {}", pct_str(shipped_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-accent" {}
                    (format!("已分配 {}", pct_str(allocated_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-warn" {}
                    (format!("生产中 {}", pct_str(producing_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-purple-500" {}
                    (format!("采购中 {}", pct_str(purchasing_qty)))
                }
                span class="flex items-center gap-1.5 text-[11px] text-muted" {
                    span class="w-2 h-2 rounded-full shrink-0 bg-border" {}
                    (format!("待处理 {}", pct_str(pending_qty)))
                }
            }
        }
    }
}

// ── Fulfillment Workbench ──
/// 履约工作台：需求流转统计 + ATP 满足率 + 缺口 + 需求/履约状态 + 下游单据（PP/PO/WO/OM）。
/// plan_lines 为空时返回空片段。
pub fn fulfillment_workbench(
    plan_lines: &[FulfillmentPlanLine],
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    atp_map: &HashMap<i64, Decimal>,
    demand_map: &HashMap<i64, DemandStatus>,
    reserved_map: &HashMap<i64, Decimal>,
    order_id: i64,
) -> Markup {
    if plan_lines.is_empty() {
        return html! {};
    }

    // 需求流转统计
    let mut demand_open = 0usize;
    let mut demand_processing = 0usize;
    let mut demand_done = 0usize;
    for pl in plan_lines {
        match pl.status {
            FulfillmentLineStatus::Pending => demand_open += 1,
            FulfillmentLineStatus::Allocated
            | FulfillmentLineStatus::Producing
            | FulfillmentLineStatus::Purchasing => demand_processing += 1,
            FulfillmentLineStatus::Fulfilled => demand_done += 1,
        }
    }
    let demand_total = plan_lines.len();

    html! {
        div class="bg-bg border border-border rounded-md mt-5 overflow-hidden" {
            // ── Header: 标题+badge 在左，操作按钮在右 ──
            div class="flex items-center justify-between p-4 px-5 border-b border-border-soft bg-bg" {
                div class="flex items-center gap-3" {
                    span class="text-sm font-semibold text-fg" { "履约工作台" }
                    span
                        class="bg-accent-bg text-accent rounded-full text-[11px] font-medium px-2 py-0.5"
                    { (format!("{} 行", demand_total)) }
                }
                div class="flex gap-2" {
                    button
                        class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150"
                    { (icon::refresh_icon("w-3.5 h-3.5")) "刷新状态" }
                    a   class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150"
                        href="/admin/mes/work-center"
                        title="生产需求池"
                    { (icon::grid_icon("w-3.5 h-3.5")) "生产需求池" }
                    a   class="inline-flex items-center gap-1 py-[5px] px-3 text-xs rounded-sm bg-white text-fg-2 border border-border hover:border-accent hover:text-accent font-medium cursor-pointer transition-all duration-150"
                        href=(format!("/admin/purchase/work-center?order_id={}", order_id))
                        title="查看本订单的采购需求"
                    { (icon::clipboard_document_icon("w-3.5 h-3.5")) "采购需求池" }
                }
            }
            // ── 需求流转状态卡片 ──
            div class="flex gap-3 p-4 flex-wrap" {
                div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center"
                {
                    div class="text-[11px] text-muted mb-1" { "需求总数" }
                    div class="text-[22px] font-bold font-mono tabular-nums text-fg" { (demand_total) }
                }
                div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center"
                {
                    div class="text-[11px] text-muted mb-1" { "待处理" }
                    div class="text-[22px] font-bold font-mono tabular-nums text-fg" { (demand_open) }
                }
                div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center"
                {
                    div class="text-[11px] text-muted mb-1" { "处理中" }
                    div class="text-[22px] font-bold font-mono tabular-nums text-warn" {
                        (demand_processing)
                    }
                }
                div class="flex-1 min-w-[120px] bg-surface-raised border border-border-soft rounded-md py-3 px-4 text-center"
                {
                    div class="text-[11px] text-muted mb-1" { "已完成" }
                    div class="text-[22px] font-bold font-mono tabular-nums text-success" {
                        (demand_done)
                    }
                }
            }

            table class="data-table mb-6" {
                thead {
                    tr {
                        th { "产品" }
                        th { "获取途径" }
                        th class="text-right text-[13px]" { "需求量" }
                        th class="text-right text-[13px]" { "可满足量" }
                        th class="text-right text-[13px]" { "缺口" }
                        th { "库存满足率" }
                        th { "需求状态" }
                        th { "履约状态" }
                        th { "下游单据" }
                    }
                }
                tbody {
                    @for pl in plan_lines {
                        ({
                            fulfill_plan_row(
                                pl,
                                product_names,
                                product_codes,
                                atp_map,
                                demand_map,
                                reserved_map,
                            )
                        })
                    }
                }
            }
        }
    }
}

fn fulfill_plan_row(
    pl: &FulfillmentPlanLine,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    atp_map: &HashMap<i64, Decimal>,
    demand_map: &HashMap<i64, DemandStatus>,
    reserved_map: &HashMap<i64, Decimal>,
) -> Markup {
    let p_name = names.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
    let p_code = codes.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
    let (ch_label, ch_class) = acquire_tag(pl.acquire_channel);
    let (st_label, st_class) = fulfill_status_pill(pl.status);

    // 需求状态 — 来自 demand 表的真实需求池状态（不再复用 fulfillment status）
    // 无 demand = 库存已满足（shortage=0，无需补货）；有 demand 则按 demand.status 显示
    let (demand_label, demand_style) = match demand_map.get(&pl.order_line_id) {
        None => ("✓ 已满足", "background:#d1fae5;color:#065f46;"),
        Some(DemandStatus::Pending) => ("⚠ 待补货", "background:#e5e7eb;color:#374151;"),
        Some(DemandStatus::Confirmed) => ("● 已确认", "background:#dbeafe;color:#1e40af;"),
        Some(DemandStatus::InProgress) => ("◐ 补货中", "background:#fef3c7;color:#92400e;"),
        Some(DemandStatus::Fulfilled) => ("✓ 补货完成", "background:#d1fae5;color:#065f46;"),
        Some(DemandStatus::Rejected) => ("✗ 已驳回", "background:#fee2e2;color:#991b1b;"),
    };

    // 满足率（含当前可用库存 ATP，实时反映入库后的库存变化）
    let current_atp = atp_map.get(&pl.product_id).copied().unwrap_or(Decimal::ZERO);
    let effective_qty = (pl.reserved_qty + current_atp).min(pl.required_qty);
    let effective_shortage = (pl.required_qty - effective_qty).max(Decimal::ZERO);
    let fill_pct_val = if pl.required_qty > Decimal::ZERO {
        (effective_qty / pl.required_qty * DECIMAL_100)
            .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
    } else {
        Decimal::ZERO
    };
    let fill_bar_pct = format!("width:{}%", fill_pct_val);
    let fill_pct_str = format!("{}%", fill_pct_val);
    let fill_color = if effective_qty >= pl.required_qty {
        "#10b981"
    } else if effective_qty > Decimal::ZERO {
        "#f59e0b"
    } else {
        "#ef4444"
    };

    // 下游单据链接
    let downstream_doc = match (pl.source_doc_type, pl.source_doc_id) {
        (Some(12), Some(doc_id)) => {
            // ProductionPlan
            Some(html! {
                a   href=(format!("/admin/mes/plans/{}", doc_id))
                    class="text-accent font-medium cursor-pointer font-mono tabular-nums text-xs"
                { (format!("PP-{}", doc_id)) }
            })
        }
        (Some(7), Some(doc_id)) => {
            // PurchaseOrder
            Some(html! {
                a   href=(format!("/admin/purchase/orders/{}", doc_id))
                    class="text-accent font-medium cursor-pointer font-mono tabular-nums text-xs"
                { (format!("PO-{}", doc_id)) }
            })
        }
        (Some(10), Some(doc_id)) => {
            // WorkOrder
            Some(html! {
                span class="text-accent font-medium font-mono tabular-nums text-xs"
                { (format!("WO-{}", doc_id)) }
            })
        }
        (Some(11), Some(doc_id)) => {
            // OutsourcingOrder
            Some(html! {
                a   href=(format!("/admin/om/outsourcing/{}", doc_id))
                    class="text-accent font-medium cursor-pointer font-mono tabular-nums text-xs"
                { (format!("OM-{}", doc_id)) }
            })
        }
        _ => None,
    };

    html! {
        tr  class=({
                if effective_shortage > Decimal::ZERO {
                    "text-danger"
                } else if pl.reserved_qty > Decimal::ZERO {
                    "text-warn"
                } else {
                    ""
                }
            })
        {
            td {
                div {
                    span class="block font-medium text-fg text-sm" {
                        (p_name)
                        @if reserved_map.get(&pl.product_id).copied().unwrap_or(Decimal::ZERO) > Decimal::ZERO {
                            (reservation_detail_badge(pl.product_id))
                        }
                    }
                    span class="block text-xs text-muted mt-0.5 font-mono tabular-nums" { (p_code) }
                }
            }
            td {
                span class=(format!("status-pill {}", crate::utils::status_color(ch_class))) {
                    (ch_label)
                }
            }
            td class="text-right text-[13px]" { (fmt_qty(pl.required_qty)) }
            td class="text-right text-[13px]" { (fmt_qty(effective_qty)) }
            td class="text-right text-[13px]" {
                @if effective_shortage > Decimal::ZERO {
                    span class="text-danger" { (fmt_qty(effective_shortage)) }
                } @else {
                    span class="text-success" { "0" }
                }
            }
            td {
                div class="flex items-center gap-2" {
                    div class="flex-1 overflow-hidden"
                        style="background:#e5e7eb;height:6px;border-radius:3px"
                    {
                        div style=({
                                format!(
                                    "width:{};background:{};height:100%;",
                                    fill_bar_pct,
                                    fill_color,
                                )
                            }) {}
                    }
                    span class="text-xs text-muted" { (fill_pct_str) }
                }
            }
            td {
                span
                    style=({
                        format!(
                            "padding:2px 8px;border-radius:12px;font-size:12px;{}",
                            demand_style,
                        )
                    })
                { (demand_label) }
            }
            td {
                span class=(format!("status-pill {}", crate::utils::status_color(st_class))) {
                    (st_label)
                }
            }
            td {
                @if let Some(doc) = downstream_doc { (doc) } @else {
                    span class="text-muted" { "—" }
                }
            }
        }
    }
}
