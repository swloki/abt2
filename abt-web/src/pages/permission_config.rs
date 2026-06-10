use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::identity::model::{
    ResourceActionDef, Role, RESOURCE_ACTION_DEFS,
};
use abt_core::shared::identity::RoleService;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Paths ──

const PERM_CONFIG_PATH: &str = "/admin/system/permissions";
const PERM_TOGGLE_PATH: &str = "/admin/system/permissions/toggle";
const PERM_BATCH_PATH: &str = "/admin/system/permissions/toggle-batch";

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PermConfigParams {
    pub role_id: Option<i64>,
    pub scroll_top: Option<i64>,
}

// ── Toggle Form ──

#[derive(Debug, Deserialize)]
pub struct ToggleForm {
    pub role_id: i64,
    pub resource_code: String,
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchToggleForm {
    pub role_id: i64,
    pub mode: String, // "add" or "remove"
    pub group_idx: usize,
}

// ── Permission data per role (direct + inherited) ──

struct RolePermData {
    direct: HashSet<String>,
    inherited: HashSet<String>,
}

impl RolePermData {
    /// Total effective permissions (direct + inherited union).
    fn effective_count(&self) -> usize {
        self.direct.union(&self.inherited).count()
    }
}

// ── Resource groupings (业务域分组) ──

struct ResourceGroupDef {
    name: &'static str,
    resources: &'static [&'static str],
}

const RESOURCE_GROUPS: &[ResourceGroupDef] = &[
    ResourceGroupDef {
        name: "基础数据",
        resources: &["CUSTOMER", "PRODUCT", "CATEGORY", "BOM", "BOM_CATEGORY"],
    },
    ResourceGroupDef {
        name: "库存管理",
        resources: &["WAREHOUSE", "LOCATION", "INVENTORY", "PRICE"],
    },
    ResourceGroupDef {
        name: "销售管理",
        resources: &["SALES_ORDER", "SHIPPING"],
    },
    ResourceGroupDef {
        name: "采购管理",
        resources: &["PURCHASE_ORDER"],
    },
    ResourceGroupDef {
        name: "生产管理",
        resources: &["WORK_ORDER", "INSPECTION", "COST", "LABOR_COST"],
    },
    ResourceGroupDef {
        name: "系统工具",
        resources: &["EXCEL"],
    },
    ResourceGroupDef {
        name: "系统管理",
        resources: &["USER", "ROLE", "DEPARTMENT"],
    },
];

const ACTIONS: &[&str] = &["create", "read", "update", "delete"];
const ACTION_LABELS: &[&str] = &["创建", "查看", "编辑", "删除"];

struct GroupData {
    name: &'static str,
    resources: Vec<GroupResource>,
}

struct GroupResource {
    resource_code: &'static str,
    resource_name: &'static str,
    defs: Vec<&'static ResourceActionDef>,
}

fn build_groups() -> Vec<GroupData> {
    let mut out: Vec<GroupData> = Vec::new();
    for group in RESOURCE_GROUPS {
        let mut resources: Vec<GroupResource> = Vec::new();
        for &res in group.resources {
            let mut defs: Vec<&'static ResourceActionDef> = Vec::new();
            let mut res_name: &'static str = res;
            for def in RESOURCE_ACTION_DEFS.iter() {
                if def.resource_code == res {
                    if defs.is_empty() {
                        res_name = def.resource_name;
                    }
                    defs.push(def);
                }
            }
            if !defs.is_empty() {
                resources.push(GroupResource {
                    resource_code: res,
                    resource_name: res_name,
                    defs,
                });
            }
        }
        if !resources.is_empty() {
            out.push(GroupData {
                name: group.name,
                resources,
            });
        }
    }
    out
}

fn count_unique_resources() -> usize {
    let mut seen: HashSet<&str> = HashSet::new();
    for def in RESOURCE_ACTION_DEFS {
        seen.insert(def.resource_code);
    }
    seen.len()
}

// ── Handlers ──

