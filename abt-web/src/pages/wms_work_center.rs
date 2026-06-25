use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::shared::types::{PgExecutor, ServiceContext};
use abt_core::wms::arrival_notice::model::{ReceiveArrivalNoticeReq, ReceiveItemReq};
use abt_core::wms::arrival_notice::ArrivalNoticeService;
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
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::fmt_qty;
use crate::utils::RequestContext;
use crate::state::AppState;
use abt_macros::require_permission;

/// 单端点查询参数：三选一——expand（懒加载某卡片 body）/ drawer+id（加载 drawer body）/ 均空（整页）
#[derive(Debug, Deserialize, Default)]
pub struct WorkCenterQuery {
    pub expand: Option<String>,
    pub drawer: Option<String>,
    pub id: Option<i64>,
}

/// 就地操作提交：action 决定分发，id 目标单据，items_json（收货/拣货行级明细，JSON 字符串）
#[derive(Debug, Deserialize)]
pub struct WorkCenterActionForm {
    pub action: String,
    pub id: i64,
    pub items_json: Option<String>,
}

// ── domain ↔ slug / 动作 映射 ──

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

/// 某环节当前待办计数（从 summary 读对应字段）
fn domain_count(s: &WorkCenterSummary, d: WorkCenterDomain) -> u64 {
    match d {
        WorkCenterDomain::Arrival => s.arrivals_pending,
        WorkCenterDomain::Inspection => s.inspections_pending,
        WorkCenterDomain::Pick => s.picks_pending,
        WorkCenterDomain::Outbound => s.outbounds_pending,
        WorkCenterDomain::Requisition => s.requisitions_pending,
        WorkCenterDomain::Transfer => s.transfers_pending,
        WorkCenterDomain::CycleCount => s.cycle_counts_pending,
    }
}

/// 就地操作 action → 受影响环节（决定提交后刷新哪张卡片）
fn action_domain(action: &str) -> Result<WorkCenterDomain> {
    Ok(match action {
        "receive" => WorkCenterDomain::Arrival,
        "pick" => WorkCenterDomain::Pick,
        "ship" => WorkCenterDomain::Outbound,
        "issue" => WorkCenterDomain::Requisition,
        "dispatch" | "complete" => WorkCenterDomain::Transfer,
        other => return Err(DomainError::validation(format!("未知作业动作: {other}")).into()),
    })
}

