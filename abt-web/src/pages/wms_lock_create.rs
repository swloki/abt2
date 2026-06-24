use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::wms::inventory_lock::model::CreateLockReq;
use abt_core::wms::inventory_lock::InventoryLockService;

use crate::components::icon;
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

 use abt_core::wms::warehouse::{WarehouseService, model::WarehouseFilter};
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let warehouses = state.warehouse_service()
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = lock_create_form(&warehouses);
 let page_html = admin_page(
 is_htmx,
 "新建锁库",
 &claims,
 "inventory",
 LockListPath::PATH,
 "库存管理",
 Some("新建锁库"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

pub async fn create_lock(
 _path: LockCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<CreateLockForm>,
) -> crate::errors::Result<axum::response::Response> {
 let RequestContext { state, service_ctx, .. } = ctx;
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

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 svc.create(&service_ctx, &mut tx, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let mut resp = axum::response::Response::default();
 resp.headers_mut().insert(
 axum::http::header::LOCATION,
 LockListPath::PATH.parse().unwrap(),
 );
 resp.headers_mut().insert(
 "HX-Redirect",
 LockListPath::PATH.parse().unwrap(),
 );
Ok(resp)
}

fn lock_create_form(warehouses: &[abt_core::wms::warehouse::model::Warehouse]) -> Markup {
 html! {
    div {
        // ── Back Link ──
        a   href=(format!("{}?restore=true", LockListPath::PATH))
            class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
        { (icon::chevron_left_icon("w-4 h-4")) "返回库存锁定列表" }
        // ── Page Header ──
        div class="flex items-center justify-between mb-5" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建锁库" }
        }
        form
            method="POST"
            action=(LockCreatePath::PATH)
            hx-post=(LockCreatePath::PATH)
            hx-redirect=(LockListPath::PATH)
        {
            // ── 锁库信息 ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::lock_icon("w-[18px] h-[18px]")) "锁库信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "产品ID "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="number"
                            step="any"
                            name="product_id"
                            required
                            placeholder="输入产品ID";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "仓库 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="warehouse_id"
                            required
                        {
                            option value="" { "请选择仓库" }
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "锁定数量 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="number"
                            step="any"
                            name="locked_qty"
                            required
                            placeholder="输入数量";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "锁定原因 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="lock_reason"
                            required
                        {
                            option value="客户预留" { "客户预留" }
                            option value="质量问题" { "质量问题" }
                            option value="安全库存" { "安全库存" }
                            option value="其他" { "其他" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "关联客户ID"
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="number"
                            step="any"
                            name="customer_id"
                            placeholder="可选";
                    }
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                div {}
                div class="flex gap-3" {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", LockListPath::PATH))
                    { "取消" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { "确认锁定" }
                }
            }
        }
    }
}
}
