use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::wms::enums::{RequisitionStatus, TransferStatus};
use abt_core::wms::material_requisition::model::{IssueItemReq, IssueMaterialReq};
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::wms::outbound::model::ShippingStatus;
use abt_core::wms::outbound::ShippingRequestService;
use abt_core::wms::pick_list::{model::PickItemInput, PickListService};
use abt_core::wms::transfer::TransferService;
use abt_core::wms::work_center::model::{
    PendingTask, Urgency, UrgentSummary, WorkCenterDomain, WorkCenterSummary,
};
use abt_core::wms::work_center::WorkCenterService;
use rust_decimal::Decimal;

use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::error::DomainError;
use crate::layout::page::admin_page;
use crate::routes::shipping::ShippingDetailPath;
use crate::routes::wms_arrival::ArrivalDetailPath;
use crate::routes::wms_cycle_count::CycleCountDetailPath;
use crate::routes::wms_requisition::RequisitionDetailPath;
use crate::routes::wms_transfer::TransferDetailPath;
use crate::routes::wms_work_center::{
    WcIssuePath, WcShipPath, WcTransferPath, WmsWorkCenterFragmentPath, WmsWorkCenterPath,
    WmsWorkCenterPickPath,
};
use crate::utils::fmt_qty;
use crate::utils::RequestContext;
use abt_macros::require_permission;

/// 作业中心域名（URL slug）→ 枚举
fn domain_from_str(s: &str) -> Option<WorkCenterDomain> {
    match s {
        "arrival" => Some(WorkCenterDomain::Arrival),
        "inspection" => Some(WorkCenterDomain::Inspection),
        "pick" => Some(WorkCenterDomain::Pick),
        "outbound" => Some(WorkCenterDomain::Outbound),
        "requisition" => Some(WorkCenterDomain::Requisition),
        "transfer" => Some(WorkCenterDomain::Transfer),
        "cycle-count" => Some(WorkCenterDomain::CycleCount),
        _ => None,
    }
}

fn domain_slug(d: WorkCenterDomain) -> &'static str {
    match d {
        WorkCenterDomain::Arrival => "arrival",
        WorkCenterDomain::Inspection => "inspection",
        WorkCenterDomain::Pick => "pick",
        WorkCenterDomain::Outbound => "outbound",
        WorkCenterDomain::Requisition => "requisition",
        WorkCenterDomain::Transfer => "transfer",
        WorkCenterDomain::CycleCount => "cycle-count",
    }
}

fn domain_meta(d: WorkCenterDomain) -> (&'static str, Markup) {
    match d {
        WorkCenterDomain::Arrival => ("待收货", icon::truck_icon("w-4 h-4")),
        WorkCenterDomain::Inspection => ("待质检", icon::search_icon("w-4 h-4")),
        WorkCenterDomain::Pick => ("待拣货", icon::package_icon("w-4 h-4")),
        WorkCenterDomain::Outbound => ("待发货", icon::upload_icon("w-4 h-4")),
        WorkCenterDomain::Requisition => ("待领料", icon::clipboard_list_icon("w-4 h-4")),
        WorkCenterDomain::Transfer => ("待调拨", icon::arrow_right_icon("w-4 h-4")),
        WorkCenterDomain::CycleCount => ("待盘点", icon::check_circle_icon("w-4 h-4")),
    }
}

/// 单据号深链：按环节映射到对应业务域详情页 URL。
/// 拣货无独立详情页（依附发货/作业中心），返回 None → 单据号渲染为纯文本。
/// 分层约定：abt-core 不硬编码前端 URL，跳转路径在 abt-web 层按 domain + doc_id 拼接。
fn domain_detail_url(domain: WorkCenterDomain, doc_id: i64) -> Option<String> {
    match domain {
        // 待收货 / 待质检 共用来料通知详情（inspect 在该页触发）
        WorkCenterDomain::Arrival | WorkCenterDomain::Inspection => {
            Some(ArrivalDetailPath { id: doc_id }.to_string())
        }
        WorkCenterDomain::Outbound => Some(ShippingDetailPath { id: doc_id }.to_string()),
        WorkCenterDomain::Requisition => Some(RequisitionDetailPath { id: doc_id }.to_string()),
        WorkCenterDomain::Transfer => Some(TransferDetailPath { id: doc_id }.to_string()),
        WorkCenterDomain::CycleCount => Some(CycleCountDetailPath { id: doc_id }.to_string()),
        WorkCenterDomain::Pick => None,
    }
}

