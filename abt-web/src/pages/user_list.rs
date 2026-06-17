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
use crate::routes::user::*;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct UserQueryParams {
    pub keyword: Option<String>,
    pub role_id: Option<i64>,
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
        content, &nav_filter,    );

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

    Ok(([("HX-Redirect", UserListPath::PATH)], Html(String::new())))
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
        0 => "avatar-c0",
        1 => "avatar-c1",
        2 => "avatar-c2",
        3 => "avatar-c3",
        4 => "avatar-c4",
        5 => "avatar-c5",
        6 => "avatar-c6",
        _ => "avatar-c7",
    }
}

fn initials(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() >= 2 {
        format!("{}{}", chars[0], chars[1])
    } else if chars.len() == 1 {
        chars[0].to_string()
    } else {
        "?".to_string()
    }
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
                h1 class="text-xl font-bold text-fg tracking-tight" { "用户管理" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(UserCreatePath::PATH) {
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

    let status_query = |s: &str| -> String {
        let mut qs = format!("{}?page=1", UserListPath::PATH);
        if !s.is_empty() {
            qs.push_str("&status=");
            qs.push_str(s);
        }
        qs
    };

    let tab_all = if status_filter.is_empty() { "status-tab active" } else { "status-tab" };
    let tab_active = if status_filter == "active" { "status-tab active" } else { "status-tab" };
    let tab_inactive = if status_filter == "inactive" { "status-tab active" } else { "status-tab" };

    html! {
        div class="user-list-panel" {
            // ── Stats ──
            div class="grid gap-5" {
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" {
                        (icon::users_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (total_count) }
                        div class="text-sm text-text-muted mt-1" { "用户总数" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (active_count) }
                        div class="text-sm text-text-muted mt-1" { "已激活" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" {
                        (icon::clock_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (inactive_count) }
                        div class="text-sm text-text-muted mt-1" { "已停用" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="stat-icon w-[44px] h-[44px] rounded grid place-items-center shrink-0-purple" {
                        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="w-6 h-6" {
                            path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" {}
                        }
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (super_admin_count) }
                        div class="text-sm text-text-muted mt-1" { "超级管理员" }
                    }
                }
            }

            // ── Status Tabs ──
            div class="flex gap-1 border-b" {
                button class=(tab_all)
                    hx-get=(status_query(""))
                    hx-target="closest .user-list-panel"
                    hx-swap="outerHTML"
                    hx-push-url=(UserListPath::PATH) {
                    "全部"
                    span class="count" { (total_count) }
                }
                button class=(tab_active)
                    hx-get=(status_query("active"))
                    hx-target="closest .user-list-panel"
                    hx-swap="outerHTML"
                    hx-push-url=(UserListPath::PATH) {
                    "已激活"
                    span class="count" { (active_count) }
                }
                button class=(tab_inactive)
                    hx-get=(status_query("inactive"))
                    hx-target="closest .user-list-panel"
                    hx-swap="outerHTML"
                    hx-push-url=(UserListPath::PATH) {
                    "已停用"
                    span class="count" { (inactive_count) }
                }
            }

            // ── Filter Bar ──
            div class="flex items-center gap-3 mb-5 flex-wrap" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索用户名、显示名称…"
                        value=(keyword)
                        hx-get=(UserListPath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-sync="this:replace"
                        hx-target="closest .user-list-panel"
                        hx-swap="outerHTML"
                        hx-push-url=(UserListPath::PATH)
                        hx-include="select[name=role_filter],select[name=dept_filter]";
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="role_filter"
                    hx-get=(UserListPath::PATH)
                    hx-trigger="change"
                    hx-target="closest .user-list-panel"
                    hx-swap="outerHTML"
                    hx-push-url=(UserListPath::PATH)
                    hx-include="input[name=keyword],select[name=dept_filter]" {
                    option value="" { "全部角色" }
                    @for role in all_roles {
                        @let selected = role_filter == Some(role.role_id);
                        option value=(role.role_id) selected[selected] {
                            (role.role_name)
                        }
                    }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="dept_filter"
                    hx-get=(UserListPath::PATH)
                    hx-trigger="change"
                    hx-target="closest .user-list-panel"
                    hx-swap="outerHTML"
                    hx-push-url=(UserListPath::PATH)
                    hx-include="input[name=keyword],select[name=role_filter]" {
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
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                        thead {
                            tr {
                                th { "用户信息" }
                                th { "登录名" }
                                th { "角色" }
                                th { "部门" }
                                th { "数据权限" }
                                th { "状态" }
                                th { "创建时间" }
                                th class="text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for u in &page_users {
                                (user_row(u, user_depts, can_delete))
                            }
                            @if page_users.is_empty() {
                                tr {
                                    td colspan="8" class="text-center p-6 text-text-muted text-sm" {
                                        "暂无用户数据"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Pagination ──
            (pagination::htmx_pagination_inherited(
                UserListPath::PATH,
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
        ("已激活", "status-dot active")
    } else {
        ("已停用", "status-dot inactive")
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
                    div class={"avatar-sm " (avatar_cls)} {
                        (avatar_initials)
                    }
                    div class="flex items-center gap-3-info" {
                        span class="flex items-center gap-3-name" {
                            (display_name)
                            @if u.user.is_super_admin {
                                span class="bg-[#f3e8ff] text-[#7c3aed]" { "超管" }
                            }
                        }
                        span class="flex items-center gap-3-id" {
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
                        span class="text-[10px] font-medium" { (role.role_name) }
                    }
                    @if u.roles.is_empty() {
                        span class="text-text-muted" { "—" }
                    }
                }
            }

            // Departments
            td {
                @if depts.is_empty() {
                    span class="text-text-muted" { "—" }
                } @else {
                    @for dept in depts {
                        span class="text-[10px] bg-[#f0fff0] text-[#389e0d] font-medium" { (dept.department_name) }
                    }
                }
            }

            // Data scope
            td {
                span class="text-[12px] text-text-muted" { (scope) }
            }

            // Status
            td {
                span class="flex items-center text-[13px]" {
                    span class=(dot_class) {}
                    (status_label)
                }
            }

            // Created at
            td class="font-mono tabular-nums" {
                (u.user.created_at.format("%Y-%m-%d"))
            }

            // Actions
            td onclick="event.stopPropagation()" {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑"
                        href=(edit_path.to_string()) {
                        (icon::edit_icon("w-3.5 h-3.5"))
                    }
                    @if can_delete && !u.user.is_super_admin {
                        button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title=(toggle_title)
                            hx-post=(toggle_path.to_string())
                            hx-confirm=(format!(
                                "确定要{}用户 <strong>{}</strong> 吗？",
                                if u.user.is_active { "停用" } else { "启用" },
                                display_name,
                            ))
                            hx-target="closest .user-list-panel"
                            hx-swap="outerHTML" {
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
