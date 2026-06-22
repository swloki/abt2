use axum::response::{Html, IntoResponse};
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::{DepartmentService, RoleService, UserService};
use abt_core::shared::identity::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::user::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct UserEditForm {
 pub display_name: Option<String>,
 pub is_super_admin: Option<String>,
 pub is_active: Option<String>,
 pub role_ids: Option<String>,
 pub dept_ids: Option<String>,
}

// ── Handlers ──

#[require_permission("USER", "update")]
pub async fn get_user_edit(
 path: UserEditPath,
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

 let user = user_svc
 .get_user_with_roles(&service_ctx, &mut conn, path.id)
 .await?;
 let user_depts = dept_svc
 .get_user_departments(&service_ctx, &mut conn, path.id)
 .await?;
 let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;
 let all_depts = dept_svc
 .list_departments(&service_ctx, &mut conn)
 .await?;

 let detail_path = UserDetailPath { id: path.id }.to_string();
 let title = format!("编辑用户 - {}", user.user.username);

 let content = user_edit_page(&user, &user_depts, &all_roles, &all_depts);
 let page_html = admin_page(
 is_htmx,
 &title,
 &claims,
 "system",
 &detail_path,
 "系统管理",
 Some(&user.user.username),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "update")]
pub async fn post_user_edit(
 path: UserEditPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<UserEditForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let user_svc = state.user_service();
 let dept_svc = state.department_service();

 // 1. Update display_name
 let display_name = form.display_name.filter(|s| !s.is_empty());
 user_svc
 .update_user(&service_ctx, &mut conn, path.id, display_name.as_deref())
 .await?;

 // 2. Update is_super_admin if changed
 let want_super_admin = form.is_super_admin.is_some();
 let current = user_svc
 .get_user_with_roles(&service_ctx, &mut conn, path.id)
 .await?;
 if current.user.is_super_admin != want_super_admin {
 user_svc
 .update_user_super_admin(&service_ctx, &mut conn, path.id, want_super_admin)
 .await?;
 }

 // 3. Update status if changed
 let want_active = form.is_active.is_some();
 if current.user.is_active != want_active {
 user_svc
 .update_user_status(&service_ctx, &mut conn, path.id, want_active)
 .await?;
 }

 // 4. Sync roles
 let role_ids: Vec<i64> = form
 .role_ids
 .as_deref()
 .map(|s| {
 s.split(',')
 .filter_map(|v| v.trim().parse::<i64>().ok())
 .collect()
 })
 .unwrap_or_default();
 user_svc
 .batch_assign_roles(&service_ctx, &mut conn, path.id, role_ids)
 .await?;

 // 5. Sync departments
 let dept_ids: Vec<i64> = form
 .dept_ids
 .as_deref()
 .map(|s| {
 s.split(',')
 .filter_map(|v| v.trim().parse::<i64>().ok())
 .collect()
 })
 .unwrap_or_default();
 dept_svc
 .assign_departments(&service_ctx, &mut conn, path.id, dept_ids)
 .await?;

 let redirect = UserDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Shared class strings ──

const BTN_DEFAULT: &str =
 "inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border \
  hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
  cursor-pointer transition-all duration-150 shadow-xs";

const BTN_PRIMARY: &str =
 "inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none \
  hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 \
  shadow-[0_1px_2px_rgba(37,99,235,0.2)]";

const SECTION: &str =
 "bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden";

const SECTION_HEAD: &str =
 "flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 \
  border-b border-border-soft";

const FIELD_INPUT: &str =
 "w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg \
  transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]";

// ── Components ──

fn user_edit_page(
 user: &UserWithRoles,
 user_depts: &[Department],
 all_roles: &[Role],
 all_depts: &[Department],
) -> Markup {
 let user_id = user.user.user_id;
 let edit_path = UserEditPath { id: user_id }.to_string();
 let detail_path = UserDetailPath { id: user_id }.to_string();

 // Pre-compute selected role/dept IDs
 let selected_role_ids: Vec<i64> = user.roles.iter().map(|r| r.role_id).collect();
 let selected_dept_ids: Vec<i64> = user_depts.iter().map(|d| d.department_id).collect();

 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(detail_path)
            { (icon::chevron_left_icon("w-4 h-4")) "返回用户详情" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "编辑用户" }
        }

        form id="user-edit-form" hx-post=(&edit_path) hx-swap="none" {
            // Hidden fields for multi-select values
            input
                type="hidden"
                name="role_ids"
                id="roleIdsInput"
                value=({
                    selected_role_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                }) {}
            input
                type="hidden"
                name="dept_ids"
                id="deptIdsInput"
                value=({
                    selected_dept_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                }) {}
            // ── Section 1: 基本信息 ──
            (basic_info_section(user))
            // ── Section 2: 角色分配 ──
            (role_section(all_roles, &selected_role_ids))
            // ── Section 3: 部门分配 ──
            (dept_section(all_depts, &selected_dept_ids))
            // ── Section 4: 数据权限（只读） ──
            (data_scope_section(user, user_depts))
        }
        // ── Action Bar ──
        div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
        {
            a class=(BTN_DEFAULT) href=(&detail_path) { "取消" }
            button type="submit" class=(BTN_PRIMARY) form="user-edit-form" {
                (icon::check_circle_icon("w-4 h-4"))
                "保存"
            }
        }
    }
}
}

