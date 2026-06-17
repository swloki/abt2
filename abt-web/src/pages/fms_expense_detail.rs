use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, PreEscaped, Markup};
use rust_decimal::Decimal;

use abt_core::fms::enums::{ExpenseStatus, ExpenseType};
use abt_core::fms::expense::{ExpenseReimbursementService, ExpenseReimbursementItem};
use abt_core::shared::identity::{DepartmentService, UserService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ExpenseDetailPath, ExpenseListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn expense_type_text(t: &ExpenseType) -> &'static str {
    match t {
        ExpenseType::Travel => "差旅",
        ExpenseType::Office => "办公",
        ExpenseType::Transport => "交通",
        ExpenseType::Meal => "餐饮",
        ExpenseType::Other => "其他",
    }
}

fn status_text(s: &ExpenseStatus) -> (&'static str, &'static str) {
    match s {
        ExpenseStatus::Draft => ("草稿", "status-draft"),
        ExpenseStatus::Submitted => ("已提交", "status-submitted"),
        ExpenseStatus::Approved => ("已审批", "status-active"),
        ExpenseStatus::Paid => ("已付款", "status-active"),
        ExpenseStatus::Cancelled => ("已取消", "status-inactive"),
    }
}

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_detail(path: ExpenseDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.expense_service();
    let expense = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    // 解析申请人名称
    let user_svc = state.user_service();
    let applicant_name = user_svc
        .get_user(&service_ctx, &mut conn, expense.applicant_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // 解析操作人名称
    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, expense.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // 解析部门名称
    let dept_svc = state.department_service();
    let department_name = match expense.department_id {
        Some(id) => dept_svc
            .get_department(&service_ctx, &mut conn, id)
            .await
            .map(|d| d.department_name)
            .unwrap_or_else(|_| "—".into()),
        None => "—".into(),
    };

    let (s_text, s_class) = status_text(&expense.status);
    let remark_display = if expense.remark.is_empty() {
        maud::PreEscaped("<span style=\"color:var(--muted)\">—</span>".to_string())
    } else {
        maud::PreEscaped(format!("<span style=\"color:var(--muted)\">{}</span>", expense.remark))
    };

    let content = html! {

        // 返回链接
        a.back-link href=(format!("{}?restore=true", ExpenseListPath::PATH)) {
            (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>"#))
            " 返回列表"
        }

        // 详情头
        div.detail-header {
            div.detail-title-row {
                h1.detail-no { (expense.doc_number) }
                span class=(format!("status-pill {s_class}")) { (s_text) }
            }
        }

        // 报销信息卡片
        div.info-card {
            div.info-card-title { "报销信息" }
            div.info-grid {
                div.info-item {
                    span.info-label { "单号" }
                    span.info-value.mono { (expense.doc_number) }
                }
                div.info-item {
                    span.info-label { "申请人" }
                    span.info-value style="font-weight:600" { (applicant_name) }
                }
                div.info-item {
                    span.info-label { "所属部门" }
                    span.info-value { (department_name) }
                }
                div.info-item {
                    span.info-label { "报销日期" }
                    span.info-value { (expense.expense_date.format("%Y-%m-%d")) }
                }
                div.info-item {
                    span.info-label { "报销金额" }
                    span.info-value.mono style="font-size:var(--text-lg);color:var(--accent);font-weight:700" {
                        "¥" (format!("{:.2}", expense.total_amount))
                    }
                }
                div.info-item {
                    span.info-label { "当前状态" }
                    span.info-value {
                        span class=(format!("status-pill {s_class}")) { (s_text) }
                    }
                }
                div.info-item {
                    span.info-label { "操作人" }
                    span.info-value { (operator_name) }
                }
                div.info-item {
                    span.info-label { "创建时间" }
                    span.info-value.mono style="font-size:13px" { (expense.created_at.format("%Y-%m-%d %H:%M:%S")) }
                }
                div.info-item {
                    span.info-label { "版本号" }
                    span.info-value.mono { "v" (expense.version) }
                }
                div.info-item {
                    span.info-label { "备注" }
                    (remark_display)
                }
            }
        }

        // 费用明细卡片
        (items_card(&items, expense.total_amount))
    };

    let current_path = ExpenseDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx,
        "报销单详情",
        &claims,
        "finance",
        &current_path,
        "财务管理",
        Some(ExpenseListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(html.into_string()))
}

// ── Components ──

fn items_card(items: &[ExpenseReimbursementItem], total: Decimal) -> Markup {
    html! {
        div.info-card {
            div.info-card-title { "费用明细" }
            @if items.is_empty() {
                p style="text-align:center;padding:var(--space-6);color:var(--muted)" {
                    "暂无费用明细"
                }
            } @else {
                div.data-card-scroll {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" style="min-width:700px" {
                        thead {
                            tr {
                                th { "费用类型" }
                                th style="text-align:right" { "金额" }
                                th { "说明" }
                                th { "发票号" }
                                th { "成本中心" }
                                th { "利润中心" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item))
                            }
                        }
                    }
                }
                div.amount-summary {
                    div.amount-row {
                        span.amount-label { "合计金额" }
                        span.amount-value.accent.mono { "¥" (format!("{:.2}", total)) }
                    }
                }
            }
        }
    }
}

fn item_row(item: &ExpenseReimbursementItem) -> Markup {
    let type_label = expense_type_text(&item.expense_type);
    let receipt = item.receipt_no.as_deref().unwrap_or("—");
    let cost = item.cost_center.map(|id| format!("CC-{:03}", id)).unwrap_or_else(|| "—".into());
    let profit = item.profit_center.map(|id| format!("PC-{:03}", id)).unwrap_or_else(|| "—".into());

    html! {
        tr {
            td {
                span.tag-chip.tag-key { (type_label) }
            }
            td.num-right style="font-weight:600" { "¥" (format!("{:.2}", item.amount)) }
            td { (item.description) }
            td.mono style="font-size:12px" { (receipt) }
            td { (cost) }
            td { (profit) }
        }
    }
}