/// 单据号深链：按环节映射到对应业务域详情页 URL。
/// 拣货无独立详情页（依附发货/作业中心），返回 None → 单据号渲染为纯文本。
/// 分层约定：abt-core 不硬编码前端 URL，跳转路径在 abt-web 层按 domain + doc_id 拼接。
fn domain_detail_url(domain: WorkCenterDomain, doc_id: i64) -> Option<String> {
    match domain {
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
fn render_jump_action(label: &str, url: &str) -> Markup {
    html! {
        a class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-surface border border-border-soft text-fg-2 text-xs font-semibold no-underline cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
            href=(url) {
            (label)
            (icon::arrow_right_icon("w-3 h-3"))
        }
    }
}

// ── Handlers（单端点）──

/// 作业中心唯一 GET：按 query 分支——drawer body / 卡片 body（懒加载）/ 整页
#[require_permission("INVENTORY", "read")]
pub async fn get_wms_work_center(
    _path: WmsWorkCenterPath,
    axum::extract::Query(q): axum::extract::Query<WorkCenterQuery>,
    ctx: RequestContext,
) -> Result<Html<String>> {
    // drawer body：加载某就地操作表单（点行内按钮 hx-get 填入 #wc-drawer-body）
    if let (Some(drawer), Some(id)) = (q.drawer.as_deref(), q.id) {
        return render_drawer_body(drawer, id, ctx).await;
    }
    // 卡片 body（懒加载）：展开某环节时 hx-get 填入 #d-{slug}-body
    if let Some(slug) = q.expand.as_deref() {
        let domain = domain_from_str(slug)
            .ok_or_else(|| DomainError::validation(format!("未知作业环节: {slug}")))?;
        let RequestContext {
            mut conn, state, service_ctx, ..
        } = ctx;
        let res = state
            .wms_work_center_service()
            .list_pending(&service_ctx, &mut conn, domain, PageParams::new(1, 50))
            .await
            .unwrap_or_else(|_| PaginatedResult::empty(1, 50));
        return Ok(Html(render_task_table(&res.items, domain).into_string()));
    }
    render_full_page(ctx).await
}

/// 作业中心唯一 POST：执行就地操作，返回「受影响卡片 + todo-nav」片段。
/// 客户端 hx-target=#d-{slug} 替换卡片、hx-select-oob=#todo-nav 更新摘要带、hyperscript 关 drawer。
#[require_permission("INVENTORY", "update")]
pub async fn post_work_center_action(
    _path: WmsWorkCenterPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WorkCenterActionForm>,
) -> Result<Html<String>> {
    let domain = action_domain(&form.action)?;
    let RequestContext { state, service_ctx, mut conn, .. } = ctx;
    let svc = state.wms_work_center_service();

    // 多步写事务包裹（范本 shipping_detail::ship_shipping）
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    dispatch_action(&state, &service_ctx, &mut tx, &form).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 重渲染受影响卡片（展开态，含最新队列）+ todo-nav
    let summary = svc.summary(&service_ctx, &mut conn).await.unwrap_or_default();
    let urgent = svc
        .urgent_summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    let count = domain_count(&summary, domain);
    let items = svc
        .list_pending(&service_ctx, &mut conn, domain, PageParams::new(1, 50))
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let fragment = html! {
        (render_card(domain, count, Some(render_task_table(&items, domain))))
        (render_todo_nav(&summary, &urgent))
    };
    Ok(Html(fragment.into_string()))
}

/// 按 action 分发到各域 service（均在传入事务内执行）
async fn dispatch_action(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    form: &WorkCenterActionForm,
) -> Result<()> {
    match form.action.as_str() {
        "receive" => {
            let rows: Vec<ReceiveRowJson> = parse_items_json(form)?;
            let items: Vec<ReceiveItemReq> = rows
                .into_iter()
                .map(|r| -> Result<ReceiveItemReq> {
                    Ok(ReceiveItemReq {
                        item_id: r
                            .item_id
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("item_id 解析失败: {e}")))?,
                        received_qty: r
                            .received_qty
                            .parse::<Decimal>()
                            .map_err(|e| DomainError::validation(format!("收货数量解析失败: {e}")))?,
                        batch_no: r.batch_no.filter(|s| !s.is_empty()),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            state
                .arrival_notice_service()
                .receive(ctx, db, ReceiveArrivalNoticeReq { id: form.id, items })
                .await?;
        }
        "pick" => {
            let rows: Vec<PickRowJson> = parse_items_json(form)?;
            let items: Vec<PickItemInput> = rows
                .into_iter()
                .map(|r| -> Result<PickItemInput> {
                    Ok(PickItemInput {
                        pick_list_item_id: r
                            .pick_list_item_id
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("拣货行解析失败: {e}")))?,
                        picked_qty: r
                            .picked_qty
                            .parse::<Decimal>()
                            .map_err(|e| DomainError::validation(format!("拣货数量解析失败: {e}")))?,
                        bin_id: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let svc = state.pick_list_service();
            svc.record_pick_items(ctx, db, form.id, items).await?;
            svc.complete_pick(ctx, db, form.id).await?;
        }
        "ship" => {
            state.shipping_service().ship(ctx, db, form.id).await?;
        }
        "issue" => {
            // 全量发料（仅 Confirmed 安全；issue 记库存事务用绝对量，重复发料会重复扣库存）
            let req_svc = state.material_requisition_service();
            let items_db = req_svc.list_items(ctx, db, form.id).await?;
            let issue_items = items_db
                .iter()
                .map(|it| IssueItemReq {
                    item_id: it.id,
                    issued_qty: it.requested_qty,
                    bin_id: None,
                })
                .collect::<Vec<_>>();
            req_svc
                .issue(ctx, db, IssueMaterialReq { id: form.id, items: issue_items })
                .await?;
        }
        "dispatch" => {
            state.transfer_service().dispatch(ctx, db, form.id).await?;
        }
        "complete" => {
            state.transfer_service().complete(ctx, db, form.id).await?;
        }
        other => return Err(DomainError::validation(format!("未知作业动作: {other}")).into()),
    }
    Ok(())
}

fn parse_items_json<T: serde::de::DeserializeOwned>(form: &WorkCenterActionForm) -> Result<Vec<T>> {
    Ok(serde_json::from_str::<Vec<T>>(form.items_json.as_deref().unwrap_or("[]"))
        .map_err(|e| DomainError::validation(format!("明细解析失败: {e}")))?)
}

// 行级明细走 hidden items_json（JSON 字符串），字段统一用 String（i.value 为字符串），服务端再 parse
// 对齐 quotation/sales_order 的 ItemWeb 范式（见 static/app.js lineItemCalc.collectItems）
#[derive(Debug, Deserialize)]
struct ReceiveRowJson {
    item_id: String,
    received_qty: String,
    batch_no: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PickRowJson {
    pick_list_item_id: String,
    picked_qty: String,
}

// ── 页面 / 片段渲染 ──

async fn render_full_page(ctx: RequestContext) -> Result<Html<String>> {
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
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div {
                h1 class="text-xl font-bold text-fg tracking-tight" { "仓库作业中心" }
                p class="text-sm text-muted mt-1" { "按环节展开处理 · 就地操作不跳转" }
            }
        }
        (render_todo_nav(&summary, &urgent))
        (render_card(WorkCenterDomain::Arrival, summary.arrivals_pending, None))
        (render_card(WorkCenterDomain::Inspection, summary.inspections_pending, None))
        (render_card(WorkCenterDomain::Pick, summary.picks_pending, None))
        (render_card(WorkCenterDomain::Outbound, summary.outbounds_pending, None))
        (render_card(WorkCenterDomain::Requisition, summary.requisitions_pending, None))
        (render_card(WorkCenterDomain::Transfer, summary.transfers_pending, None))
        (render_card(WorkCenterDomain::CycleCount, summary.cycle_counts_pending, None))
        // 共享 drawer overlay（各域 GET ?drawer=&id= 把 body 填入 #wc-drawer-body）
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

fn render_todo_nav(summary: &WorkCenterSummary, urgent: &UrgentSummary) -> Markup {
    let total = summary.total();
    html! {
        div id="todo-nav"
            class="sticky top-0 z-20 flex items-center gap-4 p-3 mb-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap" {
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

/// 自洽卡片：#d-{slug} 整体可被 hx-target=outerHTML 替换。
/// body=Some → 展开（含队列，操作后回填用）；body=None → 折叠（首屏，点 head 懒加载）。
fn render_card(domain: WorkCenterDomain, count: u64, body: Option<Markup>) -> Markup {
    let (label, ic) = domain_meta(domain);
    let slug = domain_slug(domain);
    let expand_url = format!("{}?expand={}", WmsWorkCenterPath::PATH, slug);
    let body_cls = if body.is_some() {
        "di-body px-5 pb-5 pt-4 border-t border-border-soft"
    } else {
        "di-body hidden px-5 pb-5 pt-4 border-t border-border-soft"
    };
    html! {
        div id=(format!("d-{slug}")) class="bg-bg border border-border-soft rounded-lg mb-3 shadow-xs overflow-hidden" {
            div id=(format!("d-{slug}-head"))
                class="flex items-center gap-3 px-5 py-4 cursor-pointer select-none hover:bg-surface-raised transition-colors"
                hx-get=(expand_url)
                hx-target=(format!("#d-{slug}-body"))
                hx-swap="innerHTML"
                _=(format!("on click toggle .hidden on #d-{slug}-body")) {
                div class="w-8 h-8 rounded-md grid place-items-center shrink-0 bg-surface text-fg-2" { (ic) }
                span class="text-sm font-semibold text-fg shrink-0" { (label) }
                span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" {
                    @if count > 0 { (count) " 笔待处理" } @else { "当前无待办" }
                }
                (icon::chevron_down_icon("w-4 h-4 text-muted shrink-0"))
            }
            div id=(format!("d-{slug}-body")) class=(body_cls) {
                @if let Some(b) = body { (b) }
            }
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
                    a class="text-accent no-underline hover:underline cursor-pointer" href=(url) { (t.doc_number) }
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
                (render_row_action(domain, t.doc_id))
            }
        }
    }
}

/// 行内操作入口：拣货/收货/发货/领料/调拨 → hx-get 加载 drawer body；质检/盘点 → 跳详情页
fn render_row_action(domain: WorkCenterDomain, doc_id: i64) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay";
    match domain {
        WorkCenterDomain::Arrival => drawer_btn("收货", "receive", doc_id, icon::truck_icon("w-3 h-3"), open_hs),
        WorkCenterDomain::Pick => drawer_btn("拣货", "pick", doc_id, icon::plus_icon("w-3 h-3"), open_hs),
        WorkCenterDomain::Outbound => drawer_btn("发货", "ship", doc_id, icon::upload_icon("w-3 h-3"), open_hs),
        WorkCenterDomain::Requisition => {
            drawer_btn("发料", "issue", doc_id, icon::clipboard_list_icon("w-3 h-3"), open_hs)
        }
        WorkCenterDomain::Transfer => {
            drawer_btn("办理", "transfer", doc_id, icon::arrow_right_icon("w-3 h-3"), open_hs)
        }
        // 待质检：inspect 5 步联动（IQC+成本+事件）太复杂，跳到货详情
        WorkCenterDomain::Inspection => {
            render_jump_action("质检", &ArrivalDetailPath { id: doc_id }.to_string())
        }
        // 待盘点：多状态多动作，跳盘点详情
        WorkCenterDomain::CycleCount => {
            render_jump_action("盘点", &CycleCountDetailPath { id: doc_id }.to_string())
        }
    }
}

/// 行内 drawer 触发按钮：hx-get 加载 drawer body 到 #wc-drawer-body，成功后打开 overlay。
fn drawer_btn(label: &str, action: &str, doc_id: i64, ic: Markup, open_hs: &str) -> Markup {
    let url = format!("{}?drawer={action}&id={doc_id}", WmsWorkCenterPath::PATH);
    html! {
        button type="button"
            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
            hx-get=(url)
            hx-target="#wc-drawer-body"
            hx-swap="innerHTML"
            _=(open_hs) {
            (ic)
            (label)
        }
    }
}

/// 共享 drawer overlay 壳：页面渲染一次，各域 GET ?drawer=&id= 填 #wc-drawer-body。
/// 显隐由 .drawer-overlay 的 .open class 控制（uno.config.ts preflight）；× / 背景点击关闭。
fn wc_drawer_shell() -> Markup {
    html! {
        div id="wc-drawer-overlay"
            class="drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]"
            _="on click[me is event.target] remove .open from me" {
            div class="drawer-panel bg-white h-full w-[460px] max-w-[92vw] flex flex-col"
                _="on click js(event) event.stopPropagation() end" {
                div id="wc-drawer-body" class="flex-1 overflow-y-auto" {}
            }
        }
    }
}

// ── drawer body（GET ?drawer=&id=）：按 action 渲染表单，提交走单端点 POST ──

async fn render_drawer_body(action: &str, id: i64, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let body = match action {
        "receive" => receive_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "pick" => pick_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "ship" => ship_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "issue" => issue_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "transfer" => transfer_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        other => return Err(DomainError::validation(format!("未知 drawer 动作: {other}")).into()),
    };
    Ok(Html(body.into_string()))
}

/// drawer 操作表单：标题栏（含×）+ form（hidden action/id，hx-post 单端点，target 受影响卡片，
/// select-oob todo-nav，成功关 drawer）包裹 inner。
fn drawer_form(
    title: &str,
    action: &str,
    id: i64,
    domain: WorkCenterDomain,
    confirm: &str,
    onsubmit: &str,
    inner: Markup,
) -> Markup {
    let target = format!("#d-{}", domain_slug(domain));
    html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        form id=(format!("wc-{action}-form"))
            hx-post=(WmsWorkCenterPath::PATH)
            hx-target=(target)
            hx-select=(target)
            hx-swap="outerHTML"
            hx-select-oob="#todo-nav:outerHTML"
            hx-confirm=(confirm)
            onsubmit=(onsubmit)
            _="on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay"
            class="px-6 py-5" {
            input type="hidden" name="action" value=(action);
            input type="hidden" name="id" value=(id);
            (inner)
        }
    }
}

/// drawer 非操作态（未拣货 / 部分发料）：标题栏 + 警示 + 跳详情链接
fn drawer_message(
    title: &str,
    doc_label: &str,
    doc_number: &str,
    msg: &str,
    link_url: &str,
    link_label: &str,
) -> Markup {
    html! {
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
                span class="text-xs text-muted font-medium" { (doc_label) " " }
                span class="text-sm font-mono font-semibold text-fg" { (doc_number) }
            }
            p class="text-sm text-warn mb-5" { (msg) }
            div class="flex justify-end" {
                a class="inline-flex items-center gap-1 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium no-underline cursor-pointer border-none hover:opacity-90"
                    href=(link_url) {
                    (link_label) (icon::arrow_right_icon("w-3.5 h-3.5"))
                }
            }
        }
    }
}

/// drawer 底部取消/提交（提交按钮在 form 内，type=submit）
fn drawer_footer(submit_label: &str) -> Markup {
    html! {
        div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
            button type="button"
                class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                _="on click remove .open from #wc-drawer-overlay" { "取消" }
            button type="submit"
                class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                { (submit_label) }
        }
    }
}

async fn receive_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let svc = state.arrival_notice_service();
    let an = svc.get(ctx, db, id).await?;
    let items = svc.list_items(ctx, db, id).await.unwrap_or_default();

    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        rows = html! {
            (rows)
            tr class="border-b border-border-soft" data-row {
                td class="py-2 px-2 text-sm text-fg" { "产品 #" (it.product_id) }
                td class="py-2 px-2 text-sm font-mono text-right" { (fmt_qty(it.declared_qty)) }
                td class="py-2 px-2 text-right" {
                    input type="hidden" data-k="item_id" name=(format!("items[{idx}][item_id]")) value=(it.id);
                    input type="number" data-k="received_qty" name=(format!("items[{idx}][received_qty]"))
                        value=(fmt_qty(it.declared_qty)) min="0" step="any"
                        class="w-20 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-bg";
                }
                td class="py-2 px-2" {
                    input type="text" data-k="batch_no" name=(format!("items[{idx}][batch_no]"))
                        value={(it.batch_no.as_deref().unwrap_or(""))}
                        class="w-24 px-2 py-1 border border-border rounded-sm text-sm font-mono bg-bg";
                }
            }
        };
    }

    let inner = html! {
        // 收货行级明细 → onsubmit 由 wcCollectItems 收成 items_json
        input type="hidden" name="items_json" value="[]";
        div class="mb-3" {
            span class="text-xs text-muted font-medium" { "来料通知 " }
            span class="text-sm font-mono font-semibold text-fg" { (an.doc_number) }
        }
        // 闭环提示：收货 ≠ 入库（入库在后续质检 Accepted 后）
        div class="mb-4 p-3 rounded-sm bg-warn-bg border border-warn/30" {
            p class="text-xs text-warn font-medium leading-relaxed" {
                "收货后单据进入「待质检」，质检通过后才正式入库并立应付账款。"
            }
        }
        table class="w-full border-collapse" {
            thead {
                tr {
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "产品" }
                    th class="text-right text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "申报" }
                    th class="text-right text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "实收" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "批次" }
                }
            }
            tbody { (rows) }
        }
        (drawer_footer("确认收货"))
    };
    Ok(drawer_form(
        "收货",
        "receive",
        id,
        WorkCenterDomain::Arrival,
        "确认收货？收货后进入待质检",
        "wcCollectItems(this)",
        inner,
    ))
}

