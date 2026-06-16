use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::master_data::product::ProductService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::dashboard::MesDashboardService;
use abt_core::wms::backflush::{BackflushFilter, BackflushService};
use abt_core::wms::material_requisition::{MaterialRequisitionService, model::RequisitionFilter};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{MaterialUsageDataPath, MaterialUsagePath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_material_usage(_path: MaterialUsagePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

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

    // Batch-load product names for dropdown display
    let product_ids: Vec<i64> = work_orders.iter().map(|wo| wo.product_id).collect();
    let products = if product_ids.is_empty() {
        Vec::new()
    } else {
        state.product_service().get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
    };
    let product_map: HashMap<i64, String> = products
        .into_iter()
        .map(|p| (p.product_id, p.pdt_name))
        .collect();

    let content = material_usage_page(&work_orders, &product_map);
    Ok(Html(admin_page(is_htmx, "物料消耗追踪", &claims, "production", MaterialUsagePath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct UsageDataParams {
    pub wo_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "read")]
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
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] text-center-empty" { "请选择工单" }
            }.into_string()));
        }
    };

    let dash_svc = state.mes_dashboard_service();
    let bf_svc = state.backflush_service();
    let req_svc = state.material_requisition_service();

    let wo_info = dash_svc.get_wo_basic_info(&service_ctx, &mut conn, wo_id).await?;
    let bom_items = dash_svc.get_bom_comparison(&service_ctx, &mut conn, wo_id).await?;
    let bf_records = bf_svc.list(
        &service_ctx, &mut conn,
        BackflushFilter { work_order_id: Some(wo_id), status: None },
        1, 50,
    ).await?;
    let requisitions = req_svc.list(
        &service_ctx, &mut conn,
        RequisitionFilter { work_order_id: Some(wo_id), ..Default::default() },
        1, 50,
    ).await?;

    // Compute summary
    let standard_qty: Decimal = bom_items.iter().map(|i| i.standard_total).sum();
    let backflush_qty: Decimal = bom_items.iter().map(|i| i.backflush_total).sum();
    let picked_total: Decimal = bom_items.iter().map(|i| i.picked_qty).sum();
    let variance = backflush_qty - standard_qty;

    let ctx = MaterialUsageContext {
        bom_items: &bom_items,
        standard_qty, backflush_qty, picked_total, variance,
        bf_records: &bf_records.items,
        requisitions: &requisitions.items,
    };
    let html_content = usage_data_fragment(&wo_info, &ctx);
    Ok(Html(html_content.into_string()))
}

fn material_usage_page(
    work_orders: &[abt_core::mes::work_order::WorkOrder],
    product_map: &HashMap<i64, String>,
) -> Markup {
    html! { div {
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "物料消耗追踪" }
            div class="flex gap-3" {
                button class="btn btn-default" {
                    (icon::download_icon(""))
                    " 导出"
                }
            }
        }

        // Filter bar
        div class="flex items-center gap-3 mb-5 flex-wrap" {
            select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="wo_id"
                hx-get=(MaterialUsageDataPath::PATH)
                hx-target="#usage-content"
                hx-trigger="change"
                hx-swap="innerHTML" {
                option value="" { "选择工单..." }
                @for wo in work_orders {
                    @let label = match product_map.get(&wo.product_id) {
                        Some(name) => format!("{} · {} ({})", wo.doc_number, name, crate::utils::fmt_qty(wo.planned_qty)),
                        None => wo.doc_number.clone(),
                    };
                    option value=(wo.id) { (label) }
                }
            }
            select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" disabled {
                option { "全部批次" }
            }
        }

        // Content area
        div id="usage-content" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] text-center-empty" { "请选择工单查看物料消耗数据" }
        }
    }}
}

struct MaterialUsageContext<'a> {
    bom_items: &'a [abt_core::mes::dashboard::model::BomCompareItem],
    standard_qty: Decimal,
    backflush_qty: Decimal,
    picked_total: Decimal,
    variance: Decimal,
    bf_records: &'a [abt_core::wms::backflush::model::BackflushRecord],
    requisitions: &'a [abt_core::wms::material_requisition::model::MaterialRequisition],
}

