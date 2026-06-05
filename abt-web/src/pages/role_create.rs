use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
// serde::Deserialize not needed — using HashMap<String, String> directly

use abt_core::shared::identity::model::{ResourceActionDef, Role, RESOURCE_ACTION_DEFS};
use abt_core::shared::identity::RoleService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::role::{RoleCreatePath, RoleListPath};
use crate::utils::RequestContext;

// ── Resource Groupings ──

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

const ACTIONS: &[&str] = &["create", "read", "update", "delete"];
const ACTION_LABELS: &[&str] = &["CREATE", "READ", "UPDATE", "DELETE"];

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
    groups.iter().map(|g| g.resources.iter().map(|r| r.defs.len()).sum::<usize>()).sum()
}

// ── Handlers ──

#[require_permission("ROLE", "create")]
pub async fn get_role_create(
    _path: RoleCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.role_service();
    let roles = svc.list_roles(&service_ctx, &mut conn).await?;

    let groups = build_groups();
    let total = total_perm_count(&groups);

    let content = role_create_page(&roles, &groups, total);
    let page_html = admin_page(
        is_htmx,
        "新建角色",
        &claims,
        "system",
        RoleCreatePath::PATH,
        "系统管理",
        Some("新建角色"),
        content,
    );

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

    // Extract permissions from "perm_RESOURCE:action" keys
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

fn role_create_page(roles: &[Role], groups: &[GroupData], total: usize) -> Markup {
    html! {
        form id="role-form"
              method="POST"
              action=(RoleCreatePath::PATH)
              hx-post=(RoleCreatePath::PATH)
              hx-swap="none" {

            // ── Page Header ──
            div.page-header {
                h1.page-title { "新建角色" }
                div.page-actions {
                    a.btn.btn-default href=(RoleListPath::PATH) { "取消" }
                    button.btn.btn-primary type="submit" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "保存"
                    }
                }
            }

            // ── Section 1: Role Info ──
            div.info-card {
                div.info-card-title {
                    (icon::lock_icon("w-[18px] h-[18px] text-accent"))
                    "角色信息"
                }
                div.form-grid {
                    div.form-field {
                        label { "角色名称 " span.text-danger { "*" } }
                        input.form-input type="text" name="role_name" required placeholder="如：销售经理" {}
                    }
                    div.form-field {
                        label { "角色代码 " span.text-danger { "*" } }
                        input.form-input type="text" name="role_code" required placeholder="如：sales_manager（英文+下划线）" {}
                        span.form-hint { "唯一标识，对应 JWT claims 中的 role_codes" }
                    }
                    div.form-field {
                        label { "上级角色" }
                        select.form-select name="parent_role_id" {
                            option value="" { "无（顶级角色）" }
                            @for role in roles {
                                @if !role.is_system_role {
                                    option value=(role.role_id) { (role.role_name) }
                                }
                            }
                        }
                        span.form-hint { "层级关系（预留），权限支持继承 + 环检测" }
                    }
                    div.form-field {
                        label { "角色类型" }
                    label.checkbox-label.mt-checkbox {
                            input type="checkbox" name="is_system" value="1" {}
                            span { "内置角色（不可删除）" }
                        }
                    }
                    div.form-field.field-full {
                        label { "描述" }
                        textarea.form-input name="description" rows="3" placeholder="角色用途说明" {}
                    }
                }
            }

            // ── Section 2: Permission Config ──
            div.info-card {
                div.info-card-title {
                    (icon::sliders_icon("w-[18px] h-[18px] text-accent"))
                    "权限配置"
                }
                p.perm-hint {
                    "为该角色分配资源操作权限。格式："
                    code { "RESOURCE:ACTION" }
                    "，角色权限取并集后存入 RolePermissionCache。"
                }

                div.perm-toolbar {
                    div.perm-toolbar-left {
                        "已选择 "
                        span.perm-count id="permCount" { "0" }
                        " / "
                        span id="permTotal" { (total) }
                        " 项权限"
                    }
                    div.perm-actions {
                        button.perm-action-btn type="button" data-action="select-all" onclick="setAll(true)" { "全选" }
                        button.perm-action-btn type="button" data-action="clear-all" onclick="setAll(false)" { "清空" }
                    }
                }

                div.perm-groups id="permGroups" {
                    @for (gi, group) in groups.iter().enumerate() {
                        (perm_group(gi, group))
                    }
                }
            }
        }

        // Inline script for permission matrix interactivity
        script {
            (PreEscaped(raw_perm_script()))
        }
    }
}

fn perm_group(gi: usize, group: &GroupData) -> Markup {
    let resource_count = group.resources.len();
    html! {
        div.perm-group data-group=(gi) {
            div.perm-group-head onclick=(format!("toggleGroup({})", gi)) {
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
                                      onchange=(format!("toggleGroupAction({},'{}',this.checked)", gi, action)) {}
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
                    (perm_resource_row(res))
                }
            }
        }
    }
}

fn perm_resource_row(res: &GroupResource) -> Markup {
    html! {
        div.perm-row {
            div.perm-resource {
                (res.name)
                " "
                span.perm-code { (res.code) }
            }
            @for action in ACTIONS {
                div.perm-cell {
                    input type="checkbox"
                          name={ "perm_" (res.code) ":" (action) }
                          data-action=(action)
                          data-resource=(res.code)
                          onchange="updateCount()" {}
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
    me('#permCount').textContent = checked;
    me('#permTotal').textContent = total;
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

updateCount();
"#
}
