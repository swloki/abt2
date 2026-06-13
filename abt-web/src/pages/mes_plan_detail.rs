use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::mes::enums::{PlanItemStatus, PlanStatus, PlanType, WorkOrderStatus};
use abt_core::mes::production_plan::{
    BatchReleaseResult, ProductionPlan, ProductionPlanItem, ProductionPlanService,
    ReleaseValidation,
};
use abt_core::mes::work_order::{WorkOrder, WorkOrderService};
use abt_core::shared::audit_log::{AuditLog, AuditLogQuery, AuditLogService};
use abt_core::shared::enums::audit::AuditAction;
use abt_core::shared::identity::UserService;

use crate::components::detail::{detail_tabs, tab_panel};
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::OrderDetailPath;
use crate::routes::mes_plan::{PlanConfirmPath, PlanDetailPath, PlanListPath, PlanReleasePath};
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
        &data.work_orders, &data.audit_logs, &data.validations, None,
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
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let result = state
        .production_plan_service()
        .release_to_work_orders(&service_ctx, &mut conn, path.plan_id)
        .await?;

    // 重新加载详情数据（plan 状态已变）
    let data = load_plan_detail_raw(&state, &service_ctx, &mut conn, path.plan_id).await?;

    let content = plan_detail_page(
        &data.plan, &data.items, &data.op_name, &data.product_names,
        &data.work_orders, &data.audit_logs, &data.validations, Some(&result),
    );
    let page_html = admin_page(
        is_htmx, "生产计划详情", &claims, "production",
        &format!("/admin/mes/plans/{}", path.plan_id),
        "生产管理", Some(PlanListPath::PATH), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
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
    release_result: Option<&BatchReleaseResult>,
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

            // 下达结果横幅（即时反馈）
            @if let Some(result) = release_result {
                (release_result_banner(result))
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
                            button class="btn btn-primary" type="button" {
                                (icon::rocket_icon("w-4 h-4"))
                                "确认并下达"
                                (maud::PreEscaped(r#"<script>me().on('click',function(){me('#release-dialog').classAdd('is-open')})</script>"#))
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
                ("result", "下达结果"),
                ("log", "操作日志"),
            ]))

            (tab_panel("detail", true, tab_detail(items, product_names, &val_map)))
            (tab_panel("result", false, tab_result(work_orders, product_names)))
            (tab_panel("log", false, tab_log(audit_logs)))

            // 确认下达 Modal（Confirmed 状态）
            @if plan.status == PlanStatus::Confirmed {
                div class="modal-overlay" id="release-dialog" {
                    div class="modal" {
                        div class="modal-head" {
                            h2 { "确认下达生产计划？" }
                        }
                        div class="modal-body" {
                            p class="modal-desc" {
                                "下达后将根据计划明细生成对应工单、生产批次和工序记录。请确认以下校验结果："
                            }

                            // 预校验结果
                            div class="release-preview" {
                                @if items.is_empty() {
                                    p class="modal-desc" { "暂无计划明细" }
                                } @else {
                                    @for item in items {
                                        @let pname = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                                        @let val = val_map.get(&item.id).copied();
                                        div class="release-preview-item" {
                                            div class="ri-header" {
                                                span class="ri-product" { (pname) " · " (crate::utils::fmt_qty(item.planned_qty)) "件" }
                                                (completeness_dots(val))
                                            }
                                            @if let Some(v) = val {
                                                @if !v.has_published_bom {
                                                    div class="ri-warn" { "⚠ 未找到已发布的 BOM" }
                                                }
                                                @if !v.has_routing {
                                                    div class="ri-warn" { "⚠ 未配置工艺路线" }
                                                }
                                                @for s in &v.material_shortages {
                                                    div class="ri-warn" {
                                                        (format!("⚠ 物料短缺：需求 {}，库存 {}，缺口 {}",
                                                            crate::utils::fmt_qty(s.required_qty),
                                                            crate::utils::fmt_qty(s.available_qty),
                                                            crate::utils::fmt_qty(s.shortage_qty)))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div class="modal-foot" {
                            button class="btn btn-default" type="button" {
                                "取消"
                                (maud::PreEscaped(r#"<script>me().on('click',function(){me('#release-dialog').classRemove('is-open')})</script>"#))
                            }
                            button class="btn btn-primary"
                                hx-post=(PlanReleasePath { plan_id: plan.id }.to_string())
                                hx-disabled-elt="this" {
                                (icon::rocket_icon("w-4 h-4"))
                                "确认下达"
                            }
                        }
                    }
                }
            }
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

// ── Tab: 下达结果（已生成工单列表）──

fn tab_result(
    work_orders: &[WorkOrder],
    product_names: &HashMap<i64, String>,
) -> Markup {
    let success_count = work_orders.iter().filter(|w| !matches!(w.status, WorkOrderStatus::Cancelled)).count();

    html! {
        @if work_orders.is_empty() {
            div class="empty-row" { "暂无已下达工单（计划尚未下达或无成功项）" }
        } @else {
            // Summary
            div class="release-summary" {
                span { "✅ 成功: " strong { (success_count) } " 个工单" }
            }

            div style="margin-top:var(--space-5)" {
                @for wo in work_orders {
                    @let pname = product_names.get(&wo.product_id).map(|s| s.as_str()).unwrap_or("—");
                    @let (wo_label, wo_bg, wo_color) = wo_status_label(&wo.status);
                    @let is_cancelled = matches!(wo.status, WorkOrderStatus::Cancelled);
                    div class=(if is_cancelled { "release-result-item fail" } else { "release-result-item success" }) {
                        div class="ri-header" {
                            div class="ri-product" {
                                (if is_cancelled { "❌ " } else { "✅ " })
                                (pname) " · " (crate::utils::fmt_qty(wo.planned_qty)) "件"
                            }
                            (status_pill(wo_label, wo_bg, wo_color))
                        }
                        div class="ri-detail" {
                            span { "工单: " (wo.doc_number) }
                            @if let Some(steps) = wo.total_steps {
                                span { "工序: " (steps) "步" }
                            }
                            span { "排程: " (wo.scheduled_start.format("%m-%d")) " 至 " (wo.scheduled_end.format("%m-%d")) }
                        }
                        div class="ri-actions" {
                            a class="btn btn-default btn-sm"
                                href=(OrderDetailPath { id: wo.id }.to_string()) {
                                "→ 工单详情"
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

// ── 下达结果横幅（即时反馈）──

fn release_result_banner(result: &BatchReleaseResult) -> Markup {
    let success_count = result.successful_work_orders.len();
    let fail_count = result.failed_items.len();

    let shortage_warnings: Vec<String> = result
        .validations
        .iter()
        .flat_map(|v| {
            v.material_shortages.iter().map(|s| {
                format!(
                    "物料短缺：需求 {}，库存 {}，缺口 {}",
                    crate::utils::fmt_qty(s.required_qty),
                    crate::utils::fmt_qty(s.available_qty),
                    crate::utils::fmt_qty(s.shortage_qty),
                )
            })
        })
        .collect();

    let (banner_bg, banner_border, icon_markup) = if fail_count > 0 {
        ("rgba(245,63,63,0.06)", "#f53f3f", icon::circle_alert_icon("w-5 h-5"))
    } else if !shortage_warnings.is_empty() {
        ("rgba(250,140,22,0.08)", "#fa8c16", icon::alert_triangle_icon("w-5 h-5"))
    } else {
        ("rgba(82,196,26,0.08)", "var(--success)", icon::check_circle_icon("w-5 h-5"))
    };

    html! {
        div class="release-result-banner" style=(format!(
            "margin-bottom:var(--space-4);padding:var(--space-4) var(--space-5);border-radius:var(--radius-lg);border-left:3px solid {banner_border};background:{banner_bg}"
        )) {
            div style="display:flex;align-items:center;gap:var(--space-2);font-weight:600;font-size:var(--text-sm)" {
                (icon_markup)
                span { (format!("{success_count} 个工单下达成功")) }
                @if fail_count > 0 {
                    span style=(format!("margin-left:var(--space-2);color:{banner_border}")) {
                        (format!("{fail_count} 个工单下达失败"))
                    }
                }
            }
            @if fail_count > 0 {
                div style="margin-top:var(--space-2)" {
                    @for fail in &result.failed_items {
                        div style="font-size:var(--text-xs);color:#f53f3f;margin-top:var(--space-1)" {
                            (format!("第 {} 项失败：{}", fail.index + 1, fail.error))
                        }
                    }
                }
            }
            @if !shortage_warnings.is_empty() {
                div style="margin-top:var(--space-2)" {
                    @for w in &shortage_warnings {
                        div style="font-size:var(--text-xs);color:#fa8c16;margin-top:var(--space-1)" {
                            (icon::alert_triangle_icon("w-3-5 h-3-5")) " " (w)
                        }
                    }
                }
            }
        }
    }
}
