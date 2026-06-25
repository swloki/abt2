use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::shared::types::{PgExecutor, ServiceContext};
use abt_core::wms::enums::{RequisitionStatus, TransferStatus};
use abt_core::wms::material_requisition::model::{IssueItemReq, IssueMaterialReq};
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::wms::outbound::model::ShippingStatus;
use abt_core::wms::outbound::ShippingRequestService;
use abt_core::wms::pick_list::{model::PickItemInput, PickListService};
use abt_core::wms::transfer::TransferService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::work_center::model::{
    PendingTask, TaskSourceKind, Urgency, UrgentSummary, WorkCenterDomain, WorkCenterSummary,
};
use abt_core::wms::work_center::WorkCenterService;
use abt_core::shared::document_sequence::DocumentSequenceService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::enums::DocumentType;
use abt_core::wms::enums::TransactionType;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::components::icon;
use abt_core::wms::stock_in::{PoStockInRow, PurchaseStockInService, ReceiveAndStockInReq};
use abt_core::wms::inventory_transaction::{model::RecordTransactionReq, InventoryTransactionService};
use abt_core::mes::work_order::WorkOrderService;
use crate::errors::Result;
use abt_core::shared::types::error::DomainError;
use crate::layout::page::admin_page;
use crate::routes::shipping::ShippingDetailPath;
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

/// 就地操作提交：action 决定分发，id 目标单据，items_json（收货/拣货行级明细，JSON 字符串）。
/// idempotency_key 仅收货入库用（防双击重复入库），其他 action 不传 = None。
#[derive(Debug, Deserialize)]
pub struct WorkCenterActionForm {
    pub action: String,
    pub id: i64,
    pub items_json: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

// ── domain ↔ slug / 动作 映射 ──

fn domain_from_str(s: &str) -> Option<WorkCenterDomain> {
    match s {
        "arrival" => Some(WorkCenterDomain::Arrival),
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
        "receive_po" | "receive_wo" => WorkCenterDomain::Arrival,
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
        // Arrival（PO/工单）详情按 source_kind 在 render_task_row 拼，这里返回 None
        WorkCenterDomain::Arrival => None,
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
        "receive_po" => {
            // 采购 PO 直收入库闭环（取消来料通知后）：receive_and_stock_in 事务内
            // record 库存 + 回写 PO received_qty/状态 + 立应付 + 成本。幂等由 service 内 try_claim。
            let rows: Vec<ReceiveRowJson> = parse_items_json(form)?;
            let po_rows: Vec<PoStockInRow> = rows
                .into_iter()
                .map(|r| -> Result<PoStockInRow> {
                    Ok(PoStockInRow {
                        order_item_id: r
                            .order_item_id
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| DomainError::validation("缺少订单明细行 order_item_id"))?
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("order_item_id 解析失败: {e}")))?,
                        product_id: r
                            .product_id
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("product_id 解析失败: {e}")))?,
                        received_qty: r
                            .received_qty
                            .parse::<Decimal>()
                            .map_err(|e| DomainError::validation(format!("收货数量解析失败: {e}")))?,
                        batch_no: r.batch_no.filter(|s| !s.is_empty()),
                        warehouse_id: r
                            .warehouse_id
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| DomainError::validation("每行必须选择目标仓库"))?
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("仓库解析失败: {e}")))?,
                        bin_id: parse_opt_i64(&r.bin_id, "目标库位")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            state
                .purchase_stock_in_service()
                .receive_and_stock_in(
                    ctx,
                    db,
                    ReceiveAndStockInReq {
                        po_id: form.id,
                        rows: po_rows,
                        delivery_note: None,
                        remark: None,
                        idempotency_key: form.idempotency_key.clone(),
                    },
                )
                .await?;
        }
        "receive_wo" => {
            // 生产工单入库：仅 record 库存（source=work_order），不立应付、不回写 completed_qty（报工已累加）
            let rows: Vec<ReceiveRowJson> = parse_items_json(form)?;
            let inv_svc = state.inventory_transaction_service();
            let wh_svc = state.warehouse_service();
            let wo = state.work_order_service().find_by_id(ctx, db, form.id).await?;
            let doc_number = state
                .document_sequence_service()
                .next_number(ctx, db, DocumentType::StockReceipt)
                .await?;
            for r in rows {
                let product_id = r
                    .product_id
                    .parse::<i64>()
                    .map_err(|e| DomainError::validation(format!("product_id 解析失败: {e}")))?;
                let qty = r
                    .received_qty
                    .parse::<Decimal>()
                    .map_err(|e| DomainError::validation(format!("收货数量解析失败: {e}")))?;
                let warehouse_id = r
                    .warehouse_id
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| DomainError::validation("必须选择目标仓库"))?
                    .parse::<i64>()
                    .map_err(|e| DomainError::validation(format!("仓库解析失败: {e}")))?;
                let bin_id = parse_opt_i64(&r.bin_id, "目标库位")?;
                let zone_id = wh_svc
                    .get_or_create_default_zone(ctx, db, warehouse_id)
                    .await
                    .ok()
                    .map(|z| z.id);
                let default_bin = if let Some(zid) = zone_id {
                    wh_svc
                        .list_bins(ctx, db, zid, None, 1, 1)
                        .await
                        .ok()
                        .and_then(|x| x.items.first().map(|b| b.id))
                } else {
                    None
                };
                inv_svc
                    .record(
                        ctx,
                        db,
                        RecordTransactionReq {
                            doc_number: Some(doc_number.clone()),
                            delivery_no: None,
                            source_doc_number: Some(wo.doc_number.clone()),
                            transaction_type: TransactionType::ProductionReceipt,
                            product_id,
                            warehouse_id,
                            zone_id,
                            bin_id: bin_id.or(default_bin),
                            batch_no: r.batch_no.filter(|s| !s.is_empty()),
                            quantity: qty,
                            unit_cost: None,
                            source_type: "work_order".to_string(),
                            source_id: form.id,
                            remark: None,
                        },
                    )
                    .await?;
            }
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
                        warehouse_id: parse_opt_i64(&r.warehouse_id, "拣货仓库")?,
                        bin_id: parse_opt_i64(&r.bin_id, "拣货库位")?,
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

