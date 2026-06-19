use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::shared::identity::DepartmentService;
use abt_core::shared::identity::RoleService;
use abt_core::shared::identity::UserService;
use abt_core::shared::identity::model::{Department, Role, UserWithRoles};
use abt_core::shared::types::{PgExecutor, ServiceContext};

use crate::components::icon;
use crate::components::pagination;
use crate::layout::page::admin_page;
use crate::components::tabs::{status_tabs_with_oob, TabItem};
use crate::routes::user::*;
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct UserQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub role_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub dept_id: Option<i64>,
 pub status: Option<String>,
 pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("USER", "read")]
pub async fn get_user_list(
 _path: UserListPath,
 ctx: RequestContext,
 Query(params): Query<UserQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("USER", "create").await;
 let can_delete = ctx.has_permission("USER", "delete").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let user_svc = state.user_service();
 let role_svc = state.role_service();
 let dept_svc = state.department_service();

 let users = user_svc
 .list_users_with_roles(&service_ctx, &mut conn)
 .await?;
 let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;
 let all_depts = dept_svc.list_departments(&service_ctx, &mut conn).await?;

 let user_depts = load_user_departments(&dept_svc, &service_ctx, &mut conn, &users).await;

 let content = user_list_page(&users, &all_roles, &all_depts, &user_depts, &params, can_create, can_delete);
 let page_html = admin_page(
 is_htmx,
 "用户管理",
 &claims,
 "system",
 UserListPath::PATH,
 "系统管理",
 Some("用户管理"),
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "delete")]
pub async fn delete_user(
 path: UserDeletePath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.user_service();

 svc.delete_user(&service_ctx, &mut conn, path.id).await?;

 Ok(([("HX-Redirect", UserListPath::PATH)], Html(String::new())))
}

#[require_permission("USER", "update")]
pub async fn toggle_user_status(
    path: UserToggleStatusPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.user_service();

    let user = svc
        .get_user_with_roles(&service_ctx, &mut conn, path.id)
        .await?;
    let new_status = !user.user.is_active;
    svc.update_user_status(&service_ctx, &mut conn, path.id, new_status)
        .await?;

    // Event-driven refresh: emit `userToggled`, the filter form listens on it
    // and re-submits (keeping current filters/tab/pagination) instead of a
    // full-page HX-Redirect that would discard that state.
    Ok(([("HX-Trigger", "userToggled")], Html(String::new())))
}

// ── Helpers ──

async fn load_user_departments(
 dept_svc: &impl DepartmentService,
 ctx: &ServiceContext,
 conn: PgExecutor<'_>,
 users: &[UserWithRoles],
) -> HashMap<i64, Vec<Department>> {
 let mut map = HashMap::new();
 for u in users {
 if let Ok(depts) = dept_svc.get_user_departments(ctx, conn, u.user.user_id).await {
 map.insert(u.user.user_id, depts);
 }
 }
 map
}

fn filter_users<'a>(
 users: &'a [UserWithRoles],
 user_depts: &HashMap<i64, Vec<Department>>,
 keyword: &str,
 role_id: Option<i64>,
 dept_id: Option<i64>,
 status: &str,
) -> Vec<&'a UserWithRoles> {
 users
 .iter()
 .filter(|u| {
 if !keyword.is_empty() {
 let kw = keyword.to_lowercase();
 let matches = u.user.username.to_lowercase().contains(&kw)
 || u
 .user
 .display_name
 .as_ref()
 .is_some_and(|d| d.to_lowercase().contains(&kw));
 if !matches {
 return false;
 }
 }
 if let Some(rid) = role_id
 && !u.roles.iter().any(|r| r.role_id == rid) {
 return false;
 }
 if let Some(did) = dept_id {
 let user_dept_ids = user_depts
 .get(&u.user.user_id)
 .map(|v| v.iter().map(|d| d.department_id).collect::<Vec<_>>())
 .unwrap_or_default();
 if !user_dept_ids.contains(&did) {
 return false;
 }
 }
 if status == "active" && !u.user.is_active {
 return false;
 }
 if status == "inactive" && u.user.is_active {
 return false;
 }
 true
 })
 .collect()
}

