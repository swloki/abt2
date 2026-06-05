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
const PERM_SAVE_PATH: &str = "/admin/system/permissions/save";

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PermConfigParams {
    pub role_id: Option<i64>,
}

// ── Resource groupings (业务域分组) ──

struct ResourceGroupDef {
    name: &'static str,
    icon_cls: &'static str,
    resources: &'static [&'static str],
}

const RESOURCE_GROUPS: &[ResourceGroupDef] = &[
    ResourceGroupDef {
        name: "基础数据",
        icon_cls: "g1",
        resources: &["CUSTOMER", "PRODUCT", "CATEGORY", "BOM", "BOM_CATEGORY"],
    },
    ResourceGroupDef {
        name: "仓储库存",
        icon_cls: "g2",
        resources: &["WAREHOUSE", "LOCATION", "INVENTORY", "PRICE"],
    },
    ResourceGroupDef {
        name: "业务单据",
        icon_cls: "g3",
        resources: &["SALES_ORDER", "PURCHASE_ORDER"],
    },
    ResourceGroupDef {
        name: "生产质检",
        icon_cls: "g4",
        resources: &["WORK_ORDER", "INSPECTION", "COST", "LABOR_COST"],
    },
    ResourceGroupDef {
        name: "系统管理",
        icon_cls: "g5",
        resources: &["USER", "ROLE", "DEPARTMENT"],
    },
];

const ACTIONS: &[&str] = &["create", "read", "update", "delete"];
const ACTION_LABELS: &[&str] = &["创建", "查看", "编辑", "删除"];

struct GroupData {
    name: &'static str,
    icon_cls: &'static str,
    resources: Vec<GroupResource>,
}

struct GroupResource {
    resource_code: &'static str,
    resource_name: &'static str,
    defs: Vec<&'static ResourceActionDef>,
}

/// Build the display groups: for each business group, collect resources in declared order,
/// and for each resource, collect its 4 action defs (filtered from RESOURCE_ACTION_DEFS).
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
                icon_cls: group.icon_cls,
                resources,
            });
        }
    }
    out
}

