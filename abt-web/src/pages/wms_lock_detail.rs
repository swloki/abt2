use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::inventory_lock::InventoryLockService;
use abt_core::wms::enums::LockStatus;

use crate::layout::page::admin_page;
use crate::routes::wms_inventory_lock::{LockDetailPath, LockListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct LockActionForm {
    pub action: String,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_lock_detail(
    path: crate::routes::wms_inventory_lock::LockDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inventory_lock_service();

    let lock = svc.get(&service_ctx, &mut conn, path.id).await?;

    let content = lock_detail_page(&lock);
    let page_html = admin_page(
        is_htmx,
        &format!("{} · 锁库详情", lock.doc_number),
        &claims,
        "inventory",
        LockListPath::PATH,
        "库存管理",
        Some("锁库详情"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn post_lock_action(
    path: crate::routes::wms_inventory_lock::LockDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<LockActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_lock_service();

    match form.action.as_str() {
        "release" => svc.release(&service_ctx, &mut conn, path.id).await?,
        "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
        _ => {}
    }

    let redirect_url = LockDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        redirect_url.parse().unwrap(),
    );
    *resp.status_mut() = axum::http::StatusCode::SEE_OTHER;

    Ok(resp)
}

// ── Helpers ──

fn status_label(s: &LockStatus) -> &'static str {
    match s {
        LockStatus::Active => "生效",
        LockStatus::Released => "已释放",
        LockStatus::Cancelled => "已作废",
    }
}

fn status_class(s: &LockStatus) -> &'static str {
    match s {
        LockStatus::Active => "status-progress",
        LockStatus::Released => "status-completed",
        LockStatus::Cancelled => "status-cancelled",
    }
}

// ── Components ──

fn lock_detail_page(lock: &abt_core::wms::inventory_lock::model::InventoryLock) -> Markup {
    let sl = status_label(&lock.status);
    let sc = status_class(&lock.status);
    let detail_path = LockDetailPath { id: lock.id }.to_string();
    let is_active = matches!(lock.status, LockStatus::Active);

    html! {
        div {
            a class="back-link" href=(LockListPath::PATH) {
                (crate::components::icon::chevron_left_icon("w-4 h-4"))
                "返回库存锁定列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        span class="detail-no mono" { (lock.doc_number) }
                        span class=(format!("status-pill {sc}")) { (sl) }
                    }
                }
                @if is_active {
                    div class="page-actions" {
                        button class="btn btn-default"
                            hx-post=(detail_path)
                            hx-vals=r#"{"action":"release"}"#
                            hx-confirm="确定要释放此锁定吗？释放后库存将恢复可用。"
                            hx-redirect=(detail_path) {
                            (crate::components::icon::lock_icon("w-4 h-4"))
                            "释放锁定"
                        }
                        button class="btn btn-danger"
                            hx-post=(detail_path)
                            hx-vals=r#"{"action":"cancel"}"#
                            hx-confirm="确定要作废此锁库单吗？此操作不可撤销。"
                            hx-redirect=(detail_path) {
                            (crate::components::icon::x_icon("w-4 h-4"))
                            "作废"
                        }
                    }
                }
            }

            div class="info-card" {
                div class="info-card-title" { "锁库信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "锁库单号" }
                        span class="info-value mono" { (lock.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "产品编码" }
                        span class="info-value mono" { "产品#" (lock.product_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "产品名称" }
                        span class="info-value" {
                            span style="color:var(--muted)" { "—" }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "锁定仓库" }
                        span class="info-value" { "仓库#" (lock.warehouse_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "锁定数量" }
                        span class="info-value mono" { (lock.locked_qty) }
                    }
                    div class="info-item" {
                        span class="info-label" { "锁定原因" }
                        span class="info-value" { (lock.lock_reason) }
                    }
                    div class="info-item" {
                        span class="info-label" { "关联客户" }
                        span class="info-value" {
                            @if let Some(cid) = lock.customer_id {
                                "客户#" (cid)
                            } @else {
                                span style="color:var(--muted)" { "—" }
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { "操作员#" (lock.operator_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" {
                            (lock.created_at.format("%Y-%m-%d %H:%M"))
                        }
                    }
                }
            }
        }
    }
}
