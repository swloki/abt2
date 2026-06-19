use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::settings::{model::UpdateWmsSettingsReq, WmsSettingsService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_settings::WmsSettingsPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub cycle_count_variance_threshold: Option<String>,
}

#[require_permission("INVENTORY", "read")]
pub async fn get_wms_settings(
    _path: WmsSettingsPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.wms_settings_service();
    let settings = svc.get(&service_ctx, &mut conn).await?;

    let content = settings_page(&settings.cycle_count_variance_threshold);
    let page_html = admin_page(
        is_htmx,
        "WMS 参数配置",
        &claims,
        "inventory",
        WmsSettingsPath::PATH,
        "库存管理",
        Some("参数配置"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn update_wms_settings(
    _path: WmsSettingsPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SettingsForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.wms_settings_service();

    let threshold = form
        .cycle_count_variance_threshold
        .and_then(|s| s.parse().ok())
        .unwrap_or(rust_decimal::Decimal::ZERO);

    svc.update(&service_ctx, &mut conn, UpdateWmsSettingsReq {
        cycle_count_variance_threshold: threshold,
    })
    .await?;

    let redirect = WmsSettingsPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn settings_page(threshold: &rust_decimal::Decimal) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "WMS 参数配置" }
            }
            form hx-post=(WmsSettingsPath::PATH) hx-swap="none" {
                // ── 盘点差异审批 ──
                div class="data-card" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        "盘点差异审批"
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "盘点差异金额阈值"
                        }
                        input type="number" step="0.01" min="0"
                            name="cycle_count_variance_threshold"
                            value=(threshold)
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]";
                        span class="text-muted" {
                            "盘点差异金额（Σ |差异量| × 单价）超过此值时进入"
                            span class="font-medium text-fg-2" { " 待审批 " }
                            "状态，需人工通过后才调账；未超则直接调账。设为 0 表示任何差异都需审批。"
                        }
                    }
                }

                // ── Actions ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href="/admin/wms/cycle-counts" { "返回盘点列表" }
                    button type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                        "保存配置"
                    }
                }
            }
        }
    }
}