/// 跳转类操作按钮（质检 / 盘点）：纯链接到对应详情页，次级按钮样式。
/// 无 hyperscript `_=`（纯 `<a href>` 跳转），避免在链接上 halt 阻止导航。
fn render_jump_action(label: &str, url: &str) -> Markup {
    html! {
        a class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-surface border border-border-soft text-fg-2 text-xs font-semibold no-underline cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
            href=(url) {
            (label)
            (icon::arrow_right_icon("w-3 h-3"))
        }
    }
}

// ── Handlers ──

/// 仓库作业中心首页（锚点条 + 7 disclosure 折叠 + 拣货 drawer overlay）
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

    let svc = state.wms_work_center_service();
    let summary = svc.summary(&service_ctx, &mut conn).await.unwrap_or_default();
    let urgent = svc
        .urgent_summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = html! {
        // ── Header ──
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div {
                h1 class="text-xl font-bold text-fg tracking-tight" { "仓库作业中心" }
                p class="text-sm text-muted mt-1" { "按环节展开处理 · 就地操作不跳转" }
            }
        }

        // ── 锚点条（todo-nav，sticky 吸顶）──
        (render_todo_nav(&summary, &urgent))

        // ── 7 disclosure 折叠区 ──
        (render_disclosure(WorkCenterDomain::Arrival, summary.arrivals_pending))
        (render_disclosure(WorkCenterDomain::Inspection, summary.inspections_pending))
        (render_disclosure(WorkCenterDomain::Pick, summary.picks_pending))
        (render_disclosure(WorkCenterDomain::Outbound, summary.outbounds_pending))
        (render_disclosure(WorkCenterDomain::Requisition, summary.requisitions_pending))
        (render_disclosure(WorkCenterDomain::Transfer, summary.transfers_pending))
        (render_disclosure(WorkCenterDomain::CycleCount, summary.cycle_counts_pending))

        // ── 共享 drawer overlay（各域 GET 端点把「标题+表单+footer」填入 #wc-drawer-body）──
        (wc_drawer_shell())
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

/// disclosure 懒加载：返回某环节的待办队列片段（填入 .di-body）
#[require_permission("INVENTORY", "read")]
pub async fn get_domain_fragment(
    path: WmsWorkCenterFragmentPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let domain = domain_from_str(&path.domain)
        .ok_or_else(|| DomainError::validation(format!("未知作业环节: {}", path.domain)))?;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let res = state
        .wms_work_center_service()
        .list_pending(&service_ctx, &mut conn, domain, PageParams::new(1, 50))
        .await
        .unwrap_or_else(|_| PaginatedResult::empty(1, 50));
    Ok(Html(render_task_table(&res.items, domain).into_string()))
}

/// 拣货 drawer body：返回明细录入表单（点「拣货」按钮 hx-get 加载）
#[require_permission("INVENTORY", "read")]
pub async fn get_pick_drawer(path: WmsWorkCenterPickPath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.pick_list_service();
    let pl = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    let body = html! {
        // 标题栏（含关闭按钮，关闭共享 overlay）
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { "录入拣货" }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        form id="wc-pick-form" hx-post=(WmsWorkCenterPickPath { id: path.id }.to_string())
            hx-swap="none"
            class="px-6 py-5" {
            div class="mb-4" {
                span class="text-xs text-muted font-medium" { "拣货单 " }
                span class="text-sm font-mono font-semibold text-fg" { (pl.doc_number) }
            }
            table class="w-full border-collapse" {
                thead {
                    tr {
                        th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "产品" }
                        th class="text-right text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "申请" }
                        th class="text-right text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "本次拣货" }
                    }
                }
                tbody {
                    @for (idx, it) in items.iter().enumerate() {
                        tr class="border-b border-border-soft" {
                            td class="py-2 px-2 text-sm text-fg" { "产品 #" (it.product_id) }
                            td class="py-2 px-2 text-sm font-mono text-right" { (fmt_qty(it.requested_qty)) }
                            td class="py-2 px-2 text-right" {
                                input type="hidden" name=(format!("items[{idx}][pick_list_item_id]")) value=(it.id);
                                input type="number" name=(format!("items[{idx}][picked_qty]"))
                                    value=(fmt_qty(it.picked_qty)) min="0"
                                    class="w-20 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-bg";
                            }
                        }
                    }
                }
            }
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                button type="button"
                    class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    _="on click remove .open from #wc-drawer-overlay" { "取消" }
                button type="submit"
                    class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                    { "确认拣货" }
            }
        }
    };
    Ok(Html(body.into_string()))
}

