use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use rust_decimal::Decimal;

use abt_core::fms::enums::{ExpenseStatus, ExpenseType};
use abt_core::fms::expense::{ExpenseReimbursementService, ExpenseReimbursementItem, ExpenseReimbursement};
use abt_core::shared::identity::{DepartmentService, UserService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ExpenseDetailPath, ExpenseListPath, ExpenseApprovePath, ExpensePayPath, ExpenseSubmitPath};
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
        ExpenseStatus::Approved => ("已通过", "status-active"),
        ExpenseStatus::Paid => ("已付款", "status-active"),
        ExpenseStatus::Cancelled => ("已取消", "status-inactive"),
        ExpenseStatus::SupervisorApproved => ("直属上级已批", "status-submitted"),
        ExpenseStatus::FinanceApproved => ("财务已审", "status-submitted"),
    }
}

// ── Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_detail(path: ExpenseDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.expense_service();
    let expense = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    // 审批进度
    let progress = svc.get_approval_progress(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    // 附件
    let attachments = svc.list_attachments(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

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

    // Pre-compute approve button label
    let approve_text = match expense.status {
        ExpenseStatus::Submitted => "直属上级审批通过",
        ExpenseStatus::SupervisorApproved => "财务审批通过",
        ExpenseStatus::FinanceApproved => "总经理审批通过",
        _ => "",
    };

    let content = html! {
        // 返回链接
        a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
          href=(format!("{}?restore=true", ExpenseListPath::PATH)) {
            (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>"#))
            " 返回列表"
        }

        // 标题行
        div class="flex items-center gap-4 mb-6" {
            h1 class="text-xl font-bold font-mono tabular-nums" { (expense.doc_number) }
            span class=(format!("status-pill {}", crate::utils::status_color(s_class))) { (s_text) }
        }

        // ── 审批进度条 ──
        (approval_progress_bar(&progress))

        // ── 操作按钮（按当前 status 条件渲染）──
        div class="flex gap-3 mb-6" {
            @if expense.status == ExpenseStatus::Draft {
                a class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
                  hx-post=(ExpenseSubmitPath { id: path.id }.to_string())
                  hx-swap="none"
                  _="on 'htmx:afterRequest'[detail.xhr.status < 400] location.reload()" {
                    "提交审批"
                }
            } @else if matches!(expense.status, ExpenseStatus::Submitted | ExpenseStatus::SupervisorApproved | ExpenseStatus::FinanceApproved) {
                a class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-success text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
                  hx-post=(ExpenseApprovePath { id: path.id }.to_string())
                  hx-swap="none"
                  _="on 'htmx:afterRequest'[detail.xhr.status < 400] location.reload()" {
                    (approve_text)
                }
            } @else if expense.status == ExpenseStatus::Approved {
                button type="button"
                  class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-warning text-white text-sm font-medium hover:opacity-90 cursor-pointer transition-all duration-150"
                  _="on click add .is-open to #pay-modal" {
                    "出纳付款"
                }
            }
        }

        // ── 报销信息卡片 ──
        (info_card(&expense, &applicant_name, &department_name, &operator_name, s_text, s_class))

        // ── 费用明细卡片 ──
        (items_card(&items, expense.total_amount))

        // ── 附件展示 ──
        @if !attachments.is_empty() {
            (attachments_card(&attachments))
        }

        // ── 付款信息（已付款时展示）──
        @if expense.status == ExpenseStatus::Paid {
            (payment_info_card(&expense))
        }

        // ── 付款弹窗 ──
        (pay_modal(path.id))
    };

    let current_path = ExpenseDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx, "报销单详情", &claims, "finance", &current_path,
        "财务管理", Some(ExpenseListPath::PATH), content, &nav_filter,
    );
    Ok(Html(html.into_string()))
}

#[require_permission("FMS", "update")]
pub async fn submit(path: ExpenseSubmitPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.expense_service();
    svc.submit(&service_ctx, &mut conn, path.id).await?;
    let redirect = ExpenseDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// 统一审批入口 — 根据当前状态自动判定审批阶段
#[require_permission("FMS", "update")]
pub async fn approve(path: ExpenseApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.expense_service();

    // 先获取当前状态
    let expense = svc.get(&service_ctx, &mut conn, path.id).await?;

    match expense.status {
        ExpenseStatus::Submitted => {
            svc.supervisor_approve(&service_ctx, &mut conn, path.id,
                abt_core::fms::expense::model::SupervisorApproveReq { remark: None },
            ).await?;
        }
        ExpenseStatus::SupervisorApproved => {
            svc.finance_approve(&service_ctx, &mut conn, path.id,
                abt_core::fms::expense::model::FinanceApproveReq { remark: None },
            ).await?;
        }
        ExpenseStatus::FinanceApproved => {
            svc.approve(&service_ctx, &mut conn, path.id).await?;
        }
        _ => {
            return Ok(([("HX-Redirect", ExpenseDetailPath { id: path.id }.to_string())], Html(String::new())));
        }
    }

    let redirect = ExpenseDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// 付款表单
#[derive(Debug, serde::Deserialize)]
pub struct PayForm {
    payment_bank: String,
    payment_date: String,
    payment_remark: String,
}

#[require_permission("FMS", "update")]
pub async fn pay(path: ExpensePayPath, ctx: RequestContext, axum::Form(form): axum::Form<PayForm>) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.expense_service();
    svc.pay(&service_ctx, &mut conn, path.id, abt_core::fms::expense::model::PayReq {
        payment_bank: form.payment_bank,
        payment_remark: form.payment_remark,
        payment_date: chrono::NaiveDate::parse_from_str(&form.payment_date, "%Y-%m-%d")
            .unwrap_or(chrono::Utc::now().date_naive()),
    }).await?;
    let redirect = ExpenseDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

/// 审批进度条
fn approval_progress_bar(progress: &[abt_core::fms::expense::model::ApprovalProgressNode]) -> Markup {
    if progress.is_empty() {
        return html! {};
    }

    let total = progress.len();
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
            div class="text-base font-semibold text-fg mb-5 pb-3 border-b border-border-soft" { "审批进度" }
            div class="flex items-center justify-between" {
                @for (i, node) in progress.iter().enumerate() {
                    div class="flex flex-col items-center flex-1" {
                        // 节点圆点
                        div class=(format!(
                            "w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold mb-2 {}",
                            match node.status.as_str() {
                                "completed" => "bg-success text-white",
                                "current" => "bg-accent text-white ring-4 ring-accent/20",
                                _ => "bg-border-soft text-muted",
                            }
                        )) {
                            @if node.status == "completed" {
                                (PreEscaped(r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M20 6L9 17l-5-5"/></svg>"#))
                            } @else if node.status == "current" {
                                span { ((i + 1).to_string()) }
                            } @else {
                                span { ((i + 1).to_string()) }
                            }
                        }
                        // 标签
                        span class=(format!(
                            "text-xs font-medium mb-1 {}",
                            if node.status == "current" { "text-accent" } else if node.status == "completed" { "text-fg" } else { "text-muted" }
                        )) { (node.label) }
                        // 操作人
                        @if let Some(ref name) = node.operator_name {
                            span class="text-xs text-muted" { (name) }
                        } @else if node.status == "current" {
                            span class="text-xs text-accent" { "处理中" }
                        } @else {
                            span class="text-xs text-muted" { "—" }
                        }
                    }
                    // 连接线（最后一个节点不加）
                    @if i < total - 1 {
                        div class=(format!(
                            "flex-1 h-0.5 mx-2 mt-[-28px] {}",
                            if node.status == "completed" { "bg-success" } else { "bg-border-soft" }
                        )) {}
                    }
                }
            }
        }
    }
}

/// 报销信息卡片
fn info_card(
    expense: &ExpenseReimbursement,
    applicant_name: &str,
    department_name: &str,
    operator_name: &str,
    s_text: &str,
    s_class: &str,
) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "报销信息" }
            div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "单号" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" { (expense.doc_number) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "申请人" }
                    span class="text-sm text-fg font-semibold" { (applicant_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "所属部门" }
                    span class="text-sm text-fg font-medium" { (department_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "报销日期" }
                    span class="text-sm text-fg font-medium" { (expense.expense_date.format("%Y-%m-%d")) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "报销金额" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums text-accent font-bold text-lg" {
                        "¥" (format!("{:.2}", expense.total_amount))
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "单据张数" }
                    span class="text-sm text-fg font-medium" { (expense.sheet_count) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "当前状态" }
                    span class="text-sm text-fg font-medium" {
                        span class=(format!("status-pill {}", crate::utils::status_color(s_class))) { (s_text) }
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "操作人" }
                    span class="text-sm text-fg font-medium" { (operator_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "创建时间" }
                    span class="text-[13px] text-fg font-medium font-mono tabular-nums" { (expense.created_at.format("%Y-%m-%d %H:%M:%S")) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "备注" }
                    span class="text-sm text-muted" {
                        @if expense.remark.is_empty() { "—" } @else { (expense.remark) }
                    }
                }
            }
        }
    }
}

