use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::bom::BomQueryService;
use abt_core::mes::enums::{BatchStatus, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{ProductionBatch, ProductionBatchService, SplitReq, WorkOrderRouting};
use abt_core::mes::work_report::{ReportListFilter, ReportListItem, WorkReportService};
use abt_core::mes::work_order::{WorkOrder, WorkOrderService};
use abt_core::shared::audit_log::{AuditLog, AuditLogQuery, AuditLogService};
use abt_core::shared::enums::audit::AuditAction;

use crate::components::detail::{detail_tabs, tab_panel};
use crate::components::{drawer, icon, product_picker, routing_picker};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{
 OrderCancelPath, OrderClosePath, OrderDetailPath, OrderListPath, OrderReleasePath,
 OrderRoutingApplyFromRoutingPath, OrderRoutingDeletePath, OrderRoutingEditPath,
 OrderRoutingLoadRecentPath, OrderSplitPath, OrderUnreleasePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn wo_status_label(s: &WorkOrderStatus) -> (&'static str, &'static str, &'static str) {
 use WorkOrderStatus::*;
 match s {
 Draft => ("待计划", "rgba(0,0,0,0.04)", "var(--muted)"),
 Planned => ("已计划", "rgba(22,119,255,0.08)", "var(--accent)"),
 Released => ("已下达", "rgba(82,196,26,0.08)", "var(--success)"),
 InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
 Closed => ("已关闭", "rgba(114,46,209,0.08)", "#722ed1"),
 Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
 }
}

fn status_pill(label: &str, bg: &str, color: &str) -> Markup {
 html! {
 span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium whitespace-nowrap"
 style=(format!("background:{bg};color:{color}")) {
 (label)
 }
 }
}

fn batch_status_pill(s: BatchStatus) -> Markup {
 let (l, bg, c) = match s {
 BatchStatus::Pending => ("待生产", "rgba(0,0,0,0.04)", "var(--muted)"),
 BatchStatus::InProgress => ("进行中", "rgba(22,119,255,0.08)", "var(--accent)"),
 BatchStatus::Suspended => ("已暂停", "rgba(250,140,22,0.08)", "#fa8c16"),
 BatchStatus::PendingReceipt => ("待入库", "rgba(22,119,255,0.08)", "var(--accent)"),
 BatchStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
 BatchStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
 };
 status_pill(l, bg, c)
}

fn shift_label(s: ShiftType) -> &'static str {
 match s {
 ShiftType::Day => "白班",
 ShiftType::Night => "夜班",
 }
}

fn audit_action_label(a: AuditAction) -> &'static str {
 match a {
 AuditAction::Create => "创建",
 AuditAction::Update => "更新",
 AuditAction::Delete => "删除",
 AuditAction::Transition => "状态流转",
 }
}