#[require_permission("ROLE", "read")]
pub async fn get_permission_config(
    ctx: RequestContext,
    Query(params): Query<PermConfigParams>,
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
    let svc = state.role_service();

    let all_roles = svc.list_roles(&service_ctx, &mut conn).await?;
    let roles: Vec<Role> = all_roles
        .into_iter()
        .filter(|r| r.role_code != "super_admin")
        .collect();

    // Build role_id → role_name map for parent name lookup
    let role_name_map: HashMap<i64, String> = roles
        .iter()
        .map(|r| (r.role_id, r.role_name.clone()))
        .collect();

    let mut perm_data_by_role: HashMap<i64, RolePermData> = HashMap::new();
    for r in &roles {
        let rwp = svc
            .get_role_with_permissions(&service_ctx, &mut conn, r.role_id)
            .await?;
        perm_data_by_role.insert(r.role_id, RolePermData {
            direct: rwp.permissions.into_iter().collect(),
            inherited: rwp.inherited_permissions.into_iter().collect(),
        });
    }

    let selected_id = params
        .role_id
        .or_else(|| roles.first().map(|r| r.role_id))
        .filter(|id| roles.iter().any(|r| r.role_id == *id));

    let groups = build_groups();
    let scroll_top = params.scroll_top;

    // HTMX partial: swap the entire perm-page content
    if is_htmx && selected_id.is_some() {
        let content = perm_page_content(
            &roles, &perm_data_by_role, &role_name_map, selected_id, &groups, scroll_top,
        );
        return Ok(Html(content.into_string()));
    }

    let page_content = perm_page_content(
        &roles, &perm_data_by_role, &role_name_map, selected_id, &groups, scroll_top,
    );

    let page_html = admin_page(
        is_htmx,
        "权限配置",
        &claims,
        "system",
        PERM_CONFIG_PATH,
        "系统管理",
        Some("权限配置"),
        page_content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// Toggle a single permission: add if missing, remove if present.
/// Fires `permUpdated` event to trigger panel refresh.
#[require_permission("ROLE", "update")]
pub async fn post_permission_toggle(
    ctx: RequestContext,
    Form(form): Form<ToggleForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.role_service();

    // Safety check: system roles cannot be modified
    let all_roles = svc.list_roles(&service_ctx, &mut conn).await?;
    let role = all_roles
        .into_iter()
        .find(|r| r.role_id == form.role_id)
        .ok_or_else(|| {
            crate::errors::WebError::from(
                abt_core::shared::types::DomainError::not_found("角色不存在")
            )
        })?;
    if role.is_system_role {
        return Err(
            crate::errors::WebError::from(
                abt_core::shared::types::DomainError::validation("内置角色权限不可修改")
            )
        );
    }

    let key = format!("{}:{}", form.resource_code, form.action);
    let permission = (form.resource_code.clone(), form.action.clone());

    // Get current permissions and toggle
    let rwp = svc
        .get_role_with_permissions(&service_ctx, &mut conn, form.role_id)
        .await?;
    let current: HashSet<String> = rwp.permissions.into_iter().collect();

    if current.contains(&key) {
        svc.remove_permissions(&service_ctx, &mut conn, form.role_id, vec![permission])
            .await?;
    } else {
        svc.assign_permissions(&service_ctx, &mut conn, form.role_id, vec![permission])
            .await?;
    }

    // Return empty body with event trigger — panel refreshes via hx-select-oob
    Ok(([("HX-Trigger", "permUpdated")], Html(String::new())))
}

/// Batch toggle: assign or remove all permissions in a group.
#[require_permission("ROLE", "update")]
pub async fn post_permission_toggle_batch(
    ctx: RequestContext,
    Form(form): Form<BatchToggleForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.role_service();

    // Safety check: system roles cannot be modified
    let all_roles = svc.list_roles(&service_ctx, &mut conn).await?;
    let role = all_roles
        .into_iter()
        .find(|r| r.role_id == form.role_id)
        .ok_or_else(|| {
            crate::errors::WebError::from(
                abt_core::shared::types::DomainError::not_found("角色不存在")
            )
        })?;
    if role.is_system_role {
        return Err(
            crate::errors::WebError::from(
                abt_core::shared::types::DomainError::validation("内置角色权限不可修改")
            )
        );
    }

    // Build permission list from the group
    let permissions = get_group_permissions(form.group_idx);
    if permissions.is_empty() {
        return Ok(([("HX-Trigger", "permUpdated")], Html(String::new())));
    }

    match form.mode.as_str() {
        "add" => {
            svc.assign_permissions(&service_ctx, &mut conn, form.role_id, permissions)
                .await?;
        }
        "remove" => {
            svc.remove_permissions(&service_ctx, &mut conn, form.role_id, permissions)
                .await?;
        }
        _ => {
            return Err(
                crate::errors::WebError::from(
                    abt_core::shared::types::DomainError::validation("无效的批量操作模式")
                )
            );
        }
    }

    Ok(([("HX-Trigger", "permUpdated")], Html(String::new())))
}

/// Collect all (resource_code, action) pairs for a group by index.
fn get_group_permissions(group_idx: usize) -> Vec<(String, String)> {
    let Some(group) = RESOURCE_GROUPS.get(group_idx) else {
        return Vec::new();
    };
    let mut perms = Vec::new();
    for &res in group.resources {
        for def in RESOURCE_ACTION_DEFS.iter() {
            if def.resource_code == res {
                perms.push((res.to_string(), def.action.to_string()));
            }
        }
    }
    perms
}

// ── Helpers ──

fn avatar_gradient(name: &str) -> &'static str {
    let gradients = [
        "linear-gradient(135deg,#a78bfa,#7c3aed)",
        "linear-gradient(135deg,#22d3ee,#0e7490)",
        "linear-gradient(135deg,#34d399,#047857)",
        "linear-gradient(135deg,#fbbf24,#b45309)",
        "linear-gradient(135deg,#fb7185,#be123c)",
        "linear-gradient(135deg,#38bdf8,#0369a1)",
        "linear-gradient(135deg,#e879f9,#9333ea)",
        "linear-gradient(135deg,#a3e635,#047857)",
    ];
    let mut h: usize = 0;
    for b in name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as usize);
    }
    gradients[h % gradients.len()]
}

