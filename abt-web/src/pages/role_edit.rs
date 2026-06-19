use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum::Form;
use maud::{html, Markup};

use abt_core::shared::identity::RoleService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::role::{RoleDetailPath, RoleEditPath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("ROLE", "update")]
pub async fn get_role_edit(
 path: RoleEditPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.role_service();

 let rwp = svc.get_role_with_permissions(&service_ctx, &mut conn, path.id).await?;

 let parent_role_name = if rwp.role.parent_role_id.is_some() {
 let all_roles = svc.list_roles(&service_ctx, &mut conn).await?;
 rwp.role.parent_role_id
 .and_then(|pid| all_roles.iter().find(|r| r.role_id == pid))
 .map(|r| r.role_name.clone())
 } else {
 None
 };

 let content = role_edit_page(&rwp, parent_role_name.as_deref());
 let edit_path_str = RoleEditPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("编辑 {}", rwp.role.role_name),
 &claims,
 "system",
 &edit_path_str,
 "系统管理",
 Some(&format!("编辑 {}", rwp.role.role_name)),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROLE", "update")]
pub async fn post_role_edit(
 path: RoleEditPath,
 ctx: RequestContext,
 Form(form): Form<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.role_service();

 let role_name = form
 .get("role_name")
 .map(|s| s.trim().to_string())
 .unwrap_or_default();
 let description = form
 .get("description")
 .and_then(|s| if s.trim().is_empty() { None } else { Some(s.trim().to_string()) });

 // Only role name/description are edited here. Permissions are managed on the
 // dedicated /admin/system/permissions page — do NOT diff submitted perms here,
 // or an empty submission would wipe all of the role's permissions.
 svc.update_role(
 &service_ctx,
 &mut conn,
 path.id,
 &role_name,
 description.as_deref(),
 )
 .await?;

 let redirect = RoleDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

const INFO_CARD: &str =
 "bg-bg border border-border-soft rounded-lg overflow-hidden shadow-xs mb-5";

const INFO_CARD_TITLE: &str =
 "flex items-center gap-2 px-4 py-3 border-b border-border-soft bg-surface-raised text-sm \
  font-semibold text-fg";

fn role_edit_page(
 rwp: &abt_core::shared::identity::model::RoleWithPermissions,
 parent_role_name: Option<&str>,
) -> Markup {
 let role = &rwp.role;
 let detail_path = RoleDetailPath { id: role.role_id }.to_string();
 let edit_path = RoleEditPath { id: role.role_id }.to_string();

 html! {
 form id="role-form"
 method="POST"
 action=(&edit_path)
 hx-post=(&edit_path)
 hx-swap="none" {

 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "编辑角色" }
 }

 // ── Role Info ──
 div class=(INFO_CARD) {
 div class=(INFO_CARD_TITLE) {
 (icon::lock_icon("w-[18px] h-[18px] text-accent"))
 "角色信息"
 }
 div class="p-4" {
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="form-field" {
 label { "角色名称 " span class="text-danger" { "*" } }
 input type="text" name="role_name" required value=(role.role_name) {}
 }
 div class="form-field" {
 label { "角色编码" }
 input type="text" value=(role.role_code) disabled {}
 }
 @if let Some(pname) = parent_role_name {
 div class="form-field" {
 label { "上级角色" }
 input type="text" value=(pname) disabled {}
 }
 }
 div class="form-field field-full" {
 label { "描述" }
 textarea name="description" rows="3" placeholder="角色用途说明" {
 @if let Some(desc) = &role.description {
 (desc)
 }
 }
 }
 }
 }
 }

 // ── Permissions managed elsewhere ──
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

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 href=(&detail_path) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "保存修改"
 }
 }
 }
 }
}
