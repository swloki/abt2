use axum::response::Html;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_transfer::TransferDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::enums::TransferStatus;
use abt_core::wms::transfer::{TransferItem, TransferService};
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;
use crate::components::icon;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct TransferActionForm {
    pub action: String,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_transfer_detail(
    path: TransferDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.transfer_service();

    let transfer = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

    let from_wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, transfer.from_warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let to_wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, transfer.to_warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, transfer.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let product_svc = state.product_service();
    let mut product_codes: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut product_specs: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut product_units: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for item in &items {
        if !product_names.contains_key(&item.product_id) {
            if let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
                product_codes.insert(item.product_id, p.product_code.clone());
                product_names.insert(item.product_id, p.pdt_name.clone());
                let spec = p.meta.specification.trim().to_string();
                if !spec.is_empty() {
                    product_specs.insert(item.product_id, spec);
                }
                product_units.insert(item.product_id, p.unit.clone());
            }
        }
    }

    let detail_path = TransferDetailPath { id: path.id }.to_string();
    let content = transfer_detail_page(&transfer, &items, &detail_path, &from_wh_name, &to_wh_name, &operator_name, &product_codes, &product_names, &product_specs, &product_units);
    let page_html = admin_page(
        is_htmx,
        "调拨单详情",
        &claims,
        "inventory",
        "/admin/wms/transfers",
        "库存管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn post_transfer_action(
    path: TransferDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<TransferActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.transfer_service();

    match form.action.as_str() {
        "dispatch" => svc.dispatch(&service_ctx, &mut conn, path.id).await?,
        "complete" => svc.complete(&service_ctx, &mut conn, path.id).await?,
        "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
        _ => {}
    }

    let redirect_url = TransferDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

    Ok(resp)
}

// ── Components ──

fn transfer_detail_page(
    transfer: &abt_core::wms::transfer::InventoryTransfer,
    items: &[TransferItem],
    detail_path: &str,
    from_wh_name: &str,
    to_wh_name: &str,
    operator_name: &str,
    product_codes: &std::collections::HashMap<i64, String>,
    product_names: &std::collections::HashMap<i64, String>,
    product_specs: &std::collections::HashMap<i64, String>,
    product_units: &std::collections::HashMap<i64, String>,
) -> Markup {
    let (status_label, status_class) = match transfer.status {
        TransferStatus::Draft => ("草稿", "status-draft"),
        TransferStatus::InTransit => ("在途", "status-progress"),
        TransferStatus::Completed => ("已完成", "status-completed"),
        TransferStatus::Cancelled => ("已取消", "status-cancelled"),
    };

    html! {
        div {
            a href="/admin/wms/transfers" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回库存调拨列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (transfer.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="page-actions" {
                    (transfer_action_buttons(transfer.status, detail_path))
                }
            }

            // ── Workflow Steps ──
            (transfer_workflow_steps(transfer.status))

            // ── Info Card ──
            div class="info-card" {
                div class="info-card-title" { "调拨信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "调拨单号" }
                        span class="info-value mono" { (transfer.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "调出仓库" }
                        span class="info-value" { (from_wh_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "调入仓库" }
                        span class="info-value" { (to_wh_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "调拨日期" }
                        span class="info-value mono" { (transfer.transfer_date.to_string()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                }
            }

            // ── Items Table ──
            div class="data-card" {
                div style="padding:var(--space-5) var(--space-6) var(--space-3)" {
                    div class="info-card-title" style="border-bottom:none;padding-bottom:0;margin-bottom:0" { "调拨明细" }
                }
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th { "规格" }
                            th { "单位" }
                            th class="num-right" { "调拨数量" }
                            th { "批次号" }
                        }
                    }
                    tbody {
                        @for (i, item) in items.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td class="mono" { (product_codes.get(&item.product_id).map(|c| c.as_str()).unwrap_or("—")) }
                                td { (product_names.get(&item.product_id).map(|n| n.as_str()).unwrap_or("—")) }
                                td { (product_specs.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—")) }
                                td { (product_units.get(&item.product_id).map(|u| u.as_str()).unwrap_or("—")) }
                                td class="num-right" { (format!("{:.2}", item.quantity)) }
                                td class="mono" {
                                    @if let Some(ref batch) = item.batch_no {
                                        (batch)
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                        @if items.is_empty() {
                            tr {
                                td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无明细数据"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn transfer_action_buttons(status: TransferStatus, detail_path: &str) -> Markup {
    match status {
        TransferStatus::Draft => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此调拨单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"dispatch"}"#
                    hx-confirm="确定要发货吗？"
                    hx-redirect=(detail_path) {
                    (icon::arrow_right_icon("w-4 h-4"))
                    "发货"
                }
            }
        }
        TransferStatus::InTransit => {
            html! {
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"complete"}"#
                    hx-confirm="确定要完成调拨吗？"
                    hx-redirect=(detail_path) {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认完成"
                }
            }
        }
        _ => html! {},
    }
}

fn transfer_workflow_steps(status: TransferStatus) -> Markup {
    let steps = [
        ("草稿", TransferStatus::Draft),
        ("在途", TransferStatus::InTransit),
        ("已完成", TransferStatus::Completed),
    ];

    let current_idx = match status {
        TransferStatus::Draft => 0,
        TransferStatus::InTransit => 1,
        TransferStatus::Completed => 2,
        TransferStatus::Cancelled => 0,
    };

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx { "wf-line completed" } else { "wf-line" }) {}
                }
                div class={
                    @if i < current_idx { "wf-step completed" }
                    @else if i == current_idx { "wf-step current" }
                    @else { "wf-step" }
                } {
                    span class="wf-dot" {}
                    (label)
                }
            }
        }
    }
}
