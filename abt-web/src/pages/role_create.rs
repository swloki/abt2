use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::shared::identity::model::Role;
use abt_core::shared::identity::RoleService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::role::{RoleCreatePath, RoleListPath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("ROLE", "create")]
pub async fn get_role_create(
 _path: RoleCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;
 let svc = state.role_service();
 let roles = svc.list_roles(&service_ctx, &mut conn).await?;

 let content = role_create_page(&roles);
 let page_html = admin_page(
 is_htmx,
 "新建角色",
 &claims,
 "system",
 RoleCreatePath::PATH,
 "系统管理",
 Some("新建用户"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROLE", "create")]
pub async fn post_role_create(
 _path: RoleCreatePath,
 ctx: RequestContext,
 Form(form): Form<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.role_service();

 let role_name = form
 .get("role_name")
 .map(|s| s.trim().to_string())
 .unwrap_or_default();
 let role_code = form
 .get("role_code")
 .map(|s| s.trim().to_string())
 .unwrap_or_default();
 let description = form
 .get("description")
 .and_then(|s| if s.trim().is_empty() { None } else { Some(s.trim().to_string()) });
 let parent_role_id = form
 .get("parent_role_id")
 .and_then(|s| {
 let v = s.trim();
 if v.is_empty() {
 None
 } else {
 v.parse::<i64>().ok()
 }
 });

 let role = svc
 .create_role(
 &service_ctx,
 &mut conn,
 &role_name,
 &role_code,
 description.as_deref(),
 parent_role_id,
 )
 .await?;

 // Permissions are managed on the dedicated /admin/system/permissions page.
 // Any legacy "perm_RESOURCE:action" keys are still honored if submitted.
 let assigned: Vec<(String, String)> = form
 .keys()
 .filter(|k| k.starts_with("perm_"))
 .filter_map(|k| {
 let perm = &k[5..];
 let (resource, action) = perm.split_once(':')?;
 Some((resource.to_string(), action.to_string()))
 })
 .collect();

 if !assigned.is_empty() {
 svc.assign_permissions(&service_ctx, &mut conn, role.role_id, assigned)
 .await?;
 }

 let redirect = RoleListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

const INFO_CARD: &str =
 "bg-bg border border-border-soft rounded-lg overflow-hidden shadow-xs mb-5";

const INFO_CARD_TITLE: &str =
 "flex items-center gap-2 px-4 py-3 border-b border-border-soft bg-surface-raised text-sm \
  font-semibold text-fg";

fn role_create_page(roles: &[Role]) -> Markup {
 html! {
 form id="role-form"
 method="POST"
 action=(RoleCreatePath::PATH)
 hx-post=(RoleCreatePath::PATH)
 hx-swap="none" {

 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建角色" }
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
 input type="text" name="role_name" required placeholder="如：销售经理" {}
 }
 div class="form-field" {
 label { "角色代码 " span class="text-danger" { "*" } }
 input type="text" name="role_code" required placeholder="如：sales_manager（英文+下划线）" {}
 span class="text-xs text-muted mt-1" { "唯一标识，对应 JWT claims 中的 role_codes" }
 }
 div class="form-field" {
 label { "上级角色" }
 select name="parent_role_id" {
 option value="" { "无（顶级角色）" }
 @for role in roles {
 @if !role.is_system_role {
 option value=(role.role_id) { (role.role_name) }
 }
 }
 }
 span class="text-xs text-muted mt-1" { "选择上级角色后，该角色将自动继承上级角色的全部权限。继承的权限在权限配置页面以灰色标记，不可移除。" }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "角色类型" }
 label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
 input type="checkbox" name="is_system" value="1" class="w-4 h-4 accent-[var(--accent)] cursor-pointer" {}
 span { "内置角色（不可删除）" }
 }
 }
 div class="form-field field-full" {
 label { "描述" }
 textarea name="description" rows="3" placeholder="角色用途说明" {}
 }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 href=(format!("{}?restore=true", RoleListPath::PATH)) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "保存"
 }
 }
 }
 }
}