fn usage_data_fragment(wo_info: &abt_core::mes::dashboard::model::WoBasicInfo, ctx: &MaterialUsageContext) -> Markup {
    let status_display = match wo_info.status {
        1 => ("待计划", "pill-pending"),
        2 => ("已计划", "pill-progress"),
        3 => ("已下达", "pill-receipt"),
        4 => ("已关闭", "pill-done"),
        5 => ("已取消", "pill-suspended"),
        _ => ("—", ""),
    };
    let status_pill = html! { span class=(format!("kanban-card-pill {}", status_display.1)) { (status_display.0) } };

    html! {
        // ── WO header ──
        div class="wo-header" {
            div class="wo-header-left" {
                span class="wo-header-no" { (wo_info.doc_number) }
                span class="wo-header-product" { (wo_info.product_name.as_deref().unwrap_or("—")) }
                (status_pill)
            }
            div class="flex gap-4 text-sm text-muted" {
                span { "计划: " strong class="mono" { (crate::utils::fmt_qty(wo_info.planned_qty)) } }
                span { "完成: " strong class="text-success mono" { (crate::utils::fmt_qty(wo_info.completed_qty)) } }
                @if let Some(v) = &wo_info.bom_version {
                    span { "BOM: " strong { (v) } }
                }
            }
        }

        // ── Summary stats ──
        div class="usage-summary" {
            // BOM standard
            div class="stat-card" {
                div class="stat-icon blue" { (icon::box_icon("")) }
                div {
                    div class="stat-card-value" { (crate::utils::fmt_qty(ctx.standard_qty)) }
                    div class="stat-card-label" { "BOM 标准用量" }
                    div class="text-xs text-muted mt-1" {
                        "按完成 " (crate::utils::fmt_qty(wo_info.completed_qty)) " 件计算"
                    }
                }
            }
            // Actual picked
            div class="stat-card" {
                div class="stat-icon green" { (icon::clipboard_list_icon("")) }
                div {
                    div class="stat-card-value" { (crate::utils::fmt_qty(ctx.picked_total)) }
                    div class="stat-card-label" { "实际消耗(领料)" }
                    div class="text-xs text-muted mt-1" { "含损耗余量" }
                }
            }
            // Backflush
            div class="stat-card" {
                div class="stat-icon orange" { (icon::refresh_icon("")) }
                div {
                    div class="stat-card-value" { (crate::utils::fmt_qty(ctx.backflush_qty)) }
                    div class="stat-card-label" { "倒冲消耗" }
                }
            }
            // Variance
            div class="stat-card" {
                div class="stat-icon red" { (icon::circle_alert_icon("")) }
                div {
                    @let variance_cls = if ctx.variance > Decimal::ZERO { "text-danger" } else if ctx.variance < Decimal::ZERO { "text-success" } else { "" };
                    div class=(format!("stat-card-value {variance_cls}")) {
                        @if ctx.variance > Decimal::ZERO { "+" }
                        (crate::utils::fmt_qty(ctx.variance))
                    }
                    div class="stat-card-label" { "用量差异" }
                    @if ctx.standard_qty > Decimal::ZERO {
                        @let rate = ((ctx.variance / ctx.standard_qty) * Decimal::ONE_HUNDRED).abs();
                        div class="text-xs text-muted mt-1" {
                            "超出标准 " (crate::utils::fmt_qty(rate)) "%"
                        }
                    }
                }
            }
        }

        // ── BOM comparison table ──
        @if !ctx.bom_items.is_empty() {
            div class="section-card" {
                div class="section-card-head" {
                    (icon::box_icon(""))
                    "BOM 标准用量 vs 实际消耗"
                }
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "物料编码" }
                            th { "物料名称" }
                            th { "单位" }
                            th class="num-right" { "单件用量" }
                            th class="num-right" { "标准总量" }
                            th class="num-right" { "领料数量" }
                            th class="num-right" { "倒冲消耗" }
                            th class="num-right" { "损耗率" }
                            th class="num-right" { "差异" }
                        }}
                        tbody {
                            @for item in ctx.bom_items {
                                @let diff = item.backflush_total - item.standard_total;
                                @let diff_cls = if diff > Decimal::ZERO { "diff-positive" } else if diff < Decimal::ZERO { "diff-negative" } else { "diff-zero" };
                                @let loss_rate = if item.standard_total > Decimal::ZERO {
                                    let r = ((item.picked_qty - item.standard_total) / item.standard_total) * Decimal::ONE_HUNDRED;
                                    format!("{}%", crate::utils::fmt_qty(r))
                                } else {
                                    "—".to_string()
                                };
                                tr {
                                    td class="mono" { (item.component_code.as_deref().unwrap_or("—")) }
                                    td { (item.component_name.as_deref().unwrap_or("—")) }
                                    td { (item.unit.as_deref().unwrap_or("—")) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.per_unit_qty)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.standard_total)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.picked_qty)) }
                                    td class="num-right mono" { (crate::utils::fmt_qty(item.backflush_total)) }
                                    td class="num-right mono" { (loss_rate) }
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
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] text-center-empty" { "该工单未关联 BOM，无法显示物料对比" }
        }

        // ── Backflush detail records ──
        @if !ctx.bf_records.is_empty() {
            div class="section-card" {
                div class="section-card-head" {
                    (icon::refresh_icon(""))
                    "倒冲明细记录"
                }
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "倒冲单号" }
                            th { "完成数量" }
                            th { "倒冲日期" }
                            th { "状态" }
                        }}
                        tbody {
                            @for rec in ctx.bf_records {
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

        // ── Requisition records ──
        @if !ctx.requisitions.is_empty() {
            div class="section-card" {
                div class="section-card-head" {
                    (icon::clipboard_list_icon(""))
                    "领料记录"
                }
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" {
                        thead { tr {
                            th { "领料单号" }
                            th { "领料日期" }
                            th { "状态" }
                        }}
                        tbody {
                            @for req in ctx.requisitions {
                                tr {
                                    td class="mono" {
                                        a href=(format!("/admin/wms/requisition/{}", req.id)) class="link-cell" { (req.doc_number) }
                                    }
                                    td { (req.requisition_date) }
                                    td { (requisition_status_label(&req.status)) }
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

fn requisition_status_label(s: &abt_core::wms::enums::RequisitionStatus) -> Markup {
    use abt_core::wms::enums::RequisitionStatus;
    let (label, cls) = match s {
        RequisitionStatus::Draft => ("待确认", "pill-pending"),
        RequisitionStatus::Confirmed => ("已确认", "pill-progress"),
        RequisitionStatus::Issued => ("已发料", "pill-done"),
        RequisitionStatus::Cancelled => ("已取消", "pill-suspended"),
        RequisitionStatus::PartiallyIssued => ("部分发料", "pill-progress"),
    };
    html! { span class=(format!("kanban-card-pill {cls}")) { (label) } }
}