/// 可选整型解析：None / 空串 → None；否则 parse。用于拣货仓库/库位（wcCollectItems 收的是字符串）。
fn parse_opt_i64(s: &Option<String>, label: &str) -> Result<Option<i64>> {
    match s {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|e| DomainError::validation(format!("{label}解析失败: {e}")).into()),
    }
}

// 行级明细走 hidden items_json（JSON 字符串），字段统一用 String（i.value 为字符串），服务端再 parse
// 对齐 quotation/sales_order 的 ItemWeb 范式（见 static/app.js lineItemCalc.collectItems）
/// 收货 drawer 行级明细（直入库：每行带目标仓库/库位，提交走 stock_in_from_notice）
#[derive(Debug, Deserialize)]
struct ReceiveRowJson {
    /// 采购明细行 id（receive_po 必填；receive_wo 工单入库不用 = None）
    order_item_id: Option<String>,
    product_id: String,
    received_qty: String,
    batch_no: Option<String>,
    warehouse_id: Option<String>,
    bin_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PickRowJson {
    pick_list_item_id: String,
    picked_qty: String,
    /// 拣货定仓库（master：拣货选仓库/库位）；空 = 不指定
    warehouse_id: Option<String>,
    bin_id: Option<String>,
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
        (render_card(WorkCenterDomain::Pick, summary.picks_pending, None))
        (render_card(WorkCenterDomain::Outbound, summary.outbounds_pending, None))
        (render_card(WorkCenterDomain::Requisition, summary.requisitions_pending, None))
        (render_card(WorkCenterDomain::Transfer, summary.transfers_pending, None))
        (render_card(WorkCenterDomain::CycleCount, summary.cycle_counts_pending, None))
        // 共享 drawer overlay（各域 GET ?drawer=&id= 把 body 填入 #wc-drawer-body）
        (wc_drawer_shell())
        // 库位选择弹窗（复用 stock-in/create 的 suggest_bins 端点；收货 drawer 选目标库位）
        (wc_bin_picker_shell())
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
                (render_row_action(domain, t.doc_id, t.source_kind))
            }
        }
    }
}