// ── Page Components ──

fn perm_page_content(
    roles: &[Role],
    perm_data_by_role: &HashMap<i64, RolePermData>,
    role_name_map: &HashMap<i64, String>,
    selected_id: Option<i64>,
    groups: &[GroupData],
    scroll_top: Option<i64>,
) -> Markup {
    let total_resources = count_unique_resources();
    let total_actions = RESOURCE_ACTION_DEFS.len();
    let total_roles = roles.len();
    // "已配置" counts effective permissions (direct + inherited) across all roles
    let configured: usize = perm_data_by_role.values().map(|d| d.effective_count()).sum();
    let total_possible = total_actions * total_roles;
    let coverage_pct = if total_possible == 0 {
        0.0
    } else {
        (configured as f64 / total_possible as f64) * 100.0
    };

    html! {
        div class="perm-page" {
            // ── Stats Header ──
            div class="stats-panel" {
                div class="stats-main" {
                    div class="stats-main-left" {
                        div class="stats-icon" {
                            (icon::lock_icon("w-5 h-5"))
                        }
                        div {
                            div class="stats-title" { "权限配置" }
                            div class="stats-desc" { "管理角色与资源的访问权限" }
                        }
                    }
                }
                div class="stats-bar" id="stats-bar" {
                    div class="stat-item" {
                        span class="stat-label" { "资源" }
                        span class="stat-value" { (total_resources) }
                    }
                    div class="stat-item" {
                        span class="stat-label" { "角色" }
                        span class="stat-value" { (total_roles) }
                    }
                    div class="stat-item" {
                        span class="stat-label" { "已配置" }
                        span class="stat-value" { (configured) }
                    }
                    div class="stat-item" {
                        span class="stat-label" { "配置率" }
                        span class="stat-value accent" { (format!("{:.0}%", coverage_pct)) }
                        div class="stat-progress" {
                            div class="stat-progress-bar" style=(format!("width:{}%", coverage_pct.min(100.0))) {}
                        }
                    }
                }
            }

            // ── Main Split ──
            div class="perm-layout" {
                // ── Left: Role List ──
                div class="role-panel" {
                    div class="role-panel-head" {
                        div class="role-panel-title" {
                            (icon::users_icon("w-3.5 h-3.5"))
                            "角色列表"
                            span class="count" { (roles.len()) }
                        }
                    }
                    div class="role-list" {
                        @for role in roles {
                            (role_item(role, perm_data_by_role, selected_id))
                        }
                        @if roles.is_empty() {
                            div class="perm-empty" style="padding:20px" {
                                p { "暂无可配置角色" }
                            }
                        }
                    }
                }

                // ── Right: Permission Config ──
                @if let Some(sid) = selected_id {
                    @if let Some(role) = roles.iter().find(|r| r.role_id == sid) {
                        @let pd = perm_data_by_role.get(&sid).unwrap();
                        @let parent_name = role.parent_role_id.and_then(|pid| role_name_map.get(&pid)).map(|s| s.as_str());
                        (permission_panel(sid, role, pd, parent_name, groups, scroll_top))
                    } @else {
                        (empty_right_panel())
                    }
                } @else {
                    (empty_right_panel())
                }
            }
        }
    }
}