fn fmt_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
 dt.format("%Y-%m-%d %H:%M").to_string()
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_detail(
 path: OrderDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 claims,
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let wo_svc = state.work_order_service();
 let batch_svc = state.production_batch_service();
 let report_svc = state.work_report_service();
 let audit_svc = state.audit_log_service();

 let order = wo_svc
 .find_by_id(&service_ctx, &mut conn, path.id)
 .await?;

 let product_name = wo_svc
 .get_product_name(&mut conn, order.product_id)
 .await?
 .unwrap_or_default();

 // 工序明细
 let routings = batch_svc
 .list_routings(&service_ctx, &mut conn, order.id)
 .await
 .unwrap_or_default();

 // 生产批次
 let batches = batch_svc
 .list_by_work_order(&service_ctx, &mut conn, order.id)
 .await
 .unwrap_or_default();

 // 报工记录
 let reports = report_svc
 .list(
 &service_ctx,
 &mut conn,
 ReportListFilter {
 work_order_id: Some(order.id),
 ..Default::default()
 },
 1,
 100,
 )
 .await
 .map(|p| p.items)
 .unwrap_or_default();

 // 操作日志
 let audit_logs = audit_svc
 .query_logs(
 &service_ctx,
 &mut conn,
 AuditLogQuery {
 entity_type: Some("WorkOrder".to_string()),
 entity_id: Some(order.id),
 ..Default::default()
 },
 1,
 50,
 )
 .await
 .map(|p| p.items)
 .unwrap_or_default();
 // 完工率
 let completion_pct = if order.planned_qty > rust_decimal::Decimal::ZERO {
 ((order.completed_qty / order.planned_qty) * rust_decimal::Decimal::ONE_HUNDRED)
 .min(rust_decimal::Decimal::ONE_HUNDRED)
 } else {
 rust_decimal::Decimal::ZERO
 };

 // 是否有入库记录
 let has_receipts = order.completed_qty > rust_decimal::Decimal::ZERO;

 // 已报工 routing_id 集合 + 整单是否有报工（决定工序行的改价/删除可编辑态）
 let reported_routing_ids: std::collections::HashSet<i64> =
 reports.iter().map(|r| r.routing_id).collect();
 let order_has_report = !reports.is_empty();

 // 批量取工序产出品名（行展示用）
 let product_ids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
 let product_names: std::collections::HashMap<i64, String> = if product_ids.is_empty() {
 std::collections::HashMap::new()
 } else {
 use abt_core::master_data::product::ProductService;
 state.product_service().get_by_ids(&service_ctx, &mut conn, product_ids).await
 .unwrap_or_default().into_iter().map(|p| (p.product_id, p.pdt_name)).collect()
 };
	 // 批量取工作中心名（行展示用）
	 use abt_core::master_data::work_center::WorkCenterService;
	 let work_center_names: std::collections::HashMap<i64, String> = state.work_center_service()
	     .list_active(&service_ctx, &mut conn).await
	     .unwrap_or_default().into_iter().map(|wc| (wc.id, wc.name)).collect();

 let content = order_detail_page(
 &order, &product_name, &routings, &batches, &reports, &audit_logs,
 completion_pct, has_receipts, &reported_routing_ids, order_has_report, &product_names, &work_center_names,
 );
 let page_html = admin_page(
 is_htmx,
 "工单详情",
 &claims,
 "production",
 &format!("/admin/mes/orders/{}", path.id),
 "生产管理",
 Some(OrderListPath::PATH),
 content,
 &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn release_order(
 path: OrderReleasePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.work_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

 if order.status == WorkOrderStatus::Released {
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 return Ok(([("HX-Redirect", redirect)], Html(String::new())));
 }

 svc.release(&service_ctx, &mut conn, path.order_id, order.version)
 .await?;
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn unrelease_order(
 path: OrderUnreleasePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.work_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

 // 幂等：已是草稿则直接重定向
 if order.status == WorkOrderStatus::Draft {
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 return Ok(([("HX-Redirect", redirect)], Html(String::new())));
 }

 svc.unrelease(&service_ctx, &mut conn, path.order_id, order.version)
 .await?;
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn close_order(
 path: OrderClosePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.work_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

 if order.status == WorkOrderStatus::Closed {
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 return Ok(([("HX-Redirect", redirect)], Html(String::new())));
 }

 svc.close(&service_ctx, &mut conn, path.order_id, order.version)
 .await?;
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn cancel_order(
 path: OrderCancelPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.work_order_service();
 let order = svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;

 if order.status == WorkOrderStatus::Cancelled {
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 return Ok(([("HX-Redirect", redirect)], Html(String::new())));
 }

 svc.cancel(&service_ctx, &mut conn, path.order_id, order.version)
 .await?;
 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct SplitForm {
 pub split_qty: String,
}

/// 拆批：从工单创建额外的生产批次
#[require_permission("WORK_ORDER", "update")]
pub async fn split_order(
 path: OrderSplitPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<SplitForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let batch_svc = state.production_batch_service();

 let split_qty = form.split_qty.parse::<rust_decimal::Decimal>()
 .map_err(|_| abt_core::shared::types::DomainError::validation("数量格式错误"))?;

 if split_qty <= rust_decimal::Decimal::ZERO {
 return Err(abt_core::shared::types::DomainError::validation("拆批数量必须大于 0").into());
 }
 batch_svc.split_work_order(
 &service_ctx, &mut conn, path.order_id,
 vec![SplitReq { batch_qty: split_qty, team_id: None }],
 ).await?;

 let redirect = OrderDetailPath { id: path.order_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct RoutingEditForm {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
    pub unit_price: rust_decimal::Decimal,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub work_center_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub standard_time: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub is_outsourced: bool,
}

/// 解析产出品名（无则 #id 或空）
async fn resolve_product_name(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    pid: Option<i64>,
) -> String {
    use abt_core::master_data::product::ProductService;
    match pid {
        Some(id) => state.product_service().get_by_ids(ctx, db, vec![id]).await
            .ok().and_then(|v| v.into_iter().next())
            .map(|p| p.pdt_name).unwrap_or_else(|| format!("#{}", id)),
        None => String::new(),
    }
}

/// GET：返回编辑抽屉表单（product picker + 单价预填）
#[require_permission("WORK_ORDER", "update")]
pub async fn get_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    use abt_core::master_data::product::ProductService;
    use abt_core::master_data::work_center::WorkCenterService;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    let routing = routings.iter().find(|r| r.id == path.routing_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
    let pname = resolve_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
    let work_centers = state.work_center_service().list_active(&service_ctx, &mut conn).await.unwrap_or_default();
    // 获取工单 BOM 信息，渲染层级产品选择器
    let bom_modal: Markup = {
        let wo = state.work_order_service().find_by_id(&service_ctx, &mut conn, path.order_id).await?;
        // 优先使用工单关联的 BOM 快照
        let bom_detail = if let Some(snapshot_id) = wo.bom_snapshot_id {
            state.bom_query_service().get_snapshot_by_id(&service_ctx, &mut conn, snapshot_id).await.ok()
                .flatten()
                .map(|s| s.bom_detail)
        } else {
            None
        };
        // 备选：通过产品编码反查已发布 BOM
        let bom_detail = match bom_detail {
            Some(d) => Some(d),
            None => {
                if let Some(product) = state.product_service().get_by_ids(&service_ctx, &mut conn, vec![wo.product_id]).await.ok()
                    .and_then(|v| v.into_iter().next())
                {
                    if let Some(bom_id) = state.bom_query_service().find_published_bom_by_product_code(
                        &service_ctx, &mut conn, &product.product_code,
                    ).await.ok().flatten() {
                        state.bom_query_service().get(&service_ctx, &mut conn, bom_id).await.ok().map(|b| b.bom_detail)
                    } else { None }
                } else { None }
            }
        };
        match bom_detail {
            Some(detail) => {
                // 收集所有 product_id
                let all_pids: Vec<i64> = detail.nodes.iter().map(|n| n.product_id).collect();
                let products = state.product_service().get_by_ids(&service_ctx, &mut conn, all_pids).await.unwrap_or_default();
                let product_map: std::collections::HashMap<i64, &abt_core::master_data::product::model::Product> = products.iter().map(|p| (p.product_id, p)).collect();
                routing_bom_tree_modal(&detail, &product_map)
            }
            None => {
                // 无 BOM，使用通用产品选择器
                product_picker::product_picker_modal_with_bom_filter(
                    "routing-product-modal",
                    "routing-product-id",
                    "routing-product-display",
                    None,
                )
            }
        }
    };
    Ok(Html(html! {
        (routing_edit_form(path.order_id, path.routing_id, routing, &pname, &work_centers))
        div hx-swap-oob="innerHTML:#routing-product-modal-container" {
            (bom_modal)
        }
    }.into_string()))
}

/// POST：保存 product_id + unit_price → 关抽屉 + 触发 routingChanged 重载 data-card
#[require_permission("WORK_ORDER", "update")]
pub async fn post_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingEditForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    match svc.update_routing(&service_ctx, &mut conn, path.order_id, path.routing_id, form.product_id, form.unit_price, form.work_center_id, form.standard_time, form.is_outsourced).await {
        Ok(_updated) => {
            // 触发 data-card 重载，drawer 由 hx-on::after-request 关闭
            Ok(([("HX-Trigger", "routingChanged")], Html(String::new())))
        }
        Err(_) => {
            // 失败：OOB swap 渲染错误表单到 drawer body（不关抽屉）
            use abt_core::master_data::work_center::WorkCenterService;
            let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
            let routing = routings.iter().find(|r| r.id == path.routing_id)
                .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
            let pname = resolve_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
            let work_centers = state.work_center_service().list_active(&service_ctx, &mut conn).await.unwrap_or_default();
            Ok(([("HX-Trigger", "")], Html(html! {
                div hx-swap-oob="innerHTML:#routing-edit-drawer-body" {
                    (routing_edit_form(path.order_id, path.routing_id, routing, &pname, &work_centers))
                }
            }.into_string())))
        }
    }
}

/// 删除工序 → 触发 routingChanged 事件，由 data-card 自行 hx-get 重载
#[require_permission("WORK_ORDER", "update")]
pub async fn delete_routing(
    path: OrderRoutingDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.delete_routing(&service_ctx, &mut conn, path.order_id, path.routing_id).await?;
    Ok(([("HX-Trigger", "routingChanged")], "OK"))
}

/// POST：选择工艺路径 → 将模板工序加载为工单工序（删除旧行 → 插入模板步骤）
#[require_permission("WORK_ORDER", "update")]
pub async fn post_apply_from_routing(
    path: OrderRoutingApplyFromRoutingPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ApplyFromRoutingForm>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.load_routings_from_template(&service_ctx, &mut conn, path.order_id, form.routing_id).await?;
    let body = refresh_routing_tbody(&state, &svc, &service_ctx, &mut conn, path.order_id).await?;
    Ok(Html(body.into_string()))
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplyFromRoutingForm {
    pub routing_id: i64,
}
/// 从最近同路径工单批量加载产出品
#[require_permission("WORK_ORDER", "update")]
pub async fn load_routings_from_recent(
    path: OrderRoutingLoadRecentPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    svc.load_routings_from_recent(&service_ctx, &mut conn, path.order_id).await?;
    let body = refresh_routing_tbody(&state, &svc, &service_ctx, &mut conn, path.order_id).await?;
    Ok(Html(body.into_string()))
}

/// 重新取工序列表 + 解析产品名 → 返回 tab_routing（含加载按钮，替换 #routing-tbody-wrap）
async fn refresh_routing_tbody<T: abt_core::mes::production_batch::ProductionBatchService>(
    state: &crate::state::AppState,
    svc: &T,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    work_order_id: i64,
) -> Result<Markup> {
    use abt_core::master_data::product::ProductService;
    let routings = svc.list_routings(ctx, db, work_order_id).await?;
    let pids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
    let product_names: std::collections::HashMap<i64, String> = if pids.is_empty() {
        std::collections::HashMap::new()
    } else {
        state.product_service().get_by_ids(ctx, db, pids).await
            .unwrap_or_default().into_iter().map(|p| (p.product_id, p.pdt_name)).collect()
    };
    let empty = std::collections::HashSet::new();
    // 删除/加载只在整单零报工时触发 → order_has_report=false 近似准确
    Ok(tab_routing(&routings, &empty, false, &product_names, &std::collections::HashMap::new(), work_order_id))
}

/// 渲染单行 <tr>（只读展示）。reported_step → 编辑按钮隐藏；order_has_report=false → 显示删除按钮
fn routing_row_fragment(
    r: &WorkOrderRouting,
    is_reported_step: bool,
    order_has_report: bool,
    product_name: Option<&str>,
    work_center_name: Option<&str>,
) -> Markup {
    html! {
        tr id=(format!("routing-row-{}", r.id)) {
            td class="font-mono tabular-nums" { (r.step_no) }
            td { strong { (r.process_name.as_str()) } }
            td class="text-[13px] max-w-[180px]" {
                @if let Some(pn) = product_name { span class="truncate block" title=(pn) { (pn) } }
                @else if let Some(pid) = r.product_id { span class="text-muted" { "#" (pid) } }
                @else { "—" }
            }
            td class="text-[13px] max-w-[120px]" {
                @if let Some(name) = work_center_name { span class="truncate block" title=(name) { (name) } }
                @else if let Some(wc) = r.work_center_id { span class="text-muted" { "#" (wc) } }
                @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px] w-[80px]" {
                @if let Some(t) = r.standard_time { (crate::utils::fmt_qty(t)) } @else { "—" }
            }
            td class="font-mono tabular-nums text-right text-[13px] w-[100px]" {
                @if let Some(p) = r.unit_price { "¥" (crate::utils::fmt_qty(p)) } @else { "—" }
            }
            td {
                @if r.is_outsourced { span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-warn-bg text-warn" { "委外" } } @else { "—" }
            }
            td class="text-center whitespace-nowrap" {
                @if !is_reported_step {
                    button class="text-muted hover:text-accent cursor-pointer border-none bg-transparent p-1" title="编辑"
                        hx-get=(OrderRoutingEditPath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-target="#routing-edit-drawer-body" hx-swap="innerHTML"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #routing-edit-drawer" {
                        (icon::edit_icon("w-4 h-4"))
                    }
                } @else { "—" }
                @if !order_has_report {
                    button class="text-muted hover:text-danger cursor-pointer border-none bg-transparent p-1 ml-1" title="删除该工序"
                        hx-post=(OrderRoutingDeletePath { order_id: r.work_order_id, routing_id: r.id }.to_string())
                        hx-confirm="删除该工序并重排后续工序号？"
                        hx-swap="none" hx-disabled-elt="this" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

/// 编辑抽屉表单（product picker + 工作中心 + 单价 + 标准工时）
fn routing_edit_form(
    work_order_id: i64,
    routing_id: i64,
    r: &WorkOrderRouting,
    product_name: &str,
    work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
) -> Markup {
    html! {
        form id="routing-edit-form"
            hx-post=(OrderRoutingEditPath { order_id: work_order_id, routing_id }.to_string())
            hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#routing-edit-drawer').classList.remove('open')" {
            input type="hidden" name="product_id" id="routing-product-id"
                value=(r.product_id.map(|p| p.to_string()).unwrap_or_default());
            div class="mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1" { "产出品" }
                div class="flex gap-2" {
                    input type="text" id="routing-product-display" readonly
                        class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-surface"
                        value=(product_name) placeholder="点击右侧选择产出品…";
                    button type="button" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg-2 cursor-pointer hover:bg-surface"
                        _="on click add .is-open to #routing-product-modal" { "选择" }
                }
            }
            div class="mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1" { "工作中心" }
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" name="work_center_id" {
                    option value="" { "—" }
                    @for wc in work_centers {
                        option value=(wc.id) selected[r.work_center_id == Some(wc.id)] { (wc.name) }
                    }
                }
            }
            div class="grid grid-cols-2 gap-4 mb-4" {
                div {
                    label class="block text-xs font-medium text-fg-2 mb-1" { "计件单价（元/件）" }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                        type="number" step="any" name="unit_price" required
                        value=(r.unit_price.map(|p| p.to_string()).unwrap_or_default());
                }
                div {
                    label class="block text-xs font-medium text-fg-2 mb-1" { "标准工时（小时）" }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                        type="number" step="any" name="standard_time"
                        value=(r.standard_time.map(|t| t.to_string()).unwrap_or_default());
                }
            }
            div class="mb-4" {
                label class="flex items-center gap-2 cursor-pointer" {
                    input type="checkbox" name="is_outsourced" value="true"
                        class="w-4 h-4 accent-accent"
                        checked[r.is_outsourced] {}
                    span class="text-sm text-fg-2 select-none" { "委外工序" }
                }
            }
        }
    }
}

/// BOM 树形产品选择弹窗（层级展示 + hyperscript 搜索过滤）
fn routing_bom_tree_modal(
    detail: &abt_core::master_data::bom::model::BomDetail,
    product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
) -> Markup {
    use abt_core::master_data::bom::model::BomNode;
    // 构建父子关系映射
    let mut children_map: std::collections::HashMap<i64, Vec<&BomNode>> = std::collections::HashMap::new();
    for node in &detail.nodes {
        children_map.entry(node.parent_id).or_default().push(node);
    }
    // 每层按 order_num 排序
    for (_, children) in children_map.iter_mut() {
        children.sort_by_key(|n| n.order);
    }
    // DFS 构建树形扁平列表（带深度）
    struct TreeRow {
        product_id: i64,
        product_code: String,
        product_name: String,
        quantity: rust_decimal::Decimal,
        unit: String,
        depth: usize,
        has_children: bool,
    }
    let mut rows: Vec<TreeRow> = Vec::new();
    fn dfs(
        parent_id: i64,
        depth: usize,
        children_map: &std::collections::HashMap<i64, Vec<&BomNode>>,
        product_map: &std::collections::HashMap<i64, &abt_core::master_data::product::model::Product>,
        rows: &mut Vec<TreeRow>,
    ) {
        if let Some(children) = children_map.get(&parent_id) {
            for node in children.iter() {
                if let Some(p) = product_map.get(&node.product_id) {
                    rows.push(TreeRow {
                        product_id: p.product_id,
                        product_code: p.product_code.clone(),
                        product_name: p.pdt_name.clone(),
                        quantity: node.quantity,
                        unit: node.unit.clone().unwrap_or_else(|| p.unit.clone()),
                        depth,
                        has_children: children_map.contains_key(&node.id),
                    });
                    dfs(node.id, depth + 1, children_map, product_map, rows);
                }
            }
        }
    }
    dfs(0, 0, &children_map, product_map, &mut rows);

    html! {
        div id="routing-product-modal"
            class="fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            _="on click[me is event.target] remove .is-open from me" {
            div class="bg-bg rounded-xl w-[720px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event" {
                // Header
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { "选择 BOM 物料" }
                    button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                        _="on click remove .is-open from #routing-product-modal" { "×" }
                }
                // Search + Level filter
                div class="px-6 py-3 border-b border-border-soft shrink-0 flex gap-3" {
                    input type="text" id="bom-search-input" placeholder="搜索物料名称或编码…"
                        class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                        onkeyup="window._filterBomTree()" {}
                    @let max_depth = rows.iter().map(|r| r.depth).max().unwrap_or(0);
                    @if max_depth > 0 {
                        select id="bom-level-select" class="px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                            onchange="window._filterBomTree()" {
                            option value="" { "全部层级" }
                            @for d in 0..=max_depth {
                                option value=(d.to_string()) {
                                    @if d == 0 { "成品级 (L0)" }
                                    @else if d == 1 { "一级物料 (L1)" }
                                    @else if d == 2 { "二级物料 (L2)" }
                                    @else if d == 3 { "三级物料 (L3)" }
                                    @else if d == 4 { "四级物料 (L4)" }
                                    @else if d == 5 { "五级物料 (L5)" }
                                    @else { "L" (d) }
                                }
                            }
                        }
                    }
                }
                // Tree list
                div class="overflow-y-auto flex-1 min-h-0" {
                    @for row in &rows {
                        @let row_class = if row.depth == 0 {
                            "bg-purple-700 text-white font-medium"
                        } else if row.has_children {
                            "bg-[#ff0] text-slate-900"
                        } else {
                            ""
                        };
                        div class=(format!("bom-tree-row flex items-center gap-2 px-6 py-2 border-b border-border-soft cursor-pointer {}", row_class))
                            data-pid=(row.product_id)
                            data-pname=(row.product_name.as_str())
                            data-name=(row.product_name.to_lowercase())
                            data-code=(row.product_code.to_lowercase())
                            data-depth=(row.depth)
                            style=(format!("padding-left:{}px", 24 + row.depth * 28))
                            _="on click put my @data-pid into <input[id='routing-product-id']/>'s value then put my @data-pname into <input[id='routing-product-display']/>'s value then remove .is-open from #routing-product-modal" {
                            @if row.depth > 0 {
                                span style=(format!("margin-left:{}px", -8isize - ((row.depth as isize) - 1) * 28))
                                    class="text-border-soft text-xs shrink-0" {
                                    @for _ in 0..row.depth {
                                        span class="text-border-soft" { "└ " }
                                    }
                                }
                            }
                            span class="text-xs font-mono text-muted bg-surface rounded px-1.5 py-0.5 shrink-0" { (row.product_code.as_str()) }
                            span class="text-sm text-fg flex-1 truncate" { (row.product_name.as_str()) }
                            span class="text-xs text-muted shrink-0" { (row.quantity.to_string()) }
                            span class="text-xs text-muted w-8 shrink-0" { (row.unit.as_str()) }
                        }
                    }
                }
                // Client-side filter logic
                (maud::PreEscaped(r#"<script>
window._filterBomTree=function(){
  var q=(document.getElementById('bom-search-input')?.value||'').toLowerCase();
  var lv=document.getElementById('bom-level-select')?.value||'';
  document.querySelectorAll('.bom-tree-row').forEach(function(row){
    var name=row.getAttribute('data-name')||'';
    var code=row.getAttribute('data-code')||'';
    var depth=row.getAttribute('data-depth')||'';
    var match=q===''||name.includes(q)||code.includes(q);
    var levelOk=lv===''||depth===lv;
    row.classList.toggle('hidden',!(match&&levelOk));
  });
};
</script>"#))
                // Footer
                div class="px-6 py-4 border-t border-border-soft flex justify-end shrink-0" {
                    button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm cursor-pointer"
                        _="on click remove .is-open from #routing-product-modal" { "关闭" }
                }
            }
        }
    }
}

/// 渲染整个 <tbody>
fn routing_tbody_fragment(
    routings: &[WorkOrderRouting],
    reported_routing_ids: &std::collections::HashSet<i64>,
    order_has_report: bool,
    product_names: &std::collections::HashMap<i64, String>,
    work_center_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        tbody {
            @for r in routings {
                (routing_row_fragment(
                    r,
                    reported_routing_ids.contains(&r.id),
                    order_has_report,
                    r.product_id.and_then(|pid| product_names.get(&pid)).map(|s| s.as_str()),
                    r.work_center_id.and_then(|wid| work_center_names.get(&wid)).map(|s| s.as_str()),
                ))
            }
            @if routings.is_empty() {
                tr { td colspan="8" class="text-center text-muted text-sm" { "暂无工序明细（工单未下达或无工艺路线）" } }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn order_detail_page(
 order: &WorkOrder,
 product_name: &str,
 routings: &[WorkOrderRouting],
 batches: &[ProductionBatch],
 reports: &[ReportListItem],
 audit_logs: &[AuditLog],
 completion_pct: rust_decimal::Decimal,
 has_receipts: bool,
 reported_routing_ids: &std::collections::HashSet<i64>,
 order_has_report: bool,
 product_names: &std::collections::HashMap<i64, String>,
	 work_center_names: &std::collections::HashMap<i64, String>,
) -> Markup {
 let (status_label, status_bg, status_color) = wo_status_label(&order.status);
 let routing_tab_label = format!("工序明细 {}", routings.len());

 html! {
 div {
 // 返回
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", OrderListPath::PATH)) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回工单列表"
 }

 // Detail Header（裸 flex，非 card）
 div class="flex items-center justify-between mb-2" {
 div class="flex items-center gap-4" {
 h1 class="text-xl font-bold font-mono tabular-nums" { (order.doc_number) }
 (status_pill(status_label, status_bg, status_color))
 }
 div class="flex gap-3" {
 @if matches!(order.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" _="on click add .is-open to #unrelease-dialog" {
 "反下达"
 }
 @if completion_pct >= rust_decimal::Decimal::new(95, 2) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 hx-post=(OrderClosePath { order_id: order.id }.to_string())
 hx-confirm="确认关闭此工单？所有批次必须已完工或已取消。"
 hx-disabled-elt="this" {
 (icon::check_circle_icon("w-4 h-4"))
 "关闭工单"
 }
 } @else {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" disabled
 title=(format!("完工率 {}%，需 ≥ 95% 才能关闭", completion_pct.round_dp(1))) {
 (icon::check_circle_icon("w-4 h-4"))
 "关闭工单（完工不足）"
 }
 }
 }
 @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 hx-post=(OrderReleasePath { order_id: order.id }.to_string())
 hx-confirm="确认下达此工单？下达后将开始生产。"
 hx-disabled-elt="this" {
 (icon::rocket_icon("w-4 h-4"))
 "下达工单"
 }
 }
 @if matches!(order.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned | WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
 @if has_receipts {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90" disabled
 title="存在已完工入库记录，无法取消" {
 (icon::x_icon("w-4 h-4"))
 "取消（有入库记录）"
 }
 } @else {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
 hx-post=(OrderCancelPath { order_id: order.id }.to_string())
 hx-confirm="确认取消此工单？取消后不可恢复。"
 hx-disabled-elt="this" {
 (icon::x_icon("w-4 h-4"))
 "取消工单"
 }
 }
 }
 }
 }

 // 副标题行
 div class="flex items-center flex-wrap gap-2 text-muted text-sm mb-6" {
 span { (product_name) }
 span class="text-border" { "|" }
 span class="font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) " 件" }
 @if order.completed_qty > rust_decimal::Decimal::ZERO {
 span class="text-border" { "|" }
 span class="font-mono tabular-nums text-success" { "完成 " (crate::utils::fmt_qty(order.completed_qty)) }
 }
 @if order.scrap_qty > rust_decimal::Decimal::ZERO {
 span class="text-border" { "|" }
 span class="font-mono tabular-nums text-danger" { "报废 " (crate::utils::fmt_qty(order.scrap_qty)) }
 }
 span class="text-border" { "|" }
 span { "—" }
 @if let Some(so) = order.source_so_doc.as_ref() {
 span class="text-border" { "|" }
 span { "销售订单: " (so) }
 @if let Some(c) = order.source_customer.as_ref() {
 span class="text-muted" { " (" (c) ")" }
 }
 }
 @if let Some(pdoc) = order.source_plan_doc.as_ref() {
 span class="text-border" { "|" }
 span { "生产计划: " }
 @if let Some(pid) = order.source_plan_id {
 a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/plans/{pid}")) { (pdoc) }
 } @else { span { (pdoc) } }
 }
 }

 // Tabs
 (detail_tabs("info", &[
 ("info", "工单信息"),
 ("routing", &routing_tab_label),
 ("batches", &format!("生产批次 {}", batches.len())),
 ("reports", &format!("报工记录 {}", reports.len())),
 ("log", "操作日志"),
 ]))

 (tab_panel("info", true, tab_info(order, product_name, routings.len(), completion_pct)))
 (tab_panel("routing", false, tab_routing(routings, reported_routing_ids, order_has_report, product_names, work_center_names, order.id)))
 (tab_panel("batches", false, tab_batches(batches, routings, order)))
 (tab_panel("reports", false, tab_reports(reports)))
 (tab_panel("log", false, tab_log(audit_logs)))

 // 编辑抽屉（保存按钮用 hyperscript 触发 form submit）
 (drawer::drawer(
 "routing-edit-drawer",
 "编辑工序",
 "保存",
 "routing-edit-form",
 html! { div id="routing-edit-drawer-body" {} },
 ))
 // 产品选择弹窗容器（页面级，避免放在 drawer 内太小；由 GET /edit 通过 OOB swap 填充）
 div id="routing-product-modal-container" {}

 // ── 从工艺路径加载（routing_picker → 直接应用）──
 form hx-post=(OrderRoutingApplyFromRoutingPath { order_id: order.id }.to_string())
     hx-trigger="routingSelected from:body"
     hx-target="#routing-tbody-wrap" hx-swap="outerHTML"
     hx-disabled-elt="this" {
     input type="hidden" id="routing-id-hidden" name="routing_id";
     span id="routing-name-display" class="hidden" {}
 }
 (routing_picker::routing_picker_modal("routing-picker-modal", "routing-id-hidden", "routing-name-display"))

 // 反下达对话框
 @if matches!(order.status, WorkOrderStatus::Released) {
 div class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" id="unrelease-dialog" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 { "确认反下达？" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 p class="text-sm text-fg-2 leading-relaxed" {
 "反下达将回退工单到 "
 strong { "草稿" }
 " 状态，同时取消领料单、释放库存预留、软删除生产批次（若有报工记录则无法反下达）。此操作不可撤销。"
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" _="on click remove .is-open from #unrelease-dialog" {
 "取消"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
 hx-post=(OrderUnreleasePath { order_id: order.id }.to_string())
 hx-confirm="确认执行反下达？"
 hx-disabled-elt="this" {
 "确认反下达"
 }
 }
 }
 }
 }
 }
 }
}

// ── Tab Panels ──
fn tab_info(order: &WorkOrder, product_name: &str, routing_count: usize, completion_pct: rust_decimal::Decimal) -> Markup {
 let (sl, sb, sc) = wo_status_label(&order.status);
 html! {
 div class="flex flex-col gap-5" {
 // 生产进度
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "生产进度" }
 div class="flex items-end justify-between flex-wrap gap-4 mb-5" {
 div class="flex items-baseline gap-2" {
 span class=(format!("text-4xl font-bold font-mono tabular-nums leading-none {}", if completion_pct >= rust_decimal::Decimal::ONE_HUNDRED { "text-success" } else { "text-accent" })) {
 (completion_pct.round_dp(1)) "%"
 }
 span class="text-xs text-muted" { "完工率" }
 }
 div class="flex gap-6" {
 div class="text-right" {
 div class="text-xs text-muted mb-0.5" { "计划数量" }
 div class="text-sm font-semibold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(order.planned_qty)) }
 }
 div class="text-right" {
 div class="text-xs text-muted mb-0.5" { "已完工" }
 div class="text-sm font-semibold font-mono tabular-nums text-success" { (crate::utils::fmt_qty(order.completed_qty)) }
 }
 @if order.scrap_qty > rust_decimal::Decimal::ZERO {
 div class="text-right" {
 div class="text-xs text-muted mb-0.5" { "报废" }
 div class="text-sm font-semibold font-mono tabular-nums text-danger" { (crate::utils::fmt_qty(order.scrap_qty)) }
 }
 }
 }
 }
 div class="relative" {
 div class="h-2.5 bg-slate-50 rounded-full overflow-hidden" {
 div class=(format!("h-full rounded-full transition-all duration-500 {}", if completion_pct >= rust_decimal::Decimal::ONE_HUNDRED { "bg-gradient-to-r from-success to-success" } else { "bg-gradient-to-r from-accent to-accent-hover" }))
 style=(format!("width: {}%", completion_pct.round_dp(1)))
 {}
 }
 div class="flex justify-between mt-2 px-0.5" {
 div class="flex flex-col items-center" { div class="w-px h-1 bg-border-soft mb-1" {} span class="text-[10px] text-muted" { "0%" } }
 div class="flex flex-col items-center" { div class="w-px h-1 bg-border-soft mb-1" {} span class="text-[10px] text-muted" { "25%" } }
 div class="flex flex-col items-center" { div class="w-px h-1 bg-border-soft mb-1" {} span class="text-[10px] text-muted" { "50%" } }
 div class="flex flex-col items-center" { div class="w-px h-1 bg-border-soft mb-1" {} span class="text-[10px] text-muted" { "75%" } }
 div class="flex flex-col items-center" { div class="w-px h-1 bg-border-soft mb-1" {} span class="text-[10px] text-muted" { "100%" } }
 }
 }
 }
 // 基础信息 + 生产配置
 div class="grid gap-5 bg-bg border border-border-soft rounded-lg p-6 lg:grid-cols-2" {
 div class="flex flex-col gap-4" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "基础信息" }
 div class="grid grid-cols-2 gap-4" {
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "工单编号" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.doc_number) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "产品" } span class="text-sm text-fg font-medium" { (product_name) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "计划数量" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (crate::utils::fmt_qty(order.planned_qty)) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "状态" } span class="text-sm text-fg font-medium" { (status_pill(sl, sb, sc)) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "版本号" } span class="text-sm text-fg font-medium font-mono tabular-nums" { "v"(order.version) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "计划开始" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.scheduled_start) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "计划结束" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (order.scheduled_end) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "创建人" } span class="text-sm text-fg font-medium font-mono tabular-nums" { "#" (order.operator_id) } }
 }
 }
 div class="flex flex-col gap-4" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "生产配置" }
 div class="grid grid-cols-2 gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "BOM 快照" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { @if let Some(bid) = order.bom_snapshot_id { "#" (bid) } @else { "—" } }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "工艺路线" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { @if let Some(rid) = order.routing_id { "#" (rid) } @else { "—" } }
 }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "工序数" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (routing_count) } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "物料模式" } span class="text-sm text-fg font-medium" { "—" } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "超额容差" } span class="text-sm text-fg font-medium" { "—" } }
 div class="flex flex-col gap-1" { span class="text-xs text-muted font-medium" { "创建时间" } span class="text-sm text-fg font-medium font-mono tabular-nums" { (fmt_dt(order.created_at)) } }
 }
 }
 }
 @if !order.remark.is_empty() {
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft" { "备注" }
 p class="text-sm text-muted" { (order.remark.as_str()) }
 }
 }
 }
 }
}

