use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::work_center::WorkCenterService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

/// 仓库作业中心 — 仓库岗的待办看板（聚合 7 个业务环节的待处理单据）。
#[require_permission("INVENTORY", "read")]
pub async fn get_wms_work_center(_path: WmsWorkCenterPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let summary = state
        .wms_work_center_service()
        .summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let total = summary.total();
    let content = html! {
        // ── 页头 ──
        div class="flex items-center justify-between mb-6 flex-wrap gap-4" {
            div {
                h1 class="text-xl font-bold text-fg tracking-tight" { "仓库作业中心" }
                p class="text-sm text-muted mt-1" { "查看和管理所有待处理的仓库作业任务" }
            }
            div class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-accent-bg text-accent text-sm font-semibold border border-accent/20" {
                (icon::check_circle_icon("w-4 h-4"))
                "您有 "
                strong class="font-mono tabular-nums" { (total) }
                " 项待办"
            }
        }

        // ── 待办卡片网格（按业务流向排列）──
        div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-5 mb-8" {
            (todo_card("/admin/wms/arrivals", &icon::truck_icon("w-5 h-5"), "bg-accent-bg text-accent", summary.arrivals_pending, "待收货", "PO 到货待接收"))
            (todo_card("/admin/wms/arrivals", &icon::search_icon("w-5 h-5"), "bg-warn-bg text-warn", summary.inspections_pending, "待质检", "来料待检验"))
            (todo_card("/admin/wms/shipping", &icon::package_icon("w-5 h-5"), "bg-accent-bg text-accent", summary.picks_pending, "待拣货", "发货单待拣货"))
            (todo_card("/admin/wms/shipping", &icon::upload_icon("w-5 h-5"), "bg-warn-bg text-warn", summary.outbounds_pending, "待发货", "已拣货待发出"))
            (todo_card("/admin/wms/requisitions", &icon::clipboard_list_icon("w-5 h-5"), "bg-success-bg text-success", summary.requisitions_pending, "待领料", "生产领料待出库"))
            (todo_card("/admin/wms/transfers", &icon::arrow_right_icon("w-5 h-5"), "bg-accent-bg text-accent", summary.transfers_pending, "待调拨", "库间调拨在途"))
            (todo_card("/admin/wms/cycle-counts", &icon::check_circle_icon("w-5 h-5"), "bg-success-bg text-success", summary.cycle_counts_pending, "待盘点", "库存盘点任务"))
        }
    };

    let page_html = admin_page(
        is_htmx,
        "仓库作业中心",
        &claims,
        "inventory",
        WmsWorkCenterPath::PATH,
        "库存管理",
        Some("仓库作业中心"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// 待办分区卡片：图标 + 数量徽章（>0 红色脉冲）/ =0 muted「当前无待办」
fn todo_card(path: &str, icon_markup: &Markup, color_class: &str, count: u64, title: &str, desc: &str) -> Markup {
    let has_todo = count > 0;
    let card_class = if has_todo {
        "data-card flex flex-col items-center gap-3 p-6 text-center cursor-pointer hover:border-accent hover:-translate-y-0.5 hover:shadow-md transition-all"
    } else {
        "data-card flex flex-col items-center gap-3 p-6 text-center cursor-pointer opacity-70 hover:opacity-100 transition-all"
    };
    html! {
        a href=(path) class=(card_class) {
            div class="relative" {
                div class=(format!("w-11 h-11 rounded-md grid place-items-center {color_class}")) {
                    (icon_markup)
                }
                @if has_todo {
                    span class="absolute -top-1.5 -right-1.5 min-w-[22px] h-[22px] px-1 rounded-full bg-danger text-white text-xs font-bold flex items-center justify-center font-mono tabular-nums leading-none" {
                        (count)
                    }
                }
            }
            div class="text-sm font-semibold text-fg" { (title) }
            @if has_todo {
                div class="text-lg font-bold font-mono tabular-nums text-accent leading-tight" {
                    (count) " 笔待处理"
                }
            } @else {
                div class="text-sm font-medium text-muted" { "当前无待办" }
            }
            div class="text-xs text-muted leading-relaxed" { (desc) }
        }
    }
}
