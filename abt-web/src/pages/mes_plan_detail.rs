use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::mes::enums::{PlanItemStatus, PlanStatus, PlanType, WorkOrderStatus};
use abt_core::mes::production_plan::{
    ProductionPlan, ProductionPlanItem, ProductionPlanService, ReleaseValidation, WorkOrderPlanItem,
};
use abt_core::mes::work_order::{WorkOrder, WorkOrderService};
use abt_core::shared::audit_log::{AuditLog, AuditLogQuery, AuditLogService};
use abt_core::shared::enums::audit::AuditAction;
use abt_core::shared::identity::UserService;

use crate::components::detail::{detail_tabs, tab_panel};
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderCancelPath, OrderDetailPath, OrderReleasePath};
use crate::routes::mes_plan::{
    PlanConfirmPath, PlanDetailPath, PlanGeneratePath, PlanGenerateReleasePath, PlanListPath,
    PlanReleaseAllPath, PlanReleasePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn plan_status_label(s: &PlanStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        PlanStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        PlanStatus::Confirmed => ("已确认", "rgba(22,119,255,0.08)", "var(--accent)"),
        PlanStatus::InProgress => ("进行中", "rgba(250,140,22,0.08)", "#fa8c16"),
        PlanStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        PlanStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

fn plan_type_label(t: &PlanType) -> &'static str {
    match t {
        PlanType::Mto => "按单生产 (MTO)",
        PlanType::Mts => "按库存备货 (MTS)",
    }
}

fn item_status_label(s: &PlanItemStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        PlanItemStatus::Planned => ("计划中", "rgba(0,0,0,0.04)", "var(--muted)"),
        PlanItemStatus::Released => ("已下达", "rgba(22,119,255,0.08)", "var(--accent)"),
        PlanItemStatus::InProduction => ("生产中", "rgba(250,140,22,0.08)", "#fa8c16"),
        PlanItemStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        PlanItemStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

fn wo_status_label(s: &WorkOrderStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        WorkOrderStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        WorkOrderStatus::Planned => ("已计划", "rgba(0,0,0,0.04)", "var(--muted)"),
        WorkOrderStatus::Released => ("已下达", "rgba(22,119,255,0.08)", "var(--accent)"),
        WorkOrderStatus::InProduction => ("生产中", "rgba(250,173,20,0.08)", "#faad14"),
        WorkOrderStatus::Closed => ("已完工", "rgba(82,196,26,0.08)", "var(--success)"),
        WorkOrderStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

fn status_pill(label: &str, bg: &str, color: &str) -> Markup {
    html! {
        span class="status-pill" style=(format!("background:{bg};color:{color}")) {
            (label)
        }
    }
}

fn audit_action_label(a: &AuditAction) -> &'static str {
    match a {
        AuditAction::Create => "创建计划",
        AuditAction::Update => "更新",
        AuditAction::Delete => "删除",
        AuditAction::Transition => "状态流转",
    }
}

fn fmt_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// 完整度圆点（BOM / Routing / 物料）
fn completeness_dots(val: Option<&ReleaseValidation>) -> Markup {
    let (bom, routing, material) = match val {
        Some(v) => (v.has_published_bom, v.has_routing, v.material_shortages.is_empty()),
        None => (false, false, false),
    };
    html! {
        div class="completeness-dots" {
            span class=(if bom { "comp-dot filled" } else { "comp-dot empty" })
                title=(if bom { "BOM ✓" } else { "BOM ✗" }) {}
            span class=(if routing { "comp-dot filled" } else { "comp-dot empty" })
                title=(if routing { "Routing ✓" } else { "Routing ✗" }) {}
            span class=(if material { "comp-dot filled" } else { "comp-dot warn" })
                title=(if material { "物料 ✓" } else { "物料 ⚠" }) {}
        }
    }
}

fn priority_label(p: i32) -> (&'static str, &'static str) {
    match p {
        1 => ("高", "var(--danger)"),
        2 => ("中", "var(--warn)"),
        _ => ("低", "var(--muted)"),
    }
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_plan_detail(
    path: PlanDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let data = load_plan_detail_raw(&state, &service_ctx, &mut conn, path.id).await?;

    let content = plan_detail_page(
        &data.plan, &data.items, &data.op_name, &data.product_names,
        &data.work_orders, &data.audit_logs, &data.validations,
    );
    let page_html = admin_page(
        is_htmx, "生产计划详情", &claims, "production",
        &format!("/admin/mes/plans/{}", path.id),
        "生产管理", Some(PlanListPath::PATH), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn confirm_plan(
    path: PlanConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_plan_service().confirm(&service_ctx, &mut conn, path.plan_id).await?;

    let redirect = PlanDetailPath { id: path.plan_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn release_plan(
    path: PlanReleasePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state
        .production_plan_service()
        .release_to_work_orders(&service_ctx, &mut conn, path.plan_id)
        .await?;

    let redirect = PlanDetailPath { id: path.plan_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct GenerateForm {
    pub items_json: String,
}

/// POST /plans/{id}/generate — 从规划项生成 Draft 工单
#[require_permission("WORK_ORDER", "create")]
pub async fn generate_work_orders(
    path: PlanGeneratePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<GenerateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let items: Vec<WorkOrderPlanItem> = serde_json::from_str(&form.items_json).map_err(|e| {
        crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(format!(
            "规划数据格式错误：{e}"
        )))
    })?;

    state
        .production_plan_service()
        .generate_work_orders(&service_ctx, &mut conn, path.plan_id, items)
        .await?;

    let redirect = format!("/admin/mes/plans/{}?tab=planning", path.plan_id);
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &redirect)
        .body(axum::body::Body::empty())
        .unwrap())
}

/// POST /plans/{id}/release-all — 批量下达该计划所有 Draft 工单
#[require_permission("WORK_ORDER", "update")]
pub async fn release_all_work_orders(
    path: PlanReleaseAllPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let wo_svc = state.work_order_service();
    let plan_svc = state.production_plan_service();

    let draft_orders: Vec<_> = wo_svc
        .list_by_plan(&service_ctx, &mut conn, path.plan_id)
        .await?
        .into_iter()
        .filter(|wo| wo.status == WorkOrderStatus::Draft)
        .collect();

    let mut successful = Vec::new();
    for wo in &draft_orders {
        match wo_svc.release(&service_ctx, &mut conn, wo.id, wo.version).await {
            Ok(()) => successful.push(wo.id),
            Err(e) => tracing::warn!(work_order_id = wo.id, error = %e, "release-all failed"),
        }
    }

    // 首个成功 release → 计划状态 InProgress
    if !successful.is_empty()
        && let Ok(plan) = plan_svc.find_by_id(&service_ctx, &mut conn, path.plan_id).await
        && plan.status == PlanStatus::Confirmed
    {
        let _ = plan_svc.mark_in_progress(&mut conn, path.plan_id).await;
    }

    let redirect = format!("/admin/mes/plans/{}?tab=planning", path.plan_id);
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &redirect)
        .body(axum::body::Body::empty())
        .unwrap())
}

/// POST /plans/{id}/generate-and-release — 快速通道：生成 Draft + 立即全部 release
#[require_permission("WORK_ORDER", "create")]
pub async fn generate_and_release(
    path: PlanGenerateReleasePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<GenerateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let items: Vec<WorkOrderPlanItem> = serde_json::from_str(&form.items_json).map_err(|e| {
        crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(format!(
            "规划数据格式错误：{e}"
        )))
    })?;

    let plan_svc = state.production_plan_service();
    let wo_svc = state.work_order_service();

    let wo_ids = plan_svc
        .generate_work_orders(&service_ctx, &mut conn, path.plan_id, items)
        .await?;

    for wo_id in &wo_ids {
        if let Ok(wo) = wo_svc.find_by_id(&service_ctx, &mut conn, *wo_id).await
            && let Err(e) = wo_svc.release(&service_ctx, &mut conn, *wo_id, wo.version).await
        {
            tracing::warn!(work_order_id = wo_id, error = %e, "generate-and-release: release failed");
        }
    }

    // 计划状态 → InProgress
    if let Ok(plan) = plan_svc.find_by_id(&service_ctx, &mut conn, path.plan_id).await
        && plan.status == PlanStatus::Confirmed
    {
        let _ = plan_svc.mark_in_progress(&mut conn, path.plan_id).await;
    }

    let redirect = format!("/admin/mes/plans/{}?tab=planning", path.plan_id);
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &redirect)
        .body(axum::body::Body::empty())
        .unwrap())
}

// ── Data Loader ──

struct PlanDetailData {
    plan: ProductionPlan,
    items: Vec<ProductionPlanItem>,
    product_names: HashMap<i64, String>,
    op_name: String,
    work_orders: Vec<WorkOrder>,
    audit_logs: Vec<AuditLog>,
    validations: Vec<ReleaseValidation>,
}


#[allow(clippy::too_many_arguments)]
async fn load_plan_detail_raw(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    conn: abt_core::shared::types::PgExecutor<'_>,
    plan_id: i64,
) -> Result<PlanDetailData> {
    let svc = state.production_plan_service();
    let wo_svc = state.work_order_service();
    let user_svc = state.user_service();
    let product_svc = state.product_service();
    let audit_svc = state.audit_log_service();

    let plan = svc.find_by_id(service_ctx, conn, plan_id).await?;
    let items = svc.list_items(service_ctx, conn, plan_id).await.unwrap_or_default();
    let validations = svc.pre_validate(service_ctx, conn, plan_id).await.unwrap_or_default();
    let work_orders = wo_svc.list_by_plan(service_ctx, conn, plan_id).await.unwrap_or_default();

    let audit_logs = audit_svc
        .query_logs(
            service_ctx, conn,
            AuditLogQuery {
                entity_type: Some("ProductionPlan".to_string()),
                entity_id: Some(plan.id),
                ..Default::default()
            },
            1, 50,
        )
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let op_name = user_svc
        .get_user(service_ctx, conn, plan.operator_id)
        .await
        .ok()
        .and_then(|u| u.display_name)
        .unwrap_or_else(|| format!("#{}", plan.operator_id));

    let product_names: HashMap<i64, String> = if items.is_empty() {
        HashMap::new()
    } else {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        product_svc
            .get_by_ids(service_ctx, conn, product_ids)
            .await
            .map(|ps| ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect())
            .unwrap_or_default()
    };

    Ok(PlanDetailData { plan, items, product_names, op_name, work_orders, audit_logs, validations })
}

// ── Page ──

#[allow(clippy::too_many_arguments)]
fn plan_detail_page(
    plan: &ProductionPlan,
    items: &[ProductionPlanItem],
    op_name: &str,
    product_names: &HashMap<i64, String>,
    work_orders: &[WorkOrder],
    audit_logs: &[AuditLog],
    validations: &[ReleaseValidation],
) -> Markup {
    let (status_label, status_bg, status_color) = plan_status_label(&plan.status);
    let type_label = plan_type_label(&plan.plan_type);
    let total_qty: rust_decimal::Decimal = items.iter().map(|i| i.planned_qty).sum();
    let val_map: HashMap<i64, &ReleaseValidation> = validations.iter().map(|v| (v.plan_item_id, v)).collect();

    html! {
        div {
            // 返回
            a class="back-link" href=(PlanListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回计划列表"
            }

            // Detail Header
            div class="detail-header" {
                // 标题行
                div class="detail-title-row" {
                    div class="detail-doc-no mono" {
                        span { (plan.doc_number) }
                        (status_pill(status_label, status_bg, status_color))
                    }
                    div class="page-actions" {
                        @if plan.status == PlanStatus::Confirmed {
                            a class="btn btn-primary" href=(format!("/admin/mes/plans/{}?tab=planning", plan.id)) {
                                (icon::rocket_icon("w-4 h-4"))
                                "工单规划"
                            }
                        }
                        @if plan.status == PlanStatus::Draft {
                            button class="btn btn-primary"
                                hx-post=(PlanConfirmPath { plan_id: plan.id }.to_string())
                                hx-confirm="确认此生产计划？确认后进入已确认状态。" {
                                (icon::check_circle_icon("w-4 h-4"))
                                "确认计划"
                            }
                        }
                    }
                }

                // 来源追溯
                div class="detail-sub-row" {
                    span { "创建人：" (op_name) }
                    span class="sep" { "|" }
                    span { (fmt_dt(plan.created_at)) }
                    @if !plan.remark.is_empty() {
                        span class="sep" { "|" }
                        span class="muted" { (plan.remark.as_str()) }
                    }
                }

                // 信息 Grid（4 列）
                div class="detail-info-grid" {
                    div class="detail-info-item" {
                        span class="detail-info-label" { "计划日期" }
                        span class="detail-info-value mono" { (plan.plan_date) }
                    }
                    div class="detail-info-item" {
                        span class="detail-info-label" { "排产类型" }
                        span class="detail-info-value" { (type_label) }
                    }
                    div class="detail-info-item" {
                        span class="detail-info-label" { "生产中心" }
                        span class="detail-info-value" { "—" }
                    }
                    div class="detail-info-item" {
                        span class="detail-info-label" { "计划数量" }
                        span class="detail-info-value mono" {
                            (format!("{} 项 · {} 件", items.len(), crate::utils::fmt_qty(total_qty)))
                        }
                    }
                }
            }

            // Tabs
            (detail_tabs("detail", &[
                ("detail", &format!("计划明细 {}", items.len())),
                ("planning", "工单管理"),
                ("log", "操作日志"),
            ]))

            (tab_panel("detail", true, tab_detail(items, product_names, &val_map)))
            (tab_panel("planning", false, tab_planning(plan, items, product_names, &val_map, work_orders)))
            (tab_panel("log", false, tab_log(audit_logs)))
        }
    }
}

// ── Tab: 计划明细（9 列表格）──

fn tab_detail(
    items: &[ProductionPlanItem],
    product_names: &HashMap<i64, String>,
    val_map: &HashMap<i64, &ReleaseValidation>,
) -> Markup {
    html! {
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "#" }
                            th { "产品" }
                            th class="num-right" { "数量" }
                            th { "排程" }
                            th { "BOM/工艺" }
                            th { "完整度" }
                            th { "优先级" }
                            th { "状态" }
                        }
                    }
                    tbody {
                        @for (idx, item) in items.iter().enumerate() {
                            @let pname = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                            @let (item_label, item_bg, item_color) = item_status_label(&item.status);
                            @let val = val_map.get(&item.id).copied();
                            @let (p_label, p_color) = priority_label(item.priority);
                            tr {
                                td class="line-num" { (idx + 1) }
                                td { (pname) }
                                td class="num-right mono" { (crate::utils::fmt_qty(item.planned_qty)) }
                                td {
                                    div class="cell-stack" {
                                        span { (item.scheduled_start.format("%m-%d")) }
                                        span class="sub" { "至 " (item.scheduled_end.format("%m-%d")) }
                                    }
                                }
                                td {
                                    div class="cell-stack" {
                                        @if let Some(bom_id) = item.bom_snapshot_id {
                                            span class="mono" { "BS-" (bom_id) }
                                        } @else {
                                            span class="muted" { "—" }
                                        }
                                        @if let Some(r_id) = item.routing_id {
                                            span class="sub" { "路线 #" (r_id) }
                                        } @else {
                                            span class="sub muted" { "无路线" }
                                        }
                                    }
                                }
                                td { (completeness_dots(val)) }
                                td style=(format!("color:{p_color};font-weight:500")) { (p_label) }
                                td { (status_pill(item_label, item_bg, item_color)) }
                            }
                        }
                        @if items.is_empty() {
                            tr {
                                td colspan="8" class="empty-row" { "暂无计划明细" }
                            }
                        }
                    }
                }
            }
        }
    }
}


