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
    );
    let page_html = admin_page(
        is_htmx,
        "角色管理",
        &claims,
        "system",
        RoleListPath::PATH,
        "系统管理",
        Some("角色管理"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROLE", "read")]
pub async fn get_role_table(
    ctx: RequestContext,
    Query(params): Query<RoleQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let (roles, perm_counts, total_perms, user_map) =
        gather_role_data(&state, &service_ctx, &mut conn).await?;
    let filtered = filter_roles(&roles, &params);

    Ok(Html(
        role_table_fragment(&filtered, &params, &perm_counts, total_perms, &user_map)
            .into_string(),
    ))
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
    if let Some(kw) = &params.keyword {
        if !kw.is_empty() {
            let lower = kw.to_lowercase();
            result.retain(|r| {
                r.role_name.to_lowercase().contains(&lower)
                    || r.role_code.to_lowercase().contains(&lower)
                    || r.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&lower))
            });
        }
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
) -> Markup {
    let system_count = roles.iter().filter(|r| r.is_system_role).count();
    let custom_count = roles.len() - system_count;
    let total = roles.len();

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "角色管理" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(RoleCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建角色"
                    }
                }
            }

            // ── Stats ──
            div class="role-stats" {
                div class="stat-card" {
                    div class="stat-icon purple" {
                        (icon::lock_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (total) }
                        div class="stat-label" { "角色总数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (system_count) }
                        div class="stat-label" { "内置角色" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon blue" {
                        (icon::plus_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (custom_count) }
                        div class="stat-label" { "自定义角色" }
                    }
                }
            }

            // ── Filter + Data Table (HTMX panel) ──
            (role_table_fragment(roles, params, perm_counts, total_perms, user_map))
        }
    }
}

fn role_table_fragment(
    roles: &[&Role],
    params: &RoleQueryParams,
    perm_counts: &HashMap<i64, usize>,
    total_perms: usize,
    user_map: &HashMap<i64, Vec<UserBrief>>,
) -> Markup {
    html! {
        div class="role-list-panel" {
            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索角色名称、角色代码…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(RoleTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .role-list-panel"
                        hx-swap="outerHTML"
                        hx-include="select[name='role_type']";
                }
                select class="filter-select" name="role_type"
                    hx-get=(RoleTablePath::PATH)
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
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "角色名称" }
                                th { "角色代码" }
                                th { "类型" }
                                th { "权限数" }
                                th { "关联用户" }
                                th { "描述" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for (role, depth) in &sort_roles_by_hierarchy(roles) {
                                (role_row(role, *depth, perm_counts, total_perms, user_map))
                            }
                            @if roles.is_empty() {
                                tr {
                                    td colspan="7" class="empty-state" {
                                        "暂无角色数据"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Pagination info ──
            div class="pagination" {
                span class="pagination-info" {
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
                        span class="text-muted mr-1" { "└ " }
                    }
                    a href=(detail_path) class="role-name-link" { strong { (role.role_name) } }
                }
            }
            // Role Code
            td class="mono text-xs" {
                (role.role_code)
            }
            // Type
            td {
                @if role.is_system_role {
                    span class="tag-sys" { "内置" }
                } @else {
                    span class="tag-custom" { "自定义" }
                }
            }
            // Permission Count
            td {
                @if perm_count == total_perms {
                    span class="perm-all" { "全部权限" }
                } @else {
                    div class="perm-count" {
                        span { (perm_count) }
                        div class="perm-count-bar" {
                            div class="perm-count-fill" style=(format!("width:{}%", perm_pct)) {}
                        }
                    }
                }
            }
            // Users
            td {
                div class="user-avatars" {
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
            td class="role-desc" {
                @if let Some(desc) = &role.description {
                    (desc)
                } @else {
                    "—"
                }
            }
            // Actions
            td {
                div class="row-actions" {
                    a class="row-action-btn" title="编辑" href=(edit_path) {
                        (icon::edit_icon("w-3.5 h-3.5"))
                    }
                    @if !role.is_system_role {
                        button type="button" class="row-action-btn text-danger" title="删除"
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