fn avatar_color_class(name: &str) -> &'static str {
 let code = name.chars().next().map_or(0, |c| c as u32);
 match code % 8 {
 0 => "bg-blue-500",
 1 => "bg-purple-500",
 2 => "bg-green-500",
 3 => "bg-orange-500",
 4 => "bg-pink-500",
 5 => "bg-indigo-500",
 6 => "bg-teal-500",
 _ => "bg-cyan-500",
 }
}

fn initials(name: &str) -> String {
 name.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
}

fn build_query_string(params: &UserQueryParams) -> String {
 let mut q = vec![];
 if let Some(ref kw) = params.keyword {
 if !kw.is_empty() {
 q.push(format!("keyword={kw}"));
 }
 }
 if let Some(r) = params.role_id {
 q.push(format!("role_id={r}"));
 }
 if let Some(d) = params.dept_id {
 q.push(format!("dept_id={d}"));
 }
 if let Some(ref s) = params.status {
 if !s.is_empty() {
 q.push(format!("status={s}"));
 }
 }
 q.join("&")
}

fn data_scope_label(u: &UserWithRoles) -> &'static str {
 if u.user.is_super_admin {
 return "All";
 }
 for r in &u.roles {
 if r.role_code.contains("manager")
 || r.role_code.contains("_mgr")
 || r.role_code == "warehouse_admin"
 {
 return "Department";
 }
 }
 "Self"
}

// ── Components ──

fn user_list_page(
 users: &[UserWithRoles],
 all_roles: &[Role],
 all_depts: &[Department],
 user_depts: &HashMap<i64, Vec<Department>>,
 params: &UserQueryParams,
 can_create: bool,
 can_delete: bool,
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-2xl font-bold text-fg tracking-tight" { "用户管理" }
 div class="flex gap-3" {
 @if can_create {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(UserCreatePath::PATH) {
 (icon::plus_icon("w-4 h-4"))
 "新建用户"
 }
 }
 }
 }
 (user_table_fragment(users, all_roles, all_depts, user_depts, params, can_delete))
 }
 }
}

fn user_table_fragment(
 users: &[UserWithRoles],
 all_roles: &[Role],
 all_depts: &[Department],
 user_depts: &HashMap<i64, Vec<Department>>,
 params: &UserQueryParams,
 can_delete: bool,
) -> Markup {
 let keyword = params.keyword.as_deref().unwrap_or("");
 let role_filter = params.role_id;
 let dept_filter = params.dept_id;
 let status_filter = params.status.as_deref().unwrap_or("");
 let page = params.page.unwrap_or(1).max(1);
 let page_size: u32 = 15;

 // Stats
 let total_count = users.len();
 let active_count = users.iter().filter(|u| u.user.is_active).count();
 let inactive_count = total_count - active_count;
 let super_admin_count = users.iter().filter(|u| u.user.is_super_admin).count();

 let filtered = filter_users(users, user_depts, keyword, role_filter, dept_filter, status_filter);

 // Pagination
 let total = filtered.len() as u64;
 let total_pages = if page_size > 0 {
 (total as u32).div_ceil(page_size)
 } else {
 1
 };
 let skip = ((page - 1) * page_size) as usize;
 let page_users: Vec<_> = filtered.into_iter().skip(skip).take(page_size as usize).collect();


 html! {
div class="user-list-panel" id="user-list-panel" {
 // ── Stats ──
div id="user-stats" class="grid grid-cols-4 gap-5" {
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 bg-accent-bg text-accent" {
 (icon::users_icon("w-6 h-6"))
 }
 div {
div class="text-2xl font-bold font-mono tabular-nums text-fg" { (total_count) }
 div class="text-sm text-muted mt-1" { "用户总数" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 bg-success-bg text-success" {
 (icon::check_circle_icon("w-6 h-6"))
 }
 div {
div class="text-2xl font-bold font-mono tabular-nums text-fg" { (active_count) }
 div class="text-sm text-muted mt-1" { "已激活" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 bg-[#fef3c7] text-[#d97706]" {
 (icon::clock_icon("w-6 h-6"))
 }
 div {
div class="text-2xl font-bold font-mono tabular-nums text-fg" { (inactive_count) }
 div class="text-sm text-muted mt-1" { "已停用" }
 }
 }
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 bg-purple-bg text-purple" {
 svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="w-6 h-6" {
 path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" {}
 }
 }
 div {
div class="text-2xl font-bold font-mono tabular-nums text-fg" { (super_admin_count) }
 div class="text-sm text-muted mt-1" { "超级管理员" }
 }
 }
 }

(status_tabs_with_oob(
    UserListPath::PATH,
    ".data-card",
    "#user-filter-form",
    "#status-tabs,#user-stats,#user-filter-form",
    &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count as u64) },
        TabItem { value: "active".to_string(), label: "已激活", count: Some(active_count as u64) },
        TabItem { value: "inactive".to_string(), label: "已停用", count: Some(inactive_count as u64) },
    ],
    status_filter,
    "status",
))