fn basic_info_section(user: &UserWithRoles) -> Markup {
 let display_name_val = user.user.display_name.as_deref().unwrap_or("");
 let is_super_admin = user.user.is_super_admin;
 let is_active = user.user.is_active;

 html! {
    div class=(SECTION) {
        div class=(SECTION_HEAD) { (icon::user_icon("w-[18px] h-[18px]")) "基本信息" }
        div class="grid grid-cols-2 gap-4 gap-x-6 mb-2" {
            // 用户名 (disabled)
            div class="flex flex-col" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "登录名" }
                input
                    class=(format!("{FIELD_INPUT} opacity-60 cursor-not-allowed"))
                    type="text"
                    value=(&user.user.username)
                    disabled {}
                span class="text-xs text-muted mt-1" { "登录名不可修改" }
            }
            // 显示名称
            div class="flex flex-col" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                    "显示名称 "
                    span class="text-danger" { "*" }
                }
                input
                    class=(FIELD_INPUT)
                    type="text"
                    name="display_name"
                    value=(display_name_val)
                    placeholder="中文名称，如 张明" {}
            }
            // 超级管理员
            div class="flex flex-col" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "超级管理员" }
                label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
                    input
                        type="checkbox"
                        name="is_super_admin"
                        value="true"
                        class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                        checked[is_super_admin] {}
                    span { "设为超级管理员（绕过所有权限检查）" }
                }
                span class="text-xs text-muted mt-1" { "超级管理员拥有所有资源的完全访问权限，请谨慎授予" }
            }
            // 激活状态
            div class="flex flex-col" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "激活状态" }
                label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
                    input
                        type="checkbox"
                        name="is_active"
                        value="true"
                        class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                        checked[is_active] {}
                    span { "用户已激活，可正常登录系统" }
                }
            }
        }
    }
}
}

fn role_section(roles: &[Role], selected_ids: &[i64]) -> Markup {
 html! {
    div class=(SECTION) {
        div class=(SECTION_HEAD) { (icon::lock_icon("w-[18px] h-[18px]")) "角色分配" }
        p class="text-[13px] text-muted mb-4 leading-relaxed" { "用户可拥有多个角色，权限取所有角色的并集。" }
        div class="grid gap-2" {
            @for role in roles {
                @let is_sel = selected_ids.contains(&role.role_id);
                label
                    class="pick-item flex items-center gap-3 border rounded-md cursor-pointer transition-colors border-border-soft hover:border-border [&.selected]:border-accent [&.selected]:bg-accent-bg"
                {
                    input
                        type="checkbox"
                        name="role"
                        value=(role.role_id)
                        class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                        checked[is_sel] {}
                    span
                        class=({
                            format!(
                                "w-7 h-7 rounded-lg flex items-center justify-center text-[9px] font-bold text-white shrink-0 bg-[{}] ",
                                role_color(&role.role_code),
                            )
                        })
                    { (short_code(&role.role_code)) }
                    span class="text-sm font-medium text-fg" { (role.role_name) }
                    @if role.is_system_role {
                        span
                            class="text-[10px] font-medium px-[6px] py-[1px] rounded-[3px] bg-warn-bg text-warn"
                        { "内置" }
                    }
                }
            }
            script {
                ({
                    maud::PreEscaped(
                        r#"
(function(){
 var grid = document.currentScript.parentElement;
 grid.querySelectorAll('.pick-item').forEach(function(lbl){
 lbl.addEventListener('change', function(){
 var inp = lbl.querySelector('input');
 lbl.classList.toggle('selected', inp.checked);
 document.querySelector('#roleIdsInput').value = Array.from(document.querySelectorAll('input[name="role"]:checked')).map(function(c){return c.value}).join(',');
 document.querySelector('#deptIdsInput').value = Array.from(document.querySelectorAll('input[name="dept"]:checked')).map(function(c){return c.value}).join(',');
 });
 });
})();
"#,
                    )
                })
            }
        }
    }
}
}

