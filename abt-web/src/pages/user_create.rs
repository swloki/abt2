use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::shared::identity::{DepartmentService, RoleService, UserService};
use abt_core::shared::identity::model::*;
use abt_macros::require_permission;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::user::{UserCreatePath, UserListPath};
use crate::utils::RequestContext;

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UserCreateForm {
    pub username: String,
    pub display_name: Option<String>,
    pub password: String,
    pub confirm_password: String,
    pub is_super_admin: Option<String>,
    pub is_active: Option<String>,
    pub data_scope: Option<String>,
    pub role_ids: Option<String>,
    pub dept_ids: Option<String>,
}

// ── Handlers ──

#[require_permission("USER", "create")]
pub async fn get_user_create(
    _path: UserCreatePath,
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

    let role_svc = state.role_service();
    let dept_svc = state.department_service();

    let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;
    let all_depts = dept_svc
        .list_departments(&service_ctx, &mut conn)
        .await?;

    let content = user_create_page(&all_roles, &all_depts);
    let page_html = admin_page(
        is_htmx,
        "新建用户",
        &claims,
        "system",
        UserCreatePath::PATH,
        "系统管理",
        Some("新建用户"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "create")]
pub async fn post_user_create(
    _path: UserCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<UserCreateForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    // Validate password confirmation
    if form.password != form.confirm_password {
        return Err(abt_core::shared::types::DomainError::validation(
            "密码与确认密码不匹配".to_string(),
        ).into());
    }

    let user_svc = state.user_service();
    let dept_svc = state.department_service();

    let display_name = form.display_name.filter(|s| !s.is_empty());
    let is_super_admin = form.is_super_admin.is_some();
    let is_active = form.is_active.is_some();

    let user = user_svc
        .create_user(
            &service_ctx,
            &mut conn,
            &form.username,
            &form.password,
            display_name.as_deref(),
            is_super_admin,
        )
        .await?;

    // If unchecked "active", deactivate (insert defaults to true)
    if !is_active {
        user_svc
            .update_user_status(&service_ctx, &mut conn, user.user_id, false)
            .await?;
    }

    // Assign roles
    if let Some(role_ids_str) = &form.role_ids {
        let role_ids: Vec<i64> = role_ids_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();
        if !role_ids.is_empty() {
            user_svc
                .batch_assign_roles(&service_ctx, &mut conn, user.user_id, role_ids)
                .await?;
        }
    }

    // Assign departments
    if let Some(dept_ids_str) = &form.dept_ids {
        let dept_ids: Vec<i64> = dept_ids_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();
        if !dept_ids.is_empty() {
            dept_svc
                .assign_departments(&service_ctx, &mut conn, user.user_id, dept_ids)
                .await?;
        }
    }

    let redirect = UserListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn user_create_page(roles: &[Role], departments: &[Department]) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建用户" }
            }

            form id="user-form"
                  hx-post=(UserCreatePath::PATH)
                  hx-swap="none" {

                // Hidden fields for multi-select values
                input type="hidden" name="role_ids" id="roleIdsInput" {}
                input type="hidden" name="dept_ids" id="deptIdsInput" {}
                input type="hidden" name="data_scope" id="dataScopeInput" value="Department" {}

                // ── Section 1: 基本信息 ──
                (basic_info_section())

                // ── Section 2: 角色分配 ──
                (role_section(roles))

                // ── Section 3: 部门分配 ──
                (dept_section(departments))

                // ── Section 4: 数据权限 ──
                (data_scope_section())

                // ── Action Bar ──
                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", UserListPath::PATH)) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "保存"
                        }
                    }
                }
            }
        }
    }
}