// ── Filter Bar ──
form id="user-filter-form" class="flex items-center gap-3 mb-5 flex-wrap"
hx-get=(UserListPath::PATH)
hx-target=".data-card" hx-select=".data-card" hx-select-oob="#status-tabs,#user-stats,#user-filter-form"
hx-swap="outerHTML"
hx-push-url="true"
hx-trigger="change, keyup changed delay:300ms from:input[name=keyword], userToggled from:body" {
 input type="hidden" name="status" value=(status_filter);
 div class="relative w-[280px] shrink-0 [&_[class*=i-lucide]]:absolute [&_[class*=i-lucide]]:left-3 [&_[class*=i-lucide]]:top-1/2 [&_[class*=i-lucide]]:-translate-y-1/2 [&_[class*=i-lucide]]:w-4 [&_[class*=i-lucide]]:h-4 [&_[class*=i-lucide]]:text-muted" {
 (icon::search_icon("w-4 h-4"))
 input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
 placeholder="搜索用户名、显示名称…"
 value=(keyword);
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="role_id" {
 option value="" { "全部角色" }
 @for role in all_roles {
 @let selected = role_filter == Some(role.role_id);
 option value=(role.role_id) selected[selected] {
 (role.role_name)
 }
 }
 }
 select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="dept_id" {
 option value="" { "全部部门" }
 @for dept in all_depts {
 @let selected = dept_filter == Some(dept.department_id);
 option value=(dept.department_id) selected[selected] {
 (dept.department_name)
 }
 }
 }
}

 // ── Data Table ──
 div class="data-card" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th { "用户信息" }
 th { "登录名" }
 th { "角色" }
 th { "部门" }
 th { "数据权限" }
 th { "状态" }
 th { "创建时间" }
 th class="!text-right" { "操作" }
 }
 }
 tbody {
 @for u in &page_users {
 (user_row(u, user_depts, can_delete))
 }
 @if page_users.is_empty() {
 tr {
 td colspan="8" class="text-center p-6 text-muted text-sm" {
 "暂无用户数据"
 }
 }
 }
 }
 }
 }
 }

 // ── Pagination ──
 (pagination::pagination(
 UserListPath::PATH,
 &build_query_string(params),
 total,
 page,
 total_pages,
 ))
 }
 }
}