fn total_perm_count(groups: &[GroupData]) -> usize {
    groups
        .iter()
        .map(|g| g.resources.iter().map(|r| r.defs.len()).sum::<usize>())
        .sum()
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

    let mut perms_by_role: HashMap<i64, HashSet<String>> = HashMap::new();
    for r in &roles {
        let rwp = svc
            .get_role_with_permissions(&service_ctx, &mut conn, r.role_id)
            .await?;
        perms_by_role.insert(r.role_id, rwp.permissions.into_iter().collect());
    }

    let selected_id = params
        .role_id
        .or_else(|| roles.first().map(|r| r.role_id))
        .filter(|id| roles.iter().any(|r| r.role_id == *id));

    let groups = build_groups();

    // HTMX partial: swap the two-panel layout
    if is_htmx && selected_id.is_some() {
        let content = html! {
            div.perm-layout {
                div.perm-role-list {
                    (role_list_panel(&roles, &perms_by_role, selected_id))
                }
                @if let Some(sid) = selected_id {
                    @if let Some(role) = roles.iter().find(|r| r.role_id == sid) {
                        @let perms = perms_by_role.get(&sid).cloned().unwrap_or_default();
                        (permission_panel(sid, role, &perms, &groups))
                    } @else {
                        (empty_right_panel())
                    }
                } @else {
                    (empty_right_panel())
                }
            }
        };
        return Ok(Html(content.into_string()));
    }

    let content = permission_config_page(&roles, &perms_by_role, selected_id, &groups);

    let page_html = admin_page(
        is_htmx,
        "权限配置",
        &claims,
        "system",
        PERM_CONFIG_PATH,
        "系统管理",
        Some("权限配置"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROLE", "update")]
pub async fn post_permission_save(
    ctx: RequestContext,
    Form(form): Form<HashMap<String, String>>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.role_service();

    let role_id: i64 = form
        .get("role_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            abt_core::shared::types::DomainError::validation("missing role_id in form".to_string())
        })?;

    let assigned: HashSet<String> = form
        .iter()
        .filter(|(k, _)| k.starts_with("perm_"))
        .map(|(k, _)| k[5..].to_string())
        .filter_map(|s| {
            let underscore = s.find('_')?;
            let (rid_str, rest) = s.split_at(underscore);
            let rid: i64 = rid_str.parse().ok()?;
            if rid != role_id {
                return None;
            }
            Some(rest[1..].to_string())
        })
        .collect();

    let rwp = svc
        .get_role_with_permissions(&service_ctx, &mut conn, role_id)
        .await?;
    let current: HashSet<String> = rwp.permissions.into_iter().collect();

    let to_add: Vec<(String, String)> = assigned
        .difference(&current)
        .filter_map(|p| parse_permission(p))
        .collect();
    let to_remove: Vec<(String, String)> = current
        .difference(&assigned)
        .filter_map(|p| parse_permission(p))
        .collect();

    if !to_add.is_empty() {
        svc.assign_permissions(&service_ctx, &mut conn, role_id, to_add)
            .await?;
    }
    if !to_remove.is_empty() {
        svc.remove_permissions(&service_ctx, &mut conn, role_id, to_remove)
            .await?;
    }

    let target = format!("{}?role_id={}", PERM_CONFIG_PATH, role_id);
    Ok(([("HX-Redirect", target)], Html(String::new())))
}

// ── Helpers ──

fn parse_permission(perm: &str) -> Option<(String, String)> {
    let (resource, action) = perm.split_once(':')?;
    Some((resource.to_string(), action.to_string()))
}

fn avatar_gradient(ch: char) -> (&'static str, &'static str) {
    match ch {
        'A'..='F' => ("#667eea", "#764ba2"),
        'G'..='L' => ("#f093fb", "#f5576c"),
        'M'..='R' => ("#4facfe", "#00f2fe"),
        'S'..='Z' => ("#43e97b", "#38f9d7"),
        _ => ("#a18cd1", "#fbc2eb"),
    }
}

// ── Components ──

fn permission_config_page(
    roles: &[Role],
    perms_by_role: &HashMap<i64, HashSet<String>>,
    selected_id: Option<i64>,
    groups: &[GroupData],
) -> Markup {
    html! {
        div.perm-config-page {
            // ── Page Header ──
            div.page-header {
                h1.page-title {
                    (icon::lock_icon("icon-sm"))
                    "权限配置"
                }
                div.page-actions {
                    a.btn.btn-default href="/admin/system/roles" {
                        (icon::return_arrow_icon("icon-sm"))
                        "返回角色列表"
                    }
                }
            }

            // ── Stats bar ──
            (stats_bar(roles, perms_by_role))

            // ── Two-panel layout ──
            div.perm-layout {
                div.perm-role-list {
                    (role_list_panel(roles, perms_by_role, selected_id))
                }
                @if let Some(sid) = selected_id {
                    @if let Some(role) = roles.iter().find(|r| r.role_id == sid) {
                        @let perms = perms_by_role.get(&sid).cloned().unwrap_or_default();
                        (permission_panel(sid, role, &perms, groups))
                    } @else {
                        (empty_right_panel())
                    }
                } @else {
                    (empty_right_panel())
                }
            }
        }

        script { (raw_perm_script()) }
    }
}

fn stats_bar(
    roles: &[Role],
    perms_by_role: &HashMap<i64, HashSet<String>>,
) -> Markup {
    let total_resources = count_unique_resources();
    let total_actions = RESOURCE_ACTION_DEFS.len();
    let editable_roles = roles.iter().filter(|r| !r.is_system_role).count();
    let total_roles = roles.len();
    let configured: usize = perms_by_role.values().map(|s| s.len()).sum();
    let total_possible = total_actions * total_roles;
    let coverage_pct = if total_possible == 0 {
        0.0
    } else {
        (configured as f64 / total_possible as f64) * 100.0
    };

    html! {
        div.data-card {
            div.perm-stat-grid {
                div {
                    div.perm-stat-label { "资源总数" }
                    div.perm-stat-value { (total_resources) }
                }
                div {
                    div.perm-stat-label { "角色总数 / 可编辑" }
                    div.perm-stat-value {
                        (total_roles)
                        " / " (editable_roles)
                    }
                }
                div {
                    div.perm-stat-label { "已配置权限" }
                    div.perm-stat-value { (configured) }
                }
                div {
                    div.perm-stat-label { "覆盖率" }
                    div.perm-stat-value { (format!("{:.1}%", coverage_pct)) }
                }
            }
            div.perm-coverage-bar {
                div.perm-coverage-fill style=(format!("width:{}%", coverage_pct.min(100.0))) {}
            }
        }
    }
}

fn role_list_panel(
    roles: &[Role],
    perms_by_role: &HashMap<i64, HashSet<String>>,
    selected_id: Option<i64>,
) -> Markup {
    html! {
        div.data-card.perm-role-list-card {
            div.perm-panel-head {
                div.perm-panel-title {
                    (icon::users_icon("icon-sm"))
                    "角色列表"
                }
                span.perm-group-count {
                    "(" (roles.len()) ")"
                }
            }
            div.data-card-scroll.perm-role-scroll {
                @for role in roles {
                    (role_item(role, perms_by_role, selected_id))
                }
                @if roles.is_empty() {
                    div.perm-empty-state {
                        "暂无可配置角色"
                    }
                }
            }
        }
    }
}

fn role_item(
    role: &Role,
    perms_by_role: &HashMap<i64, HashSet<String>>,
    selected_id: Option<i64>,
) -> Markup {
    let is_selected = selected_id == Some(role.role_id);
    let count = perms_by_role
        .get(&role.role_id)
        .map(|s| s.len())
        .unwrap_or(0);
    let first_char = role.role_name.chars().next().unwrap_or('?');
    let (from, to) = avatar_gradient(first_char);
    let target_url = format!("{}?role_id={}", PERM_CONFIG_PATH, role.role_id);

    let item_cls = if is_selected {
        "perm-role-item active"
    } else {
        "perm-role-item"
    };

    let badge_cls = if count > 0 {
        "perm-role-badge has-perms"
    } else {
        "perm-role-badge no-perms"
    };

    html! {
        div
            class=(item_cls)
            hx-get=(target_url)
            hx-target=".perm-layout"
            hx-swap="outerHTML" {

            div.perm-role-avatar
                style=(format!("background:linear-gradient(135deg,{from},{to})")) {
                (first_char.to_uppercase())
            }
            div.perm-role-info {
                div.perm-role-name {
                    (role.role_name)
                    @if role.is_system_role {
                        span.perm-role-sys-tag { "系统" }
                    }
                }
                div.perm-role-code { (role.role_code) }
            }
            div {
                span class=(badge_cls) { (count) }
            }
        }
    }
}

fn empty_right_panel() -> Markup {
    html! {
        div id="perm-right-panel" class="perm-right-panel" {
            div.data-card.perm-full-card {
                div.perm-empty-state {
                    div.perm-empty-icon {
                        (icon::lock_icon(""))
                    }
                    div.perm-empty-text { "请从左侧选择一个角色" }
                }
            }
        }
    }
}

/// Right-side panel: permission matrix for one role, wrapped in a form.
fn permission_panel(
    role_id: i64,
    role: &Role,
    perms: &HashSet<String>,
    groups: &[GroupData],
) -> Markup {
    let is_read_only = role.is_system_role;
    let save_action = PERM_SAVE_PATH.to_string();
    let form_id = format!("perm-form-{}", role_id);
    let total = total_perm_count(groups);

    html! {
        div id="perm-right-panel" class="perm-right-panel" {
            div.data-card.perm-full-card.perm-flex-col {
                div.perm-panel-head {
                    div.perm-panel-title {
                        (icon::sliders_icon("icon-sm"))
                        (role.role_name)
                        span.perm-subtitle {
                            "权限配置"
                        }
                    }
                    div {
                        @if is_read_only {
                            span.perm-notice {
                                (icon::lock_icon("icon-xs"))
                                " 系统角色不可修改"
                            }
                        } @else {
                            button.btn.btn-primary.btn-sm type="submit" form=(form_id) {
                                (icon::check_circle_icon("icon-sm"))
                                "保存权限"
                            }
                        }
                    }
                }

                div.data-card-scroll.perm-body-scroll {
                    p.perm-hint {
                        "为该角色分配资源操作权限。格式："
                        code { "RESOURCE:ACTION" }
                        "，角色权限取并集后存入 RolePermissionCache。"
                    }

                    form id=(form_id) action=(save_action) method="POST" {
                        input type="hidden" name="role_id" value=(role_id);

                        div.perm-toolbar {
                            div.perm-toolbar-left {
                                "已选择 "
                                span.perm-count id="permCount" { "0" }
                                " / "
                                span id="permTotal" { (total) }
                                " 项权限"
                            }
                            @if !is_read_only {
                                div.perm-actions {
                                    button.perm-action-btn type="button" data-action="select-all" { "全选" }
                                    button.perm-action-btn type="button" data-action="clear-all" { "清空" }
                                }
                            }
                        }

                        div.perm-groups id="permGroups" {
                            @for (gi, group) in groups.iter().enumerate() {
                                (perm_group(gi, group, perms, is_read_only, role_id))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn perm_group(
    gi: usize,
    group: &GroupData,
    perms: &HashSet<String>,
    is_read_only: bool,
    role_id: i64,
) -> Markup {
    let resource_count = group.resources.len();

    html! {
        div.perm-group data-group=(gi) {
            div.perm-group-head onclick={ "toggleGroup(" (gi) ")" } {
                div.perm-group-head-left {
                    span.perm-group-icon class=(format!("perm-group-icon {}", group.icon_cls)) {
                        (group_icon_svg(group.icon_cls))
                    }
                    span.perm-group-name { (group.name) }
                    span.perm-group-count { "(" (resource_count) ")" }
                }
                div.perm-group-actions {
                    @for (ai, action) in ACTIONS.iter().enumerate() {
                        span.perm-group-toggle {
                            label onclick="event.stopPropagation()" {
                                input type="checkbox"
                                      data-group-action=(action)
                                      data-group-idx=(gi)
                                      onchange={ "toggleGroupAction(" (gi) ",'" (action) "',this.checked)" }
                                      disabled[is_read_only] {}
                                " " (ACTION_LABELS[ai])
                            }
                        }
                    }
                    svg.perm-group-arrow.open width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {
                        path d="M6 9l6 6 6-6" {}
                    }
                }
            }
            div.perm-group-body id={ "groupBody" (gi) } {
                // Column headers
                div.perm-row.perm-row-header {
                    div { "资源" }
                    @for label in ACTION_LABELS {
                        div.perm-cell-header { (label) }
                    }
                }
                // Resource rows
                @for res in &group.resources {
                    (perm_resource_row(res, perms, is_read_only, role_id))
                }
            }
        }
    }
}

fn perm_resource_row(
    res: &GroupResource,
    perms: &HashSet<String>,
    is_read_only: bool,
    role_id: i64,
) -> Markup {
    let disabled_cls = if is_read_only { " disabled" } else { "" };

    html! {
        div class={ "perm-row" (disabled_cls) } {
            div.perm-resource {
                (res.resource_name)
                " "
                span.perm-code { (res.resource_code) }
            }
            @for action in ACTIONS {
                div.perm-cell {
                    @if let Some(_def) = res.defs.iter().find(|d| d.action == *action) {
                        @let key = format!("{}:{}", res.resource_code, action);
                        @let field_name = format!("perm_{}_{}:{}", role_id, res.resource_code, action);
                        @let is_checked = perms.contains(&key);
                        input type="checkbox"
                              name=(field_name)
                              value="on"
                              checked[is_checked]
                              disabled[is_read_only]
                              data-action=(action)
                              data-resource=(res.resource_code)
                              onchange="updateCount()" {}
                    }
                }
            }
        }
    }
}

fn group_icon_svg(cls: &str) -> Markup {
    match cls {
        "g1" => html! {
            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" {}
            }
        },
        "g2" => html! {
            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                path d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1" {}
            }
        },
        "g3" => html! {
            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" {}
            }
        },
        "g4" => html! {
            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                path d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" {}
            }
        },
        "g5" => html! {
            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" {
                path d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" {}
                circle cx="12" cy="12" r="3" {}
            }
        },
        _ => html! {},
    }
}

// ── Inline script for permission matrix interactivity ──

fn raw_perm_script() -> &'static str {
    r#"
function toggleGroup(g) {
    var body = me('#groupBody' + g);
    var group = body.parentElement;
    var arrow = group.querySelector('.perm-group-arrow');
    body.classList.toggle('collapsed');
    arrow.classList.toggle('open');
}

function toggleGroupAction(g, action, checked) {
    var body = me('#groupBody' + g);
    var cbs = body.querySelectorAll('input[data-action="' + action + '"]');
    for (var i = 0; i < cbs.length; i++) { cbs[i].checked = checked; }
    updateCount();
}

function setAll(checked) {
    var cbs = any('#permGroups input[data-resource]');
    for (var i = 0; i < cbs.length; i++) { if (!cbs[i].disabled) cbs[i].checked = checked; }
    var gToggles = any('input[data-group-action]');
    for (var i = 0; i < gToggles.length; i++) { if (!gToggles[i].disabled) gToggles[i].checked = checked; }
    updateCount();
}

function updateCount() {
    var total = any('#permGroups input[data-resource]').length;
    var checked = any('#permGroups input[data-resource]:checked').length;
    var el = me('#permCount');
    if (el) el.textContent = checked;
    var tot = me('#permTotal');
    if (tot) tot.textContent = total;
    var groupCount = any('.perm-group').length;
    var ACTIONS = ['create','read','update','delete'];
    for (var g = 0; g < groupCount; g++) {
        for (var a = 0; a < ACTIONS.length; a++) {
            var allCbs = any('#groupBody' + g + ' input[data-action="' + ACTIONS[a] + '"]');
            var allChecked = true;
            var anyChecked = false;
            for (var i = 0; i < allCbs.length; i++) {
                if (!allCbs[i].checked) allChecked = false;
                if (allCbs[i].checked) anyChecked = true;
            }
            var toggle = me('input[data-group-action="' + ACTIONS[a] + '"][data-group-idx="' + g + '"]');
            if (toggle) {
                toggle.checked = allChecked && allCbs.length > 0;
                toggle.indeterminate = anyChecked && !allChecked;
            }
        }
    }
}

document.addEventListener('DOMContentLoaded', function() {
    updateCount();
    var selectAllBtn = me('[data-action="select-all"]');
    var clearAllBtn = me('[data-action="clear-all"]');
    if (selectAllBtn) selectAllBtn.addEventListener('click', function() { setAll(true); });
    if (clearAllBtn) clearAllBtn.addEventListener('click', function() { setAll(false); });
});
"#
}
