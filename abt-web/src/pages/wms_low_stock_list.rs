use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::shared::types::pagination::PaginatedResult;
use abt_core::wms::enums::LowStockAlertStatus;
use abt_core::wms::low_stock_alert::{model::LowStockAlertFilter, LowStockAlert};
use abt_core::wms::low_stock_alert::service::LowStockAlertService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::master_data::product::ProductService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_low_stock::{LowStockAckPath, LowStockListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("INVENTORY", "read")]
pub async fn get_low_stock_list(ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let alerts: PaginatedResult<LowStockAlert> = state
        .low_stock_alert_service()
        .list(&service_ctx, &mut conn, LowStockAlertFilter::default(), 1, 200)
        .await?;

    // 解析产品名 / 仓名
    let mut product_names: HashMap<i64, String> = HashMap::new();
    let mut wh_names: HashMap<i64, String> = HashMap::new();
    let psvc = state.product_service();
    let wsvc = state.warehouse_service();
    for a in &alerts.items {
        if !product_names.contains_key(&a.product_id)
            && let Ok(p) = psvc.get(&service_ctx, &mut conn, a.product_id).await
        {
            product_names.insert(a.product_id, p.pdt_name);
        }
        if !wh_names.contains_key(&a.warehouse_id)
            && let Ok(w) = wsvc.get(&service_ctx, &mut conn, a.warehouse_id).await
        {
            wh_names.insert(a.warehouse_id, w.name);
        }
    }

    let active_count = alerts
        .items
        .iter()
        .filter(|a| matches!(a.status, LowStockAlertStatus::Active))
        .count();

    let content = low_stock_page(&alerts.items, &product_names, &wh_names, active_count);
    let page_html = admin_page(
        is_htmx,
        "低库存预警",
        &claims,
        "inventory",
        LowStockListPath::PATH,
        "库存管理",
        Some("低库存预警"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn ack_alert(
    path: LowStockAckPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state
        .low_stock_alert_service()
        .ack(&service_ctx, &mut conn, path.id)
        .await?;

    let redirect = LowStockListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn status_label(s: &LowStockAlertStatus) -> &'static str {
    match s {
        LowStockAlertStatus::Active => "待处理",
        LowStockAlertStatus::Acknowledged => "已确认",
    }
}

fn low_stock_page(
    alerts: &[LowStockAlert],
    product_names: &HashMap<i64, String>,
    wh_names: &HashMap<i64, String>,
    active_count: usize,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "低库存预警" }
                @if active_count > 0 {
                    span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold text-danger bg-danger-bg" {
                        (active_count) " 项待处理"
                    }
                }
            }

            div class="data-card" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "产品" }
                            th { "仓库" }
                            th class="text-right text-[13px]" { "当前库存" }
                            th class="text-right text-[13px]" { "安全库存" }
                            th { "状态" }
                            th { "触发时间" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for a in alerts {
                            tr {
                                td { (product_names.get(&a.product_id).cloned().unwrap_or_else(|| format!("产品#{}", a.product_id))) }
                                td { (wh_names.get(&a.warehouse_id).cloned().unwrap_or_else(|| format!("仓库#{}", a.warehouse_id))) }
                                td class="text-right text-[13px] font-mono tabular-nums text-danger font-semibold" { (format!("{:.2}", a.current_qty)) }
                                td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", a.safety_stock)) }
                                td {
                                    @if matches!(a.status, LowStockAlertStatus::Active) {
                                        span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-danger-bg text-danger" { (status_label(&a.status)) }
                                    } @else {
                                        span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-success-bg text-success" { (status_label(&a.status)) }
                                    }
                                }
                                td class="text-muted text-[13px]" { (a.created_at.format("%Y-%m-%d %H:%M").to_string()) }
                                td {
                                    @if matches!(a.status, LowStockAlertStatus::Active) {
                                        button class="inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-xs font-medium cursor-pointer transition-all duration-150"
                                            hx-post=(LowStockAckPath { id: a.id }.to_string())
                                            hx-confirm="确认此低库存预警已处理？"
                                            hx-redirect=(LowStockListPath::PATH) {
                                            "确认"
                                        }
                                    } @else {
                                        span class="text-muted text-[13px]" { "—" }
                                    }
                                }
                            }
                        }
                        @if alerts.is_empty() {
                            tr {
                                td colspan="7" class="text-center text-muted py-10" {
                                    "暂无低库存预警"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