/// 行内操作入口：拣货/收货/发货/领料/调拨 → hx-get 加载 drawer body；质检/盘点 → 跳详情页
fn render_row_action(domain: WorkCenterDomain, doc_id: i64, source_kind: TaskSourceKind) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay";
    match domain {
        WorkCenterDomain::Arrival => match source_kind {
            TaskSourceKind::PurchaseOrder => drawer_btn("收货", "receive_po", doc_id, icon::truck_icon("w-3 h-3"), open_hs),
            TaskSourceKind::WorkOrder => drawer_btn("入库", "receive_wo", doc_id, icon::package_icon("w-3 h-3"), open_hs),
        },
        WorkCenterDomain::Pick => drawer_btn("拣货", "pick", doc_id, icon::plus_icon("w-3 h-3"), open_hs),
        WorkCenterDomain::Outbound => drawer_btn("发货", "ship", doc_id, icon::upload_icon("w-3 h-3"), open_hs),
        WorkCenterDomain::Requisition => {
            drawer_btn("发料", "issue", doc_id, icon::clipboard_list_icon("w-3 h-3"), open_hs)
        }
        WorkCenterDomain::Transfer => {
            drawer_btn("办理", "transfer", doc_id, icon::arrow_right_icon("w-3 h-3"), open_hs)
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

/// 库位选择弹窗壳：复用 stock-in/create 的 suggest_bins 端点（按产品+仓库 SameMerge 推荐）。
/// z-[1001] 盖在 drawer overlay（z-[1000]）之上；× / 背景点击关闭。
fn wc_bin_picker_shell() -> Markup {
    html! {
        div id="bin-picker"
            class="fixed inset-0 z-[1001] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            _="on click[me is event.target] remove .is-open" {
            div class="modal bg-bg rounded-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="font-bold text-base text-fg" { "选择入库库位" }
                    button type="button"
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                        _="on click remove .is-open from #bin-picker" { "×" }
                }
                div id="bin-picker-results" class="overflow-y-auto flex-1 min-h-0" {
                    div class="text-center text-muted py-10 text-sm" { "点击物料行的「自动分配」加载推荐库位…" }
                }
            }
        }
    }
}

// ── drawer body（GET ?drawer=&id=）：按 action 渲染表单，提交走单端点 POST ──

