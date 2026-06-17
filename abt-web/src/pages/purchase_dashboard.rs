use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::supplier::model::{SupplierQuery, SupplierStatus};
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::{
    MiscRequestStatus, PaymentStatus, PurchaseOrderStatus, PurchaseQuotationStatus,
    PurchaseReconStatus, PurchaseReturnStatus,
};
use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::quotation::PurchaseQuotationService;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_dashboard::PurchaseDashboardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handler ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_purchase_dashboard(
    _path: PurchaseDashboardPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let pq_svc = state.purchase_quotation_service();
    let po_svc = state.purchase_order_service();
    let pr_svc = state.purchase_return_service();
    let pay_svc = state.payment_request_service();
    let precon_svc = state.purchase_reconciliation_service();
    let misc_svc = state.misc_request_service();
    let supplier_svc = state.supplier_service();
    let page = PageParams::new(1, 1);
    let big_page = PageParams::new(1, 200);

    // 1) Active suppliers (Qualified)
    let active_suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                status: Some(SupplierStatus::Qualified),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 2) Pending quotations (Draft — 待比价)
    let pending_quotations = pq_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::quotation::model::PurchaseQuotationQuery {
                status: Some(PurchaseQuotationStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 3) In-progress orders (Draft + Confirmed + PartiallyReceived)
    let draft_orders = po_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::order::model::PurchaseOrderQuery {
                status: Some(PurchaseOrderStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let confirmed_orders = po_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::order::model::PurchaseOrderQuery {
                status: Some(PurchaseOrderStatus::Confirmed),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let partial_orders = po_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::order::model::PurchaseOrderQuery {
                status: Some(PurchaseOrderStatus::PartiallyReceived),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let in_progress_orders = draft_orders + confirmed_orders + partial_orders;

    // 4) Pending payment amount (Draft + Approved)
    let pending_payments = pay_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::payment::model::PaymentRequestQuery {
                status: Some(PaymentStatus::Draft),
                ..Default::default()
            },
            big_page.clone(),
        )
        .await
        .map(|r| {
            r.items
                .iter()
                .map(|p| p.amount)
                .sum::<rust_decimal::Decimal>()
        })
        .unwrap_or(rust_decimal::Decimal::ZERO);

    let approved_payments = pay_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::payment::model::PaymentRequestQuery {
                status: Some(PaymentStatus::Approved),
                ..Default::default()
            },
            big_page.clone(),
        )
        .await
        .map(|r| {
            r.items
                .iter()
                .map(|p| p.amount)
                .sum::<rust_decimal::Decimal>()
        })
        .unwrap_or(rust_decimal::Decimal::ZERO);

    let pending_payment_total = pending_payments + approved_payments;

    // 5) Returns in processing (Draft + Confirmed + Shipped)
    let draft_returns = pr_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::return_order::model::PurchaseReturnQuery {
                status: Some(PurchaseReturnStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let confirmed_returns = pr_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::return_order::model::PurchaseReturnQuery {
                status: Some(PurchaseReturnStatus::Confirmed),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let shipped_returns = pr_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::return_order::model::PurchaseReturnQuery {
                status: Some(PurchaseReturnStatus::Shipped),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);
    let returns_in_progress = draft_returns + confirmed_returns + shipped_returns;
    // 6) Draft reconciliations
    let draft_recons = precon_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::reconciliation::model::PurchaseReconciliationQuery {
                status: Some(PurchaseReconStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 7) Pending misc requests (Draft)
    let pending_misc = misc_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::misc_request::model::MiscRequestQuery {
                status: Some(MiscRequestStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 8) Draft payments count (for todo)
    let draft_payments = pay_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::purchase::payment::model::PaymentRequestQuery {
                status: Some(PaymentStatus::Draft),
                ..Default::default()
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let stats = DashboardStats {
        active_suppliers,
        pending_quotations,
        in_progress_orders,
        pending_payment_total,
        returns_in_progress,
        draft_recons,
        draft_payments,
        pending_misc,
    };

    let content = purchase_dashboard_content(&stats);
    let page_html = admin_page(
        is_htmx,
        "采购总览",
        &claims,
        "purchase",
        PurchaseDashboardPath::PATH,
        "采购管理",
        None,
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Data ──

struct DashboardStats {
    active_suppliers: u64,
    pending_quotations: u64,
    in_progress_orders: u64,
    pending_payment_total: rust_decimal::Decimal,
    returns_in_progress: u64,
    draft_recons: u64,
    draft_payments: u64,
    pending_misc: u64,
}

// ── Main content (matches prototype 02-index.html) ──

fn purchase_dashboard_content(stats: &DashboardStats) -> Markup {
    html! {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "采购管理总览" }
            div class="flex gap-3" {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
                    (icon::download_icon("w-4 h-4"))
                    " 导出报表"
                }
            }
        }

        // ── Stat Cards (5 columns, icon + value layout) ──
        div style="display:grid;grid-template-columns:repeat(5,1fr);gap:var(--space-4);margin-bottom:var(--space-8)" {
            // 活跃供应商
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" {
                    (icon::building_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (stats.active_suppliers.to_string()) }
                    div class="text-sm text-text-muted mt-1" { "活跃供应商" }
                }
            }
            // 待比价报价
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" {
                    (icon::clipboard_list_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (stats.pending_quotations.to_string()) }
                    div class="text-sm text-text-muted mt-1" { "待比价报价" }
                }
            }
            // 进行中订单
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" {
                    (icon::clipboard_document_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (stats.in_progress_orders.to_string()) }
                    div class="text-sm text-text-muted mt-1" { "进行中订单" }
                }
            }
            // 待付款金额
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 red" {
                    (icon::payment_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" {
                        @if stats.pending_payment_total == rust_decimal::Decimal::ZERO {
                            "¥0"
                        } @else {
                            (format_amount(stats.pending_payment_total))
                        }
                    }
                    div class="text-sm text-text-muted mt-1" { "待付款金额" }
                }
            }
            // 退货处理中
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" {
                    (icon::return_arrow_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (stats.returns_in_progress.to_string()) }
                    div class="text-sm text-text-muted mt-1" { "退货处理中" }
                }
            }
        }

        // ── 2-column: 待办事项 + 快捷入口 ──
        div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-6);margin-bottom:var(--space-8)" {
            // 待办事项
            div {
                div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)" {
                    h2 class="text-lg font-semibold text-fg" { "待办事项" }
                }
                div class="data-card" {
                    @if stats.pending_quotations > 0 {
                        (todo_item("status-pill status-progress", "待比价", &format!("{} 份采购报价待比价", stats.pending_quotations), "需处理"))
                    }
                    @if stats.in_progress_orders > 0 {
                        (todo_item("status-pill status-draft", "待确认", &format!("{} 笔采购订单待确认", stats.in_progress_orders), "需处理"))
                    }
                    @if stats.returns_in_progress > 0 {
                        (todo_item("status-pill status-info", "退货中", &format!("{} 笔退货处理中", stats.returns_in_progress), "需处理"))
                    }
                    @if stats.draft_recons > 0 {
                        (todo_item("status-pill status-progress", "对账中", &format!("{} 份对账单待确认", stats.draft_recons), "需处理"))
                    }
                    @if stats.draft_payments > 0 {
                        (todo_item("status-pill status-progress", "待审批", &format!("{} 笔付款申请待审批", stats.draft_payments), "需处理"))
                    }
                    @if stats.pending_misc > 0 {
                        (todo_item("status-pill status-progress", "待审批", &format!("{} 笔零星请购待审批", stats.pending_misc), "需处理"))
                    }
                    @if stats.pending_quotations == 0 && stats.in_progress_orders == 0 && stats.returns_in_progress == 0 && stats.draft_recons == 0 && stats.draft_payments == 0 && stats.pending_misc == 0 {
                        (todo_item_last("status-pill status-completed", "无待办", "所有采购事项已处理完毕", ""))
                    }
                }
            }
            // 快捷入口
            div {
                div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)" {
                    h2 class="text-lg font-semibold text-fg" { "快捷入口" }
                }
                div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-3)" {
                    (quick_link_card("/admin/purchase/demand-pool", "采购需求池", "外购需求聚合"))
                    (quick_link_card("/admin/purchase/quotations", "采购报价", "报价管理"))
                    (quick_link_card("/admin/purchase/orders", "采购订单", "订单管理"))
                    (quick_link_card("/admin/purchase/returns", "采购退货", "退货管理"))
                    (quick_link_card("/admin/purchase/reconciliations", "采购对账", "对账管理"))
                    (quick_link_card("/admin/purchase/payments", "付款申请", "付款管理"))
                    (quick_link_card("/admin/purchase/misc-requests", "零星请购", "请购管理"))
                }
            }
        }
        // ── 采购业务流程 (info-card with circular icons) ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-bottom:var(--space-8)" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "采购业务流程" }
            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-6) 0;flex-wrap:wrap;gap:var(--space-3)" {
                // 供应商
                (flow_circle(&icon::building_icon("w-5 h-5"), "供应商", "主数据", "rgba(22,119,255,0.1)", "rgba(22,119,255,0.2)", "var(--accent)"))
                (arrow_right_svg())
                // 采购报价
                (flow_circle(&icon::clipboard_list_icon("w-5 h-5"), "采购报价", "阳光比价", "rgba(250,173,20,0.1)", "rgba(250,173,20,0.2)", "var(--warn)"))
                (arrow_right_svg())
                // 采购订单
                (flow_circle(&icon::clipboard_document_icon("w-5 h-5"), "采购订单", "下达采购", "rgba(82,196,26,0.1)", "rgba(82,196,26,0.2)", "var(--success)"))
                (arrow_right_svg())
                // 采购对账
                (flow_circle(&icon::clipboard_list_icon("w-5 h-5"), "采购对账", "月度结算", "rgba(22,119,255,0.1)", "rgba(22,119,255,0.2)", "var(--info)"))
                (arrow_right_svg())
                // 付款申请
                (flow_circle(&icon::payment_icon("w-5 h-5"), "付款申请", "三单匹配", "rgba(255,77,79,0.1)", "rgba(255,77,79,0.2)", "var(--danger)"))
            }
            // Branch links
            div style="display:flex;gap:var(--space-6);margin-top:var(--space-4);justify-content:center" {
                div style="display:flex;align-items:center;gap:var(--space-2);font-size:12px;color:var(--muted)" {
                    svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#7c3aed" stroke-width="1.8" { path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" {} }
                    a href="/admin/purchase/demand-pool" style="color:#7c3aed;font-weight:500" { "采购需求池（销售订单驱动）" }
                }
                div style="display:flex;align-items:center;gap:var(--space-2);font-size:12px;color:var(--muted)" {
                    svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="1.8" { path d="M3 10h10a5 5 0 015 5v2M3 10l4-4M3 10l4 4" {} }
                    a href="/admin/purchase/returns" style="color:var(--danger);font-weight:500" { "采购退货（逆向）" }
                }
                div style="display:flex;align-items:center;gap:var(--space-2);font-size:12px;color:var(--muted)" {
                    svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="1.8" { path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" {} }
                    a href="/admin/purchase/misc-requests" style="color:var(--success);font-weight:500" { "零星请购（非生产物资）" }
                }
            }
        }

        // ── 最近活动 ──
        div {
            h2 style="font-size:var(--text-lg);font-weight:600;margin-bottom:var(--space-4)" { "最近活动" }
            div class="data-card" {
                (activity_row("status-pill status-info", "订单", "采购订单 PO-2026-05-00123 状态变更为", "部分收货", "30 分钟前"))
                (activity_row("status-pill status-progress", "报价", "供应商「昆山宏达钢材」提交了新的采购报价 PQ-2026-05-00089", "", "2 小时前"))
                (activity_row("status-pill status-completed", "付款", "付款申请 PAY-2026-05-00045 已完成付款", "", "昨天"))
                (activity_row("status-pill status-progress", "退货", "采购退货 PRT-2026-05-00012 已发出，等待供应商确认", "", "昨天"))
                (activity_row("status-pill status-shipped", "对账", "采购对账单 PR-2026-05-00008 已确认", "", "2 天前"))
                (activity_row_last("status-pill status-progress", "请购", "零星请购 MISC-2026-05-00031 已审批通过", "", "3 天前"))
            }
        }
    }
}