async fn pick_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let svc = state.pick_list_service();
    let pl = svc.find_by_id(ctx, db, id).await?;
    let items = svc.list_items(ctx, db, id).await.unwrap_or_default();

    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        rows = html! {
            (rows)
            tr class="border-b border-border-soft" data-row {
                td class="py-2 px-2 text-sm text-fg" { "产品 #" (it.product_id) }
                td class="py-2 px-2 text-sm font-mono text-right" { (fmt_qty(it.requested_qty)) }
                td class="py-2 px-2 text-right" {
                    input type="hidden" data-k="pick_list_item_id" name=(format!("items[{idx}][pick_list_item_id]")) value=(it.id);
                    input type="number" data-k="picked_qty" name=(format!("items[{idx}][picked_qty]"))
                        value=(fmt_qty(it.picked_qty)) min="0" step="any"
                        class="w-20 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-bg";
                }
            }
        };
    }

    let inner = html! {
        input type="hidden" name="items_json" value="[]";
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
            tbody { (rows) }
        }
        (drawer_footer("确认拣货"))
    };
    Ok(drawer_form(
        "录入拣货",
        "pick",
        id,
        WorkCenterDomain::Pick,
        "确认拣货？",
        "wcCollectItems(this)",
        inner,
    ))
}

async fn ship_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let s = state.shipping_service().find_by_id(ctx, db, id).await?;
    if s.status == ShippingStatus::Picking {
        let inner = html! {
            div class="mb-3" {
                span class="text-xs text-muted font-medium" { "发货单 " }
                span class="text-sm font-mono font-semibold text-fg" { (s.doc_number) }
            }
            p class="text-sm text-muted mb-2" { "拣货已完成。确认发出将扣减库存、立应收账款并回写销售订单。" }
            (drawer_footer("确认发出"))
        };
        Ok(drawer_form(
            "确认发出",
            "ship",
            id,
            WorkCenterDomain::Outbound,
            "确认已发出？将扣减库存并立应收",
            "",
            inner,
        ))
    } else {
        // Confirmed 等未拣货：不能直接 ship，跳详情
        Ok(drawer_message(
            "未拣货",
            "发货单",
            &s.doc_number,
            "该单尚未拣货，无法直接发出。请先完成拣货。",
            &ShippingDetailPath { id }.to_string(),
            "去详情页拣货",
        ))
    }
}

