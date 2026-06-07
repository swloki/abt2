use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::mes::enums::{PlanItemStatus, PlanStatus};
use abt_core::mes::production_plan::ProductionPlanService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
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

fn plan_type_label(t: &abt_core::mes::enums::PlanType) -> &'static str {
    match t {
        abt_core::mes::enums::PlanType::Mto => "按单生产 (MTO)",
        abt_core::mes::enums::PlanType::Mts => "按库存备货 (MTS)",
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

fn status_pill(label: &str, bg: &str, color: &str) -> Markup {
    html! {
        span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 10px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{bg};color:{color}")) {
            (label)
        }
    }
}

// ── Handlers ──

#[require_permission("MES", "read")]
pub async fn get_plan_detail(
    path: PlanDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_plan_service();
    let user_svc = state.user_service();
    let product_svc = state.product_service();

    let plan = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let op_name = user_svc
        .get_user(&service_ctx, &mut conn, plan.operator_id)
        .await
        .ok()
        .and_then(|u| u.display_name)
        .unwrap_or_default();

    let product_names: HashMap<i64, String> = if items.is_empty() {
        HashMap::new()
    } else {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .map(|ps| ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect())
            .unwrap_or_default()
    };

    let content = plan_detail_page(&plan, &items, &op_name, &product_names);
    let page_html = admin_page(
        is_htmx,
        "生产计划详情",
        &claims,
        "production",
        &format!("/admin/mes/plans/{}", path.id),
        "生产管理",
        Some("生产计划"),
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("MES", "write")]
pub async fn confirm_plan(
    path: PlanConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_plan_service().confirm(&service_ctx, &mut conn, path.plan_id).await?;

    let redirect = PlanDetailPath { id: path.plan_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("MES", "write")]
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

// ── Components ──

fn plan_detail_page(
    plan: &abt_core::mes::production_plan::ProductionPlan,
    items: &[abt_core::mes::production_plan::ProductionPlanItem],
    op_name: &str,
    product_names: &HashMap<i64, String>,
) -> Markup {
    let (status_label, status_bg, status_color) = plan_status_label(&plan.status);
    let type_label = plan_type_label(&plan.plan_type);

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(PlanListPath::PATH) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回生产计划列表"
            }

            // ── Detail Header ──
            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (plan.doc_number) }
                        (status_pill(status_label, status_bg, status_color))
                    }
                }
                div class="page-actions" {
                    @if plan.status == PlanStatus::Draft {
                        button class="btn btn-primary"
                            hx-post=(PlanConfirmPath { plan_id: plan.id }.to_string())
                            hx-confirm="确认此生产计划？确认后计划将进入已确认状态。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认计划"
                        }
                    }
                    @if plan.status == PlanStatus::Confirmed {
                        button class="btn btn-primary"
                            hx-post=(PlanReleasePath { plan_id: plan.id }.to_string())
                            hx-confirm="下达此生产计划？将根据计划行生成对应工单。" {
                            (icon::rocket_icon("w-4 h-4"))
                            "下达计划"
                        }
                    }
                }
            }

            // ── Plan Info ──
            div class="info-card" {
                div class="info-card-title" { "计划信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "计划编号" }
                        span class="info-value mono" { (plan.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "计划日期" }
                        span class="info-value mono" { (plan.plan_date) }
                    }
                    div class="info-item" {
                        span class="info-label" { "排产类型" }
                        span class="info-value" { (type_label) }
                    }
                    div class="info-item" {
                        span class="info-label" { "状态" }
                        span class="info-value" { (status_pill(status_label, status_bg, status_color)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建人" }
                        span class="info-value" { (op_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" style="font-size:12px" { (plan.created_at.format("%Y-%m-%d %H:%M")) }
                    }
                }
            }

            // ── Remark ──
            @if !plan.remark.is_empty() {
                div class="info-card" style="margin-top:var(--space-4)" {
                    div class="info-card-title" { "备注" }
                    p style="color:var(--muted);font-size:var(--text-sm)" { (plan.remark.as_str()) }
                }
            }

            // ── Plan Items Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "序号" }
                                th { "产品名称" }
                                th class="num-right" { "计划数量" }
                                th { "计划开始" }
                                th { "计划结束" }
                                th class="num-right" { "优先级" }
                                th { "状态" }
                            }
                        }
                        tbody {
                            @for (idx, item) in items.iter().enumerate() {
                                @let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                                @let (item_label, item_bg, item_color) = item_status_label(&item.status);
                                tr {
                                    td class="line-num" { (idx + 1) }
                                    td { (product_name) }
                                    td class="num-right" { (crate::utils::fmt_qty(item.planned_qty)) }
                                    td { (item.scheduled_start) }
                                    td { (item.scheduled_end) }
                                    td class="num-right" { (item.priority) }
                                    td { (status_pill(item_label, item_bg, item_color)) }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无计划明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