/// 付款信息卡片（已付款时展示）
fn payment_info_card(expense: &ExpenseReimbursement) -> Markup {
    let bank = expense.payment_bank.as_deref().unwrap_or("—");
    let remark = expense.payment_remark.as_deref().unwrap_or("—");
    let date = expense.payment_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());

    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "付款信息" }
            div class="grid gap-5 [grid-template-columns:repeat(auto-fill,minmax(200px,1fr))]" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "付款银行" }
                    span class="text-sm text-fg font-medium" { (bank) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "付款日期" }
                    span class="text-sm text-fg font-medium" { (date) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "付款备注" }
                    span class="text-sm text-muted" { (remark) }
                }
            }
        }
    }
}

/// 费用明细卡片
fn items_card(items: &[ExpenseReimbursementItem], total: Decimal) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "费用明细" }
            @if items.is_empty() {
                p class="text-center text-muted p-6" { "暂无费用明细" }
            } @else {
                div class="overflow-x-auto" {
                    table class="data-table" style="min-width:700px" {
                        thead {
                            tr {
                                th { "费用类型" }
                                th class="text-right" { "金额" }
                                th { "发生日期" }
                                th { "说明" }
                                th { "发票号" }
                                th { "有无发票" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item))
                            }
                        }
                    }
                }
                div class="flex items-center justify-end gap-6 mt-4 pt-4 border-t border-border-soft" {
                    span class="text-xs text-muted" { "合计金额" }
                    span class="text-lg font-bold font-mono tabular-nums text-accent" { "¥" (format!("{:.2}", total)) }
                }
            }
        }
    }
}