async fn issue_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let req_svc = state.material_requisition_service();
    let req = req_svc.get(ctx, db, id).await?;
    if req.status == RequisitionStatus::Confirmed {
        let items = req_svc.list_items(ctx, db, id).await.unwrap_or_default();
        let mut rows = html! {};
        for it in &items {
            rows = html! {
                (rows)
                div class="flex items-center justify-between px-3 py-2" {
                    span class="text-sm text-fg-2" { "产品 #" (it.product_id) }
                    span class="text-sm font-mono text-muted" { "申请 " (fmt_qty(it.requested_qty)) }
                }
            };
        }
        let inner = html! {
            div class="mb-3" {
                span class="text-xs text-muted font-medium" { "领料单 " }
                span class="text-sm font-mono font-semibold text-fg" { (req.doc_number) }
            }
            p class="text-sm text-muted mb-4" { "共 " (items.len()) " 项，将按申请量全量发料。" }
            div class="rounded-sm border border-border-soft divide-y divide-border-soft mb-4" { (rows) }
            (drawer_footer("确认发料"))
        };
        Ok(drawer_form(
            "发料",
            "issue",
            id,
            WorkCenterDomain::Requisition,
            "确认全量发料？将扣减库存并计入工单成本",
            "",
            inner,
        ))
    } else {
        // PartiallyIssued 等：issue 记绝对量，就地重复发料会重复扣库存 → 跳详情
        Ok(drawer_message(
            "发料",
            "领料单",
            &req.doc_number,
            "该单已部分发料。继续发料请在详情页操作（避免重复扣库存）。",
            &RequisitionDetailPath { id }.to_string(),
            "去详情页发料",
        ))
    }
}

async fn transfer_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let trf = state.transfer_service().get(ctx, db, id).await?;
    let items = state.transfer_service().get_items(ctx, db, id).await.unwrap_or_default();
    let (title, action, hint, btn_label) = match trf.status {
        TransferStatus::Draft => ("调出", "dispatch", "确认调出将从源仓扣减库存、单据进入在途。", "确认调出"),
        TransferStatus::InTransit => ("到货确认", "complete", "确认到货将把库存计入目标仓、完成调拨。", "确认到货"),
        _ => ("调拨", "complete", "该单当前状态不可就地操作。", "确认"),
    };
    let inner = html! {
        div class="mb-3" {
            span class="text-xs text-muted font-medium" { "调拨单 " }
            span class="text-sm font-mono font-semibold text-fg" { (trf.doc_number) }
        }
        p class="text-sm text-muted mb-2" { "仓 " (trf.from_warehouse_id) " → " (trf.to_warehouse_id) " · 共 " (items.len()) " 项" }
        p class="text-sm text-muted mb-5" { (hint) }
        (drawer_footer(btn_label))
    };
    Ok(drawer_form(
        title,
        action,
        id,
        WorkCenterDomain::Transfer,
        "确认执行此调拨操作？",
        "",
        inner,
    ))
}
