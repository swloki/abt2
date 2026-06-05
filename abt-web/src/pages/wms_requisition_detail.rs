use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::enums::RequisitionStatus;
use abt_core::wms::material_requisition::model::MaterialRequisition;
use abt_core::wms::material_requisition::repo::MaterialRequisitionRepo;
use abt_core::wms::material_requisition::MaterialRequisitionService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Status Label ──

fn status_label(s: RequisitionStatus) -> (&'static str, &'static str) {
    match s {
        RequisitionStatus::Draft => ("草稿", "status-draft"),
        RequisitionStatus::Confirmed => ("已确认", "status-confirmed"),
        RequisitionStatus::Issued => ("已发料", "status-completed"),
        RequisitionStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Workflow Steps ──

fn workflow_steps(status: RequisitionStatus) -> Markup {
    let steps: &[&str] = &["草稿", "已确认", "已发料"];
    let completed: Vec<bool> = match status {
        RequisitionStatus::Draft => vec![false, false, false],
        RequisitionStatus::Confirmed => vec![true, true, false],
        RequisitionStatus::Issued => vec![true, true, true],
        RequisitionStatus::Cancelled => vec![true, false, false],
    };
    let current_idx = match status {
        RequisitionStatus::Draft => Some(0),
        RequisitionStatus::Confirmed => Some(1),
        RequisitionStatus::Issued => Some(2),
        RequisitionStatus::Cancelled => None,
    };

    html! {
        div class="workflow-steps" {
            @for (i, label) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if completed[i] { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = match current_idx {
                    Some(ci) if ci == i => "wf-step current",
                    _ if completed[i] => "wf-step completed",
                    _ => "wf-step",
                };
                div class=(step_class) {
                    span class="wf-dot" {}
                    (label)
                }
            }
        }
    }
}

// ── Variance Color ──

fn variance_style(v: Decimal) -> (String, &'static str) {
    if v == Decimal::ZERO {
        ("0".into(), "color:var(--success)")
    } else if v < Decimal::ZERO {
        (v.to_string(), "color:var(--danger)")
    } else {
        (format!("+{v}"), "color:var(--warn)")
    }
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_requisition_detail(
    path: RequisitionDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.material_requisition_service();

    let requisition = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = MaterialRequisitionRepo::get_items(&mut conn, path.id)
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    let content = requisition_detail_page(&requisition, &items);
    let detail_path = RequisitionDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 领料单详情", requisition.doc_number),
        &claims,
        "inventory",
        &detail_path,
        "库存管理",
        Some(&requisition.doc_number),
        content,
    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn requisition_detail_page(
    requisition: &MaterialRequisition,
    items: &[abt_core::wms::material_requisition::model::MaterialReqItem],
) -> Markup {
    let (status_text, status_class) = status_label(requisition.status);
    let is_draft = requisition.status == RequisitionStatus::Draft;
    let is_confirmed = requisition.status == RequisitionStatus::Confirmed;

    html! {
        div {
            a href=(RequisitionListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回领料单列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (requisition.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="page-actions" {
                    @if is_draft || is_confirmed {
                        button class="btn btn-default" {
                            (icon::x_icon("w-4 h-4"))
                            "取消"
                        }
                    }
                    @if is_confirmed {
                        button class="btn btn-primary" {
                            (icon::bolt_icon("w-4 h-4"))
                            "确认发料"
                        }
                    }
                    @if is_draft {
                        button class="btn btn-primary" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认"
                        }
                    }
                }
            }

            (workflow_steps(requisition.status))

            // ── 领料信息 ──
            div class="info-card" {
                div class="info-card-title" { "领料信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "单据编号" }
                        span class="info-value mono" { (requisition.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联工单" }
                        span class="info-value mono" { "WO-" (requisition.work_order_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "领料仓库" }
                        span class="info-value" { "—" }
                    }
                    div class="info-item" {
                        span class="info-label" { "领料日期" }
                        span class="info-value mono" { (requisition.requisition_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { "—" }
                    }
                }
            }

            // ── 行项明细 ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品" }
                                th class="num-right" { "需求数量" }
                                th class="num-right" { "实领数量" }
                                th class="num-right" { "差异量" }
                                th { "储位" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                @let (variance_text, variance_style) = variance_style(item.variance_qty);
                                tr {
                                    td class="mono" { (i + 1) }
                                    td { "产品 #" (item.product_id) }
                                    td class="num-right" { (item.requested_qty) }
                                    td class="num-right" { (item.issued_qty) }
                                    td class="num-right" style=(variance_style) { (variance_text) }
                                    td { "—" }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无领料明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