fn role_item(
    role: &Role,
    perm_data_by_role: &HashMap<i64, RolePermData>,
    selected_id: Option<i64>,
) -> Markup {
    let is_selected = selected_id == Some(role.role_id);
    let count = perm_data_by_role
        .get(&role.role_id)
        .map(|d| d.effective_count())
        .unwrap_or(0);
    let first_char = role.role_name.chars().next().unwrap_or('?');
    let gradient = avatar_gradient(&role.role_name);
    let target_url = format!("{}?role_id={}", PERM_CONFIG_PATH, role.role_id);

    let item_cls = if is_selected {
        "role-item active"
    } else {
        "role-item"
    };

    html! {
        button
            type="button"
            class=(item_cls)
            hx-get=(target_url)
            hx-target=".perm-page"
            hx-swap="outerHTML" {

            div class="role-avatar" style=(format!("background:{}", gradient)) {
                (first_char.to_uppercase())
            }
            div class="role-item-info" {
                span class="role-item-name" { (role.role_name) }
                span class="role-item-meta" { "已授权 " (count) " 项" }
            }
        }
    }
}

fn empty_right_panel() -> Markup {
    html! {
        div class="perm-panel" {
            div class="perm-empty" {
                div class="perm-empty-icon" {
                    (icon::users_icon("w-7 h-7"))
                }
                h4 { "选择一个角色" }
                p { "在左侧角色列表中选择一个角色，查看并配置其权限" }
            }
        }
    }
}

fn permission_panel(
    role_id: i64,
    role: &Role,
    pd: &RolePermData,
    parent_name: Option<&str>,
    groups: &[GroupData],
    scroll_top: Option<i64>,
) -> Markup {
    let is_read_only = role.is_system_role;
    let first_char = role.role_name.chars().next().unwrap_or('?');
    let gradient = avatar_gradient(&role.role_name);
    let effective_count = pd.effective_count();
    let inherited_count = pd.inherited.len();
    let refresh_url = format!("{}?role_id={}", PERM_CONFIG_PATH, role_id);
    let hx_vals = "js:{scroll_top: Math.round(document.querySelector('.perm-body')?.scrollTop || 0)}".to_string();

    html! {
        div class="perm-panel"
            hx-trigger="permUpdated from:body"
            hx-get=(refresh_url)
            hx-select=".perm-panel"
            hx-swap="outerHTML"
            hx-select-oob="#stats-bar"
            hx-vals=(hx_vals) {

            // ── Role header ──
            div class="perm-role-head" {
                div class="perm-role-head-inner" {
                    div class="perm-role-head-left" {
                        div class="perm-role-avatar-lg" style=(format!("background:{}", gradient)) {
                            (first_char.to_uppercase())
                        }
                        div {
                            div class="perm-role-name-lg" { (role.role_name) }
                            div class="perm-role-sub" {
                                @if inherited_count > 0 {
                                    "已授权 "
                                    span class="hl" { (effective_count) }
                                    " 项（含 "
                                    span class="hl" { (inherited_count) }
                                    " 项继承）"
                                } @else {
                                    "已授权 "
                                    span class="hl" { (effective_count) }
                                    " 项权限"
                                }
                            }
                        }
                    }
                    div class="perm-legend" {
                        span class="perm-legend-item" {
                            span class="perm-legend-dot on" {}
                            "已授权"
                        }
                        span class="perm-legend-item" {
                            span class="perm-legend-dot inherited" {}
                            "继承"
                        }
                        span class="perm-legend-item" {
                            span class="perm-legend-dot off" {}
                            "未授权"
                        }
                    }
                }
            }

            // ── System role read-only hint ──
            @if is_read_only {
                div class="perm-readonly-hint" {
                    (icon::info_icon("w-4 h-4"))
                    span { "内置角色的权限由系统预设，不可修改。如需自定义权限，请新建角色。" }
                }
            }

            // ── Inherited permission hint ──
            @if inherited_count > 0 && !is_read_only {
                div class="perm-inherit-hint" {
                    (icon::info_icon("w-4 h-4"))
                    span {
                        "以下灰色标记的权限继承自上级角色「"
                        @if let Some(pn) = parent_name {
                            (pn)
                        }
                        "」，不可在当前角色中修改。"
                    }
                }
            }

            // ── Permission body ──
            div class="perm-body" {
                div class="perm-groups" {
                    @for (gi, group) in groups.iter().enumerate() {
                        (perm_group(gi, group, pd, is_read_only, role_id))
                    }
                }
                // Restore scroll position immediately during swap (no flicker)
                @if let Some(st) = scroll_top {
                    (maud::PreEscaped(format!("<script>document.querySelector('.perm-body').scrollTop={st};</script>")))
                }
            }
        }
    }
}

