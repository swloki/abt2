use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::wms::pick_list::{model::PickItemInput, PickListService};
use abt_core::wms::work_center::model::{
    PendingTask, Urgency, UrgentSummary, WorkCenterDomain, WorkCenterSummary,
};
use abt_core::wms::work_center::WorkCenterService;
use rust_decimal::Decimal;

use crate::components::icon;
use crate::errors::Result;
use abt_core::shared::types::error::DomainError;
use crate::layout::page::admin_page;
use crate::routes::wms_work_center::{
    WmsWorkCenterFragmentPath, WmsWorkCenterPath, WmsWorkCenterPickPath,
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

        // ── 拣货 drawer overlay（body 由 hx-get 填充）──
        div id="pick-overlay"
            class="fixed inset-0 bg-slate-900/40 opacity-0 invisible pointer-events-none transition-opacity duration-200 z-[90] open:opacity-100 open:visible open:pointer-events-auto" {
            div id="pick-drawer"
                class="fixed top-0 right-0 h-full w-[460px] max-w-[92vw] bg-bg shadow-lg translate-x-full transition-transform duration-300 flex flex-col z-[91]" {
                div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
                    div class="font-bold text-base text-fg" { "录入拣货" }
                    button type="button"
                        class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                        _="on click closePickDrawer()" {
                        (icon::x_icon("w-4 h-4"))
                    }
                }
                div id="pick-drawer-body" class="flex-1 overflow-y-auto px-6 py-5" {
                    // 由 hx-get /admin/wms/work-center/pick/{id} 填充
                }
            }
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
        form hx-post=(WmsWorkCenterPickPath { id: path.id }.to_string())
            hx-swap="none" {
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
                    _="on click closePickDrawer()" { "取消" }
                button type="submit"
                    class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                    { "确认拣货" }
            }
            // drawer 打开（hyperscript：htmx 填充后调 openPickDrawer 显示 overlay）
        }
        // htmx afterRequest 打开 drawer（写在脚本里，见 base 布局；此处用 inline 兜底）
        script { (maud::PreEscaped(
            "document.addEventListener('htmx:afterRequest', function(e){ \
               if(e.target && e.target.closest('[hx-target=\"#pick-drawer-body\"]')){ \
                 var o=document.getElementById('pick-overlay'); \
                 if(o){ o.classList.add('open'); var d=document.getElementById('pick-drawer'); if(d){ d.classList.remove('translate-x-full'); } } \
               } \
             });"
        ))}
    };
    Ok(Html(body.into_string()))
}

/// 拣货提交：record_pick_items + complete_pick（事务包裹），HX-Redirect 整页刷新
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

    let redirect = WmsWorkCenterPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
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

fn render_todo_nav(summary: &WorkCenterSummary, urgent: &UrgentSummary) -> Markup {
    let total = summary.total();
    html! {
        div class="sticky top-0 z-20 flex items-center gap-4 p-3 mb-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap" {
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
            td class="py-3 px-3 text-sm font-mono text-accent font-semibold" { (t.doc_number) }
            td class="py-3 px-3 text-sm text-fg-2" { (t.counterparty) }
            td class="py-3 px-3 text-sm text-muted" { (t.summary) }
            td class="py-3 px-3 text-sm font-mono text-fg-2" { (expected) }
            td class="py-3 px-3" {
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {urgency_cls}")) {
                    (urgency_label)
                }
            }
            td class="py-3 px-3 text-right" {
                @if domain == WorkCenterDomain::Pick {
                    button type="button"
                        class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                        hx-get=(WmsWorkCenterPickPath { id: t.doc_id }.to_string())
                        hx-target="#pick-drawer-body"
                        hx-swap="innerHTML"
                        { (icon::plus_icon("w-3 h-3")) "拣货" }
                } @else {
                    span class="text-xs text-muted" { "—" }
                }
            }
        }
    }
}
