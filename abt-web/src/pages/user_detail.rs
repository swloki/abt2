use std::collections::BTreeMap;

use axum::Form;
use axum::response::{Html, IntoResponse};
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::{DepartmentService, PermissionService, RoleService, UserService};
use abt_core::shared::identity::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::user::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("USER", "read")]
pub async fn get_user_detail(
 path: UserDetailPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
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
 let perm_svc = state.permission_service();

 let user = user_svc
 .get_user_with_roles(&service_ctx, &mut conn, path.id)
 .await?;
 let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;
 let all_depts = dept_svc
 .list_departments(&service_ctx, &mut conn)
 .await?;
 let user_depts = dept_svc
 .get_user_departments(&service_ctx, &mut conn, path.id)
 .await?;

 // Resolve effective permissions for this user's roles
 let role_ids: Vec<i64> = user.roles.iter().map(|r| r.role_id).collect();
 let perm_strings = perm_svc.get_user_permissions(&role_ids).await?;
 let grouped_perms = group_permissions_by_resource(&perm_strings);

 // Compute data scope
 let data_scope_label = compute_data_scope(&user, &user_depts);

 let content = user_detail_page(&user, &all_roles, &all_depts, &user_depts, &grouped_perms, data_scope_label);
 let detail_path_str = UserDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 用户详情", user.user.username),
 &claims,
 "system",
 &detail_path_str,
 "系统管理",
 Some(&user.user.username),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "update")]
pub async fn post_role_assign(
 path: UserRoleAssignPath,
 ctx: RequestContext,
 Form(form): Form<RoleAssignForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.user_service();

 let desired: Vec<i64> = form
 .role_ids
 .split(',')
 .filter_map(|s| s.trim().parse::<i64>().ok())
 .collect();

 let current_user = svc
 .get_user_with_roles(&service_ctx, &mut conn, path.id)
 .await?;
 let current: Vec<i64> = current_user.roles.iter().map(|r| r.role_id).collect();

 let to_add: Vec<i64> = desired
 .iter()
 .filter(|id| !current.contains(id))
 .copied()
 .collect();
 let to_remove: Vec<i64> = current
 .iter()
 .filter(|id| !desired.contains(id))
 .copied()
 .collect();

 if !to_add.is_empty() {
 svc.assign_roles(&service_ctx, &mut conn, path.id, to_add)
 .await?;
 }
 if !to_remove.is_empty() {
 svc.remove_roles(&service_ctx, &mut conn, path.id, to_remove)
 .await?;
 }

 let redirect = UserDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("USER", "update")]
pub async fn post_dept_assign(
 path: UserDeptAssignPath,
 ctx: RequestContext,
 Form(form): Form<DeptAssignForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.department_service();

 let desired: Vec<i64> = form
 .dept_ids
 .split(',')
 .filter_map(|s| s.trim().parse::<i64>().ok())
 .collect();

 let current_depts = svc
 .get_user_departments(&service_ctx, &mut conn, path.id)
 .await?;
 let current: Vec<i64> = current_depts.iter().map(|d| d.department_id).collect();

 let to_add: Vec<i64> = desired
 .iter()
 .filter(|id| !current.contains(id))
 .copied()
 .collect();
 let to_remove: Vec<i64> = current
 .iter()
 .filter(|id| !desired.contains(id))
 .copied()
 .collect();

 if !to_add.is_empty() {
 svc.assign_departments(&service_ctx, &mut conn, path.id, to_add)
 .await?;
 }
 if !to_remove.is_empty() {
 svc.remove_departments(&service_ctx, &mut conn, path.id, to_remove)
 .await?;
 }

 let redirect = UserDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("USER", "update")]