async fn render_drawer_body(action: &str, id: i64, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let body = match action {
        "receive_po" => po_receive_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "receive_wo" => wo_receive_drawer_body(&state, &service_ctx, &mut conn, id).await?,
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

async fn po_receive_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let po_svc = state.purchase_order_service();
    let po = po_svc.get(ctx, db, id).await?;
    let items = po_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();

    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        let pending = it.quantity - it.received_qty;
        if pending <= Decimal::ZERO {
            continue; // 已收完的行跳过
        }
        rows = html! {
            (rows)
            div class="border-b border-border-soft py-3" data-row {
                div class="flex items-start justify-between mb-2 gap-2" {
                    div class="min-w-0" {
                        div class="text-sm text-fg font-medium truncate" {
                            (product_map.get(&it.product_id).map(|p| p.pdt_name.clone()).unwrap_or_else(|| format!("产品 #{}", it.product_id)))
                        }
                        div class="text-xs text-muted truncate" {
                            (product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default())
                        }
                    }
                    span class="text-xs text-muted shrink-0 mt-0.5" { "待收 " (fmt_qty(pending)) }
                }
                input type="hidden" data-k="order_item_id" name=(format!("items[{idx}][order_item_id]")) value=(it.id);
                input type="hidden" data-k="product_id" name=(format!("items[{idx}][product_id]")) value=(it.product_id);
                div class="grid grid-cols-2 gap-2 mb-2" {
                    div {
                        label class="block text-xs text-muted mb-1" { "实收" }
                        input type="number" data-k="received_qty" name=(format!("items[{idx}][received_qty]"))
                            value=(fmt_qty(pending)) min="0" step="any"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm font-mono text-right bg-bg";
                    }
                    div {
                        label class="block text-xs text-muted mb-1" { "批次" }
                        input type="text" data-k="batch_no" name=(format!("items[{idx}][batch_no]"))
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm font-mono bg-bg";
                    }
                }
                div class="grid grid-cols-2 gap-2" {
                    div {
                        label class="block text-xs text-muted mb-1" { "目标仓库 " span class="text-danger" { "*" } }
                        select data-k="warehouse_id" name=(format!("items[{idx}][warehouse_id]"))
                            _="on change call wcResetBin(me)"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg" {
                            option value="" disabled selected { "选择仓库" }
                            @for w in &warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div {
                        label class="block text-xs text-muted mb-1" { "目标库位" }
                        input type="hidden" data-k="bin_id" name=(format!("items[{idx}][bin_id]")) value="";
                        button type="button"
                            _="on click call wcOpenBinPicker(me)"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg text-fg-2 hover:bg-surface truncate text-left" {
                            span class="bin-label" { "自动分配" }
                        }
                    }
                }
            }
        };
    }

    let inner = html! {
        // 幂等键：drawer body 加载时生成（防双击重复入库），顶层字段不进 items_json
        input type="hidden" name="idempotency_key"
            _="on load js me.value = crypto.randomUUID?.() || (Date.now()+Math.random()).toString(36) end" {};
        input type="hidden" name="items_json" value="[]";
        div class="mb-3" {
            span class="text-xs text-muted font-medium" { "采购订单 " }
            span class="text-sm font-mono font-semibold text-fg" { (po.doc_number) }
        }
        div class="mb-4 p-3 rounded-sm bg-accent-bg border border-accent/30" {
            p class="text-xs text-accent font-medium leading-relaxed" {
                "确认后直接入库，并自动回写采购订单收货量、立应付账款。"
            }
        }
        (rows)
        (drawer_footer("确认入库"))
    };
    Ok(drawer_form(
        "采购收货入库",
        "receive_po",
        id,
        WorkCenterDomain::Arrival,
        "确认收货入库？将直接入库并回写采购订单",
        "wcReceiveSubmit(this)",
        inner,
    ))
}

