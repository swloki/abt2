use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use crate::errors::Result;
use crate::routes::wms_backflush::BackflushDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::backflush::{BackflushItem, BackflushService};
use abt_core::wms::enums::BackflushStatus;
use crate::components::icon;

#[require_permission("WMS", "read")]
pub async fn get_backflush_detail(
    path: BackflushDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.backflush_service();

    let record = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

    let content = backflush_detail_page(&record, &items);
    let page_html = admin_page(
        is_htmx,
        "倒冲记录详情",
        &claims,
        "inventory",
        "/admin/wms/backflushes",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

fn backflush_detail_page(
    record: &abt_core::wms::backflush::BackflushRecord,
    items: &[BackflushItem],
) -> Markup {
    let (status_label, status_class) = match record.status {
        BackflushStatus::Draft => ("草稿", "status-draft"),
        BackflushStatus::Executed => ("已执行", "status-completed"),
        BackflushStatus::Adjusted => ("已调整", "status-confirmed"),
    };

    let over_count = items.iter().filter(|i| i.is_over_threshold).count();
    let max_rate = items.iter()
        .map(|i| i.variance_rate.abs())
        .max()
        .unwrap_or(Decimal::ZERO);

    let show_adjust = matches!(record.status, BackflushStatus::Executed);

    html! {
        div {
            a href="/admin/wms/backflushes" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回倒冲记录列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (record.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::printer_icon("w-4 h-4"))
                        "打印"
                    }
                    @if show_adjust {
                        button class="btn btn-primary" {
                            "确认调整"
                        }
                    }
                }
            }

            // ── Status Flow ──
            (backflush_status_flow(record.status))

            // ── Info Card ──
            div class="info-card" {
                div class="info-card-title" { "倒冲信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "单据编号" }
                        span class="info-value mono" { (record.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联工单" }
                        span class="info-value mono" { "—" }
                    }
                    div class="info-item" {
                        span class="info-label" { "完工产品" }
                        span class="info-value" { "—" }
                    }
                    div class="info-item" {
                        span class="info-label" { "完工数量" }
                        span class="info-value mono" { (record.completed_qty.to_string()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "倒冲日期" }
                        span class="info-value mono" { (record.backflush_date.to_string()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "差异阈值" }
                        span class="info-value mono" { (format!("{}%", record.variance_threshold)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "状态" }
                        span class="info-value" {
                            span class=(format!("status-pill {status_class}")) { (status_label) }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { "—" }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div class="data-card-title" { "倒冲物料明细" }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "子件编码" }
                                th { "子件名称" }
                                th { "单位" }
                                th class="num-right" { "BOM理论用量" }
                                th class="num-right" { "实际倒冲量" }
                                th class="num-right" { "差异量" }
                                th class="num-right" { "差异率" }
                                th class="num-right" { "超标" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                (backflush_item_row(i + 1, item))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细数据"
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Summary Bar ──
                @if !items.is_empty() {
                    div class="summary-bar" {
                        div class="summary-item" {
                            div class="summary-value" { (items.len()) }
                            div class="summary-label" { "总子件数" }
                        }
                        div class="summary-item" {
                            div class=(if over_count > 0 { "summary-value danger" } else { "summary-value" }) { (over_count) }
                            div class="summary-label" { "超标项数" }
                        }
                        div class="summary-item" {
                            div class=(if max_rate > Decimal::ZERO { "summary-value danger" } else { "summary-value" }) {
                                @if max_rate > Decimal::ZERO {
                                    "+" (max_rate.to_string()) "%"
                                } @else {
                                    "0%"
                                }
                            }
                            div class="summary-label" { "最大差异率" }
                        }
                    }
                }
            }
        }
    }
}

fn backflush_item_row(idx: usize, item: &BackflushItem) -> Markup {
    let variance_sign = if item.variance_qty >= Decimal::ZERO { "+" } else { "" };
    let rate_sign = if item.variance_rate >= Decimal::ZERO { "+" } else { "" };
    let has_variance = item.variance_qty != Decimal::ZERO;

    html! {
        tr {
            td class="mono" { (idx) }
            td class="mono" { "—" }
            td { "—" }
            td { "—" }
            td class="num-right" { (item.theoretical_qty.to_string()) }
            td class="num-right" { (item.actual_qty.to_string()) }
            td class="num-right" style=(if has_variance { "color:var(--danger)" } else { "" }) {
                (variance_sign) (item.variance_qty.to_string())
            }
            td class="num-right" style=(if has_variance { "color:var(--danger)" } else { "" }) {
                (rate_sign) (item.variance_rate.to_string()) "%"
            }
            td class="num-right" {
                @if item.is_over_threshold {
                    span class="exceed-cell" { "✓" }
                } @else {
                    span style="color:var(--muted)" { "✗" }
                }
            }
        }
    }
}

fn backflush_status_flow(status: BackflushStatus) -> Markup {
    let steps = [
        ("草稿", BackflushStatus::Draft),
        ("已执行", BackflushStatus::Executed),
        ("已调整", BackflushStatus::Adjusted),
    ];

    let current_idx = match status {
        BackflushStatus::Draft => 0,
        BackflushStatus::Executed => 1,
        BackflushStatus::Adjusted => 2,
    };

    html! {
        div class="status-flow" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    span class="status-flow-arrow" { "→" }
                }
                span class=(if i < current_idx { "status-flow-step done" }
                    else if i == current_idx { "status-flow-step current" }
                    else { "status-flow-step" }) { (label) }
            }
        }
    }
}
