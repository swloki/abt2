use std::collections::HashMap;

use axum::Form;
use axum::response::{Html, IntoResponse};
use maud::{html, Markup};

use abt_core::shared::identity::RoleService;
use abt_core::shared::identity::UserService;
use abt_core::shared::identity::model::{RESOURCE_ACTION_DEFS, ResourceActionDef};

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::role::{RoleDetailPath, RoleEditPath, RoleListPath, RolePermissionPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Resource Groupings (same layout as role_create / role_edit) ──

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
        name: "库存管理",
        icon_cls: "g2",
        resources: &["WAREHOUSE", "LOCATION", "INVENTORY", "PRICE"],
    },
    ResourceGroupDef {
        name: "销售管理",
        icon_cls: "g3",
        resources: &["SALES_ORDER", "SHIPPING"],
    },
    ResourceGroupDef {
        name: "采购管理",
        icon_cls: "g3",
        resources: &["PURCHASE_ORDER"],
    },
    ResourceGroupDef {
        name: "生产管理",
        icon_cls: "g4",
        resources: &["WORK_ORDER", "INSPECTION", "COST", "LABOR_COST"],
    },
    ResourceGroupDef {
        name: "系统管理",
        icon_cls: "g5",
        resources: &["USER", "ROLE", "DEPARTMENT"],
    },
];

struct GroupResource {
    code: &'static str,
    name: &'static str,
    defs: Vec<&'static ResourceActionDef>,
}

struct GroupData {
    name: &'static str,
    icon_cls: &'static str,
    resources: Vec<GroupResource>,
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
                    code: res,
                    name: res_name,
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
    groups.iter()
        .map(|g| g.resources.iter().map(|r| r.defs.len()).sum::<usize>())
        .sum()
}

// ── Handlers ──

