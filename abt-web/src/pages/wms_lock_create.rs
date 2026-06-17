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
    #[serde(deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
    #[serde(deserialize_with = "crate::utils::empty_as_none")]
    pub warehouse_id: Option<i64>,
    pub locked_qty: String,
    pub lock_reason: String,
    #[serde(deserialize_with = "crate::utils::empty_as_none")]
    pub customer_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_lock_create(
    _path: LockCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
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
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "create")]
pub async fn create_lock(
    _path: LockCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateLockForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_lock_service();

    let product_id = form.product_id.ok_or_else(|| crate::errors::WebError::from(abt_core::shared::types::DomainError::validation("请选择产品")))?;
    let warehouse_id = form.warehouse_id.ok_or_else(|| crate::errors::WebError::from(abt_core::shared::types::DomainError::validation("请选择仓库")))?;

    let locked_qty: Decimal = form.locked_qty.parse()
        .map_err(|e| crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(format!("无效数量: {e}"))))?;

    let req = CreateLockReq {
        product_id,
        warehouse_id,
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
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            form method="POST" action=(LockCreatePath::PATH)
                hx-post=(LockCreatePath::PATH)
                hx-redirect=(LockListPath::PATH) {

                div class="bg-bg border border-border rounded p-6" {
                    div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品ID" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="product_id" required placeholder="输入产品ID";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "仓库" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "锁定数量" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="locked_qty" step="0.01" required placeholder="输入数量";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "锁定原因" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="lock_reason" required {
                                option value="客户预留" { "客户预留" }
                                option value="质量问题" { "质量问题" }
                                option value="安全库存" { "安全库存" }
                                option value="其他" { "其他" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联客户ID（可选）" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="customer_id" placeholder="可选";
                        }
                    }
                }

                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", LockListPath::PATH)) { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                        "确认锁定"
                    }
                }
            }
        }
    }
}