/// 生产工单入库 drawer：完工产品（completed_qty - 已入库量）上架，仅记库存（不立应付、不回写工单完工量）
async fn wo_receive_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let wo = state.work_order_service().find_by_id(ctx, db, id).await?;
    let product = state.product_service().get(ctx, db, wo.product_id).await?;
    let received: Decimal = state
        .inventory_transaction_service()
        .find_by_source(ctx, db, "work_order", id)
        .await
        .unwrap_or_default()
        .iter()
        .map(|t| t.quantity)
        .sum();
    let pending = wo.completed_qty - received;
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let body = html! {
        input type="hidden" name="items_json" value="[]";
        div class="mb-3" {
            span class="text-xs text-muted font-medium" { "生产工单 " }
            span class="text-sm font-mono font-semibold text-fg" { (wo.doc_number) }
        }
        div class="mb-3 text-xs text-muted" {
            "完工 " (fmt_qty(wo.completed_qty)) " · 已入库 " (fmt_qty(received)) " · 待入库 "
            span class="text-fg font-medium" { (fmt_qty(pending)) }
        }
        @if pending <= Decimal::ZERO {
            div class="mb-4 p-3 rounded-sm bg-warn-bg border border-warn/30" {
                p class="text-xs text-warn font-medium" { "该工单完工产品已全部入库，无需操作。" }
            }
        } @else {
            div class="border-b border-border-soft py-3" data-row {
                div class="flex items-start justify-between mb-2 gap-2" {
                    div class="min-w-0" {
                        div class="text-sm text-fg font-medium truncate" { (product.pdt_name) }
                        div class="text-xs text-muted truncate" { (product.product_code) }
                    }
                }
                input type="hidden" data-k="product_id" name="items[0][product_id]" value=(wo.product_id);
                div class="grid grid-cols-2 gap-2 mb-2" {
                    div {
                        label class="block text-xs text-muted mb-1" { "入库量" }
                        input type="number" data-k="received_qty" name="items[0][received_qty]"
                            value=(fmt_qty(pending)) min="0" step="any"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm font-mono text-right bg-bg";
                    }
                    div {
                        label class="block text-xs text-muted mb-1" { "批次" }
                        input type="text" data-k="batch_no" name="items[0][batch_no]"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm font-mono bg-bg";
                    }
                }
                div class="grid grid-cols-2 gap-2" {
                    div {
                        label class="block text-xs text-muted mb-1" { "目标仓库 " span class="text-danger" { "*" } }
                        select data-k="warehouse_id" name="items[0][warehouse_id]"
                            _="on change call wcResetBin(me)"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg" {
                            option value="" disabled selected { "选择仓库" }
                            @for w in &warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div {
                        label class="block text-xs text-muted mb-1" { "目标库位" }
                        input type="hidden" data-k="bin_id" name="items[0][bin_id]" value="";
                        button type="button"
                            _="on click call wcOpenBinPicker(me)"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg text-fg-2 hover:bg-surface truncate text-left" {
                            span class="bin-label" { "自动分配" }
                        }
                    }
                }
            }
            div class="mb-4 p-3 rounded-sm bg-accent-bg border border-accent/30" {
                p class="text-xs text-accent font-medium leading-relaxed" {
                    "生产入库仅登记库存（不计应付、不回写工单完工量——报工时已累加）。"
                }
            }
            (drawer_footer("确认入库"))
        }
    };
    Ok(drawer_form(
        "生产入库",
        "receive_wo",
        id,
        WorkCenterDomain::Arrival,
        "确认生产入库？",
        "wcReceiveSubmit(this)",
        body,
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
    // 拣货定仓库（master）：列出仓库供每行选择 + 可选库位
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();

    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        rows = html! {
            (rows)
            tr class="border-b border-border-soft" data-row {
                td class="py-2 px-2 text-sm text-fg" {
                    div class="truncate" {
                        (product_map.get(&it.product_id).map(|p| p.pdt_name.clone()).unwrap_or_else(|| format!("产品 #{}", it.product_id)))
                    }
                    div class="text-xs text-muted truncate" {
                        (product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default())
                    }
                }
                td class="py-2 px-2 text-sm font-mono text-right" { (fmt_qty(it.requested_qty)) }
                td class="py-2 px-2" {
                    select data-k="warehouse_id" name=(format!("items[{idx}][warehouse_id]"))
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg mb-1" {
                        option value="" { "选择仓库" }
                        @for w in &warehouses {
                            option value=(w.id) { (w.name) }
                        }
                    }
                    input type="number" data-k="bin_id" name=(format!("items[{idx}][bin_id]")) placeholder="库位ID 可选"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-bg";
                }
                td class="py-2 px-2 text-right" {
                    input type="hidden" data-k="pick_list_item_id" name=(format!("items[{idx}][pick_list_item_id]")) value=(it.id);
                    input type="number" data-k="picked_qty" name=(format!("items[{idx}][picked_qty]"))
                        value=(fmt_qty(it.picked_qty)) min="0" step="any"
                        class="w-24 px-3 py-2 border border-border rounded-sm text-sm font-mono text-right bg-bg";
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
                    th class="text-left text-xs font-semibold text-muted py-2 px-2 border-b border-border-soft" { "仓库 / 库位" }
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
    if s.status != ShippingStatus::Picking {
        // Confirmed 等未拣货：不能直接 ship，跳详情
        return Ok(drawer_message(
            "未拣货",
            "发货单",
            &s.doc_number,
            "该单尚未拣货，无法直接发出。请先完成拣货。",
            &ShippingDetailPath { id }.to_string(),
            "去详情页拣货",
        ));
    }

    let items = state.shipping_service().list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let customer = state.customer_service().get(ctx, db, s.customer_id).await.ok();
    // 拣货明细：shipping_item.id → (warehouse_id, bin_id, picked_qty)，ship 扣库存即用此仓库/库位
    let pick_svc = state.pick_list_service();
    let mut pick_map: HashMap<i64, (Option<i64>, Option<i64>, Decimal)> = HashMap::new();
    if let Ok(Some(pl)) = pick_svc.find_by_outbound(ctx, db, id).await
        && let Ok(pick_items) = pick_svc.list_items(ctx, db, pl.id).await
    {
        for p in pick_items {
            pick_map.insert(p.outbound_item_id, (p.warehouse_id, p.bin_id, p.picked_qty));
        }
    }
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let total_qty: Decimal = items.iter().map(|i| i.requested_qty).sum();

    let mut rows = html! {};
    for it in &items {
        let prod = product_map.get(&it.product_id);
        let prod_name = prod
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let prod_code = prod.map(|p| p.product_code.clone()).unwrap_or_default();
        let (pwh, pbin, ppicked) = pick_map
            .get(&it.id)
            .cloned()
            .unwrap_or((None, None, Decimal::ZERO));
        let wh_label = match pwh {
            Some(w) => warehouses
                .iter()
                .find(|x| x.id == w)
                .map(|x| x.name.clone())
                .unwrap_or_else(|| format!("#{w}")),
            None => "—".to_string(),
        };
        rows = html! {
            (rows)
            div class="border-b border-border-soft py-3" {
                div class="flex items-start justify-between mb-1 gap-2" {
                    div class="min-w-0" {
                        div class="text-sm text-fg font-medium truncate" { (prod_name) }
                        div class="text-xs text-muted truncate" { (prod_code) }
                    }
                    span class="text-xs text-muted shrink-0 mt-0.5" { "本次 " (fmt_qty(it.requested_qty)) }
                }
                div class="text-xs text-muted flex items-center gap-3 flex-wrap" {
                    span { "申请 " (fmt_qty(it.requested_qty)) }
                    span { "已发 " (fmt_qty(it.shipped_qty)) }
                    @if ppicked > Decimal::ZERO {
                        span { "已拣 " (fmt_qty(ppicked)) }
                    }
                }
                div class="text-xs text-fg-2 mt-1 flex items-center gap-1" {
                    (icon::package_icon("w-3 h-3"))
                    "拣货仓 " (wh_label)
                    @if let Some(b) = pbin { " · 库位 #" (b) }
                }
            }
        };
    }

    let inner = html! {
        div class="mb-3 flex items-baseline gap-2 flex-wrap" {
            span class="text-xs text-muted font-medium" { "发货单 " }
            span class="text-sm font-mono font-semibold text-fg" { (s.doc_number) }
        }
        div class="mb-4 grid grid-cols-2 gap-3 text-xs" {
            div {
                span class="text-muted" { "客户 " }
                span class="text-fg-2" { (customer.as_ref().map(|c| c.name.as_str()).unwrap_or("—")) }
            }
            div {
                span class="text-muted" { "预计发货 " }
                span class="text-fg-2 font-mono" {
                    (s.expected_ship_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                }
            }
        }
        div class="mb-4 p-3 rounded-sm bg-warn-bg border border-warn/30" {
            p class="text-xs text-warn font-medium leading-relaxed" {
                "确认发出将按拣货仓库扣减库存、立应收账款并回写销售订单。"
            }
        }
        (rows)
        div class="mt-3 flex items-center justify-between text-xs text-muted" {
            span { "共 " (items.len()) " 项" }
            span class="font-mono" { "本次发出合计 " (fmt_qty(total_qty)) }
        }
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
        let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
            .product_service()
            .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect();
        let mut rows = html! {};
        for it in &items {
            rows = html! {
                (rows)
                div class="flex items-center justify-between px-3 py-2 gap-2" {
                    div class="min-w-0" {
                        div class="text-sm text-fg-2 truncate" {
                            (product_map.get(&it.product_id).map(|p| p.pdt_name.clone()).unwrap_or_else(|| format!("产品 #{}", it.product_id)))
                        }
                        div class="text-xs text-muted truncate" {
                            (product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default())
                        }
                    }
                    span class="text-sm font-mono text-muted shrink-0" { "申请 " (fmt_qty(it.requested_qty)) }
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