fn user_row(
 u: &UserWithRoles,
 user_depts: &HashMap<i64, Vec<Department>>,
 can_delete: bool,
) -> Markup {
 let detail_path = UserDetailPath { id: u.user.user_id };
 let edit_path = UserEditPath { id: u.user.user_id };
 let toggle_path = UserToggleStatusPath { id: u.user.user_id };


 let display_name = u.user.display_name.as_deref().unwrap_or(&u.user.username);
 let avatar_initials = initials(display_name);
 let avatar_cls = avatar_color_class(display_name);

 let (status_label, dot_class) = if u.user.is_active {
 ("已激活", "w-1.5 h-1.5 rounded-full bg-green-500 shrink-0")
 } else {
 ("已停用", "w-1.5 h-1.5 rounded-full bg-gray-400 shrink-0")
 };

 let depts = user_depts
 .get(&u.user.user_id)
 .map(|d| d.as_slice())
 .unwrap_or(&[]);
 let scope = data_scope_label(u);

 let toggle_title = if u.user.is_active { "停用" } else { "启用" };

 html! {
 tr onclick=(format!("location.href='{}'", detail_path)) {
 // User info
 td {
 div class="flex items-center gap-3" {
 div class={"w-8 h-8 rounded-[10px] flex items-center justify-center text-xs font-semibold shrink-0 text-white " (avatar_cls)} {
 (avatar_initials)
 }
div class="flex flex-col gap-[2px]" {
span class="flex items-center gap-[6px] text-[13px] font-semibold text-fg" {
 (display_name)
 @if u.user.is_super_admin {
 span class="text-[10px] font-semibold px-[6px] py-[2px] rounded-[3px] bg-purple-bg text-purple border border-[#e8d5ff] tracking-[0.02em]" { "超管" }
 }
 }
span class="text-xs text-muted" {
 "ID: " (u.user.user_id)
 }
 }
 }
 }

 // Login name
 td class="font-mono tabular-nums" {
 (u.user.username)
 }

 // Roles
 td {
 div class="flex flex-wrap gap-[4px]" {
 @for role in &u.roles {
 span class="text-[10px] font-medium px-[7px] py-[2px] rounded-[3px] bg-[#e8f4ff] text-accent border border-[#d6e4ff]" { (role.role_name) }
 }
 @if u.roles.is_empty() {
 span class="text-muted" { "—" }
 }
 }
 }

 // Departments
 td {
 @if depts.is_empty() {
 span class="text-muted" { "—" }
 } @else {
 @for dept in depts {
 span class="text-[10px] font-medium px-[7px] py-[2px] rounded-[3px] bg-success-bg text-success border border-[#d1f5e0]" { (dept.department_name) }
 }
 }
 }

 // Data scope
 td {
 span class="text-xs text-muted" { (scope) }
 }

 // Status
 td {
 span class="flex items-center gap-1 text-[13px]" {
 span class=(dot_class) {}
 (status_label)
 }
 }

 // Created at
 td class="font-mono tabular-nums text-xs text-muted" {
 (u.user.created_at.format("%Y-%m-%d"))
 }

 // Actions
 td _="on click halt the event" {
div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_button]:w-[28px] [&_button]:h-[28px] [&_button]:grid [&_button]:place-items-center [&_button]:rounded-sm [&_button]:cursor-pointer [&_button]:bg-surface [&_button]:hover:bg-accent-bg [&_[class*=i-lucide]]:w-3.5 [&_[class*=i-lucide]]:h-3.5" {
 a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑"
 href=(edit_path.to_string()) {
 (icon::edit_icon("w-3.5 h-3.5"))
 }
 @if can_delete && !u.user.is_super_admin {
button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title=(toggle_title)
hx-post=(toggle_path.to_string())
hx-swap="none"
hx-confirm=(format!(
    "确定要{}用户 <strong>{}</strong> 吗？",
    if u.user.is_active { "停用" } else { "启用" },
    display_name,
)) {
 @if u.user.is_active {
 svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-3.5 h-3.5" {
 path d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" {}
 }
 } @else {
 svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-3.5 h-3.5" {
 path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" {}
 }
 }
 }
 }
 }
 }
 }
 }
}