/// 拣货提交：record_pick_items + complete_pick（事务包裹）；广播 taskDone 联动刷新 + closeWcDrawer
#[require_permission("INVENTORY", "update")]
pub async fn post_pick_items(
    path: WmsWorkCenterPickPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PickItemsForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;

    // 多步写（record + complete）事务包裹，防半失败残留
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let svc = state.pick_list_service();
    let items: Vec<PickItemInput> = form
        .items
        .into_iter()
        .map(|r| PickItemInput {
            pick_list_item_id: r.pick_list_item_id,
            picked_qty: r.picked_qty,
            bin_id: None, // MVP 不录库位
        })
        .collect();
    svc.record_pick_items(&service_ctx, &mut tx, path.id, items)
        .await?;
    svc.complete_pick(&service_ctx, &mut tx, path.id).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    // 就地联动（非整页刷新）：taskDone 触发 disclosure 队列 + todo-nav 计数刷新；
    // closeWcDrawer 关闭共享 drawer。拣货后该单出列、待拣货计数下降，停留本页继续处理
    Ok(([("HX-Trigger", r#"{"taskDone":"","closeWcDrawer":""}"#)], Html(String::new())))
}

/// 发货 drawer body：按实时状态分流——Picking→确认发出表单；否则（Confirmed 未拣货）→提示需先拣货 + 跳详情。
/// ship 前必须 pick（shipping_detail 仅 Picking 显示「确认发出」），故 Confirmed 不就地 ship。
#[require_permission("INVENTORY", "read")]
pub async fn get_wc_ship_drawer(path: WcShipPath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let s = state
        .shipping_service()
        .find_by_id(&service_ctx, &mut conn, path.id)
        .await?;

    let body = html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" {
                @if s.status == ShippingStatus::Picking { "确认发出" } @else { "未拣货" }
            }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        @if s.status == ShippingStatus::Picking {
            form id="wc-ship-form" hx-post=(WcShipPath { id: path.id }.to_string())
                hx-swap="none" hx-confirm="确认已发出？将扣减库存并立应收账款"
                class="px-6 py-5" {
                div class="mb-3" {
                    span class="text-xs text-muted font-medium" { "发货单 " }
                    span class="text-sm font-mono font-semibold text-fg" { (s.doc_number) }
                }
                p class="text-sm text-muted mb-5" {
                    "拣货已完成。确认发出将扣减库存、立应收账款并回写销售订单。"
                }
                div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                    button type="button"
                        class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                        _="on click remove .open from #wc-drawer-overlay" { "取消" }
                    button type="submit"
                        class="px-4 py-2 rounded-sm bg-success text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                        { "确认发出" }
                }
            }
        } @else {
            // Confirmed 等未拣货状态：不能直接 ship
            div class="px-6 py-5" {
                div class="mb-3" {
                    span class="text-xs text-muted font-medium" { "发货单 " }
                    span class="text-sm font-mono font-semibold text-fg" { (s.doc_number) }
                }
                p class="text-sm text-warn mb-5" { "该单尚未拣货，无法直接发出。请先完成拣货。" }
                div class="flex justify-end" {
                    a class="inline-flex items-center gap-1 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium no-underline cursor-pointer border-none hover:opacity-90"
                        href=(ShippingDetailPath { id: path.id }.to_string()) {
                        "去详情页拣货" (icon::arrow_right_icon("w-3.5 h-3.5"))
                    }
                }
            }
        }
    };
    Ok(Html(body.into_string()))
}

/// 发货提交：ship（事务包裹，6 步联动：扣库存+立应收+SO回写）；广播 taskDone + closeWcDrawer
#[require_permission("SHIPPING", "update")]
pub async fn post_wc_ship(path: WcShipPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;

    // ship 多步写事务包裹（范本 shipping_detail::ship_shipping）：半失败回滚，避免数量已提交但库存/AR/状态未动
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    state
        .shipping_service()
        .ship(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    // 就地联动：发货后该单出列、待发货计数下降；closeWcDrawer 关闭共享 drawer
    Ok(([("HX-Trigger", r#"{"taskDone":"","closeWcDrawer":""}"#)], Html(String::new())))
}

/// 领料 drawer body：Confirmed→全量发料（明细只读 + 确认按钮）；PartiallyIssued→去详情页。
/// issue 记库存事务用绝对量（quantity = -issued_qty），就地重复发料会重复扣库存，故部分发料不在就地（跳详情页）。
#[require_permission("INVENTORY", "read")]
pub async fn get_wc_issue_drawer(path: WcIssuePath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.material_requisition_service();
    let req = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    let body = html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { "发料" }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        @if req.status == RequisitionStatus::Confirmed {
            form id="wc-issue-form" hx-post=(WcIssuePath { id: path.id }.to_string())
                hx-swap="none" hx-confirm="确认全量发料？将扣减库存并计入工单成本"
                class="px-6 py-5" {
                div class="mb-3" {
                    span class="text-xs text-muted font-medium" { "领料单 " }
                    span class="text-sm font-mono font-semibold text-fg" { (req.doc_number) }
                }
                p class="text-sm text-muted mb-4" { "共 " (items.len()) " 项，将按申请量全量发料。" }
                div class="rounded-sm border border-border-soft divide-y divide-border-soft mb-4" {
                    @for it in &items {
                        div class="flex items-center justify-between px-3 py-2" {
                            span class="text-sm text-fg-2" { "产品 #" (it.product_id) }
                            span class="text-sm font-mono text-muted" { "申请 " (fmt_qty(it.requested_qty)) }
                        }
                    }
                }
                div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                    button type="button"
                        class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                        _="on click remove .open from #wc-drawer-overlay" { "取消" }
                    button type="submit"
                        class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                        { "确认发料" }
                }
            }
        } @else {
            // PartiallyIssued 等：issue 记绝对量，就地重复发料会重复扣库存 → 跳详情页
            div class="px-6 py-5" {
                div class="mb-3" {
                    span class="text-xs text-muted font-medium" { "领料单 " }
                    span class="text-sm font-mono font-semibold text-fg" { (req.doc_number) }
                }
                p class="text-sm text-warn mb-5" {
                    "该单已部分发料。继续发料请在详情页操作（避免重复扣库存）。"
                }
                div class="flex justify-end" {
                    a class="inline-flex items-center gap-1 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium no-underline cursor-pointer border-none hover:opacity-90"
                        href=(RequisitionDetailPath { id: path.id }.to_string()) {
                        "去详情页发料" (icon::arrow_right_icon("w-3.5 h-3.5"))
                    }
                }
            }
        }
    };
    Ok(Html(body.into_string()))
}

/// 领料提交：全量发料 issue（事务包裹，5 步联动：扣库存+消耗预留+成本分录）；广播 taskDone + closeWcDrawer
#[require_permission("INVENTORY", "update")]
pub async fn post_wc_issue(path: WcIssuePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    // 全量发料：issued_qty = requested_qty（对齐 wms_requisition_detail 快速发料；仅 Confirmed 安全）
    let items = svc.list_items(&service_ctx, &mut tx, path.id).await?;
    let issue_items: Vec<IssueItemReq> = items
        .iter()
        .map(|it| IssueItemReq {
            item_id: it.id,
            issued_qty: it.requested_qty,
            bin_id: None,
        })
        .collect();
    svc.issue(
        &service_ctx,
        &mut tx,
        IssueMaterialReq { id: path.id, items: issue_items },
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    Ok(([("HX-Trigger", r#"{"taskDone":"","closeWcDrawer":""}"#)], Html(String::new())))
}

/// 调拨 drawer body：按状态分流——Draft→调出（dispatch）；InTransit→到货确认（complete）
#[require_permission("INVENTORY", "read")]
pub async fn get_wc_transfer_drawer(path: WcTransferPath, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.transfer_service();
    let trf = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .get_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    // 队列仅含 Draft + InTransit；其余状态兜底（不可达）
    let (title, action, hint, btn_label) = match trf.status {
        TransferStatus::Draft => ("调出", "dispatch", "确认调出将从源仓扣减库存、单据进入在途。", "确认调出"),
        TransferStatus::InTransit => {
            ("到货确认", "complete", "确认到货将把库存计入目标仓、完成调拨。", "确认到货")
        }
        _ => ("调拨", "complete", "该单当前状态不可就地操作。", "确认"),
    };

    let body = html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div class="px-6 py-5" {
            div class="mb-3" {
                span class="text-xs text-muted font-medium" { "调拨单 " }
                span class="text-sm font-mono font-semibold text-fg" { (trf.doc_number) }
            }
            p class="text-sm text-muted mb-2" {
                "仓 " (trf.from_warehouse_id) " → " (trf.to_warehouse_id) " · 共 " (items.len()) " 项"
            }
            p class="text-sm text-muted mb-5" { (hint) }
            form id="wc-transfer-form" hx-post=(WcTransferPath { id: path.id }.to_string())
                hx-swap="none"
                class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                input type="hidden" name="action" value=(action);
                button type="button"
                    class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    _="on click remove .open from #wc-drawer-overlay" { "取消" }
                button type="submit"
                    class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                    { (btn_label) }
            }
        }
    };
    Ok(Html(body.into_string()))
}

/// 调拨提交：dispatch / complete（事务包裹，2 步联动：库存事务+状态机）；广播 taskDone + closeWcDrawer
#[require_permission("INVENTORY", "update")]
pub async fn post_wc_transfer(
    path: WcTransferPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WcTransferActionForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let svc = state.transfer_service();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    match form.action.as_str() {
        "dispatch" => svc.dispatch(&service_ctx, &mut tx, path.id).await?,
        "complete" => svc.complete(&service_ctx, &mut tx, path.id).await?,
        other => return Err(DomainError::validation(format!("未知调拨动作: {other}")).into()),
    }
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    Ok(([("HX-Trigger", r#"{"taskDone":"","closeWcDrawer":""}"#)], Html(String::new())))
}

#[derive(Debug, Deserialize)]
pub struct WcTransferActionForm {
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct PickRowForm {
    pub pick_list_item_id: i64,
    pub picked_qty: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct PickItemsForm {
    pub items: Vec<PickRowForm>,
}

// ── 渲染辅助 ──

/// 共享 drawer overlay 壳：页面渲染一次，各域 drawer GET 端点把「标题栏+表单+footer」
/// 填入 #wc-drawer-body。显隐由 .drawer-overlay 的 .open class 控制（见 uno.config.ts
/// preflight：默认 display:none，.open 时 display:flex + panel 平移入）。
///
/// 关闭路径有二：① 点遮罩背景（on click[me is event.target]）；② drawer 提交后后端
/// HX-Trigger 广播 closeWcDrawer（on closeWcDrawer from:body）。打开由各域 row 按钮
/// `_="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay"` 完成。
fn wc_drawer_shell() -> Markup {
    html! {
        div id="wc-drawer-overlay"
            class="drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]"
            _="on click[me is event.target] remove .open from me on closeWcDrawer from:body remove .open from me" {
            div class="drawer-panel bg-white h-full w-[460px] max-w-[92vw] flex flex-col"
                _="on click js(event) event.stopPropagation() end" {
                div id="wc-drawer-body" class="flex-1 overflow-y-auto" {
                    // 由各域 drawer GET 端点填充（标题栏 + 表单 + footer）
                }
            }
        }
    }
}

fn render_todo_nav(summary: &WorkCenterSummary, urgent: &UrgentSummary) -> Markup {
    let total = summary.total();
    html! {
        // taskDone 联动：drawer 提交后重拉整页 content，hx-select 取出本 nav 替换自身 → 计数下降、处理完的 chip 消失
        div id="todo-nav"
            class="sticky top-0 z-20 flex items-center gap-4 p-3 mb-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap"
            hx-get=(WmsWorkCenterPath::PATH)
            hx-select="#todo-nav"
            hx-target="this"
            hx-swap="outerHTML"
            hx-trigger="taskDone from:body"
            hx-disinherit="hx-select" {
            div class="flex flex-col items-center pr-4 border-r border-border-soft shrink-0" {
                span class="text-xl font-bold font-mono tabular-nums text-accent leading-tight" { (total) }
                span class="text-xs text-muted font-medium" { "待办" }
            }
            div class="flex items-center gap-2 flex-wrap" {
                (nav_chip("arrival", "待收货", summary.arrivals_pending))
                (nav_chip("inspection", "待质检", summary.inspections_pending))
                (nav_chip("pick", "待拣货", summary.picks_pending))
                (nav_chip("outbound", "待发货", summary.outbounds_pending))
                (nav_chip("requisition", "待领料", summary.requisitions_pending))
                (nav_chip("transfer", "待调拨", summary.transfers_pending))
                (nav_chip("cycle-count", "待盘点", summary.cycle_counts_pending))
            }
            @if urgent.overdue_count > 0 || urgent.soon_count > 0 {
                div class="flex items-center gap-2 ml-auto" {
                    @if urgent.overdue_count > 0 {
                        span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-xs font-semibold" {
                            (icon::circle_alert_icon("w-3 h-3")) (urgent.overdue_count) " 逾期"
                        }
                    }
                    @if urgent.soon_count > 0 {
                        span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-xs font-semibold" {
                            (icon::bell_icon("w-3 h-3")) (urgent.soon_count) " 临期"
                        }
                    }
                }
            }
        }
    }
}

fn nav_chip(slug: &str, label: &str, count: u64) -> Markup {
    if count == 0 {
        return html! {};
    }
    html! {
        a class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface border border-border-soft text-sm font-semibold text-fg-2 no-underline cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
            href={(format!("#d-{slug}"))}
            _=(format!("on click halt the event then call document.getElementById('d-{slug}').scrollIntoView({{behavior:'smooth',block:'center'}}) then trigger click on #d-{slug}-head")) {
            (label)
            span class="font-mono font-bold text-accent" { (count) }
        }
    }
}

fn render_disclosure(domain: WorkCenterDomain, count: u64) -> Markup {
    let (label, ic) = domain_meta(domain);
    let slug = domain_slug(domain);
    let frag = WmsWorkCenterFragmentPath {
        domain: slug.to_string(),
    }
    .to_string();
    html! {
        div class="bg-bg border border-border-soft rounded-lg mb-3 shadow-xs overflow-hidden"
            id=(format!("d-{slug}")) {
            div class="flex items-center gap-3 px-5 py-4 cursor-pointer select-none hover:bg-surface-raised transition-colors"
                id=(format!("d-{slug}-head"))
                hx-get=(frag)
                hx-target="next .di-body"
                hx-swap="innerHTML"
                hx-trigger="click, taskDone from:body"
                _="on click toggle .hidden on next .di-body" {
                div class="w-8 h-8 rounded-md grid place-items-center shrink-0 bg-surface text-fg-2" { (ic) }
                span class="text-sm font-semibold text-fg shrink-0" { (label) }
                span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" {
                    @if count > 0 { (count) " 笔待处理" } @else { "当前无待办" }
                }
                (icon::chevron_down_icon("w-4 h-4 text-muted shrink-0"))
            }
            div class="di-body hidden px-5 pb-5 pt-4 border-t border-border-soft" {}
        }
    }
}

fn render_task_table(tasks: &[PendingTask], domain: WorkCenterDomain) -> Markup {
    if tasks.is_empty() {
        return html! {
            div class="mt-2 p-4 text-center text-sm text-muted bg-surface rounded-md" { "暂无待办" }
        };
    }
    html! {
        table class="w-full border-collapse mt-2" {
            thead {
                tr {
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "单号" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "对象" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "摘要" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "到期" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "紧急度" }
                    th class="text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "操作" }
                }
            }
            tbody {
                @for t in tasks {
                    (render_task_row(t, domain))
                }
            }
        }
    }
}

fn render_task_row(t: &PendingTask, domain: WorkCenterDomain) -> Markup {
    let (urgency_label, urgency_cls) = match t.urgency {
        Urgency::Overdue => ("逾期", "bg-danger-bg text-danger"),
        Urgency::Soon => ("临期", "bg-warn-bg text-warn"),
        Urgency::Normal => ("正常", "bg-surface text-muted"),
    };
    let expected = t
        .expected_at
        .map(|d| d.format("%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    html! {
        tr class="border-b border-border-soft last:border-b-0" {
            td class="py-3 px-3 text-sm font-mono text-accent font-semibold" {
                @if let Some(url) = domain_detail_url(domain, t.doc_id) {
                    a class="text-accent no-underline hover:underline cursor-pointer" href=(url) {
                        (t.doc_number)
                    }
                } @else {
                    (t.doc_number)
                }
            }
            td class="py-3 px-3 text-sm text-fg-2" { (t.counterparty) }
            td class="py-3 px-3 text-sm text-muted" { (t.summary) }
            td class="py-3 px-3 text-sm font-mono text-fg-2" { (expected) }
            td class="py-3 px-3" {
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {urgency_cls}")) {
                    (urgency_label)
                }
            }
            td class="py-3 px-3 text-right" {
                @match domain {
                    WorkCenterDomain::Pick => {
                        button type="button"
                            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                            hx-get=(WmsWorkCenterPickPath { id: t.doc_id }.to_string())
                            hx-target="#wc-drawer-body"
                            hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay"
                            { (icon::plus_icon("w-3 h-3")) "拣货" }
                    }
                    // 待质检：inspect 是 5 步联动（IQC 门禁+成本+事件），太复杂不就地，跳到货详情
                    WorkCenterDomain::Inspection => {
                        (render_jump_action("质检", &ArrivalDetailPath { id: t.doc_id }.to_string()))
                    }
                    // 待盘点：多状态多动作（start/count/complete/adjust/approve），不就地，跳盘点详情
                    WorkCenterDomain::CycleCount => {
                        (render_jump_action("盘点", &CycleCountDetailPath { id: t.doc_id }.to_string()))
                    }
                    // 待发货：drawer 按状态分流（Picking→确认发出 / Confirmed→需先拣货）
                    WorkCenterDomain::Outbound => {
                        button type="button"
                            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                            hx-get=(WcShipPath { id: t.doc_id }.to_string())
                            hx-target="#wc-drawer-body"
                            hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay"
                            { (icon::upload_icon("w-3 h-3")) "发货" }
                    }
                    // 待领料：drawer 按状态分流（Confirmed→全量发料 / PartiallyIssued→去详情页）
                    WorkCenterDomain::Requisition => {
                        button type="button"
                            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                            hx-get=(WcIssuePath { id: t.doc_id }.to_string())
                            hx-target="#wc-drawer-body"
                            hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay"
                            { (icon::clipboard_list_icon("w-3 h-3")) "发料" }
                    }
                    // 待调拨：drawer 按状态分流（Draft→调出 / InTransit→到货确认）
                    WorkCenterDomain::Transfer => {
                        button type="button"
                            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                            hx-get=(WcTransferPath { id: t.doc_id }.to_string())
                            hx-target="#wc-drawer-body"
                            hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay"
                            { (icon::arrow_right_icon("w-3 h-3")) "办理" }
                    }
                    // 待收货：就地 drawer 见 PR5，当前占位
                    WorkCenterDomain::Arrival => {
                        span class="text-xs text-muted" { "—" }
                    }
                }
            }
        }
    }
}