// ── Sub-components ──

fn format_amount(d: rust_decimal::Decimal) -> String {
    let f: f64 = d.try_into().unwrap_or(0.0);
    if f >= 1_000_000.0 {
        format!("¥{:.1}M", f / 1_000_000.0)
    } else if f >= 10_000.0 {
        format!("¥{:.0}K", f / 1_000.0)
    } else {
        format!("¥{}", f as u64)
    }
}

fn flow_circle(
    svg_icon: &Markup,
    label: &str,
    desc: &str,
    bg_from: &str,
    bg_to: &str,
    icon_color: &str,
) -> Markup {
    html! {
        div style="display:flex;flex-direction:column;align-items:center;gap:var(--space-2);min-width:100px" {
            div style=(format!("width:40px;height:40px;border-radius:50%;background:linear-gradient(135deg,{},{});display:grid;place-items:center", bg_from, bg_to)) {
                span style=(format!("color:{}", icon_color)) { (svg_icon) }
            }
            span style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { (label) }
            span style="font-size:11px;color:var(--muted)" { (desc) }
        }
    }
}

fn arrow_right_svg() -> Markup {
    html! {
        svg viewBox="0 0 40 20" style="flex-shrink:0;margin:0 var(--space-2)" width="40" height="20" {
            path d="M0 10h32M26 5l6 5-6 5" fill="none" stroke="var(--border)" stroke-width="2" {}
        }
    }
}