fn dept_section(departments: &[Department], selected_ids: &[i64]) -> Markup {
 html! {
    div class=(SECTION) {
        div class=(SECTION_HEAD) { (icon::building_icon("w-[18px] h-[18px]")) "部门分配" }
        p class="text-[13px] text-muted mb-4 leading-relaxed" { "用户可归属多个部门（多对多关系）。" }
        div class="grid gap-2" {
            @for dept in departments {
                @let is_sel = selected_ids.contains(&dept.department_id);
                label
                    class="pick-item flex items-center gap-3 border rounded-md cursor-pointer transition-colors border-border-soft hover:border-border [&.selected]:border-accent [&.selected]:bg-accent-bg"
                {
                    input
                        type="checkbox"
                        name="dept"
                        value=(dept.department_id)
                        class="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                        checked[is_sel] {}
                    span
                        class=({
                            format!(
                                "w-7 h-7 rounded-lg flex items-center justify-center text-[9px] font-bold text-white shrink-0 bg-[{}] ",
                                dept_color(&dept.department_code),
                            )
                        })
                    { (short_code(&dept.department_code)) }
                    span class="text-sm font-medium text-fg" { (dept.department_name) }
                    @if !dept.is_active {
                        span
                            class="text-[10px] font-medium px-[6px] py-[1px] rounded-[3px] bg-danger-bg text-danger"
                        { "停用" }
                    }
                }
            }
            script {
                ({
                    maud::PreEscaped(
                        r#"
(function(){
 var grid = document.currentScript.parentElement;
 grid.querySelectorAll('.pick-item').forEach(function(lbl){
 lbl.addEventListener('change', function(){
 var inp = lbl.querySelector('input');
 lbl.classList.toggle('selected', inp.checked);
 document.querySelector('#roleIdsInput').value = Array.from(document.querySelectorAll('input[name="role"]:checked')).map(function(c){return c.value}).join(',');
 document.querySelector('#deptIdsInput').value = Array.from(document.querySelectorAll('input[name="dept"]:checked')).map(function(c){return c.value}).join(',');
 });
 });
})();
"#,
                    )
                })
            }
        }
    }
}
}

fn data_scope_section(user: &UserWithRoles, user_depts: &[Department]) -> Markup {
 let is_super_admin = user.user.is_super_admin;
 let has_departments = !user_depts.is_empty();

 html! {
    div class=(SECTION) {
        div class=(SECTION_HEAD) { (shield_check_icon("w-[18px] h-[18px]")) "数据权限 (DataScope)" }
        p class="text-[13px] text-muted mb-4 leading-relaxed" {
            "数据范围由角色配置决定，不支持在用户级别单独修改。以下为当前用户的实际数据范围。"
        }
        div class="grid grid-cols-3 gap-3" {
            // All scope
            ({
                scope_card(
                    is_super_admin,
                    "bg-accent-bg text-accent",
                    r#"<path d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064" /><circle cx="12" cy="12" r="10" />"#,
                    "All — 全部数据",
                    "可查看系统中所有数据，通常授予管理层",
                    None,
                )
            })
            // Department scope
            ({
                scope_card(
                    !is_super_admin && has_departments,
                    "bg-success-bg text-success",
                    r#"<path d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5" />"#,
                    "Department — 本部门",
                    "仅可查看所属部门的数据",
                    if !is_super_admin && has_departments {
                        Some(user_depts)
                    } else {
                        None
                    },
                )
            })
            // Self scope
            ({
                scope_card(
                    !is_super_admin && !has_departments,
                    "bg-surface text-muted",
                    r#"<path d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />"#,
                    "Self — 仅本人",
                    "仅可查看自己创建的数据",
                    None,
                )
            })
        }
    }
}
}

fn scope_card(selected: bool, icon_cls: &str, paths: &str, title: &str, desc: &str, dept_tags: Option<&[Department]>) -> Markup {
 let border_cls = if selected {
 "border-accent bg-accent-bg"
 } else {
 "border-border-soft"
 };
 html! {
    div class=(format!("p-4 border rounded-md {border_cls}")) {
        div class=({
                format!(
                    "w-10 h-10 rounded-md flex items-center justify-center mx-auto mb-2 {icon_cls}",
                )
            })
        {
            svg viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                class="w-5 h-5"
            { (maud::PreEscaped(paths)) }
        }
        div class="text-sm font-semibold text-fg text-center" { (title) }
        div class="text-xs text-muted text-center mt-1" { (desc) }
        @if let Some(depts) = dept_tags {
            div class="flex flex-wrap gap-1 justify-center mt-2" {
                @for dept in depts {
                    span
                        class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-surface text-slate-500"
                    { (&dept.department_name) }
                }
            }
        }
    }
}
}

// ── Helpers ──

/// Deterministic color from role code for badge background
fn role_color(code: &str) -> &'static str {
 match code {
 "SA" => "#7c3aed",
 "SM" => "#1677ff",
 "PM" => "#13c2c2",
 "WH" => "#fa8c16",
 "FM" => "#52c41a",
 "SP" => "#d46b08",
 "QC" => "#ff4d4f",
 _ => "#8c8c8c",
 }
}

/// Deterministic color from department code for badge background
fn dept_color(code: &str) -> &'static str {
 match code {
 "GO" => "#7c3aed",
 "SA" => "#1677ff",
 "PU" => "#13c2c2",
 "WH" => "#fa8c16",
 "FI" => "#52c41a",
 "QC" => "#ff4d4f",
 _ => "#8c8c8c",
 }
}

/// Take up to 2 uppercase characters from code for badge display
fn short_code(code: &str) -> String {
 code.chars().take(2).collect()
}

/// Inline SVG icon for shield check (not in icon module)
fn shield_check_icon(c: &str) -> Markup {
 html! {
    svg class=(c) viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
        path
            d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" {}
    }
}
}