// ── Tab: 操作日志（audit timeline）──

fn tab_log(logs: &[AuditLog]) -> Markup {
    html! {
        @if logs.is_empty() {
            div class="empty-row" { "暂无操作日志" }
        } @else {
            div class="audit-timeline" {
                @for log in logs {
                    div class="audit-item" {
                        div class="audit-dot" {}
                        div class="audit-title" { (audit_action_label(&log.action)) }
                        div class="audit-meta" {
                            span class="mono" { (fmt_dt(log.created_at)) }
                            span class="sep" { "|" }
                            span { "操作人 #" (log.operator_id) }
                        }
                        @if let Some(changes) = log.changes.as_ref() {
                            div class="audit-desc" { (changes) }
                        }
                    }
                }
            }
        }
    }
}


// ── Tab: 工单规划（上方待规划明细 + 下方 Draft 工单）──

fn tab_planning(
    plan: &ProductionPlan,
    items: &[ProductionPlanItem],
    product_names: &HashMap<i64, String>,
    val_map: &HashMap<i64, &ReleaseValidation>,
    work_orders: &[WorkOrder],
) -> Markup {
    // 活跃工单的 plan_item_id 集合（Draft/Released/InProduction）
    let active_plan_item_ids: std::collections::HashSet<i64> = work_orders
        .iter()
        .filter(|wo| matches!(wo.status, WorkOrderStatus::Draft | WorkOrderStatus::Released | WorkOrderStatus::InProduction))
        .filter_map(|wo| wo.plan_item_id)
        .collect();

    // 上方：无活跃工单的明细项
    let pending_items: Vec<&ProductionPlanItem> = items.iter().filter(|item| !active_plan_item_ids.contains(&item.id)).collect();

    // 下方：Draft 工单
    let draft_orders: Vec<&WorkOrder> = work_orders.iter().filter(|wo| wo.status == WorkOrderStatus::Draft).collect();

    // 已下达工单（Released/InProduction/Closed）
    let released_orders: Vec<&WorkOrder> = work_orders.iter()
        .filter(|wo| matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction | WorkOrderStatus::Closed))
        .collect();

    let can_plan = matches!(plan.status, PlanStatus::Confirmed | PlanStatus::InProgress);

    html! {
        div class="wo-planning" {
            // ── 上方区块：待规划明细 ──
            @if can_plan {
                div class="planning-section" {
                    h3 class="planning-section-title" style="font-size:var(--text-base);font-weight:600;margin-bottom:var(--space-3)" {
                        "待规划明细 " span class="muted" { "(" (pending_items.len()) ")" }
                    }

                    @if pending_items.is_empty() {
                        div class="empty-row" { "所有明细已生成工单" }
                    } @else {
                        form id="wo-planning-form"
                            hx-post={(PlanGeneratePath { plan_id: plan.id }.to_string())}
                            hx-swap="none" {

                            div class="data-card" {
                                div class="data-card-scroll" {
                                    table class="data-table" {
                                        thead {
                                            tr {
                                                th style="width:32px" { input type="checkbox" class="wo-check-all" checked; }
                                                th { "产品" }
                                                th class="num-right" { "数量" }
                                                th { "排程(起→止)" }
                                                th { "工艺路线" }
                                                th { "完整度" }
                                                th { "操作" }
                                            }
                                        }
                                        tbody id="wo-planning-body" {
                                            @for item in &pending_items {
                                                @let pname = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                                                @let val = val_map.get(&item.id).copied();
                                                tr class="wo-plan-row"
                                                    data-plan-item-id=(item.id)
                                                    data-product-id=(item.product_id) {
                                                    td {
                                                        input type="checkbox" class="wo-check" checked;
                                                    }
                                                    td { (pname) }
                                                    td class="num-right mono wo-qty" { (crate::utils::fmt_qty(item.planned_qty)) }
                                                    td style="white-space:nowrap" {
                                                        input type="date" class="form-input wo-start" value=(item.scheduled_start) style="width:130px;display:inline-block";
                                                        " → "
                                                        input type="date" class="form-input wo-end" value=(item.scheduled_end) style="width:130px;display:inline-block";
                                                    }
                                                    td {
                                                        @match val {
                                                            Some(v) if v.has_routing => { "有" }
                                                            _ => { span class="muted" { "无（虚拟默认）" } }
                                                        }
                                                    }
                                                    td { (completeness_dots(val)) }
                                                    td {
                                                        button type="button" class="btn btn-default btn-sm"
                                                            _="on click call openSplitDialog(me)" { "拆分" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            input type="hidden" name="items_json" id="items_json" {};

                            div style="margin-top:var(--space-4);display:flex;gap:var(--space-3)" {
                                button type="submit" class="btn btn-primary"
                                    onclick="document.getElementById('items_json').value=collectPlanItems()" {
                                    (icon::rocket_icon("w-4 h-4"))
                                    "生成草稿工单"
                                }
                                button type="submit" class="btn btn-default"
                                    formaction=(PlanGenerateReleasePath { plan_id: plan.id }.to_string())
                                    onclick="document.getElementById('items_json').value=collectPlanItems()" {
                                    "一键生成并下达"
                                }
                            }
                        }
                    }
                }
            }

            // ── 下方区块：Draft 工单列表 ──
            @if !draft_orders.is_empty() {
                div class="planning-section" style=@if can_plan { "margin-top:var(--space-6)" } @else { "" } {
                    h3 class="planning-section-title" style="font-size:var(--text-base);font-weight:600;margin-bottom:var(--space-3)" {
                        "草稿工单 " span class="muted" { "(" (draft_orders.len()) ")" }
                    }

                    div class="data-card" {
                        div class="data-card-scroll" {
                            table class="data-table" {
                                thead {
                                    tr {
                                        th { "工单号" }
                                        th { "产品" }
                                        th class="num-right" { "数量" }
                                        th { "排程" }
                                        th { "状态" }
                                        th { "操作" }
                                    }
                                }
                                tbody {
                                    @for wo in &draft_orders {
                                        @let pname = product_names.get(&wo.product_id).map(|s| s.as_str()).unwrap_or("—");
                                        tr {
                                            td class="mono" { (wo.doc_number) }
                                            td { (pname) }
                                            td class="num-right mono" { (crate::utils::fmt_qty(wo.planned_qty)) }
                                            td style="white-space:nowrap" { (wo.scheduled_start.format("%m-%d")) " → " (wo.scheduled_end.format("%m-%d")) }
                                            td { (status_pill("草稿", "rgba(250,140,22,0.08)", "#fa8c16")) }
                                            td style="white-space:nowrap" {
                                                button class="btn btn-primary btn-sm"
                                                    hx-post=(OrderReleasePath { order_id: wo.id }.to_string())
                                                    hx-confirm="确认下达此工单？"
                                                    hx-disabled-elt="this" {
                                                    "下达"
                                                }
                                                button class="btn btn-danger btn-sm"
                                                    hx-post=(OrderCancelPath { order_id: wo.id }.to_string())
                                                    hx-confirm="确认取消此草稿工单？" {
                                                    "取消"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div style="margin-top:var(--space-4)" {
                        button class="btn btn-primary"
                            hx-post=(PlanReleaseAllPath { plan_id: plan.id }.to_string())
                            hx-confirm="确认全部下达？"
                            hx-disabled-elt="this" {
                            (icon::rocket_icon("w-4 h-4"))
                            "全部下达"
                        }
                    }
                }
            }

            // ── 已下达工单区（Released/InProduction/Closed）──
            @if !released_orders.is_empty() {
                div class="planning-section" style="margin-top:var(--space-6)" {
                    h3 class="planning-section-title" style="font-size:var(--text-base);font-weight:600;margin-bottom:var(--space-3)" {
                        "已下达工单 " span class="muted" { "(" (released_orders.len()) ")" }
                    }

                    div class="data-card" {
                        ul class="work-order-list" {
                            @for wo in &released_orders {
                                @let pname = product_names.get(&wo.product_id).map(|s| s.as_str()).unwrap_or("—");
                                @let (wo_label, _wo_bg, _wo_color) = wo_status_label(&wo.status);
                                @let status_cls = match wo.status {
                                    WorkOrderStatus::InProduction => "in-production",
                                    WorkOrderStatus::Closed => "closed",
                                    _ => "released",
                                };
                                li class="work-order-item" {
                                    div class={"wo-status-bar " (status_cls)} {}
                                    div class="wo-row-content" {
                                        // 工单号
                                        div class="wo-order-num" { (wo.doc_number) }
                                        // 产品信息
                                        div class="wo-product-info" {
                                            div class="wo-product-name" { (pname) }
                                            div class="wo-product-meta" {
                                                span { span class="wo-meta-label" { "数量 " } (crate::utils::fmt_qty(wo.planned_qty)) "件" }
                                                span { span class="wo-meta-label" { "排程 " } (wo.scheduled_start.format("%m-%d")) " → " (wo.scheduled_end.format("%m-%d")) }
                                            }
                                        }
                                        // 右侧：状态 + 进度 + 操作
                                        div class="wo-right-info" {
                                            (status_pill(wo_label, _wo_bg, _wo_color))
                                            @if let (Some(done), Some(total)) = (wo.completed_steps, wo.total_steps) {
                                                div class="wo-step" {
                                                    span class="wo-step-text" { (done) "/" (total) "步" }
                                                    div class="wo-step-bar" {
                                                        div class={"wo-step-fill " (status_cls)}
                                                            style=(format!("width: {}%", if total > 0 { done * 100 / total } else { 0 })) {}
                                                    }
                                                }
                                            }
                                            a class="wo-action-btn"
                                                href=(OrderDetailPath { id: wo.id }.to_string()) {
                                                "工单详情"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 空状态 ──
            @if pending_items.is_empty() && draft_orders.is_empty() && released_orders.is_empty() {
                div class="empty-row" style="padding:var(--space-8);text-align:center" {
                    "暂无工单数据"
                }
            }


            // 拆分弹窗（可复用 input_dialog 组件）
            @if can_plan && !pending_items.is_empty() {
                (crate::components::input_dialog::input_dialog(
                    "split-dialog",
                    "拆分明细项",
                    html! {
                        "将当前明细项拆分为两份，各拆分行可独立调整排程和参数。"
                    },
                    "split-input",
                    "第一份数量",
                    "number",
                    "输入数量",
                    "1",
                    "确认拆分",
                    "call doSplit()",
                ))
            }
            // 加载规划 JS
            script src="/wo-planning.js" {}
        }
    }
}