fn tab_routing(
 routings: &[WorkOrderRouting],
 reported_routing_ids: &std::collections::HashSet<i64>,
 order_has_report: bool,
 product_names: &std::collections::HashMap<i64, String>,
	 work_center_names: &std::collections::HashMap<i64, String>,
 order_id: i64,
) -> Markup {
 html! {
 div id="routing-tbody-wrap" {
 // 批量加载产出品（仅整单未报工时可用）
 @if !order_has_report {
 div class="flex justify-end gap-3 mb-3" {
 button type="button" class="inline-flex items-center gap-1 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
     _="on click add .is-open to #routing-picker-modal"
     title="选择工艺路径，预填产出品和单价后应用到工单工序" {
     (icon::download_icon("w-3.5 h-3.5"))
     "从工艺路径加载"
 }
 button type="button" class="inline-flex items-center gap-1 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
     hx-post=(OrderRoutingLoadRecentPath { order_id }.to_string())
     hx-target="#routing-tbody-wrap" hx-swap="outerHTML" hx-disabled-elt="this"
     title="从最近一个同工艺路径、已设产出品的工单复制" {
     (icon::copy_icon("w-3.5 h-3.5"))
     "从最近工单加载"
 }
 }
 }
 // 工序定义表（执行进度已迁移至 batch_routing_progress，由批次维度页面展示）
 div id="routing-card" class="data-card"
     hx-get=(OrderDetailPath { id: order_id }.to_string())
     hx-trigger="routingChanged from:body"
     hx-select="#routing-card"
     hx-select-oob="#tab-batches"
     hx-swap="outerHTML"
     hx-disinherit="hx-select" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "序号" }
 th { "工序名称" }
 th { "产出品" }
 th { "工作中心" }
 th class="text-right text-[13px]" { "标准工时" }
 th class="text-right text-[13px]" { "计件单价" }
 th { "委外" }
 th { "操作" }
 }
 }
 (routing_tbody_fragment(routings, reported_routing_ids, order_has_report, product_names, work_center_names))
 }
 }
 }
 }
 }
}

