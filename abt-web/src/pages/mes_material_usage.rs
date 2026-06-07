use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::mes::dashboard::MesDashboardService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::wms::backflush::{BackflushFilter, BackflushService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{MaterialUsageDataPath, MaterialUsagePath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_material_usage(_path: MaterialUsagePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    // Load work orders for dropdown (non-cancelled)
    let wo_svc = state.work_order_service();
    let filter = abt_core::mes::work_order::WorkOrderFilter {
        status: None,
        product_id: None,
        keyword: None,
        date_from: None,
        date_to: None,
    };
    let wo_result = wo_svc.list(&service_ctx, &mut conn, filter, 1, 100).await?;
    let work_orders = wo_result.items;

    let content = material_usage_page(&work_orders);
    Ok(Html(admin_page(is_htmx, "物料消耗追踪", &claims, "production", MaterialUsagePath::PATH, "生产管理", None, content).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct UsageDataParams {
    pub wo_id: Option<i64>,
}

#[require_permission("MES", "read")]
pub async fn load_usage_data(
    _path: MaterialUsageDataPath,
    ctx: RequestContext,
    Query(params): Query<UsageDataParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let wo_id = match params.wo_id {
        Some(id) => id,
        None => {
            return Ok(Html(html! {
                div class="info-card text-center-empty" { "请选择工单" }
            }.into_string()));
        }
    };

    let dash_svc = state.mes_dashboard_service();
    let bf_svc = state.backflush_service();

    let wo_info = dash_svc.get_wo_basic_info(&service_ctx, &mut conn, wo_id).await?;
    let bom_items = dash_svc.get_bom_comparison(&service_ctx, &mut conn, wo_id).await?;
    let bf_records = bf_svc.list(
        &service_ctx, &mut conn,
        BackflushFilter { work_order_id: Some(wo_id), status: None },
        1, 50,
    ).await?;

    // Compute summary
    let standard_qty: Decimal = bom_items.iter().map(|i| i.standard_total).sum();
    let backflush_qty: Decimal = bom_items.iter().map(|i| i.backflush_total).sum();
    let variance = backflush_qty - standard_qty;

    let html_content = usage_data_fragment(&wo_info, &bom_items, standard_qty, backflush_qty, variance, &bf_records.items);
    Ok(Html(html_content.into_string()))
}

fn material_usage_page(
    work_orders: &[abt_core::mes::work_order::WorkOrder],
) -> Markup {
    html! { div {
        div class="page-header" {
            h1 class="page-title" { "物料消耗追踪" }
        }

        // Filter bar - work order selector
        div class="filter-bar" {
            select class="form-select" name="wo_id"
                hx-get=(MaterialUsageDataPath::PATH)
                hx-target="#usage-content"
                hx-trigger="change"
                hx-swap="innerHTML" {
                option value="" { "选择工单..." }
                @for wo in work_orders {
                    @if wo.status != abt_core::mes::enums::WorkOrderStatus::Cancelled {
                        option value=(wo.id) {
                            (wo.doc_number)
                        }
                    }
                }
            }
        }

        // Content area
        div id="usage-content" {
            div class="info-card text-center-empty" { "请选择工单查看物料消耗数据" }
        }
    }}
}

fn usage_data_fragment(
    wo_info: &abt_core::mes::dashboard::model::WoBasicInfo,
    bom_items: &[abt_core::mes::dashboard::model::BomCompareItem],
    standard_qty: Decimal,
    backflush_qty: Decimal,
    variance: Decimal,
    bf_records: &[abt_core::wms::backflush::model::BackflushRecord],
) -> Markup {
    let status_display = match wo_info.status {
        1 => ("待计划", "pill-pending"),
        2 => ("已计划", "pill-progress"),
        3 => ("已下达", "pill-receipt"),
        4 => ("已关闭", "pill-done"),
        5 => ("已取消", "pill-suspended"),
        _ => ("—", ""),
    };

    let variance_color = if variance > Decimal::ZERO { "color:var(--danger)" } else if variance < Decimal::ZERO { "color:var(--success)" } else { "" };

    html! {
        // WO header
        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "工单号" } span class="mono" { (wo_info.doc_number) } }
                div class="info-item" { label { "产品" } span { (wo_info.product_name.as_deref().unwrap_or("—")) } }
                div class="info-item" { label { "状态" } span class=(format!("kanban-card-pill {}", status_display.1)) { (status_display.0) } }
                div class="info-item" { label { "计划数量" } span class="mono" { (crate::utils::fmt_qty(wo_info.planned_qty)) } }
                div class="info-item" { label { "完成数量" } span class="mono" { (crate::utils::fmt_qty(wo_info.completed_qty)) } }
            }
        }

        // Summary stats
        div class="usage-summary" {
            div class="stat-card" {
                div class="stat-card-value" { (crate::utils::fmt_qty(standard_qty)) }
                div class="stat-card-label" { "BOM 标准用量" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-progress" { (crate::utils::fmt_qty(backflush_qty)) }
                div class="stat-card-label" { "倒冲消耗" }
            }
            div class="stat-card" {
                div class="stat-card-value" style=(variance_color) {
                    @if variance > Decimal::ZERO { "+" }
                    (crate::utils::fmt_qty(variance))
                }
                div class="stat-card-label" { "用量差异" }
            }
            div class="stat-card" {
                @if standard_qty > Decimal::ZERO {
                    @let rate = ((variance / standard_qty) * Decimal::ONE_HUNDRED).abs();
                    div class="stat-card-value" style=(variance_color) {
                        (crate::utils::fmt_qty(rate)) "%"
                    }
                } @else {
                    div class="stat-card-value" { "—" }
                }
                div class="stat-card-label" { "差异率" }
            }
        }

        // BOM comparison table
        @if !bom_items.is_empty() {
            div class="data-card" {
                div class="data-card-header" {
                    span class="data-card-title" { "BOM 标准用量 vs 倒冲消耗" }
                }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "物料编码" }
                            th { "物料名称" }
                            th { "单位" }
                            th class="num-right" { "单件用量" }
                            th class="num-right" { "标准总量" }
                            th class="num-right" { "倒冲消耗" }
                            th class="num-right" { "差异" }
                        }}
                        tbody {
                            @for item in bom_items {
                                @let diff = item.backflush_total - item.standard_total;
                                @let diff_cls = if diff > Decimal::ZERO { "diff-positive" } else if diff < Decimal::ZERO { "diff-negative" } else { "diff-zero" };
                                tr {
                                    td class="mono" { (item.component_code.as_deref().unwrap_or("—")) }
                                    td { (item.component_name.as_deref().unwrap_or("—")) }
                                    td { (item.unit.as_deref().unwrap_or("—")) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.per_unit_qty)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.standard_total)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.backflush_total)) }
                                    td class="num-right" {
                                        span class=(format!("diff-indicator {diff_cls}")) {
                                            @if diff > Decimal::ZERO { "+" }
                                            (crate::utils::fmt_qty(diff))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } @else {
            div class="info-card text-center-empty" { "该工单未关联 BOM，无法显示物料对比" }
        }

        // Backflush records
        @if !bf_records.is_empty() {
            div class="data-card" {
                div class="data-card-header" {
                    span class="data-card-title" { "倒冲消耗记录" }
                }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "倒冲单号" }
                            th { "完成数量" }
                            th { "倒冲日期" }
                            th { "状态" }
                        }}
                        tbody {
                            @for rec in bf_records {
                                tr {
                                    td class="mono" {
                                        a href=(format!("/admin/wms/backflush/{}", rec.id)) class="link-cell" { (rec.doc_number) }
                                    }
                                    td class="num-right mono" { (crate::utils::fmt_qty(rec.completed_qty)) }
                                    td { (rec.backflush_date) }
                                    td { (backflush_status_label(&rec.status)) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn backflush_status_label(s: &abt_core::wms::enums::BackflushStatus) -> Markup {
    use abt_core::wms::enums::BackflushStatus;
    let (label, cls) = match s {
        BackflushStatus::Draft => ("待处理", "pill-pending"),
        BackflushStatus::Executed => ("已完成", "pill-done"),
        BackflushStatus::Adjusted => ("已调整", "pill-progress"),
    };
    html! { span class=(format!("kanban-card-pill {cls}")) { (label) } }
}
