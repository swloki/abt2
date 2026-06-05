use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::wms::inventory_lock::model::CreateLockReq;
use abt_core::wms::inventory_lock::InventoryLockService;

use crate::layout::page::admin_page;
use crate::routes::wms_inventory_lock::{LockCreatePath, LockListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct CreateLockForm {
    pub product_id: i64,
    pub warehouse_id: i64,
    pub locked_qty: String,
    pub lock_reason: String,
    pub customer_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_lock_create(
    _path: LockCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;

    let content = lock_create_form();
    let page_html = admin_page(
        is_htmx,
        "新建锁库",
        &claims,
        "inventory",
        LockListPath::PATH,
        "库存管理",
        Some("新建锁库"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_lock(
    _path: LockCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateLockForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_lock_service();

    let locked_qty: Decimal = form.locked_qty.parse()
        .map_err(|e| crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(format!("无效数量: {e}"))))?;

    let req = CreateLockReq {
        product_id: form.product_id,
        warehouse_id: form.warehouse_id,
        locked_qty,
        lock_reason: form.lock_reason,
        customer_id: form.customer_id,
    };

    svc.create(&service_ctx, &mut conn, req).await?;

    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        LockListPath::PATH.parse().unwrap(),
    );
    resp.headers_mut().insert(
        "HX-Redirect",
        LockListPath::PATH.parse().unwrap(),
    );
    *resp.status_mut() = axum::http::StatusCode::SEE_OTHER;

    Ok(resp)
}

// ── Components ──

fn lock_create_form() -> Markup {
    html! {
        div class="data-card" {
            form method="POST" action=(LockCreatePath::PATH)
                hx-post=(LockCreatePath::PATH)
                hx-redirect=(LockListPath::PATH) {

                div class="wms-form-section" {
                    div class="wms-form-grid" {
                        div class="form-field" {
                            label class="form-label" { "产品ID" }
                            input class="form-input" type="number" name="product_id" required placeholder="输入产品ID";
                        }
                        div class="form-field" {
                            label class="form-label" { "仓库" }
                            select class="form-select" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "锁定数量" }
                            input class="form-input" type="number" name="locked_qty" step="0.01" required placeholder="输入数量";
                        }
                        div class="form-field" {
                            label class="form-label" { "锁定原因" }
                            select class="form-select" name="lock_reason" required {
                                option value="客户预留" { "客户预留" }
                                option value="质量问题" { "质量问题" }
                                option value="安全库存" { "安全库存" }
                                option value="其他" { "其他" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "关联客户ID（可选）" }
                            input class="form-input" type="number" name="customer_id" placeholder="可选";
                        }
                    }
                }

                div class="create-action-bar" {
                    a class="btn btn-default" href=(LockListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        "确认锁定"
                    }
                }
            }
        }
    }
}
