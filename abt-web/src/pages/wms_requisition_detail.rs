use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::enums::RequisitionStatus;
use abt_core::wms::material_requisition::model::{IssueItemReq, IssueMaterialReq, MaterialRequisition};
use abt_core::wms::material_requisition::repo::MaterialRequisitionRepo;
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct RequisitionActionForm {
    pub action: String,
}

// ── Status Label ──

fn status_label(s: RequisitionStatus) -> (&'static str, &'static str) {
    match s {
        RequisitionStatus::Draft => ("草稿", "status-draft"),
        RequisitionStatus::Confirmed => ("已确认", "status-confirmed"),
        RequisitionStatus::Issued => ("已发料", "status-completed"),
        RequisitionStatus::Cancelled => ("已取消", "status-cancelled"),
        RequisitionStatus::PartiallyIssued => ("部分发料", "status-progress"),
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
        RequisitionStatus::PartiallyIssued => vec![true, true, false],
    };
    let current_idx = match status {
        RequisitionStatus::Draft => Some(0),
        RequisitionStatus::Confirmed => Some(1),
        RequisitionStatus::Issued => Some(2),
        RequisitionStatus::Cancelled => None,
        RequisitionStatus::PartiallyIssued => Some(1),
    };

    html! {
        div class="flex items-center" {
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
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    (label)
                }
            }
        }
    }
}

// ── Variance Color ──

fn variance_color_class(v: Decimal) -> (String, &'static str) {
    if v == Decimal::ZERO {
        ("0".into(), "text-success")
    } else if v < Decimal::ZERO {
        (format!("{:.2}", v), "text-danger")
    } else {
        (format!("+{:.2}", v), "text-warn")
    }
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_requisition_detail(
    path: RequisitionDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.material_requisition_service();

    let requisition = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = MaterialRequisitionRepo::get_items(&mut conn, path.id)
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    let wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, requisition.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, requisition.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let product_svc = state.product_service();
    let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for item in &items {
        if !product_names.contains_key(&item.product_id)
            && let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
                product_names.insert(item.product_id, format!("{} ({})", p.pdt_name, p.product_code));
            }
    }

    let detail_path = RequisitionDetailPath { id: path.id }.to_string();
    let content = requisition_detail_page(&requisition, &items, &detail_path, &wh_name, &operator_name, &product_names);
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 领料单详情", requisition.doc_number),
        &claims,
        "inventory",
        &detail_path,
        "库存管理",
        Some(&requisition.doc_number),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn post_requisition_action(
    path: RequisitionDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RequisitionActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();

    match form.action.as_str() {
        "confirm" => svc.confirm(&service_ctx, &mut conn, path.id).await?,
        "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
        "issue" => {
            // 快速发料：实发数量 = 需求数量
            let items = MaterialRequisitionRepo::get_items(&mut conn, path.id)
                .await
                .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
            let issue_items: Vec<IssueItemReq> = items.iter()
                .map(|item| IssueItemReq {
                    item_id: item.id,
                    issued_qty: item.requested_qty,
                    bin_id: None,
                })
                .collect();
            svc.issue(&service_ctx, &mut conn, IssueMaterialReq {
                id: path.id,
                items: issue_items,
            }).await?;
        }
        _ => {}
    }

    let redirect_url = RequisitionDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

    Ok(resp)
}

// ── Components ──

fn requisition_detail_page(
    requisition: &MaterialRequisition,
    items: &[abt_core::wms::material_requisition::model::MaterialReqItem],
    detail_path: &str,
    wh_name: &str,
    operator_name: &str,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(requisition.status);

    html! {
        div {
            a href=(format!("{}?restore=true", RequisitionListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回领料单列表"
            }

            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        h1 class="text-2xl font-extrabold font-font-mono tabular-nums" { (requisition.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="flex gap-3" {
                    (requisition_action_buttons(requisition.status, detail_path))
                }
            }

            (workflow_steps(requisition.status))

            // ── 领料信息 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "领料信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "单据编号" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (requisition.doc_number) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "关联工单" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { "WO-" (requisition.work_order_id) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "领料仓库" }
                        span class="text-sm text-fg font-medium" { (wh_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "领料日期" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (requisition.requisition_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "操作员" }
                        span class="text-sm text-fg font-medium" { (operator_name) }
                    }
                }
            }

            // ── 行项明细 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品" }
                                th class="text-right text-[13px]" { "需求数量" }
                                th class="text-right text-[13px]" { "实领数量" }
                                th class="text-right text-[13px]" { "差异量" }
                                th { "工序" }
                                th { "批次" }
                                th { "储位" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                @let (variance_text, variance_class) = variance_color_class(item.variance_qty);
                                tr {
                                    td class="font-mono tabular-nums" { (i + 1) }
                                    td { (product_names.get(&item.product_id).map(|n| n.as_str()).unwrap_or("—")) }
                                    td class="text-right text-[13px]" { (format!("{:.2}", item.requested_qty)) }
                                    td class="text-right text-[13px]" { (format!("{:.2}", item.issued_qty)) }
                                    td class=(format!("num-right {}", variance_class)) { (variance_text) }
                                    td class="font-mono tabular-nums" { (item.operation_id.map(|id| format!("#{}", id)).unwrap_or_else(|| "—".into())) }
                                    td class="font-mono tabular-nums" { (item.batch_id.map(|id| format!("#{}", id)).unwrap_or_else(|| "—".into())) }
                                    td { (item.bin_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into())) }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="8" class="text-center text-text-muted text-sm" { "暂无领料明细" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn requisition_action_buttons(status: RequisitionStatus, detail_path: &str) -> Markup {
    match status {
        RequisitionStatus::Draft => {
            html! {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"confirm"}"#
                    hx-confirm="确定要确认此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认"
                }
            }
        }
        RequisitionStatus::Confirmed => {
            html! {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此领料单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"issue"}"#
                    hx-confirm="确定要确认发料吗？实发数量将按需求数量自动填写。"
                    hx-redirect=(detail_path) {
                    (icon::bolt_icon("w-4 h-4"))
                    "确认发料"
                }
            }
        }
        RequisitionStatus::Issued | RequisitionStatus::PartiallyIssued => {
            html! {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button"
                    _="on click add .is-open to #return-modal" {
                    (icon::return_arrow_icon("w-4 h-4"))
                    "退料"
                }
            }
        }
        _ => html! {},
    }
}
