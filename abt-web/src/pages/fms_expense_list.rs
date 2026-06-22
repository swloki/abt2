use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::fms::enums::{ExpenseStatus, ExpenseType};
use abt_core::fms::expense::model::{ExpenseFilter, ExpenseReimbursement};
use abt_core::fms::expense::ExpenseReimbursementService;
use abt_core::shared::identity::DepartmentService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ExpenseCreatePath, ExpenseDetailPath, ExpenseListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

#[allow(dead_code)]
fn expense_type_label(t: &ExpenseType) -> (&'static str, &'static str, &'static str) {
 match t {
 ExpenseType::Travel => ("差旅", "bg-accent-bg", "text-accent"),
 ExpenseType::Office => ("办公", "bg-purple-bg", "text-purple"),
 ExpenseType::Transport => ("交通", "bg-success-bg", "text-success"),
 ExpenseType::Meal => ("餐饮", "bg-warn-bg", "text-warn-700"),
 ExpenseType::Other => ("其他", "bg-accent-bg", "text-muted"),
 }
}

fn expense_status_label(s: &ExpenseStatus) -> (&'static str, &'static str, &'static str) {
 match s {
 ExpenseStatus::Draft => ("草稿", "bg-accent-bg", "text-muted"),
 ExpenseStatus::Submitted => ("已提交", "bg-accent-bg", "text-accent"),
 ExpenseStatus::Approved => ("已通过", "bg-success-bg", "text-success"),
 ExpenseStatus::Paid => ("已付款", "bg-success-50", "text-success-600"),
 ExpenseStatus::Cancelled => ("已取消", "bg-danger-100", "text-danger"),
 ExpenseStatus::SupervisorApproved => ("直属上级已批", "bg-accent-bg", "text-accent"),
 ExpenseStatus::FinanceApproved => ("财务已审", "bg-purple-bg", "text-purple"),
 }
}

/// 解析申请人姓名和部门名称
async fn resolve_names(
 state: &crate::state::AppState,
 ctx: &abt_core::shared::types::ServiceContext,
 db: &mut abt_core::shared::types::PgPoolConn,
 items: &[ExpenseReimbursement],
) -> (HashMap<i64, String>, HashMap<i64, String>) {
 // 解析申请人
 let applicant_ids: Vec<i64> = items.iter().map(|e| e.applicant_id).collect();
 let mut applicant_names: HashMap<i64, String> = HashMap::new();
 if !applicant_ids.is_empty() {
 let svc = state.user_service();
 if let Ok(users) = UserService::get_users_by_ids(&svc, ctx, &mut *db, applicant_ids).await {
 for u in users {
 applicant_names.insert(u.user.user_id, u.user.display_name.unwrap_or(u.user.username));
 }
 }
 }

 // 解析部门
 let dept_ids: Vec<i64> = items.iter().filter_map(|e| e.department_id).collect();
 let mut dept_names: HashMap<i64, String> = HashMap::new();
 if !dept_ids.is_empty() {
 let svc = state.department_service();
 for did in &dept_ids {
 if let Ok(dept) = svc.get_department(ctx, &mut *db, *did).await {
 dept_names.insert(dept.department_id, dept.department_name);
 }
 }
 }

 (applicant_names, dept_names)
}

fn build_query_string(params: &ExpenseQueryParams) -> String {
 let mut parts = Vec::new();
 if let Some(v) = params.status {
 parts.push(format!("status={v}"));
 }
 if let Some(v) = params.applicant_id {
 parts.push(format!("applicant_id={v}"));
 }
 if let Some(v) = params.department_id {
 parts.push(format!("department_id={v}"));
 }
 if let Some(v) = &params.expense_date_from {
 parts.push(format!("expense_date_from={v}"));
 }
 if let Some(v) = &params.expense_date_to {
 parts.push(format!("expense_date_to={v}"));
 }
 if let Some(v) = params.page
 && v > 1 {
 parts.push(format!("page={v}"));
 }
 if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ExpenseQueryParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub applicant_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub department_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub expense_date_from: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub expense_date_to: Option<String>,
}

// ── Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_list(
 _path: ExpenseListPath,
 ctx: RequestContext,
 Query(params): Query<ExpenseQueryParams>,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("FMS", "create").await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.expense_service();

 let filter = build_filter(&params);
 let page_num = params.page.unwrap_or(1);
 let result = svc
 .list(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page_num, 20))
 .await?;

 let (applicant_names, dept_names) = resolve_names(&state, &service_ctx, &mut conn, &result.items).await;

 // 获取所有部门和用户用于筛选下拉
 let all_depts = state.department_service().list_departments(&service_ctx, &mut conn).await.unwrap_or_default();
 let all_users = UserService::list_users_with_roles(&state.user_service(), &service_ctx, &mut conn).await.unwrap_or_default();

 let content = expense_list_page(&result, &params, &applicant_names, &dept_names, &all_depts, &all_users, can_create);
 let page_html = admin_page(
 is_htmx,
 "费用报销",
 &claims,
 "finance",
 ExpenseListPath::PATH,
 "财务管理",
 None,
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

fn build_filter(params: &ExpenseQueryParams) -> ExpenseFilter {
 let status_vec = match params.status {
 // 新 Tab 映射: 1=待审批(Submitted+SupervisorApproved+FinanceApproved), 2=已通过(Approved), 3=已付款(Paid)
 Some(1) => vec![ExpenseStatus::Submitted, ExpenseStatus::SupervisorApproved, ExpenseStatus::FinanceApproved],
 Some(2) => vec![ExpenseStatus::Approved],
 Some(3) => vec![ExpenseStatus::Paid],
 Some(4) => vec![ExpenseStatus::Draft],
 Some(5) => vec![ExpenseStatus::Cancelled],
 _ => vec![],
 };
 ExpenseFilter {
 status: status_vec,
 applicant_id: params.applicant_id,
 department_id: params.department_id,
 expense_date_from: params.expense_date_from.as_deref().and_then(|s| s.parse().ok()),
 expense_date_to: params.expense_date_to.as_deref().and_then(|s| s.parse().ok()),
 }
}

// ── Components ──

fn expense_list_page(
 result: &PaginatedResult<ExpenseReimbursement>,
 params: &ExpenseQueryParams,
 applicant_names: &HashMap<i64, String>,
 dept_names: &HashMap<i64, String>,
 all_depts: &[abt_core::shared::identity::Department],
 all_users: &[abt_core::shared::identity::UserWithRoles],
 can_create: bool,
) -> Markup {
 html! {
    div {
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "费用报销" }
            div class="flex gap-3" {
                button
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    type="button"
                { (icon::download_icon("w-4 h-4")) "导出" }
                @if can_create {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        href=(ExpenseCreatePath::PATH)
                    { (icon::plus_icon("w-4 h-4")) "新建报销" }
                }
            }
        }
        ({
            expense_table_fragment(
                result,
                params,
                applicant_names,
                dept_names,
                all_depts,
                all_users,
            )
        })
    }
}
}