pub async fn post_change_password(
 path: UserChangePasswordPath,
 ctx: RequestContext,
 Form(form): Form<ChangePasswordForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.user_service();

 // Validate password confirmation
 if form.new_password != form.confirm_password {
 return Err(abt_core::shared::types::DomainError::validation(
 "新密码与确认密码不匹配".to_string(),
 ).into());
 }

 svc.admin_reset_password(
 &service_ctx,
 &mut conn,
 path.id,
 &form.new_password,
 )
 .await?;

 let redirect = UserDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct RoleAssignForm {
 pub role_ids: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeptAssignForm {
 pub dept_ids: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordForm {
 pub new_password: String,
 pub confirm_password: String,
}

// ── Helpers ──

/// Extract up to 2 leading characters, uppercased, for avatar/badge initials.
fn get_initials(s: &str) -> String {
 s.chars().take(2).collect::<String>().to_uppercase()
}

/// Check whether a role (by code) is flagged as a system/built-in role.
fn is_role_system(role_code: &str, all_roles: &[Role]) -> bool {
 all_roles
 .iter()
 .any(|r| r.role_code == role_code && r.is_system_role)
}

/// Compute data scope label: All (super admin), Department (has depts), or Self.
fn compute_data_scope(user: &UserWithRoles, user_depts: &[Department]) -> &'static str {
 if user.user.is_super_admin {
 "All"
 } else if !user_depts.is_empty() {
 "Department"
 } else {
 "Self"
 }
}

/// Chinese display names for permission resources (matching prototype design).
fn resource_display_name(code: &str) -> &str {
 match code {
 "CUSTOMER" => "客户管理",
 "PRODUCT" => "产品管理",
 "CATEGORY" => "分类管理",
 "BOM" => "BOM管理",
 "BOM_CATEGORY" => "BOM分类",
 "WAREHOUSE" => "仓库管理",
 "LOCATION" => "库位管理",
 "INVENTORY" => "库存管理",
 "PRICE" => "价格管理",
 "SALES_ORDER" => "销售订单",
 "PURCHASE_ORDER" => "采购订单",
 "WORK_ORDER" => "工单管理",
 "INSPECTION" => "质检管理",
 "COST" => "成本管理",
 "LABOR_COST" => "人工成本",
 "USER" => "用户管理",
 "ROLE" => "角色管理",
 "DEPARTMENT" => "部门管理",
 "SHIPPING" => "发货管理",
 _ => code,
 }
}

/// Group flat `"RESOURCE:action"` strings into `(resource_code, display_name, actions)`.
fn group_permissions_by_resource(perms: &[String]) -> Vec<(String, String, Vec<String>)> {
 let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
 for perm in perms {
 if let Some((resource, action)) = perm.split_once(':') {
 map.entry(resource.to_string())
 .or_default()
 .push(action.to_string());
 }
 }

 let mut result = Vec::new();
 for (code, actions) in map {
 let name = resource_display_name(&code).to_string();
 result.push((code, name, actions));
 }
 result
}

// ── Resource icon mapper (matching prototype design) ──

/// Returns an appropriate icon for each permission resource type.
fn resource_icon(code: &str, c: &str) -> Markup {
 match code {
 "SALES_ORDER" => icon::file_text_icon(c),
 "PURCHASE_ORDER" => icon::clipboard_document_icon(c),
 "CUSTOMER" => icon::users_icon(c),
 "PRODUCT" => icon::box_icon(c),
 "CATEGORY" => icon::grid_icon(c),
 "BOM" | "BOM_CATEGORY" => icon::clipboard_list_icon(c),
 "WAREHOUSE" => icon::building_icon(c),
 "LOCATION" => icon::building_icon(c),
 "INVENTORY" => icon::package_icon(c),
 "PRICE" => icon::currency_icon(c),
 "SHIPPING" => icon::truck_icon(c),
 "WORK_ORDER" => icon::bolt_icon(c),
 "INSPECTION" => icon::check_circle_icon(c),
 "COST" | "LABOR_COST" => icon::currency_icon(c),
 "USER" => icon::user_icon(c),
 "ROLE" => icon::lock_icon(c),
 "DEPARTMENT" => icon::building_icon(c),
 _ => icon::box_icon(c),
 }
}

fn info_circle_icon(c: &str) -> Markup {
 icon::svg(
 r#"<circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>"#,
 c,
 )
}

fn shield_check_icon(c: &str) -> Markup {
 icon::svg(
 r#"<path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"/>"#,
 c,
 )
}

// ── Shared class strings ──

const BTN_DEFAULT_SM: &str =
 "inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border \
  hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
  cursor-pointer transition-all duration-150";

const D_CARD: &str =
 "bg-bg border border-border-soft rounded-lg overflow-hidden shadow-xs";

const D_CARD_HEAD: &str =
 "py-3 px-4 border-b border-border-soft flex justify-between items-center bg-surface-raised";

const TAG_PILL_BASE: &str =
 "text-[10px] px-2 py-[2px] rounded-[3px] font-semibold tracking-[0.02em]";

// ── Page Component ──

fn user_detail_page(
 user: &UserWithRoles,
 all_roles: &[Role],
 all_depts: &[Department],
 user_depts: &[Department],
 grouped_perms: &[(String, String, Vec<String>)],
 data_scope: &str,
) -> Markup {
 let user_id = user.user.user_id;
 let display_name = user
 .user
 .display_name
 .as_deref()
 .unwrap_or(&user.user.username);
 let avatar_initials = get_initials(&user.user.username);
 let edit_path = UserEditPath { id: user_id }.to_string();
 let list_path = UserListPath.to_string();
 let role_assign_path = UserRoleAssignPath { id: user_id }.to_string();
 let dept_assign_path = UserDeptAssignPath { id: user_id }.to_string();
 let password_path = UserChangePasswordPath { id: user_id }.to_string();

 let current_role_ids: Vec<i64> = user.roles.iter().map(|r| r.role_id).collect();
 let current_dept_ids: Vec<i64> = user_depts.iter().map(|d| d.department_id).collect();

 let total_perms: usize = grouped_perms
 .iter()
 .map(|(_, _, actions)| actions.len())
 .sum();

 html! {
 div class="space-y-5" {
 // ── Back Link ──
 a class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg transition-colors"
 href=(format!("{list_path}?restore=true")) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回用户列表"
 }

 // ── Profile Hero ──
 div class="flex items-center justify-between p-5 px-6 bg-bg border border-border-soft rounded-xl shadow-xs" {
 div class="flex items-center gap-4" {
 div class="w-[52px] h-[52px] rounded-lg flex items-center justify-center text-[18px] font-bold text-white shrink-0 bg-[linear-gradient(135deg,#1677ff,#4096ff)] shadow-[0_3px_10px_rgba(22,119,255,0.2)]" {
 (avatar_initials)
 }
 div {
 h2 class="text-[18px] font-bold text-fg mb-[3px] tracking-[-0.01em]" { (display_name) }
 div class="flex items-center gap-[6px] flex-wrap" {
 span class="inline-flex items-center px-[7px] py-[1px] bg-surface border border-border rounded-[3px] font-mono text-[11px] font-semibold text-accent tracking-[0.04em]" {
 (&user.user.username)
 }
 span class="text-xs text-muted" {
 "ID: " (user_id) " · 创建于 "
 (user.user.created_at.format("%Y-%m-%d"))
 }
 }
 div class="flex gap-[4px] mt-[4px]" {
 @if user.user.is_active {
 span class=(format!("{TAG_PILL_BASE} bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0]")) { "已激活" }
 } @else {
 span class=(format!("{TAG_PILL_BASE} bg-surface text-[#8c8c8c] border border-border")) { "未激活" }
 }
 @if user.user.is_super_admin {
 span class=(format!("{TAG_PILL_BASE} bg-[#f3e8ff] text-[#7c3aed] border border-[#e8d5ff]")) { "超级管理员" }
 }
 @for dept in user_depts {
 span class=(format!("{TAG_PILL_BASE} bg-[#e8f4ff] text-[#1677ff] border border-[#d6e4ff]")) { (&dept.department_name) }
 }
 span class=(format!("{TAG_PILL_BASE} bg-[#e8f4ff] text-[#1677ff] border border-[#d6e4ff]")) { (data_scope) }
 }
 }
 }
 div class="flex gap-[6px]" {
 a class=(BTN_DEFAULT_SM) href=(edit_path) {
 (icon::edit_icon("w-3.5 h-3.5"))
 "编辑"
 }
 button type="button" class=(BTN_DEFAULT_SM)
 _="on click add .is-open to #reset-pw-modal" {
 (icon::lock_icon("w-3.5 h-3.5"))
 "重置密码"
 }
 }
 }

 // ── Stats Row ──
 div class="flex gap-3" {
 (stat_item("bg-[#1677ff]", &user.roles.len().to_string(), "个角色", false))
 (stat_item("bg-[#52c41a]", &user_depts.len().to_string(), "个部门", false))
 (stat_item("bg-[#7c3aed]", &total_perms.to_string(), "项权限", false))
 (stat_item("bg-[#faad14]", data_scope, "数据范围", true))
 }

 // ── Two-Column Grid ──
 div class="grid grid-cols-2 gap-5" {
 // ── LEFT COLUMN ──
 div class="space-y-5" {
 // Basic Info Card
 div class=(D_CARD) {
 div class=(D_CARD_HEAD) {
 h3 class="text-[13px] font-semibold flex items-center gap-[6px] text-fg" {
 (info_circle_icon("w-3.5 h-3.5"))
 "基本信息"
 }
 }
 div class="p-4" {
 div class="border border-border-soft rounded-lg overflow-hidden" {
 (info_row("用户 ID", html! { span class="text-fg font-medium font-mono text-xs text-accent" { "#" (format!("{:03}", user_id)) } }))
 (info_row("登录名", html! { span class="text-fg font-medium font-mono text-xs text-accent" { (&user.user.username) } }))
 (info_row("显示名称", html! { span class="text-fg font-medium" { (display_name) } }))
 (info_row("超级管理员", html! {
 @if user.user.is_super_admin {
 span class="text-fg font-medium" { "是" }
 } @else {
 span class="text-muted font-medium" { "否" }
 }
 }))
 (info_row("激活状态", html! {
 @if user.user.is_active {
 span class="text-[#389e0d] font-medium" { "已激活" }
 } @else {
 span class="text-muted font-medium" { "未激活" }
 }
 }))
 (info_row("数据权限", html! { span class="text-fg font-medium" { (data_scope) } }))
 (info_row("创建时间", html! { span class="text-fg font-medium font-mono text-xs text-accent" { (user.user.created_at.format("%Y-%m-%d %H:%M")) } }))
 (info_row("最后更新", html! {
 @if let Some(updated) = &user.user.updated_at {
 span class="text-fg font-medium font-mono text-xs text-accent" { (updated.format("%Y-%m-%d %H:%M")) }
 } @else {
 span class="text-muted font-medium" { "—" }
 }
 }))
 }
 }
 }

 // Departments Card
 div class=(D_CARD) {
 div class=(D_CARD_HEAD) {
 h3 class="text-[13px] font-semibold flex items-center gap-[6px] text-fg" {
 (icon::building_icon("w-3.5 h-3.5"))
 "所属部门"
 }
 div class="flex items-center gap-2" {
 span class="text-[11px] text-muted bg-bg px-2 py-[1px] rounded-full border border-border-soft" {
 (user_depts.len())
 }
 button type="button" class="inline-flex items-center justify-center w-6 h-6 rounded-sm text-muted hover:text-accent hover:bg-accent-bg cursor-pointer transition-colors"
 title="管理部门"
 _="on click add .is-open to #dept-assign-modal" {
 (icon::edit_icon("w-3.5 h-3.5"))
 }
 }
 }
 div class="p-4" {
 @if user_depts.is_empty() {
 p class="text-sm text-muted" { "暂未分配部门" }
 } @else {
 @for dept in user_depts {
 div class="flex items-center gap-3 p-3 border border-border-soft rounded-md mb-2 transition-colors last:mb-0 hover:border-accent" {
 span class="w-7 h-7 rounded-lg flex items-center justify-center text-[9px] font-bold text-white shrink-0 bg-[linear-gradient(135deg,#1677ff,#4096ff)]" {
 (get_initials(&dept.department_code))
 }
 div class="flex-1 min-w-0" {
 div class="text-[13px] font-semibold text-fg leading-[1.3]" { (&dept.department_name) }
 div class="text-[11px] text-muted font-mono" { (&dept.department_code) }
 }
 }
 }
 }
 }
 }
 }

 // ── RIGHT COLUMN ──
 div class="space-y-5" {
 // Roles Card
 div class=(D_CARD) {
 div class=(D_CARD_HEAD) {
 h3 class="text-[13px] font-semibold flex items-center gap-[6px] text-fg" {
 (icon::lock_icon("w-3.5 h-3.5"))
 "已分配角色"
 }
 div class="flex items-center gap-2" {
 span class="text-[11px] text-muted bg-bg px-2 py-[1px] rounded-full border border-border-soft" {
 (user.roles.len())
 }
 button type="button" class="inline-flex items-center justify-center w-6 h-6 rounded-sm text-muted hover:text-accent hover:bg-accent-bg cursor-pointer transition-colors"
 title="管理角色"
 _="on click add .is-open to #role-assign-modal" {
 (icon::edit_icon("w-3.5 h-3.5"))
 }
 }
 }
 div class="p-4" {
 @if user.roles.is_empty() {
 p class="text-sm text-muted" { "暂未分配角色" }
 } @else {
 @for role in &user.roles {
 div class="flex items-center gap-3 p-3 border border-border-soft rounded-md mb-2 transition-colors last:mb-0 hover:border-accent" {
 span class="w-7 h-7 rounded-lg flex items-center justify-center text-[9px] font-bold text-white shrink-0 bg-[linear-gradient(135deg,#1677ff,#4096ff)]" {
 (get_initials(&role.role_code))
 }
 div class="flex-1 min-w-0" {
 div class="text-[13px] font-semibold text-fg leading-[1.3]" { (&role.role_name) }
 div class="text-[11px] text-muted font-mono" { (&role.role_code) }
 }
 @if is_role_system(&role.role_code, all_roles) {
 span class="text-[10px] px-[6px] py-[1px] rounded-[3px] font-medium bg-[#fff7e6] text-[#d46b08] border border-[#ffe7ba]" { "内置" }
 }
 }
 }
 }
 }
 }

 // Permission Preview Card
 div class=(D_CARD) {
 div class=(D_CARD_HEAD) {
 h3 class="text-[13px] font-semibold flex items-center gap-[6px] text-fg" {
 (shield_check_icon("w-3.5 h-3.5"))
 "权限预览"
 }
 span class="text-[11px] text-muted bg-bg px-2 py-[1px] rounded-full border border-border-soft" {
 (format!("{} 项", total_perms))
 }
 }
 div class="p-4 max-h-[320px] overflow-y-auto" {
 @if grouped_perms.is_empty() {
 p class="text-sm text-muted" { "暂无权限" }
 } @else {
 @for (code, name, actions) in grouped_perms {
 div class="mb-3 last:mb-0" {
 div class="text-xs font-semibold text-fg mb-2 flex items-center gap-[6px]" {
 (resource_icon(code, "w-3 h-3"))
 " " (name) " (" (code) ")"
 }
 div class="flex flex-wrap gap-[4px]" {
 @for action in actions {
 (perm_chip(&action))
 }
 }
 }
 }
 }
 }
 }
 }
 }

 // ── Modals ──
 (role_assign_modal(&role_assign_path, all_roles, &current_role_ids))
 (dept_assign_modal(&dept_assign_path, all_depts, &current_dept_ids))
 (reset_password_modal(&password_path))
 }
 }
}

fn stat_item(dot_class: &str, value: &str, label: &str, small_value: bool) -> Markup {
 let value_class = if small_value {
 "text-xs font-bold text-fg leading-none"
 } else {
 "text-[15px] font-bold text-fg leading-none"
 };
 html! {
 div class="flex items-center gap-2 py-[10px] px-4 bg-bg border border-border-soft rounded-md flex-1 shadow-xs" {
 span class=(format!("w-2 h-2 rounded-full shrink-0 {dot_class}")) {}
 b class=(value_class) { (value) }
 span class="text-[11px] text-muted ml-[2px]" { (label) }
 }
 }
}

fn info_row(label: &str, value: Markup) -> Markup {
 html! {
 div class="flex items-center py-[9px] px-4 text-[13px] border-b border-border-soft last:border-b-0" {
 span class="w-[80px] shrink-0 text-muted text-xs" { (label) }
 (value)
 }
 }
}

fn perm_chip(action: &str) -> Markup {
 let cls = match action.to_lowercase().as_str() {
 "read" => "bg-[#e8f4ff] text-[#1677ff] border border-[#d6e4ff]",
 "create" => "bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0]",
 "update" | "write" => "bg-[#fff8eb] text-[#d46b08] border border-[#ffe7ba]",
 "delete" => "bg-[#fff2f0] text-[#cf1322] border border-[#ffccc7]",
 _ => "text-muted border border-border-soft",
 };
 html! {
 span class=(format!("text-[11px] px-2 py-[2px] rounded-[3px] font-medium font-mono {cls}")) {
 (action.to_uppercase())
 }
 }
}

// ── Modals ──

fn role_assign_modal(action: &str, all_roles: &[Role], current_ids: &[i64]) -> Markup {
 html! {
 div id="role-assign-modal" class="modal-overlay fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
 _="on click[me is event.target] remove .is-open" {
 form id="role-assign-form" class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
 hx-post=(action) hx-swap="none" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="text-base font-semibold text-fg" { "管理角色" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
 _="on click remove .is-open from closest .modal-overlay then reset #role-assign-form" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 input type="hidden" name="role_ids" id="role-ids-input" {}
 div class="flex flex-col gap-1" {
 @for role in all_roles {
 label class="flex items-center gap-2.5 py-2 px-2 rounded-sm cursor-pointer hover:bg-surface transition-colors" {
 input type="checkbox" class="role-checkbox w-4 h-4 accent-[var(--accent)] cursor-pointer"
 value=(role.role_id) checked[current_ids.contains(&role.role_id)];
 span class="text-sm text-fg" { (role.role_name) }
 @if role.is_system_role {
 span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-[#e6f4ff] text-accent" { "系统" }
 }
 @if let Some(desc) = &role.description {
 span class="ml-auto text-xs text-muted" { (desc) }
 }
 }
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class=(BTN_DEFAULT_SM)
 _="on click remove .is-open from closest .modal-overlay then reset #role-assign-form" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 onclick="document.querySelector('#role-ids-input').value=Array.from(document.querySelectorAll('.role-checkbox:checked')).map(function(c){return c.value}).join(',')" {
 "保存"
 }
 }
 }
 }
 }
}

fn dept_assign_modal(action: &str, all_depts: &[Department], current_ids: &[i64]) -> Markup {
 html! {
 div id="dept-assign-modal" class="modal-overlay fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
 _="on click[me is event.target] remove .is-open" {
 form id="dept-assign-form" class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
 hx-post=(action) hx-swap="none" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="text-base font-semibold text-fg" { "管理部门" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
 _="on click remove .is-open from closest .modal-overlay then reset #dept-assign-form" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 input type="hidden" name="dept_ids" id="dept-ids-input" {}
 div class="flex flex-col gap-1" {
 @for dept in all_depts {
 label class="flex items-center gap-2.5 py-2 px-2 rounded-sm cursor-pointer hover:bg-surface transition-colors" {
 input type="checkbox" class="dept-checkbox w-4 h-4 accent-[var(--accent)] cursor-pointer"
 value=(dept.department_id) checked[current_ids.contains(&dept.department_id)];
 span class="text-sm text-fg" { (dept.department_name) }
 span class="text-xs text-muted font-mono" { (dept.department_code) }
 @if !dept.is_active {
 span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff2f0] text-[#cf1322]" { "停用" }
 }
 }
 }
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class=(BTN_DEFAULT_SM)
 _="on click remove .is-open from closest .modal-overlay then reset #dept-assign-form" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
 onclick="document.querySelector('#dept-ids-input').value=Array.from(document.querySelectorAll('.dept-checkbox:checked')).map(function(c){return c.value}).join(',')" {
 "保存"
 }
 }
 }
 }
 }
}

fn reset_password_modal(action: &str) -> Markup {
 html! {
 div id="reset-pw-modal" class="modal-overlay fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
 _="on click[me is event.target] remove .is-open" {
 form id="reset-pw-form" class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
 hx-post=(action) hx-swap="none" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 class="text-base font-semibold text-fg" { "重置密码" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
 _="on click remove .is-open from closest .modal-overlay then reset #reset-pw-form" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 p class="text-muted text-sm mb-4" { "为该用户设置新密码，重置后立即生效。" }
 div class="mb-4" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "新密码 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="password" name="new_password" required
 minlength="8" placeholder="至少 8 位，含字母和数字" {}
 }
 div {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "确认密码 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="password" name="confirm_password" required
 minlength="8" placeholder="再次输入新密码" {}
 }
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button type="button" class=(BTN_DEFAULT_SM)
 _="on click remove .is-open from closest .modal-overlay then reset #reset-pw-form" { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "确认重置"
 }
 }
 }
 }
 }
}
