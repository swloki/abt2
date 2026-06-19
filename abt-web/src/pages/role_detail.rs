use std::collections::HashMap;

use axum::Form;
use axum::response::{Html, IntoResponse};
use maud::{html, Markup};

use abt_core::shared::identity::RoleService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::role::{RoleDetailPath, RoleEditPath, RoleListPath, RolePermissionPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("ROLE", "read")]
pub async fn get_role_detail(
 path: RoleDetailPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let role_svc = state.role_service();
 let user_svc = state.user_service();

 let rwp = role_svc.get_role_with_permissions(&service_ctx, &mut conn, path.id).await?;

 // Load all roles once for parent/child resolution
 let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;

 // Resolve parent role name
 let parent_role_name = rwp.role.parent_role_id
 .and_then(|pid| all_roles.iter().find(|r| r.role_id == pid))
 .map(|r| r.role_name.clone());

 // Find child roles (roles whose parent_role_id == current role_id)
 let child_roles: Vec<_> = all_roles.iter()
 .filter(|r| r.parent_role_id == Some(path.id))
 .collect();

 // Count users assigned to this role
 let all_users = user_svc.list_users_with_roles(&service_ctx, &mut conn).await?;
 let user_count = all_users.iter()
 .filter(|u| u.roles.iter().any(|r| r.role_id == path.id))
 .count();

 let direct_count = rwp.permissions.len();
 let inherited_count = rwp.inherited_permissions.len();

 let content = role_detail_page(
 &rwp,
 parent_role_name.as_deref(),
 &child_roles,
 user_count,
 direct_count,
 inherited_count,
 );

 let detail_path_str = RoleDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 角色详情", rwp.role.role_name),
 &claims,
 "system",
 &detail_path_str,
 "系统管理",
 Some(&rwp.role.role_name),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// Keep handler for backward compatibility — permission editing lives on
/// /admin/system/permissions now.
#[require_permission("ROLE", "update")]
pub async fn post_permission_assign(
 path: RolePermissionPath,
 ctx: RequestContext,
 Form(form): Form<HashMap<String, String>>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.role_service();

 let assigned: std::collections::HashSet<String> = form
 .keys()
 .filter(|k| k.starts_with("perm_"))
 .map(|k| k[5..].to_string())
 .collect();

 let rwp = svc.get_role_with_permissions(&service_ctx, &mut conn, path.id).await?;
 let current: std::collections::HashSet<String> = rwp.permissions.into_iter().collect();

 let to_add: Vec<(String, String)> = assigned
 .difference(&current)
 .filter_map(|p| parse_permission(p))
 .collect();
 let to_remove: Vec<(String, String)> = current
 .difference(&assigned)
 .filter_map(|p| parse_permission(p))
 .collect();

 if !to_add.is_empty() {
 svc.assign_permissions(&service_ctx, &mut conn, path.id, to_add).await?;
 }
 if !to_remove.is_empty() {
 svc.remove_permissions(&service_ctx, &mut conn, path.id, to_remove).await?;
 }

 let detail = RoleDetailPath { id: path.id };
 Ok(([("HX-Redirect", detail.to_string())], Html(String::new())))
}

// ── Helpers ──

fn parse_permission(perm: &str) -> Option<(String, String)> {
 let (resource, action) = perm.split_once(':')?;
 Some((resource.to_string(), action.to_string()))
}

fn get_initials(s: &str) -> String {
 s.chars().take(2).collect::<String>().to_uppercase()
}

// ── Shared class strings ──

const BTN_DEFAULT_SM: &str =
 "inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border \
  hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
  cursor-pointer transition-all duration-150";

const INFO_CARD: &str =
 "bg-bg border border-border-soft rounded-lg overflow-hidden shadow-xs mb-5";

const INFO_CARD_TITLE: &str =
 "flex items-center gap-2 px-4 py-3 border-b border-border-soft bg-surface-raised text-sm \
  font-semibold text-fg";

const TAG_PILL: &str =
 "text-[10px] px-2 py-[2px] rounded-[3px] font-semibold tracking-[0.02em]";

// ── Page Component ──

fn role_detail_page(
 rwp: &abt_core::shared::identity::model::RoleWithPermissions,
 parent_role_name: Option<&str>,
 child_roles: &[&abt_core::shared::identity::model::Role],
 user_count: usize,
 direct_count: usize,
 inherited_count: usize,
) -> Markup {
 let role = &rwp.role;
 let role_id = role.role_id;
 let list_path = RoleListPath.to_string();
 let edit_path = RoleEditPath { id: role_id }.to_string();
 let initials = get_initials(&role.role_name);

 html! {
 div class="space-y-5" {
 // ── Back Link ──
 a class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg transition-colors"
 href=(format!("{list_path}?restore=true")) {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回角色列表"
 }

 // ── Profile Hero ──
 div class="flex items-center justify-between p-5 px-6 bg-bg border border-border-soft rounded-xl shadow-xs" {
 div class="flex items-center gap-4" {
 div class="w-[52px] h-[52px] rounded-lg flex items-center justify-center text-[18px] font-bold text-white shrink-0 bg-[linear-gradient(135deg,#7c3aed,#a78bfa)] shadow-[0_3px_10px_rgba(124,58,237,0.2)]" {
 (initials)
 }
 div {
 h2 class="text-[18px] font-bold text-fg mb-[3px] tracking-[-0.01em]" { (&role.role_name) }
 div class="flex items-center gap-[6px] flex-wrap" {
 span class="inline-flex items-center px-[7px] py-[1px] bg-surface border border-border rounded-[3px] font-mono text-[11px] font-semibold text-accent tracking-[0.04em]" {
 (&role.role_code)
 }
 span class="text-xs text-muted" {
 "ID: " (role_id) " · 创建于 "
 (role.created_at.format("%Y-%m-%d"))
 }
 }
 div class="flex gap-[4px] mt-[4px]" {
 @if role.is_system_role {
 span class=(format!("{TAG_PILL} bg-success-bg text-success border border-success-100")) { "内置角色" }
 } @else {
 span class=(format!("{TAG_PILL} bg-accent-50 text-accent border border-accent-100")) { "自定义角色" }
 }
 @if let Some(pname) = parent_role_name {
 span class=(format!("{TAG_PILL} bg-purple-bg text-purple border border-purple-100")) { "上级: " (pname) }
 }
 }
 }
 }
 div class="flex gap-[6px]" {
 a class=(BTN_DEFAULT_SM) href=(edit_path) {
 (icon::edit_icon("w-3.5 h-3.5"))
 "编辑"
 }
 }
 }

 // ── Stats Row ──
 div class="flex gap-3" {
 (stat_item("bg-accent", &direct_count.to_string(), "项直属权限"))
 (stat_item("bg-success-500", &user_count.to_string(), "个用户"))
 (stat_item("bg-purple", &inherited_count.to_string(), "项继承权限"))
 }

 // ── Role Info Card ──
 div class=(INFO_CARD) {
 div class=(INFO_CARD_TITLE) {
 (icon::lock_icon("w-[18px] h-[18px] text-accent"))
 "角色信息"
 }
 div {
 (info_row("角色 ID", html! { span class="text-fg font-medium font-mono text-xs text-accent" { "#" (format!("{:03}", role_id)) } }))
 (info_row("角色编码", html! { span class="text-fg font-medium font-mono text-xs text-accent" { (&role.role_code) } }))
 (info_row("角色类型", html! {
 @if role.is_system_role {
 span class="text-success font-medium" { "内置角色" }
 } @else {
 span class="text-fg font-medium" { "自定义角色" }
 }
 }))
 (info_row("上级角色", html! {
 @if let Some(pname) = parent_role_name {
 span class="text-fg font-medium" { (pname) }
 } @else {
 span class="text-muted font-medium" { "无" }
 }
 }))
 (info_row("描述", html! {
 @if let Some(desc) = &role.description {
 span class="text-fg font-medium" { (desc) }
 } @else {
 span class="text-muted font-medium" { "—" }
 }
 }))
 (info_row("创建时间", html! { span class="text-fg font-medium font-mono text-xs text-accent" { (role.created_at.format("%Y-%m-%d %H:%M")) } }))
 (info_row("最后更新", html! {
 @if let Some(updated) = &role.updated_at {
 span class="text-fg font-medium font-mono text-xs text-accent" { (updated.format("%Y-%m-%d %H:%M")) }
 } @else {
 span class="text-muted font-medium" { "—" }
 }
 }))
 }
 }

 // ── Child Roles Card ──
 @if !child_roles.is_empty() {
 div class=(INFO_CARD) {
 div class=(INFO_CARD_TITLE) {
 (icon::grid_icon("w-[18px] h-[18px] text-accent"))
 "下级角色"
 span class="ml-auto text-[11px] text-muted bg-bg px-2 py-[1px] rounded-full border border-border-soft" {
 (child_roles.len())
 }
 }
 div {
 @for child in child_roles {
 div class="flex items-center px-4 py-[9px] text-[13px] border-b border-border-soft last:border-b-0" {
 span class="w-[80px] shrink-0 text-muted text-xs" {
 a href=(RoleDetailPath { id: child.role_id }.to_string()) class="text-accent hover:underline" {
 (child.role_name)
 }
 }
 span class="text-fg font-medium font-mono text-xs text-accent" { (child.role_code) }
 }
 }
 }
 }
 }

 // ── Permissions managed on /admin/system/permissions ──
 div class=(INFO_CARD) {
 div class="p-4 flex items-center justify-between gap-3" {
 div class="flex items-center gap-2 text-sm text-muted" {
 (icon::sliders_icon("w-[18px] h-[18px] text-accent"))
 "角色权限请在权限管理页面统一配置"
 }
 a class="inline-flex items-center gap-1 text-sm font-medium text-accent hover:underline"
 href="/admin/system/permissions" {
 "前往权限管理"
 (icon::chevron_right_icon("w-4 h-4"))
 }
 }
 }
 }
 }
}

fn stat_item(dot_class: &str, value: &str, label: &str) -> Markup {
 html! {
 div class="flex items-center gap-2 py-[10px] px-4 bg-bg border border-border-soft rounded-md flex-1 shadow-xs" {
 span class=(format!("w-2 h-2 rounded-full shrink-0 {dot_class}")) {}
 b class="text-[15px] font-bold text-fg leading-none" { (value) }
 span class="text-[11px] text-muted ml-[2px]" { (label) }
 }
 }
}

fn info_row(label: &str, value: Markup) -> Markup {
 html! {
 div class="flex items-center px-4 py-[9px] text-[13px] border-b border-border-soft last:border-b-0" {
 span class="w-[80px] shrink-0 text-muted text-xs" { (label) }
 (value)
 }
 }
}
