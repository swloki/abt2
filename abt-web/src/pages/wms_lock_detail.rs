use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::inventory_lock::InventoryLockService;
use abt_core::wms::enums::LockStatus;
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;
use abt_core::master_data::customer::CustomerService;

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

#[require_permission("INVENTORY", "read")]
pub async fn get_lock_detail(
    path: crate::routes::wms_inventory_lock::LockDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inventory_lock_service();

    let lock = svc.get(&service_ctx, &mut conn, path.id).await?;

    // resolve IDs
    let product_svc = state.product_service();
    let (product_code, product_name_val) = product_svc
        .get(&service_ctx, &mut conn, lock.product_id)
        .await
        .map(|p| (p.product_code, p.pdt_name))
        .unwrap_or_else(|_| (format!("产品#{}", lock.product_id), "—".into()));

    let wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, lock.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| format!("仓库#{}", lock.warehouse_id));

    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, lock.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| format!("操作员#{}", lock.operator_id));

    let customer_name = if let Some(cid) = lock.customer_id {
        state.customer_service()
            .get(&service_ctx, &mut conn, cid)
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| format!("客户#{}", cid))
    } else {
        "—".into()
    };

    let locked_qty_fmt = format!("{:.2}", lock.locked_qty);

    let content = lock_detail_page(&lock, &product_code, &product_name_val, &wh_name, &operator_name, &customer_name, &locked_qty_fmt);
    let page_html = admin_page(
        is_htmx,
        &format!("{} · 锁库详情", lock.doc_number),
        &claims,
        "inventory",
        LockListPath::PATH,
        "库存管理",
        Some("锁库详情"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
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
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

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

fn lock_detail_page(
    lock: &abt_core::wms::inventory_lock::model::InventoryLock,
    product_code: &str,
    product_name_val: &str,
    wh_name: &str,
    operator_name: &str,
    customer_name: &str,
    locked_qty_fmt: &str,
) -> Markup {
    let sl = status_label(&lock.status);
    let sc = status_class(&lock.status);
    let detail_path = LockDetailPath { id: lock.id }.to_string();
    let is_active = matches!(lock.status, LockStatus::Active);

    html! {
        div {
            a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", LockListPath::PATH)) {
                (crate::components::icon::chevron_left_icon("w-4 h-4"))
                "返回库存锁定列表"
            }

            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        span class="text-2xl font-extrabold font-mono tabular-nums" { (lock.doc_number) }
                        span class=(format!("status-pill {sc}")) { (sl) }
                    }
                }
                @if is_active {
                    div class="flex gap-3" {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            hx-post=(detail_path)
                            hx-vals=r#"{"action":"release"}"#
                            hx-confirm="确定要释放此锁定吗？释放后库存将恢复可用。"
                            hx-redirect=(detail_path) {
                            (crate::components::icon::lock_icon("w-4 h-4"))
                            "释放锁定"
                        }
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
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

            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "锁库信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "锁库单号" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (lock.doc_number) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "产品编码" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (product_code) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "产品名称" }
                        span class="text-sm text-fg font-medium" { (product_name_val) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "锁定仓库" }
                        span class="text-sm text-fg font-medium" { (wh_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "锁定数量" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (locked_qty_fmt) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "锁定原因" }
                        span class="text-sm text-fg font-medium" { (lock.lock_reason) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "关联客户" }
                        span class="text-sm text-fg font-medium" { (customer_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "操作员" }
                        span class="text-sm text-fg font-medium" { (operator_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "创建时间" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" {
                            (lock.created_at.format("%Y-%m-%d %H:%M"))
                        }
                    }
                }
            }
        }
    }
}
