//! 库存预留明细控件（纯 HTMX、自给自足、只依赖自身 URL）
//!
//! - **一个 URL = 一个组件**：`/api/reservation-detail?product_id=X` 渲染 drawer
//!   内容片段（汇总卡 + 占用明细列表）。宿主页面 SSR 调用
//!   `reservation_detail_drawer()` 放一个共享 drawer，`reservation_detail_badge()`
//!   在缺口表产品名旁渲染可点击徽标；点击徽标 → hx-get 自身 URL → 填充 drawer body
//!   → hyperscript 打开 drawer。
//! - 数据：实物 / 已预留 / 可用量来自 `StockLedgerService::query_projected_qty`；
//!   占用明细（哪些订单预留、量、状态、时间）来自
//!   `InventoryReservationService::list_active_by_product`。
//! - 用途：解释销售订单详情页「明明有库存却显示缺口」——库存被其他订单按
//!   先到先得预留占用，让占用方一目了然。

use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::shared::enums::DocumentType;
use abt_core::shared::inventory_reservation::{InventoryReservationService, ReservationDetail};
use abt_core::wms::stock_ledger::StockLedgerService;

use abt_macros::require_permission;
use crate::components::{drawer, icon};
use crate::errors::Result;
use crate::utils::{RequestContext, fmt_qty};

// ── 自身端点（组件唯一依赖的 URL）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/reservation-detail")]
pub struct ReservationDetailPath;

#[derive(Debug, Deserialize)]
pub struct ReservationDetailParams {
    pub product_id: i64,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(ReservationDetailPath::PATH, get(get_reservation_detail))
}

/// HTMX: 渲染某产品的预留明细片段（汇总卡 + 占用明细），填入共享 drawer body。
#[require_permission("SALES_ORDER", "read")]
pub async fn get_reservation_detail(
    ctx: RequestContext,
    Query(p): Query<ReservationDetailParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let stock_svc = state.stock_ledger_service();
    let resv_svc = state.inventory_reservation_service();

    // 占用明细（JOIN 销售订单 + 客户）
    let details = resv_svc
        .list_active_by_product(&service_ctx, &mut conn, p.product_id, None)
        .await
        .unwrap_or_default();

    // 现货分解：实物 / 已预留(ir Active) 来自 projected_qty
    let proj = stock_svc
        .query_projected_qty(&service_ctx, &mut conn, p.product_id, None)
        .await
        .ok();
    let actual = proj.as_ref().map(|q| q.actual).unwrap_or(Decimal::ZERO);
    let reserved = proj.as_ref().map(|q| q.reserved).unwrap_or(Decimal::ZERO);
    let on_order_po = proj.as_ref().map(|q| q.on_order_po).unwrap_or(Decimal::ZERO);
    let in_progress_wo = proj.as_ref().map(|q| q.in_progress_wo).unwrap_or(Decimal::ZERO);
    let projected = proj.as_ref().map(|q| q.projected).unwrap_or(Decimal::ZERO);
    // 可用量用 ATP（与详情页缺口表同口径：现货 − 预留，不含在途/在制）
    let available = stock_svc
        .query_available(&service_ctx, &mut conn, p.product_id, None)
        .await
        .unwrap_or(Decimal::ZERO);

    Ok(Html(
        render_detail_body(actual, reserved, available, on_order_po, in_progress_wo, projected, &details).into_string(),
    ))
}

// ── SSR 入口（宿主页面调用）──

/// 共享 drawer 容器：宿主页面（销售订单详情页）底部渲染一次。
/// 缺口表里的徽标通过 hx-get 把内容填进 `#reservation-detail-body` 并打开本 drawer。
pub fn reservation_detail_drawer() -> Markup {
    drawer::drawer_with_footer(
        "reservation-detail-drawer",
        "库存预留明细",
        html! { div id="reservation-detail-body" {} },
        html! {
            button type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                _="on click remove .open from closest .drawer-overlay"
            { "关闭" }
        },
    )
}

