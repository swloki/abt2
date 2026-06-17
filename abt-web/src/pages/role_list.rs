use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::model::{Role, User, RESOURCE_ACTION_DEFS};
use abt_core::shared::identity::{RoleService, UserService};
use abt_core::shared::types::PgExecutor;

use abt_macros::require_permission;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::role::*;
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RoleQueryParams {
    pub keyword: Option<String>,
    pub role_type: Option<String>, // "system" | "custom"
}

// ── Handlers ──

#[require_permission("ROLE", "read")]
pub async fn get_role_list(
    _path: RoleListPath,
    ctx: RequestContext,
    Query(params): Query<RoleQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("ROLE", "create").await;
    let can_delete = ctx.has_permission("ROLE", "delete").await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let (roles, perm_counts, total_perms, user_map) =
        gather_role_data(&state, &service_ctx, &mut conn).await?;
    let filtered = filter_roles(&roles, &params);

    let content = role_list_page(
        &filtered,
        &params,
        &perm_counts,
        total_perms,
        &user_map,
        can_create,
        can_delete,
    );
    let page_html = admin_page(
        is_htmx,
        "角色管理",
        &claims,
        "system",
        RoleListPath::PATH,
        "系统管理",
        Some("角色管理"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROLE", "delete")]
pub async fn delete_role(
    path: RoleDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.role_service();

    svc.delete_role(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", RoleListPath::PATH)], Html(String::new())))
}

// ── Data Gathering ──

#[derive(Clone)]
struct UserBrief {
    initials: String,
    gradient: (String, String),
}

async fn gather_role_data(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    db: PgExecutor<'_>,
) -> crate::errors::Result<(Vec<Role>, HashMap<i64, usize>, usize, HashMap<i64, Vec<UserBrief>>)> {
    let role_svc = state.role_service();
    let user_svc = state.user_service();
    let cache = &state.permission_cache;

    // 1. All roles
    let roles = role_svc.list_roles(service_ctx, db).await?;

    // 2. Permission counts per role
    let total_perms = RESOURCE_ACTION_DEFS.len();
    let mut perm_counts = HashMap::with_capacity(roles.len());
    for role in &roles {
        let perms = cache.get_merged_permissions(&[role.role_id]).await;
        perm_counts.insert(role.role_id, perms.len());
    }

    // 3. Users grouped by role
    let users_with_roles = user_svc.list_users_with_roles(service_ctx, db).await?;
    let mut user_map: HashMap<i64, Vec<UserBrief>> = HashMap::new();
    for uwr in &users_with_roles {
        if !uwr.user.is_active {
            continue;
        }
        let brief = UserBrief {
            initials: user_initials(&uwr.user),
            gradient: avatar_gradient(
                uwr.user
                    .display_name
                    .as_deref()
                    .or(Some(&uwr.user.username))
                    .unwrap_or("?")
                    .chars()
                    .next()
                    .unwrap_or('?'),
            ),
        };
        for ri in &uwr.roles {
            user_map.entry(ri.role_id).or_default().push(brief.clone());
        }
    }

    Ok((roles, perm_counts, total_perms, user_map))
}

// ── Helpers ──

fn filter_roles<'a>(
    roles: &'a [Role],
    params: &RoleQueryParams,
) -> Vec<&'a Role> {
    let mut result: Vec<&'a Role> = roles.iter().collect();

    // Type filter
    match params.role_type.as_deref() {
        Some("system") => result.retain(|r| r.is_system_role),
        Some("custom") => result.retain(|r| !r.is_system_role),
        _ => {}
    }

    // Keyword filter
    if let Some(kw) = &params.keyword
        && !kw.is_empty() {
            let lower = kw.to_lowercase();
            result.retain(|r| {
                r.role_name.to_lowercase().contains(&lower)
                    || r.role_code.to_lowercase().contains(&lower)
                    || r.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&lower))
            });
        }

    result
}