fn item_row(item: &ExpenseReimbursementItem) -> Markup {
    let type_label = expense_type_text(&item.expense_type);
    let receipt = item.receipt_no.as_deref().unwrap_or("—");
    let occurrence = item.occurrence_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
    let has_invoice = if item.has_invoice { "有" } else { "无" };

    html! {
        tr {
            td {
                span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium text-muted mr-1" { (type_label) }
            }
            td class="text-right font-semibold" { "¥" (format!("{:.2}", item.amount)) }
            td class="text-xs text-muted" { (occurrence) }
            td { (item.description) }
            td class="font-mono tabular-nums text-xs" { (receipt) }
            td class="text-xs" { (has_invoice) }
        }
    }
}

/// 附件卡片
fn attachments_card(attachments: &[abt_core::fms::expense::model::ExpenseAttachment]) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-6 shadow-[var(--shadow-card)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "报销凭证" }
            div class="flex flex-wrap gap-3" {
                @for att in attachments {
                    div class="w-24 h-24 border border-border rounded-sm flex flex-col items-center justify-center bg-surface text-muted text-xs gap-1 cursor-pointer hover:border-accent transition-colors" {
                        span class="text-xl" { "📎" }
                        span class="truncate w-full px-1 text-center" { (att.file_name) }
                    }
                }
            }
        }
    }
}

/// 付款弹窗
fn pay_modal(expense_id: i64) -> Markup {
    html! {
        div id="pay-modal"
          class="modal-overlay fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
          _="on click[me is event.target] remove .is-open" {

            div class="bg-bg border border-border rounded-lg p-6 w-full max-w-md mx-4 shadow-xl" {
                // 注：不加 _="on click halt"——halt 的 preventDefault 会阻止「确认付款」submit
                // 按钮的默认 form submit 行为，导致点击无响应。背景关闭已由上方 overlay 的
                // [me is event.target] 过滤实现，点内容不会误关，无需内层 halt。
                h2 class="text-lg font-bold text-fg mb-4" { "确认付款" }

                form hx-post=(ExpensePayPath { id: expense_id }.to_string())
                  hx-swap="none"
                  _="on 'htmx:afterRequest'[detail.xhr.status < 400] location.reload()" {

                    div class="form-field mb-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "付款银行" }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                          type="text" name="payment_bank" placeholder="请输入付款银行" required;
                    }
                    div class="form-field mb-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "付款日期" }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                          type="date" name="payment_date" required;
                    }
                    div class="form-field mb-6" {
                        label class="block text-xs font-medium text-fg-2 mb-1" { "付款备注" }
                        textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y min-h-72px"
                          name="payment_remark" placeholder="请输入付款备注" {}
                    }

                    div class="flex justify-end gap-3" {
                        button type="button"
                          class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm font-medium cursor-pointer transition-all duration-150"
                          _="on click remove .is-open from #pay-modal" {
                            "取消"
                        }
                        button type="submit"
                          class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150" {
                            "确认付款"
                        }
                    }
                }
            }
        }
    }
}