fn activity_row(
    pill_class: &str,
    pill_text: &str,
    desc: &str,
    highlight: &str,
    time: &str,
) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);border-bottom:1px solid var(--border-soft);display:flex;align-items:center;gap:var(--space-4)" {
            span class=(pill_class) style="font-size:11px;min-width:56px;justify-content:center" { (pill_text) }
            span style="flex:1" {
                (desc)
                @if !highlight.is_empty() {
                    span style="font-weight:600" { " " (highlight) }
                }
            }
            span class="text-text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn activity_row_last(
    pill_class: &str,
    pill_text: &str,
    desc: &str,
    highlight: &str,
    time: &str,
) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);display:flex;align-items:center;gap:var(--space-4)" {
            span class=(pill_class) style="font-size:11px;min-width:56px;justify-content:center" { (pill_text) }
            span style="flex:1" {
                (desc)
                @if !highlight.is_empty() {
                    span style="font-weight:600" { " " (highlight) }
                }
            }
            span class="text-text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn todo_item(status_class: &str, status_text: &str, desc: &str, time: &str) -> Markup {
    html! {
        div class="flex items-center gap-3 px-5 py-4 border-b border-border-soft cursor-pointer hover:bg-accent-bg" {
            span class=(status_class) style="font-size:11px" { (status_text) }
            span style="flex:1" { (desc) }
            span class="text-text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn todo_item_last(status_class: &str, status_text: &str, desc: &str, time: &str) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);display:flex;align-items:center;gap:var(--space-3);cursor:pointer" {
            span class=(status_class) style="font-size:11px" { (status_text) }
            span style="flex:1" { (desc) }
            span class="text-text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn quick_link_card(href: &str, title: &str, count: &str) -> Markup {
    html! {
        a href=(href) class="flex flex-col gap-1 p-4 bg-bg border border-border-soft rounded cursor-pointer no-underline" {
            span class="text-sm font-semibold text-fg" { (title) }
            span class="text-xs text-text-muted" { (count) }
        }
    }
}