fn user_initials(user: &User) -> String {
    let name = user
        .display_name
        .as_deref()
        .unwrap_or(&user.username);
    let chars: Vec<char> = name.chars().collect();
    match chars.len() {
        0 => String::from("?"),
        1 => chars[0].to_uppercase().to_string(),
        _ => format!(
            "{}{}",
            chars[0].to_uppercase(),
            chars[chars.len() - 1].to_uppercase()
        ),
    }
}

fn avatar_gradient(ch: char) -> (String, String) {
    match ch {
        'A'..='F' => ("#7c3aed".into(), "#a78bfa".into()),
        'G'..='L' => ("#1677ff".into(), "#4096ff".into()),
        'M'..='R' => ("#13c2c2".into(), "#36cfc9".into()),
        'S'..='Z' => ("#fa8c16".into(), "#ffc53d".into()),
        _ => ("#a18cd1".into(), "#fbc2eb".into()),
    }
}

/// Build a depth-sorted list from flat roles: top-level roles first, then their children recursively.
fn sort_roles_by_hierarchy<'a>(roles: &[&'a Role]) -> Vec<(&'a Role, usize)> {
    // Collect top-level roles (no parent or parent not in current list)
    let role_ids: std::collections::HashSet<i64> =
        roles.iter().map(|r| r.role_id).collect();

    let top_level: Vec<&'a Role> = roles
        .iter()
        .copied()
        .filter(|r| r.parent_role_id.is_none_or(|pid| !role_ids.contains(&pid)))
        .collect();

    let mut result = Vec::with_capacity(roles.len());
    for role in top_level {
        collect_hierarchy(role, roles, 0, &mut result);
    }
    result
}

fn collect_hierarchy<'a>(
    role: &'a Role,
    all: &[&'a Role],
    depth: usize,
    result: &mut Vec<(&'a Role, usize)>,
) {
    result.push((role, depth));
    for &child in all.iter().filter(|r| r.parent_role_id == Some(role.role_id)) {
        collect_hierarchy(child, all, depth + 1, result);
    }
}

// ── Components ──

fn role_list_page(
    roles: &[&Role],
    params: &RoleQueryParams,
    perm_counts: &HashMap<i64, usize>,
    total_perms: usize,
    user_map: &HashMap<i64, Vec<UserBrief>>,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    let system_count = roles.iter().filter(|r| r.is_system_role).count();
    let custom_count = roles.len() - system_count;
    let total = roles.len();

    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "角色管理" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(RoleCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建角色"
                        }
                    }
                }
            }

            // ── Stats ──
            div class="grid gap-5" {
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 purple" {
                        (icon::lock_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (total) }
                        div class="text-sm text-text-muted mt-1" { "角色总数" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 orange" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (system_count) }
                        div class="text-sm text-text-muted mt-1" { "内置角色" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 blue" {
                        (icon::plus_icon("w-6 h-6"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (custom_count) }
                        div class="text-sm text-text-muted mt-1" { "自定义角色" }
                    }
                }
            }

            // ── Filter + Data Table (HTMX panel) ──
            (role_table_fragment(roles, params, perm_counts, total_perms, user_map, can_create, can_delete))
        }
    }
}