#[allow(clippy::too_many_arguments)]
fn expense_table_fragment(
 result: &PaginatedResult<ExpenseReimbursement>,
 params: &ExpenseQueryParams,
 applicant_names: &HashMap<i64, String>,
 dept_names: &HashMap<i64, String>,
 all_depts: &[abt_core::shared::identity::Department],
 all_users: &[abt_core::shared::identity::UserWithRoles],
) -> Markup {
 let total_count = result.total;
 let selected_status = params.status.map(|v| v.to_string()).unwrap_or_default();

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "待审批", count: None },
 TabItem { value: "2".into(), label: "已通过", count: None },
 TabItem { value: "3".into(), label: "已付款", count: None },
 ];

 html! {
    div {
        ({
            status_tabs_with_param(
                ExpenseListPath::PATH,
                "#expense-data-card",
                "#expense-filter-form",
                tabs,
                &selected_status,
                "status",
            )
        })

        form
            class="flex items-center gap-3 mb-5 flex-wrap filter-form"
            id="expense-filter-form"
            hx-get=(ExpenseListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#expense-data-card"
            hx-select="#expense-data-card"
            hx-swap="outerHTML"
            hx-include="#expense-filter-form"
            hx-push-url="true"
        {
            select
                class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                name="status"
            {
                option value="" selected[params.status.is_none()] { "全部状态" }
                option value="1" selected[params.status == Some(1)] { "待审批" }
                option value="2" selected[params.status == Some(2)] { "已通过" }
                option value="3" selected[params.status == Some(3)] { "已付款" }
                option value="4" selected[params.status == Some(4)] { "草稿" }
                option value="5" selected[params.status == Some(5)] { "已取消" }
            }
            select
                class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                name="applicant_id"
            {
                option value="" selected[params.applicant_id.is_none()] { "全部申请人" }
                @for u in all_users {
                    option
                        value=(u.user.user_id)
                        selected[params.applicant_id == Some(u.user.user_id)]
                    { (u.user.display_name.as_deref().unwrap_or(&u.user.username)) }
                }
            }
            select
                class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                name="department_id"
            {
                option value="" selected[params.department_id.is_none()] { "全部部门" }
                @for d in all_depts {
                    option
                        value=(d.department_id)
                        selected[params.department_id == Some(d.department_id)]
                    { (d.department_name) }
                }
            }
            input
                class="w-35 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                type="date"
                name="expense_date_from"
                value=(params.expense_date_from.as_deref().unwrap_or(""));
            span class="text-xs text-muted" { "至" }
            input
                class="w-35 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                type="date"
                name="expense_date_to"
                value=(params.expense_date_to.as_deref().unwrap_or(""));
        }

        (expense_data_card(result, params, applicant_names, dept_names))
    }
}
}

fn expense_data_card(
 result: &PaginatedResult<ExpenseReimbursement>,
 params: &ExpenseQueryParams,
 applicant_names: &HashMap<i64, String>,
 dept_names: &HashMap<i64, String>,
) -> Markup {
 let query = build_query_string(params);
 html! {
    div class="data-card" id="expense-data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "单号" }
                        th { "申请人" }
                        th { "部门" }
                        th { "报销日期" }
                        th { "金额" }
                        th { "单据张数" }
                        th { "状态" }
                        th { "提交时间" }
                        th class="w-20" { "操作" }
                    }
                }
                tbody {
                    @for item in &result.items {
                        @let (status_text, status_bg, status_color) = expense_status_label(
                            &item.status,
                        );
                        @let detail_path = ExpenseDetailPath { id: item.id };
                        @let applicant_name = applicant_names
                            .get(&item.applicant_id)
                            .map(|s| s.as_str())
                            .unwrap_or("—");
                        @let dept_name = item
                            .department_id
                            .and_then(|did| dept_names.get(&did).map(|s| s.as_str()))
                            .unwrap_or("—");
                        tr class="hover:bg-accent-bg transition-colors" {
                            td {
                                a   href=(detail_path.to_string())
                                    class="text-accent font-medium font-mono tabular-nums hover:underline"
                                { (item.doc_number) }
                            }
                            td class="text-sm text-fg-2" { (applicant_name) }
                            td class="text-sm text-muted" { (dept_name) }
                            td class="text-xs text-muted" { (item.expense_date.format("%Y-%m-%d")) }
                            td class="text-right font-mono tabular-nums text-sm font-semibold" {
                                "¥"
                                (format!("{:.2}", item.total_amount))
                            }
                            td class="text-center text-sm text-muted" { (item.sheet_count) }
                            td {
                                span
                                    class=({
                                        format!(
                                            "text-xs px-2 py-0.5 rounded-full font-medium {} {}",
                                            status_bg,
                                            status_color,
                                        )
                                    })
                                { (status_text) }
                            }
                            td class="text-xs text-muted" {
                                (item.created_at.format("%Y-%m-%d %H:%M"))
                            }
                            td {
                                a   href=(detail_path.to_string())
                                    class="text-accent text-xs hover:underline"
                                { "查看" }
                            }
                        }
                    }
                    @if result.items.is_empty() {
                        tr {
                            td colspan="9" class="text-center text-muted text-sm py-8" { "暂无报销记录" }
                        }
                    }
                }
            }
        }
        ({
            pagination(
                ExpenseListPath::PATH,
                &query,
                result.total,
                result.page,
                result.total_pages,
            )
        })
    }
}
}