fn tab_batches(batches: &[ProductionBatch], routings: &[WorkOrderRouting], order: &WorkOrder) -> Markup {
 // 计算可拆批余量
 let existing_qty: rust_decimal::Decimal =
 batches.iter().map(|b| b.batch_qty).sum();
 let remaining = order.planned_qty - existing_qty;
 let can_split = matches!(order.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction);

 html! {
 // 操作栏
 @if can_split {
 div class="flex items-center gap-3 flex-wrap justify-end mb-3" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="button"
 _="on click add .is-open to #split-dialog" {
 (icon::plus_icon("w-4 h-4"))
 "新增批次"
 }
 }
 }

 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "批次号" }
 th { "流转卡号" }
 th class="text-right text-[13px]" { "完成量" }
 th class="text-right text-[13px]" { "报废量" }
 th { "当前工序" }
 th { "状态" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for b in batches {
 tr {
 td class="font-mono tabular-nums" { (b.batch_no.as_str()) }
 td class="font-mono tabular-nums" { (b.card_sn.as_str()) }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(b.batch_qty)) }
 td class="font-mono tabular-nums text-right text-[13px] text-success" { (crate::utils::fmt_qty(b.completed_qty)) }
 td class="font-mono tabular-nums text-right text-[13px] text-danger" { (crate::utils::fmt_qty(b.scrap_qty)) }
 td {
 @if b.current_step == 0 {
 span class="text-muted" { "未开始" }
 } @else {
 @let total = routings.len();
 @let sname = routings.iter()
 .find(|r| r.step_no == b.current_step)
 .map(|r| r.process_name.as_str())
 .unwrap_or("—");
 span { (b.current_step) "/" (total) " " (sname) }
 }
 }
 td { (batch_status_pill(b.status)) }
 td { a class="text-accent font-medium cursor-pointer" href=(format!("/admin/mes/batches/{}", b.id)) { "查看" } }
 }
 }
 @if batches.is_empty() {
 tr { td colspan="8" class="text-center text-muted text-sm" { @if can_split { "暂无生产批次，请点击「新增批次」创建" } @else { "暂无生产批次（工单未下达或无工艺路线）" } } }
 }
 }
 }
 }
 }

 // 拆批对话框
 @if can_split {
 div class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" id="split-dialog" {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 { "新增生产批次" }
 }
 form {
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 p class="text-sm text-fg-2 leading-relaxed" {
 "工单计划量 " strong { (crate::utils::fmt_qty(order.planned_qty)) }
 "，已分批 " strong { (crate::utils::fmt_qty(existing_qty)) }
 @if remaining > rust_decimal::Decimal::ZERO {
 "，可新增 " strong class="text-success" { (crate::utils::fmt_qty(remaining)) }
 } @else {
 "（已全部分配，可按容差新增）"
 }
 }
 div class="form-field" {
 label { "新增批次数量" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="any" name="split_qty"
 placeholder="输入数量"
 required;
 }
 // 工艺路线预览
 @if !routings.is_empty() {
 div class="form-field" {
 label { "工艺路线（该批次将依次经过以下工序）" }
 div class="flex flex-wrap bg-surface rounded-sm border border-border gap-1.5 p-2" {
 @for (i, r) in routings.iter().enumerate() {
 @if i > 0 {
 span class="flex items-center text-muted" { "\u{2192}" }
 }
 span class="items-center rounded-sm inline-flex gap-1 text-xs px-2 py-0.5 bg-surface" {
 span class="font-semibold text-accent" { (r.step_no) }
 (r.process_name.as_str())
 @if r.is_inspection_point {
 span class="inline-flex items-center rounded-full font-medium py-0.5 px-1 text-[10px] bg-accent-bg text-accent" { "检" }
 }
 @if r.is_outsourced {
 span class="inline-flex items-center rounded-full font-medium py-0.5 px-1 text-[10px] bg-warn-bg text-warn" { "外" }
 }
 }
 }
 }
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button"
 _="on click remove .is-open from #split-dialog" {
 "取消"
 }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit"
 hx-post=(OrderSplitPath { order_id: order.id }.to_string())
 hx-disabled-elt="this" {
 "确认新增"
 }
 }
 }
 }
 }
 }
 }
}

