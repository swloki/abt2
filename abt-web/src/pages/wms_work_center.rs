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

/// 仓库作业中心 — 仓库岗的待办看板（聚合 7 个业务环节的待处理单据 + 紧急/临期提醒）。
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

        // ── 待办卡片网格（蓝=收货/拣货/调拨，橙=质检/发货，绿=领料/盘点）──
        div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-5" {
            (todo_card("/admin/wms/arrivals", &icon::truck_icon("w-5 h-5"), "bg-accent-bg text-accent", "text-accent", summary.arrivals_pending, "待收货", "PO 到货待接收，核对送货单"))
            (todo_card("/admin/wms/arrivals", &icon::search_icon("w-5 h-5"), "bg-warn-bg text-warn", "text-warn", summary.inspections_pending, "待质检", "来料待检验，录入质检结果"))
            (todo_card("/admin/wms/shipping", &icon::package_icon("w-5 h-5"), "bg-accent-bg text-accent", "text-accent", summary.picks_pending, "待拣货", "发货单待拣货，到库位拣选"))
            (todo_card("/admin/wms/shipping", &icon::upload_icon("w-5 h-5"), "bg-warn-bg text-warn", "text-warn", summary.outbounds_pending, "待发货", "已拣货待发出，安排物流"))
            (todo_card("/admin/wms/requisitions", &icon::clipboard_list_icon("w-5 h-5"), "bg-success-bg text-success", "text-success", summary.requisitions_pending, "待领料", "生产领料待出库"))
            (todo_card("/admin/wms/transfers", &icon::arrow_right_icon("w-5 h-5"), "bg-accent-bg text-accent", "text-accent", summary.transfers_pending, "待调拨", "库间调拨在途，确认收货"))
            (todo_card("/admin/wms/cycle-counts", &icon::check_circle_icon("w-5 h-5"), "bg-success-bg text-success", "text-success", summary.cycle_counts_pending, "待盘点", "库存盘点任务"))
        }

        // ── 紧急 / 临期提醒（示意数据；后端到期日查询待接入）──
        div class="mt-10" {
            div class="flex items-center gap-2 mb-5" {
                (icon::circle_alert_icon("w-5 h-5 text-danger"))
                h2 class="text-lg font-bold text-fg" { "紧急 / 临期提醒" }
            }
            div class="grid grid-cols-1 md:grid-cols-3 gap-4" {
                (urgent_card("danger", "/admin/wms/arrivals", &icon::circle_alert_icon("w-4 h-4"), "AN-2026-0031 来料通知超期未收货", "逾期 3 天", "2026-06-21"))
                (urgent_card("warn", "/admin/wms/shipping", &icon::bell_icon("w-4 h-4"), "SR-2026-0076 发货单即将超期", "临期", "剩余 1 天"))
                (urgent_card("warn", "/admin/wms/shipping", &icon::bell_icon("w-4 h-4"), "PK-2026-0038 拣货单拣货超时", "超时", "已 2 小时"))
            }
            p class="text-xs text-muted mt-3" { "* 示意数据，后端到期日 / 超时阈值计算待接入" }
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

/// 待办分区卡片：图标（分组配色）+ 数量徽章（>0 红色脉冲）/ =0 muted「当前无待办」
/// icon_class / count_class 必须传字面量（UnoCSS 扫描器不识别 format! 拼接）
fn todo_card(
    path: &str,
    icon_markup: &Markup,
    icon_class: &str,
    count_class: &str,
    count: u64,
    title: &str,
    desc: &str,
) -> Markup {
    let has_todo = count > 0;
    let card_class = if has_todo {
        "data-card relative flex flex-col items-center gap-3 p-6 text-center cursor-pointer border-accent/40 bg-accent/5 hover:-translate-y-0.5 hover:shadow-md transition-all"
    } else {
        "data-card relative flex flex-col items-center gap-3 p-6 text-center cursor-pointer opacity-60 hover:opacity-100 transition-all"
    };
    html! {
        a href=(path) class=(card_class) {
            div class="relative" {
                div class=(format!("w-11 h-11 rounded-md grid place-items-center {icon_class}")) { (icon_markup) }
                @if has_todo {
                    span class="absolute -top-1.5 -right-1.5 min-w-[22px] h-[22px] px-1 rounded-full bg-danger text-white text-xs font-bold flex items-center justify-center font-mono tabular-nums leading-none animate-pulse" {
                        (count)
                    }
                }
            }
            div class="text-sm font-semibold text-fg" { (title) }
            @if has_todo {
                div class=(format!("text-lg font-bold font-mono tabular-nums leading-tight {count_class}")) {
                    (count) " 笔待处理"
                }
            } @else {
                div class="text-sm font-medium text-muted" { "当前无待办" }
            }
            div class="text-xs text-muted leading-relaxed" { (desc) }
        }
    }
}

/// 紧急 / 临期卡片：左侧色条 + 图标 + 标签 + 时间
/// level: "danger"(红) / "warn"(橙)，class 在 match 臂用字面量（UnoCSS 安全）
fn urgent_card(
    level: &str,
    path: &str,
    icon_markup: &Markup,
    label: &str,
    tag_text: &str,
    time_text: &str,
) -> Markup {
    // match 臂字面量 — UnoCSS 扫描器能识别（区别于 format! 拼接）
    let (border_cls, color_cls) = match level {
        "danger" => ("border-danger", "bg-danger-bg text-danger"),
        _ => ("border-warn", "bg-warn-bg text-warn"),
    };
    html! {
        a href=(path)
            class=(format!("data-card flex items-center gap-3 p-4 cursor-pointer border-l-4 {border_cls} hover:-translate-y-0.5 hover:shadow-md transition-all")) {
            div class=(format!("w-9 h-9 rounded-md grid place-items-center flex-shrink-0 {color_cls}")) { (icon_markup) }
            div class="flex-1 min-w-0" {
                div class="text-sm font-semibold text-fg truncate" { (label) }
                div class="flex items-center gap-3 mt-1" {
                    span class=(format!("px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide {color_cls}")) { (tag_text) }
                    span class="text-xs text-muted font-mono tabular-nums" { (time_text) }
                }
            }
        }
    }
}