#[require_permission("ROLE", "read")]
pub async fn get_role_detail(
    path: RoleDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let role_svc = state.role_service();
    let user_svc = state.user_service();

    let rwp = role_svc.get_role_with_permissions(&service_ctx, &mut conn, path.id).await?;

    // Resolve parent role name
    let parent_role_name = if rwp.role.parent_role_id.is_some() {
        let all_roles = role_svc.list_roles(&service_ctx, &mut conn).await?;
        rwp.role.parent_role_id
            .and_then(|pid| all_roles.iter().find(|r| r.role_id == pid))
            .map(|r| r.role_name.clone())
    } else {
        None
    };

    // Count users assigned to this role
    let all_users = user_svc.list_users_with_roles(&service_ctx, &mut conn).await?;
    let user_count = all_users.iter()
        .filter(|u| u.roles.iter().any(|r| r.role_id == path.id))
        .count();

    // Build grouped permission data
    let groups = build_groups();
    let total_perms = total_perm_count(&groups);

    let content = role_detail_page(
        &rwp,
        parent_role_name.as_deref(),
        user_count,
        &groups,
        total_perms,
    );

    let detail_path_str = RoleDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 角色详情", rwp.role.role_name),
        &claims,
        "system",
        &detail_path_str,
        "系统管理",
        Some(&rwp.role.role_name),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// Keep handler for backward compatibility — permission editing now lives in role_edit.
#[require_permission("ROLE", "update")]
pub async fn post_permission_assign(
    path: RolePermissionPath,
    ctx: RequestContext,
    Form(form): Form<HashMap<String, String>>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.role_service();

    let assigned: std::collections::HashSet<String> = form
        .keys()
        .filter(|k| k.starts_with("perm_"))
        .map(|k| k[5..].to_string())
        .collect();

    let rwp = svc.get_role_with_permissions(&service_ctx, &mut conn, path.id).await?;
    let current: std::collections::HashSet<String> = rwp.permissions.into_iter().collect();

    let to_add: Vec<(String, String)> = assigned
        .difference(&current)
        .filter_map(|p| parse_permission(p))
        .collect();
    let to_remove: Vec<(String, String)> = current
        .difference(&assigned)
        .filter_map(|p| parse_permission(p))
        .collect();

    if !to_add.is_empty() {
        svc.assign_permissions(&service_ctx, &mut conn, path.id, to_add).await?;
    }
    if !to_remove.is_empty() {
        svc.remove_permissions(&service_ctx, &mut conn, path.id, to_remove).await?;
    }

    let detail = RoleDetailPath { id: path.id };
    Ok(([("HX-Redirect", detail.to_string())], Html(String::new())))
}

// ── Helpers ──

fn parse_permission(perm: &str) -> Option<(String, String)> {
    let (resource, action) = perm.split_once(':')?;
    Some((resource.to_string(), action.to_string()))
}

fn get_initials(s: &str) -> String {
    s.chars().take(2).collect::<String>().to_uppercase()
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

// ── Page Component ──

fn role_detail_page(
    rwp: &abt_core::shared::identity::model::RoleWithPermissions,
    parent_role_name: Option<&str>,
    user_count: usize,
    groups: &[GroupData],
    total_perms: usize,
) -> Markup {
    let role = &rwp.role;
    let role_id = role.role_id;
    let list_path = RoleListPath.to_string();
    let edit_path = RoleEditPath { id: role_id }.to_string();
    let initials = get_initials(&role.role_name);
    let perm_count = rwp.permissions.len();
    let perm_set: std::collections::HashSet<&str> = rwp.permissions.iter().map(|s| s.as_str()).collect();

    html! {
        div {
            // ── Back Link ──
            a.back-link href=(list_path) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回角色列表"
            }

            // ── Profile Hero ──
            div.profile-hero {
                div.ph-left {
                    div.ph-avatar { (initials) }
                    div.ph-info {
                        h2 { (&role.role_name) }
                        div.ph-meta {
                            span.ph-code { (&role.role_code) }
                            span.ph-text {
                                "ID: " (role_id) " · 创建于 "
                                (role.created_at.format("%Y-%m-%d"))
                            }
                        }
                        div.ph-badges {
                            @if role.is_system_role {
                                span.tag-pill.tag-active { "内置角色" }
                            } @else {
                                span.tag-pill.tag-dept { "自定义角色" }
                            }
                            @if let Some(pname) = parent_role_name {
                                span.tag-pill.tag-super { "上级: " (pname) }
                            }
                        }
                    }
                }
                div.ph-actions {
                    @if !role.is_system_role {
                        a.btn.btn-default.btn-sm href=(edit_path) {
                            (icon::edit_icon("w-3.5 h-3.5"))
                            " 编辑"
                        }
                    }
                }
            }

            // ── Stats Row ──
            div.profile-stats {
                div.ps-item {
                    span.ps-dot.d-blue {}
                    b { (perm_count) }
                    span { "项权限" }
                }
                div.ps-item {
                    span.ps-dot.d-green {}
                    b { (user_count) }
                    span { "个用户" }
                }
                div.ps-item {
                    span.ps-dot.d-purple {}
                    b { (total_perms) }
                    span { "项可分配" }
                }
            }

            // ── Two-Column Grid ──
            div.detail-grid {
                // ── LEFT: Info Card ──
                div {
                    div.d-card {
                        div.d-card-head {
                            h3 {
                                (icon::lock_icon("w-3.5 h-3.5"))
                                " 角色信息"
                            }
                        }
                        div.d-card-body {
                            div.info-card-rows {
                                div.info-row {
                                    span.info-label { "角色 ID" }
                                    span.info-val.info-mono {
                                        "#" (format!("{:03}", role_id))
                                    }
                                }
                                div.info-row {
                                    span.info-label { "角色编码" }
                                    span.info-val.info-mono { (&role.role_code) }
                                }
                                div.info-row {
                                    span.info-label { "角色类型" }
                                    @if role.is_system_role {
                                        span.info-val.info-success { "内置角色" }
                                    } @else {
                                        span.info-val { "自定义角色" }
                                    }
                                }
                                div.info-row {
                                    span.info-label { "上级角色" }
                                    @if let Some(pname) = parent_role_name {
                                        span.info-val { (pname) }
                                    } @else {
                                        span.info-val.info-muted { "无" }
                                    }
                                }
                                div.info-row {
                                    span.info-label { "描述" }
                                    @if let Some(desc) = &role.description {
                                        span.info-val { (desc) }
                                    } @else {
                                        span.info-val.info-muted { "—" }
                                    }
                                }
                                div.info-row {
                                    span.info-label { "创建时间" }
                                    span.info-val.info-mono {
                                        (role.created_at.format("%Y-%m-%d %H:%M"))
                                    }
                                }
                                div.info-row {
                                    span.info-label { "最后更新" }
                                    @if let Some(updated) = &role.updated_at {
                                        span.info-val.info-mono {
                                            (updated.format("%Y-%m-%d %H:%M"))
                                        }
                                    } @else {
                                        span.info-val.info-muted { "—" }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── RIGHT: Permission Preview ──
                div {
                    div.d-card {
                        div.d-card-head {
                            h3 {
                                (icon::sliders_icon("w-3.5 h-3.5"))
                                " 权限配置"
                            }
                            span.d-card-count { (format!("{} / {} 项", perm_count, total_perms)) }
                        }
                        div.d-card-body.perm-preview {
                            @for group in groups {
                                (perm_preview_group(group, &perm_set))
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Permission Preview (read-only chips, grouped) ──

fn perm_preview_group(group: &GroupData, perm_set: &std::collections::HashSet<&str>) -> Markup {
    // Check if any resource in this group has assigned perms
    let has_perms = group.resources.iter().any(|res| {
        res.defs.iter().any(|def| {
            let perm = format!("{}:{}", res.code, def.action);
            perm_set.contains(perm.as_str())
        })
    });

    if !has_perms {
        return html! {};
    }

    html! {
        div.perm-preview-group {
            div.perm-preview-head {
                span.perm-group-icon class=(format!("perm-group-icon {}", group.icon_cls)) {
                    (group_icon_svg(group.icon_cls))
                }
                " " (group.name)
            }
            @for res in &group.resources {
                (perm_preview_resource(res, perm_set))
            }
        }
    }
}

fn perm_preview_resource(res: &GroupResource, perm_set: &std::collections::HashSet<&str>) -> Markup {
    // Collect which actions this resource has assigned
    let assigned: Vec<(&str, &str)> = res.defs.iter()
        .filter_map(|def| {
            let perm = format!("{}:{}", res.code, def.action);
            if perm_set.contains(perm.as_str()) {
                Some((def.action, def.action_name))
            } else {
                None
            }
        })
        .collect();

    // Skip resources with no permissions
    if assigned.is_empty() {
        return html! {};
    }

    html! {
        div.perm-chips {
            (icon::box_icon("w-3 h-3"))
            " " (res.name)
            " "
            span.ph-code { (res.code) }
            " "
            @for (action, label) in &assigned {
                @let chip_class = match *action {
                    "read" => "perm-chip pc-read",
                    "create" => "perm-chip pc-create",
                    "update" => "perm-chip pc-write",
                    "delete" => "perm-chip pc-delete",
                    _ => "perm-chip",
                };
                span class=(chip_class) { (label) }
            }
        }
    }
}