fn tab_reports(reports: &[ReportListItem]) -> Markup {
 html! {
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "报工时间" }
 th { "工序" }
 th { "报工单号" }
 th class="text-right text-[13px]" { "完成量" }
 th class="text-right text-[13px]" { "报废量" }
 th { "报工人" }
 th { "班次" }
 }
 }
 tbody {
 @for r in reports {
 tr {
 td class="font-mono tabular-nums" { (fmt_dt(r.created_at)) }
 td { (r.process_name.as_str()) }
 td { a class="text-accent font-medium cursor-pointer font-mono tabular-nums" href=(format!("/admin/mes/reports/{}", r.id)) { (r.doc_number.as_str()) } }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.completed_qty)) }
 td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(r.defect_qty)) }
 td { (r.worker_name.as_deref().unwrap_or("—")) }
 td { (shift_label(r.shift)) }
 }
 }
 @if reports.is_empty() {
 tr { td colspan="7" class="text-center text-muted text-sm" { "暂无报工记录" } }
 }
 }
 }
 }
 }
 }
}

fn tab_log(logs: &[AuditLog]) -> Markup {
 html! {
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="relative" {
 div class="absolute left-[5px] top-2 bottom-2 w-0.5 bg-border-soft" {}
 @for log in logs {
 div class="relative pl-6 pb-5 last:pb-0" {
 div class="absolute left-0 top-1 w-3 h-3 rounded-full bg-accent ring-4 ring-bg" {}
 div {
 div class="font-semibold text-sm text-fg" { (audit_action_label(log.action)) }
 div class="flex gap-2 text-xs text-muted items-center mt-1" {
 span { (fmt_dt(log.created_at)) }
 span class="text-border" { "·" }
 span { "操作人 #" (log.operator_id) }
 }
 @if let Some(changes) = log.changes.as_ref() {
 div class="text-xs text-muted mt-1" { (changes) }
 }
 }
 }
 }
 @if logs.is_empty() {
 div class="text-center text-muted text-sm py-4" { "暂无操作日志" }
 }
 }
 }
 }
}