fn perm_group(
    gi: usize,
    group: &GroupData,
    pd: &RolePermData,
    is_read_only: bool,
    role_id: i64,
) -> Markup {
    let resource_count = group.resources.len();

    html! {
        div class="perm-group" {
            div class="perm-group-head" {
                div class="perm-group-head-left" {
                    svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" {
                        path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z" {}
                        polyline points="13 2 13 9 20 9" {}
                    }
                    span class="perm-group-name" { (group.name) }
                    span class="perm-group-badge" { (resource_count) }
                }
                @if !is_read_only {
                    @let add_vals = format!("{{\"role_id\":\"{}\",\"mode\":\"add\",\"group_idx\":\"{}\"}}", role_id, gi);
                    @let rm_vals = format!("{{\"role_id\":\"{}\",\"mode\":\"remove\",\"group_idx\":\"{}\"}}", role_id, gi);
                    div class="perm-group-ops" {
                        button type="button" class="perm-group-select-all"
                            hx-post=(PERM_BATCH_PATH) hx-vals=(add_vals) hx-swap="none" { "全选" }
                        span class="sep" { "|" }
                        button type="button" class="danger perm-group-clear-all"
                            hx-post=(PERM_BATCH_PATH) hx-vals=(rm_vals) hx-swap="none" { "清空" }
                    }
                }
            }
            div class="perm-res-list" {
                @for res in &group.resources {
                    (perm_resource_row(res, pd, is_read_only, role_id))
                }
            }
        }
    }
}

fn perm_resource_row(
    res: &GroupResource,
    pd: &RolePermData,
    is_read_only: bool,
    role_id: i64,
) -> Markup {
    // Count effective (direct + inherited) for the display counter
    let mut effective_count = 0;
    for action in ACTIONS {
        let key = format!("{}:{}", res.resource_code, action);
        if pd.direct.contains(&key) || pd.inherited.contains(&key) {
            effective_count += 1;
        }
    }
    let total = res.defs.len();
    let count_cls = if effective_count == total && total > 0 {
        "perm-res-count done"
    } else {
        "perm-res-count"
    };

    html! {
        div class="perm-res-row" {
            span class="perm-res-name" title=(res.resource_name) { (res.resource_name) }
            div class="perm-res-btns" {
                @for (ai, action) in ACTIONS.iter().enumerate() {
                    @if let Some(_def) = res.defs.iter().find(|d| d.action == *action) {
                        @let key = format!("{}:{}", res.resource_code, action);
                        @let is_inherited = pd.inherited.contains(&key);
                        @let is_direct = pd.direct.contains(&key);
                        @let (btn_cls, disabled) = if is_inherited {
                            ("perm-btn inherited", true)
                        } else if is_direct {
                            ("perm-btn checked", false)
                        } else {
                            ("perm-btn unchecked", false)
                        };
                        @let hx_vals = format!(
                            "{{\"role_id\":\"{}\",\"resource_code\":\"{}\",\"action\":\"{}\"}}",
                            role_id, res.resource_code, action
                        );
                        button
                            type="button"
                            class=(btn_cls)
                            hx-post=(PERM_TOGGLE_PATH)
                            hx-vals=(hx_vals)
                            hx-swap="none"
                            data-role-id=(role_id)
                            data-resource=(res.resource_code)
                            data-action=(action)
                            disabled[is_read_only || disabled] {
                            (ACTION_LABELS[ai])
                        }
                    }
                }
            }
            span class=(count_cls) {
                (format!("{}/{}", effective_count, total))
            }
        }
    }
}