fn role_table_fragment(
    roles: &[&Role],
    params: &RoleQueryParams,
    perm_counts: &HashMap<i64, usize>,
    total_perms: usize,
    user_map: &HashMap<i64, Vec<UserBrief>>,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div class="flex-1 overflow-y-auto-panel" {
            // ── Filter Bar ──
            div class="flex items-center gap-3 mb-5 flex-wrap" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索角色名称、角色代码…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(RoleListPath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-sync="this:replace"
                        hx-target="closest .role-list-panel"
                        hx-swap="outerHTML"
                        hx-include="select[name='role_type']";
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="role_type"
                    hx-get=(RoleListPath::PATH)
                    hx-trigger="change"
                    hx-target="closest .role-list-panel"
                    hx-swap="outerHTML"
                    hx-include="input[name='keyword']" {
                    option value="" selected[params.role_type.is_none() || params.role_type.as_deref() == Some("")] { "全部类型" }
                    option value="system" selected[params.role_type.as_deref() == Some("system")] { "内置角色" }
                    option value="custom" selected[params.role_type.as_deref() == Some("custom")] { "自定义角色" }
                }
            }

            // ── Data Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                        thead {
                            tr {
                                th { "角色名称" }
                                th { "角色代码" }
                                th { "类型" }
                                th { "权限数" }
                                th { "关联用户" }
                                th { "描述" }
                                th class="!text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for (role, depth) in &sort_roles_by_hierarchy(roles) {
                                (role_row(role, *depth, perm_counts, total_perms, user_map, can_create, can_delete))
                            }
                            @if roles.is_empty() {
                                tr {
                                    td colspan="7" class="text-center p-6 text-text-muted text-sm" {
                                        "暂无角色数据"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Pagination info ──
            div class="flex items-center justify-between py-4 px-5" {
                span class="flex items-center justify-between py-4-info" {
                    "共 " (roles.len()) " 条记录"
                }
            }
        }
    }
}

fn role_row(
    role: &Role,
    depth: usize,
    perm_counts: &HashMap<i64, usize>,
    total_perms: usize,
    user_map: &HashMap<i64, Vec<UserBrief>>,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    let edit_path = RoleEditPath { id: role.role_id }.to_string();
    let detail_path = RoleDetailPath { id: role.role_id }.to_string();
    let delete_path = RoleDeletePath { id: role.role_id };

    let perm_count = perm_counts.get(&role.role_id).copied().unwrap_or(0);
    let perm_pct = if total_perms > 0 {
        (perm_count as f64 / total_perms as f64 * 100.0).round() as u32
    } else {
        0
    };

    let users = user_map.get(&role.role_id);
    let user_count = users.map_or(0, |u| u.len());

    html! {
        tr {
            // Role Name (with hierarchy indentation)
            td {
                div style=(format!("padding-left:{}px", depth * 24)) {
                    @if depth > 0 {
                        span class="text-text-muted mr-1" { "└ " }
                    }
                    a href=(detail_path) class="text-fg no-underline" { strong { (role.role_name) } }
                }
            }
            // Role Code
            td class="font-mono tabular-nums text-xs" {
                (role.role_code)
            }
            // Type
            td {
                @if role.is_system_role {
                    span class="text-[11px] rounded-full bg-[#fff7e6] text-[#fa8c16] font-medium" { "内置" }
                } @else {
                    span class="text-[11px] rounded-full bg-[#f0f5ff] text-accent font-medium" { "自定义" }
                }
            }
            // Permission Count
            td {
                @if perm_count == total_perms {
                    span class="text-[13px] text-text-muted" { "全部权限" }
                } @else {
                    div class="text-accent font-semibold" {
                        span { (perm_count) }
                        div class="text-accent font-semibold-bar" {
                            div class="text-accent font-semibold-fill" style=(format!("width:{}%", perm_pct)) {}
                        }
                    }
                }
            }
            // Users
            td {
                div class="flex items-center" {
                    @if let Some(users) = users {
                        @for u in users.iter().take(3) {
                            span class="av" style=(format!("background:linear-gradient(135deg,{},{})", u.gradient.0, u.gradient.1)) {
                                (u.initials)
                            }
                        }
                        @if user_count > 3 {
                            span class="av more" {
                                "+" (user_count - 3)
                            }
                        }
                    }
                    @if user_count == 0 {
                        span class="av more" { "0" }
                    }
                }
            }
            // Description
            td class="text-[12px] text-text-muted overflow-hidden whitespace-nowrap" {
                @if let Some(desc) = &role.description {
                    (desc)
                } @else {
                    "—"
                }
            }
            // Actions
            td {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    @if can_create {
                        a class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer" title="编辑" href=(edit_path) {
                            (icon::edit_icon("w-3.5 h-3.5"))
                        }
                    }
                    @if can_delete && !role.is_system_role {
                        button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
                            hx-post=(delete_path)
                            hx-confirm=(format!("删除后无法恢复，确定要删除角色「{}」吗？", role.role_name)) {
                            (icon::trash_icon("w-3.5 h-3.5"))
                        }
                    }
                }
            }
        }
    }
}
