use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::shared::identity::model::{Department, UserWithRoles};
use abt_core::shared::identity::{DepartmentService, UserService};
use abt_macros::require_permission;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::department::*;
use crate::utils::RequestContext;

// ── Handlers ──

/// GET /admin/system/departments — main page with tree + detail (first dept preloaded)
#[require_permission("DEPARTMENT", "read")]
pub async fn get_department_list(
 _path: DepartmentListPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_create = ctx.has_permission("DEPARTMENT", "create").await;
 let can_delete = ctx.has_permission("DEPARTMENT", "delete").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;
 let dept_svc = state.department_service();
 let user_svc = state.user_service();

 let departments = dept_svc.list_departments(&service_ctx, &mut conn).await?;

 // Preload first department's detail panel
 let first_id = departments.first().map(|d| d.department_id);
 let initial_detail = if let Some(id) = first_id {
 let dept = dept_svc
 .get_department(&service_ctx, &mut conn, id)
 .await
 .ok();
 if let Some(dept) = dept {
 let all_users = user_svc
 .list_users_with_roles(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();
 let mut members: Vec<UserWithRoles> = Vec::new();
 for u in &all_users {
 if let Ok(user_depts) = dept_svc
 .get_user_departments(&service_ctx, &mut conn, u.user.user_id)
 .await
 && user_depts.iter().any(|d| d.department_id == id) {
 members.push(u.clone());
 }
 }
 Some(detail_content_fragment(&dept, &members, can_create, can_delete))
 } else {
 None
 }
 } else {
 None
 };

 let content = department_list_page(&departments, first_id, initial_detail.as_ref(), can_create);

 let page_html = admin_page(
 is_htmx,
 "部门管理",
 &claims,
 "system",
 DepartmentListPath::PATH,
 "部门管理",
 Some("组织架构"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// GET /admin/system/departments/{id} — detail panel fragment (HTMX target)
#[require_permission("DEPARTMENT", "read")]
pub async fn get_department_detail_fragment(
 path: DepartmentDetailPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let can_create = ctx.has_permission("DEPARTMENT", "create").await;
 let can_delete = ctx.has_permission("DEPARTMENT", "delete").await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let dept_svc = state.department_service();
 let user_svc = state.user_service();
 let dept = dept_svc
 .get_department(&service_ctx, &mut conn, path.id)
 .await?;
 let all_users = user_svc
 .list_users_with_roles(&service_ctx, &mut conn)
 .await?;
 // Filter users belonging to this department
 let mut members: Vec<UserWithRoles> = Vec::new();
 for u in &all_users {
 let user_depts = dept_svc
 .get_user_departments(&service_ctx, &mut conn, u.user.user_id)
 .await?;
 if user_depts.iter().any(|d| d.department_id == path.id) {
 members.push(u.clone());
 }
 }

 let fragment = detail_content_fragment(&dept, &members, can_create, can_delete);
 Ok(Html(fragment.into_string()))
}

/// GET /admin/system/departments/create — create drawer fragment
#[require_permission("DEPARTMENT", "create")]
pub async fn get_department_create_drawer(
 _path: DepartmentCreateDrawerPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let fragment = dept_drawer_fragment(false, None);
 Ok(Html(fragment.into_string()))
}

/// POST /admin/system/departments — create dept
#[require_permission("DEPARTMENT", "create")]
pub async fn post_department_create(
 _path: DepartmentCreateDrawerPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<DeptCreateForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.department_service();

 let description = form.description.filter(|s| !s.is_empty());

 svc.create_department(
 &service_ctx,
 &mut conn,
 &form.department_name,
 &form.department_code,
 description.as_deref(),
 )
 .await?;

 let redirect = DepartmentListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// GET /admin/system/departments/{id}/edit — edit drawer fragment
#[require_permission("DEPARTMENT", "update")]
pub async fn get_department_edit_drawer(
 path: DepartmentEditPath,
 ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.department_service();
 let dept = svc.get_department(&service_ctx, &mut conn, path.id).await?;

 let fragment = dept_drawer_fragment(true, Some(&dept));
 Ok(Html(fragment.into_string()))
}

/// POST /admin/system/departments/{id}/edit — update dept
#[require_permission("DEPARTMENT", "update")]
pub async fn post_department_update(
 path: DepartmentEditPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<DeptEditForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.department_service();

 let description = form.description.filter(|s| !s.is_empty());

 svc.update_department(
 &service_ctx,
 &mut conn,
 path.id,
 &form.department_name,
 description.as_deref(),
 )
 .await?;

 // Update active status if the service supports it
 if let Some(is_active) = form.is_active {
 svc.update_department_status(&service_ctx, &mut conn, path.id, is_active)
 .await?;
 }

 let redirect = DepartmentListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST /admin/system/departments/{id}/delete — delete dept
#[require_permission("DEPARTMENT", "delete")]
pub async fn delete_department(
 path: DepartmentDeletePath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.department_service();

 svc.delete_department(&service_ctx, &mut conn, path.id)
 .await?;

 let redirect = DepartmentListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct DeptCreateForm {
 pub department_name: String,
 pub department_code: String,
 pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeptEditForm {
 pub department_name: String,
 pub description: Option<String>,
 pub is_active: Option<bool>,
}

// ── Shared class strings ──

const BTN_DEFAULT_SM: &str =
 "inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border \
  hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
  cursor-pointer transition-all duration-150 shadow-xs";

const BTN_DANGER_SM: &str =
 "inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-danger border border-border \
  hover:bg-[#fff2f0] hover:border-[rgba(220,38,38,0.3)] text-sm font-medium \
  cursor-pointer transition-all duration-150 shadow-xs";

const FIELD_INPUT: &str =
 "w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg \
  transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]";

// ── Components ──

fn department_list_page(
 departments: &[Department],
 selected_id: Option<i64>,
 initial_detail: Option<&Markup>,
 can_create: bool,
) -> Markup {
 html! {
 div {
 // ── Tree + Detail split ──
 div class="grid grid-cols-[280px_1fr] bg-white rounded-xl border border-border-soft overflow-hidden min-h-[600px]" {
 // ── Left: Tree Panel ──
 (tree_panel(departments, selected_id, can_create))

 // ── Right: Detail Panel ──
 div class="flex-1 flex flex-col bg-white" id="deptDetail" {
 @if let Some(detail) = initial_detail {
 (detail)
 } @else {
 div class="flex flex-col items-center justify-center flex-1 text-muted gap-2" {
 (icon::building_icon("w-10 h-10 opacity-40"))
 h4 class="text-sm font-medium text-fg-2" { "选择部门查看详情" }
 p class="text-xs" { "点击左侧组织架构中的部门节点" }
 }
 }
 }
 }

 // ── Drawer (create/edit) — uses preflight .drawer-overlay / .drawer-panel ──
 div class="drawer-overlay fixed inset-0 z-[1000] justify-end bg-[rgba(15,23,42,0.45)]" id="deptDrawer"
 tabindex="-1"
 _="on click[me is event.target] remove .open from #deptDrawer on keydown[event.key is 'Escape'] remove .open from #deptDrawer" {
 div class="drawer-panel bg-white h-full w-[440px] flex flex-col shadow-xl" id="drawerPanel" {
 // Content loaded via HTMX
 }
 }
 }
 }
}

fn tree_panel(departments: &[Department], selected_id: Option<i64>, can_create: bool) -> Markup {
 let count = departments.len();
 html! {
 div class="flex flex-col border-r border-border-soft bg-white" {
 // ── Top bar ──
 div class="p-4 flex justify-between items-center" {
 h3 class="flex items-center gap-1.5 text-sm font-semibold text-fg" {
 (icon::building_icon("w-[15px] h-[15px] text-muted"))
 "组织架构"
 }
 @if can_create {
 button class="w-[26px] h-[26px] border border-border rounded-sm bg-white grid place-items-center cursor-pointer text-muted hover:text-accent hover:border-accent transition-colors" title="新建部门"
 hx-get=(DepartmentCreateDrawerPath::PATH)
 hx-target="#drawerPanel"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #deptDrawer" {
 (icon::plus_icon("w-[13px] h-[13px]"))
 }
 }
 }

 // ── Search ──
 div class="px-4 pb-2" {
 input type="text" class="tree-search w-full px-3 py-1.5 border border-border-soft rounded-sm text-xs bg-surface text-fg outline-none focus:border-accent" placeholder="搜索部门…" {}
 }

 // ── Tree list ──
 div class="flex-1 overflow-y-auto p-1" id="deptTree" {
 @for dept in departments {
 (tree_item(dept, selected_id == Some(dept.department_id)))
 }
 @if departments.is_empty() {
 div class="text-center p-6 text-muted text-sm" {
 "暂无部门数据"
 }
 }
 }

 // ── Footer ──
 div class="border-t border-border-soft px-4 py-2 text-[11px] text-muted bg-surface" id="treeFoot" {
 "共 " (count) " 个部门"
 }

 // ── Search script (client-side filter by data-name / data-code) ──
 script {
 (maud::PreEscaped(r#"
document.querySelector('.tree-search').addEventListener('input', function() {
 var kw=this.value.toLowerCase(), items=document.querySelectorAll('.tree-item'), n=0;
 items.forEach(function(it){
 var show=!kw||(it.dataset.name||'').toLowerCase().indexOf(kw)!==-1||(it.dataset.code||'').toLowerCase().indexOf(kw)!==-1;
 it.style.display=show?'':'none'; if(show)n++;
 });
 var foot=document.querySelector('#treeFoot');
 if(foot) foot.textContent='共 '+(kw?n:items.length)+' 个部门'+(kw?'（筛选中）':'');
});"#))
 }
 }
 }
}

fn tree_item(dept: &Department, is_selected: bool) -> Markup {
 let code_color = dept_code_color_class(&dept.department_code);
 let detail_path = DepartmentDetailPath { id: dept.department_id }.to_string();
 let active = if is_selected { " active" } else { "" };

 html! {
 div class=(format!("tree-item flex items-center gap-2 px-3 py-2 rounded-sm cursor-pointer transition-colors hover:bg-surface [&.active]:bg-accent-bg [&.active]:text-accent{active}"))
 data-id=(dept.department_id)
 data-name=(dept.department_name)
 data-code=(dept.department_code)
 hx-get=(detail_path)
 hx-target="#deptDetail"
 hx-swap="innerHTML"
 _="on click take .active from .tree-item" {
 span class=(format!("shrink-0 w-7 h-7 rounded-md flex items-center justify-center text-[10px] font-bold {}", code_color)) {
 (dept.department_code.chars().take(2).collect::<String>())
 }
 span class="flex-1 overflow-hidden whitespace-nowrap text-sm font-medium" {
 (dept.department_name)
 }
 @if dept.is_default {
 span class="text-[10px] shrink-0 font-medium px-1.5 py-0.5 rounded bg-[#fff7e6] text-[#fa8c16]" { "默认" }
 }
 @if !dept.is_active {
 span class="text-[10px] shrink-0 font-medium px-1.5 py-0.5 rounded bg-surface text-muted" { "停用" }
 }
 }
 }
}

fn detail_content_fragment(dept: &Department, members: &[UserWithRoles], can_create: bool, can_delete: bool) -> Markup {
 let code_color = dept_code_color_class(&dept.department_code);
 let member_count = members.len();
 let edit_path = DepartmentEditPath { id: dept.department_id }.to_string();
 let delete_path = DepartmentDeletePath { id: dept.department_id }.to_string();

 let description = match &dept.description {
 Some(d) => d.as_str(),
 None => "",
 };

 let status_text = if dept.is_active { "已激活" } else { "已停用" };
 let default_text = if dept.is_default { "默认" } else { "普通" };
 let status_class = if dept.is_active { "text-success" } else { "text-muted" };

 html! {
 // ── Hero ──
 div class="p-5 border-b border-border-soft flex justify-between items-center gap-3" {
 div class="flex items-center gap-3 min-w-0" {
 div class=(format!("shrink-0 w-10 h-10 rounded-md flex items-center justify-center {}", code_color)) {
 (icon::building_icon("w-5 h-5"))
 }
 div class="min-w-0" {
 h2 class="text-base font-semibold text-fg truncate" { (dept.department_name) }
 div class="flex items-center gap-2 mt-0.5 flex-wrap" {
 span class="font-mono text-[11px] text-muted" { (dept.department_code) }
 @if !description.is_empty() {
 span class="text-[11px] text-muted truncate" { "· " (description) }
 }
 }
 }
 }
 div class="flex items-center gap-2 shrink-0" {
 @if can_create {
 button class=(BTN_DEFAULT_SM)
 hx-get=(edit_path)
 hx-target="#drawerPanel"
 hx-swap="innerHTML"
 _="on 'htmx:afterRequest' add .open to #deptDrawer" {
 (icon::edit_icon("w-3.5 h-3.5"))
 "编辑"
 }
 }
 @if can_delete && !dept.is_default {
 button class=(BTN_DANGER_SM)
 hx-confirm=(format!("确认删除部门「{}」？该操作不可恢复。", dept.department_name))
 hx-post=(delete_path)
 hx-swap="none" {
 (icon::trash_icon("w-3.5 h-3.5"))
 "删除"
 }
 }
 }
 }

 // ── Stats ──
 div class="flex gap-3 p-4 border-b border-border-soft bg-surface" {
 (stat_pill("bg-[#1677ff]", &member_count.to_string(), "名成员", None))
 (stat_pill("bg-[#52c41a]", status_text, "", Some(status_class)))
 (stat_pill("bg-[#faad14]", default_text, "部门", None))
 }

 // ── Body ──
 div class="flex-1 overflow-y-auto p-5 space-y-5" {
 // Info section
 div {
 div class="flex items-center gap-1.5 mb-2" {
 span class="text-[13px] font-semibold text-fg flex items-center gap-1.5" {
 (icon::circle_alert_icon("w-[14px] h-[14px] text-muted"))
 "基本信息"
 }
 }
 div class="bg-bg border border-border-soft rounded-lg overflow-hidden" {
 (detail_info_row("部门 ID", html! { "#" (format!("{:03}", dept.department_id)) }, true))
 (detail_info_row("部门代码", html! { (dept.department_code) }, true))
 (detail_info_row("部门状态", html! { span class=(status_class) { (status_text) } }, false))
 (detail_info_row("部门类型", html! { (default_text) "部门" }, false))
 (detail_info_row("创建时间", html! { (dept.created_at.format("%Y-%m-%d %H:%M")) }, false))
 @if let Some(updated) = &dept.updated_at {
 (detail_info_row("最后更新", html! { (updated.format("%Y-%m-%d %H:%M")) }, false))
 }
 }
 }

 // Members section
 div {
 div class="flex items-center justify-between mb-2" {
 span class="text-[13px] font-semibold text-fg flex items-center gap-1.5" {
 (icon::users_icon("w-[14px] h-[14px] text-muted"))
 "部门成员"
 }
 span class="text-[11px] text-muted bg-surface px-2 py-0.5 rounded-full border border-border-soft" { (member_count) " 人" }
 }
 @if members.is_empty() {
 div class="text-center p-6 text-muted text-sm border border-dashed border-border-soft rounded-lg" {
 "暂无成员"
 }
 } @else {
 div class="grid gap-2" {
 @for (i, m) in members.iter().enumerate() {
 @if i < 4 {
 (member_card(m))
 }
 }
 @if member_count > 4 {
 div class="text-xs text-muted text-center py-1" {
 "还有 " (member_count - 4) " 人…"
 }
 }
 }
 }
 }
 }
 }
}

fn stat_pill(dot_class: &str, value: &str, label: &str, value_cls: Option<&str>) -> Markup {
 let vcls = value_cls.unwrap_or("text-fg");
 html! {
 div class="flex items-center gap-2 bg-white rounded-md border border-border-soft flex-1 px-3 py-2" {
 span class=(format!("w-2 h-2 rounded-full shrink-0 {}", dot_class)) {}
 b class=(format!("text-sm font-bold leading-none {}", vcls)) { (value) }
 @if !label.is_empty() {
 span class="text-[11px] text-muted" { (label) }
 }
 }
 }
}

fn detail_info_row(label: &str, value: Markup, mono: bool) -> Markup {
 let val_cls = if mono { "text-fg font-medium font-mono text-xs text-accent" } else { "text-fg font-medium text-sm" };
 html! {
 div class="flex items-center justify-between px-4 py-2.5 border-b border-border-soft last:border-b-0" {
 span class="text-xs text-muted" { (label) }
 span class=(val_cls) { (value) }
 }
 }
}

fn member_card(m: &UserWithRoles) -> Markup {
 let display_name = m.user.display_name.as_deref().unwrap_or(&m.user.username);
 let initials = get_initials(display_name);
 let ava_color = avatar_color_class(&m.user.username);

 let role_names: Vec<&str> = m.roles.iter().map(|r| r.role_name.as_str()).collect();
 let role_display = role_names.first().copied().unwrap_or("—");

 html! {
 div class="flex items-center gap-2 border border-border-soft rounded-md bg-white px-3 py-2" {
 span class=(format!("inline-flex w-8 h-8 rounded-md items-center justify-center text-xs font-semibold text-white shrink-0 {}", ava_color)) {
 (initials)
 }
 div class="min-w-0" {
 div class="text-[13px] font-semibold text-fg truncate" { (display_name) }
 span class="inline-block text-[10px] font-medium px-1.5 py-0.5 rounded bg-surface text-muted" { (role_display) }
 }
 }
 }
}

fn dept_drawer_fragment(is_edit: bool, dept: Option<&Department>) -> Markup {
 let title = if is_edit { "编辑部门" } else { "新建部门" };
 let subtitle = if let Some(d) = &dept {
 if is_edit {
 format!("修改「{}」的部门信息", d.department_name)
 } else {
 "填写部门信息后保存".to_string()
 }
 } else {
 "填写部门信息后保存".to_string()
 };

 let (action_path, name_val, code_val, desc_val, is_active_val) = match &dept {
 Some(d) => (
 DepartmentEditPath { id: d.department_id }.to_string(),
 d.department_name.as_str(),
 d.department_code.as_str(),
 d.description.as_deref().unwrap_or(""),
 d.is_active,
 ),
 None => (
 DepartmentCreateDrawerPath::PATH.to_string(),
 "",
 "",
 "",
 true,
 ),
 };

 html! {
 form id="deptForm" class="flex flex-col h-full" hx-post=(action_path) hx-swap="none"
 _="on 'htmx:afterRequest' remove .open from #deptDrawer" {
 // ── Header ──
 div class="flex items-center justify-between px-6 py-4 border-b border-border-soft shrink-0" {
 div class="flex items-center gap-2.5 min-w-0" {
 div class="w-9 h-9 rounded-md flex items-center justify-center bg-accent-bg text-accent shrink-0" {
 (icon::building_icon("w-[18px] h-[18px]"))
 }
 div class="min-w-0" {
 h3 class="text-sm font-semibold text-fg truncate" { (title) }
 p class="text-xs text-muted truncate" { (subtitle) }
 }
 }
 button class="w-8 h-8 grid place-items-center rounded-sm text-muted hover:text-fg hover:bg-surface cursor-pointer border-none bg-transparent" type="button"
 _="on click remove .open from #deptDrawer" {
 (icon::x_icon("w-[18px] h-[18px]"))
 }
 }

 // ── Body ──
 div class="flex-1 overflow-y-auto p-6" {
 // ── Basic Info ──
 div class="mb-6" {
 div class="text-xs font-semibold text-fg-2 mb-3" { "基本信息" }
 div class="form-field mb-4" {
 label { "部门名称 " span class="text-danger" { "*" } }
 input class=(FIELD_INPUT) type="text" name="department_name" required placeholder="如：销售部" value=(name_val) {}
 }
 div class="form-field mb-4" {
 label { "部门代码 " span class="text-danger" { "*" } }
 @if is_edit {
 input class=(format!("{FIELD_INPUT} font-mono cursor-not-allowed opacity-70")) type="text" name="department_code"
 required placeholder="如：SA" value=(code_val) readonly {}
 } @else {
 input class=(format!("{FIELD_INPUT} font-mono")) type="text" name="department_code"
 required placeholder="如：SA" value=(code_val) {}
 }
 }
 div class="form-field" {
 label { "部门描述" }
 textarea class=(FIELD_INPUT) name="description" rows="3" placeholder="描述该部门的职责和业务范围…" {
 (desc_val)
 }
 }
 }

 // ── Settings ──
 div class="mb-6" {
 div class="text-xs font-semibold text-fg-2 mb-3" { "设置" }
 @if is_edit {
 label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer py-1.5" {
 input type="checkbox" name="is_active" value="true" class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
 checked[is_active_val] {}
 "启用部门"
 }
 } @else {
 input type="hidden" name="is_active" value="true" {}
 }
 @if let Some(d) = dept {
 @if d.is_default {
 label class="flex items-center gap-2 text-[13px] text-fg py-1.5" {
 input type="checkbox" class="w-4 h-4 accent-[var(--accent)]" checked disabled {}
 "默认部门"
 span class="text-xs text-muted" { "（系统默认部门不可取消）" }
 }
 }
 }
 }

 // ── Tip ──
 div class="bg-accent-bg border border-[rgba(37,99,235,0.1)] rounded-md p-3 px-4 text-xs text-fg-2 leading-relaxed flex gap-2" {
 (icon::circle_alert_icon("w-[15px] h-[15px] text-accent shrink-0 mt-px"))
 div { "部门代码用于系统内部标识，创建后不可修改。建议使用大写英文字母缩写，如 "
 strong { "SA" } "（销售部）、" strong { "PU" } "（采购部）。"
 }
 }
 }

 // ── Footer ──
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
 button class=(BTN_DEFAULT_SM) type="button" _="on click remove .open from #deptDrawer" { "取消" }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit" {
 (icon::check_circle_icon("w-4 h-4"))
 "保存"
 }
 }
 }
 }
}

// ── Helpers ──

/// Map department code to atomic bg+text color classes for badges/icons.
fn dept_code_color_class(code: &str) -> &'static str {
 match code.to_uppercase().as_str() {
 "GO" | "GM" => "bg-[#f3e8ff] text-[#7c3aed]",
 "SA" | "SL" => "bg-[#e8f4ff] text-[#1677ff]",
 "PU" | "PC" => "bg-[#e6fffb] text-[#13c2c2]",
 "WH" | "WM" => "bg-[#fff7e6] text-[#fa8c16]",
 "FI" | "FN" => "bg-[#f0fff0] text-[#389e0d]",
 "QC" | "QA" => "bg-[#fff2f0] text-[#cf1322]",
 _ => {
 let first = code.chars().next().unwrap_or('A');
 match first.to_ascii_uppercase() {
 'A'..='D' => "bg-[#e8f4ff] text-[#1677ff]",
 'E'..='H' => "bg-[#f0fff0] text-[#389e0d]",
 'I'..='L' => "bg-[#e6fffb] text-[#13c2c2]",
 'M'..='P' => "bg-[#fff7e6] text-[#fa8c16]",
 'Q'..='T' => "bg-[#f3e8ff] text-[#7c3aed]",
 _ => "bg-[#fff2f0] text-[#cf1322]",
 }
 }
 }
}

/// Get 2-char initials from display name.
fn get_initials(name: &str) -> String {
 let chars: Vec<char> = name.chars().collect();
 if chars.len() >= 2 {
 format!("{}{}", chars[0], chars[1])
 } else if chars.len() == 1 {
 chars[0].to_string()
 } else {
 "??".to_string()
 }
}

/// Deterministic avatar color based on username.
fn avatar_color_class(username: &str) -> &'static str {
 let hash = username.chars().map(|c| c as u32).sum::<u32>();
 match hash % 6 {
 0 => "bg-blue-500",
 1 => "bg-green-500",
 2 => "bg-purple-500",
 3 => "bg-orange-500",
 4 => "bg-pink-500",
 _ => "bg-teal-500",
 }
}