/// 产品名旁的「被占用」徽标：仅当该产品存在 Active 预留时由宿主渲染。
/// 点击 → hx-get 自身 URL → 填充 drawer body → 打开 drawer。
pub fn reservation_detail_badge(product_id: i64) -> Markup {
    let url = format!("{}?product_id={}", ReservationDetailPath::PATH, product_id);
    html! {
        span class="inline-flex items-center gap-1 ml-1.5 align-middle cursor-pointer text-warn hover:text-danger transition-colors duration-150"
            title="库存被其他订单预留，点击查看占用明细"
            hx-get=(url.as_str())
            hx-target="#reservation-detail-body"
            hx-swap="innerHTML"
            _="on 'htmx:afterRequest' add .open to #reservation-detail-drawer"
        {
            (icon::lock_icon("w-3.5 h-3.5"))
            span class="text-[11px] font-medium" { "被占用" }
        }
    }
}

// ── drawer body 渲染（handler 调用）──

fn render_detail_body(
    actual: Decimal,
    reserved: Decimal,
    available: Decimal,
    on_order_po: Decimal,
    in_progress_wo: Decimal,
    projected: Decimal,
    details: &[ReservationDetail],
) -> Markup {
    let avail_color = if available <= Decimal::ZERO {
        "text-danger"
    } else {
        "text-success"
    };
    html! {
        div class="grid grid-cols-3 gap-2 mb-3" {
            (summary_card("实物库存", actual, "text-fg"))
            (summary_card("已被预留", reserved, "text-warn"))
            (summary_card("可用量", available, avail_color))
            (summary_card("在途采购", on_order_po, "text-fg-2"))
            (summary_card("在制工单", in_progress_wo, "text-fg-2"))
            (summary_card("预计可用", projected, "text-accent"))
        }
        div class="text-xs text-muted mb-4 text-center leading-relaxed" {
            "可用量 = 现货 − 预留（与缺口表同口径）；预计可用 = 可用量 + 在途采购 + 在制工单"
        }
        @if details.is_empty() {
            div class="text-center text-sm text-muted py-8" { "该产品暂无 Active 预留记录" }
        } @else {
            div class="text-xs text-muted mb-2" { "占用明细（按时间倒序）" }
            div class="flex flex-col" {
                @for d in details {
                    (reservation_item(d))
                }
            }
        }
    }
}

fn summary_card(label: &str, value: Decimal, color_class: &str) -> Markup {
    html! {
        div class="bg-surface-raised border border-border-soft rounded-md py-2 px-2 text-center" {
            div class="text-[11px] text-muted mb-1" { (label) }
            div class=(format!("text-base font-bold font-mono tabular-nums {}", color_class)) {
                (fmt_qty(value))
            }
        }
    }
}

fn reservation_item(d: &ReservationDetail) -> Markup {
    let doc = d.source_doc_number.as_deref().unwrap_or("—");
    let customer = d.customer_name.as_deref().unwrap_or("—");
    let st_label = source_status_label(&d.source_type, d.source_status);
    let time = d.created_at.format("%Y-%m-%d %H:%M").to_string();
    let is_so = matches!(d.source_type, DocumentType::SalesOrder);
    let order_href = if is_so {
        format!("/admin/orders/{}", d.source_id)
    } else {
        String::new()
    };
    html! {
        div class="py-2.5 border-b border-border-soft last:border-b-0" {
            div class="flex items-center justify-between mb-1" {
                @if order_href.is_empty() {
                    span class="text-sm font-medium text-fg font-mono" { (doc) }
                } @else {
                    a class="text-sm font-medium text-accent font-mono hover:underline" href=(order_href.as_str()) { (doc) }
                }
                span class="text-sm font-semibold text-warn font-mono tabular-nums" {
                    (fmt_qty(d.reserved_qty))
                }
            }
            div class="flex items-center justify-between text-xs text-muted" {
                span class="truncate mr-2" { "客户：" (customer) }
                span class="shrink-0" { (st_label) " · " (time) }
            }
        }
    }
}

/// 来源单据状态文案（当前仅销售订单；其它来源显示 —）
fn source_status_label(source_type: &DocumentType, source_status: Option<i16>) -> &'static str {
    let Some(s) = source_status else {
        return "—";
    };
    if !matches!(source_type, DocumentType::SalesOrder) {
        return "—";
    }
    match s {
        1 => "草稿",
        2 => "已确认",
        4 => "部分发货",
        5 => "已发货",
        6 => "已完成",
        7 => "已取消",
        _ => "—",
    }
}