fn basic_info_section() -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                (icon::user_icon("w-[18px] h-[18px]"))
                "基本信息"
            }
            div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                // 登录名
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "登录名 " span class="required" { "*" } }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="username" required placeholder="登录账号，如 zhangm" autocomplete="off" {}
                    span class="text-xs text-muted mt-0.5" { "唯一标识，创建后不可修改" }
                }
                // 显示名称
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "显示名称 " span class="required" { "*" } }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="display_name" placeholder="中文名称，如 张明" {}
                }
                // 密码
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "密码 " span class="required" { "*" } }
                    div class="relative" {
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="password" id="password" name="password" required placeholder="8-32位，含字母和数字" {}
                        button class="password-toggle" type="button" _="on click if previous <input/>'s type is 'password' set previous <input/>'s type to 'text' else set previous <input/>'s type to 'password'" {
                            (icon::eye_icon("w-4 h-4"))
                        }
                    }
                }
                // 确认密码
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "确认密码 " span class="required" { "*" } }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="password" id="confirmPwd" name="confirm_password" required placeholder="再次输入密码" {}
                }
                // 超级管理员
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "超级管理员" }
                    label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
                        input type="checkbox" name="is_super_admin" value="true" {}
                        span { "设为超级管理员（绕过所有权限检查）" }
                    }
                    span class="text-xs text-muted mt-0.5" { "超级管理员拥有所有资源的完全访问权限，请谨慎授予" }
                }
                // 激活状态
                div class="form-group" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "激活状态" }
                    label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
                        input type="checkbox" name="is_active" value="true" checked {}
                        span { "立即激活用户" }
                    }
                }
            }
        }
    }
}

fn role_section(roles: &[Role]) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                (icon::lock_icon("w-[18px] h-[18px]"))
                "角色分配"
            }
            p class="text-[13px] text-muted mb-4 leading-relaxed" { "用户可拥有多个角色，权限取所有角色的并集。" }
            div class="grid gap-3" {
                @for role in roles {
                    label class="flex items-center gap-2 p-2 border border-border rounded-sm cursor-pointer text-[13px]" {
                        input type="checkbox" name="role" value=(role.role_id) {}
                        span.pick-code style=(format!("background:{}", role_color(&role.role_code))) { (short_code(&role.role_code)) }
                        span { (role.role_name) }
                        @if role.is_system_role {
                            span.sys-badge { "内置" }
                        }
                    }
                }
                script { (maud::PreEscaped(r#"
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
"#)) }
            }
        }
    }
}

fn dept_section(departments: &[Department]) -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                (icon::building_icon("w-[18px] h-[18px]"))
                "部门分配"
            }
            p class="text-[13px] text-muted mb-4 leading-relaxed" { "用户可归属多个部门（多对多关系）。" }
            div class="grid gap-3" {
                @for dept in departments {
                    label class="flex items-center gap-2 p-2 border border-border rounded-sm cursor-pointer text-[13px]" {
                        input type="checkbox" name="dept" value=(dept.department_id) {}
                        span.pick-code style=(format!("background:{}", dept_color(&dept.department_code))) { (short_code(&dept.department_code)) }
                        span { (dept.department_name) }
                    }
                }
                script { (maud::PreEscaped(r#"
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
"#)) }
            }
        }
    }
}

fn data_scope_section() -> Markup {
    html! {
        div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                (shield_check_icon("w-[18px] h-[18px]"))
                "数据权限 (DataScope)"
            }
            p class="text-[13px] text-muted mb-4 leading-relaxed" { "控制用户在系统中可查看的数据范围。超级管理员默认为 All。" }
            div class="grid gap-3" {
                // All
                div class="p-4 border border-border rounded-sm cursor-pointer text-center" data-value="All" _="on click take .selected from .scope-option then put my @data-value into #dataScopeInput's value" {
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-icon si-all" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                            path d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064" {}
                            circle cx="12" cy="12" r="10" {}
                        }
                    }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-title" { "All — 全部数据" }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-desc" { "可查看系统中所有数据，通常授予管理层" }
                }
                // Department (default selected)
                div class="p-4 border border-border rounded-sm cursor-pointer text-center selected" data-value="Department" _="on click take .selected from .scope-option then put my @data-value into #dataScopeInput's value" {
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-icon si-dept" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                            path d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5" {}
                        }
                    }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-title" { "Department — 本部门" }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-desc" { "仅可查看所属部门的数据" }
                }
                // Self
                div class="p-4 border border-border rounded-sm cursor-pointer text-center" data-value="Self" _="on click take .selected from .scope-option then put my @data-value into #dataScopeInput's value" {
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-icon si-self" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                            path d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" {}
                        }
                    }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-title" { "Self — 仅本人" }
                    div class="p-4 border border-border rounded-sm cursor-pointer text-center-desc" { "仅可查看自己创建的数据" }
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
            path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" {}
        }
    }
}

