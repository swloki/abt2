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
        content, &nav_filter,    );

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

// ── Components ──

fn department_list_page(
    departments: &[Department],
    selected_id: Option<i64>,
    initial_detail: Option<&Markup>,
    can_create: bool,
) -> Markup {
    html! {
        div class="dept-layout" {
            // ── Left: Tree Panel ──
            (tree_panel(departments, selected_id, can_create))

            // ── Right: Detail Panel ──
            div class="dept-detail" id="deptDetail" {
                @if let Some(detail) = initial_detail {
                    (detail)
                } @else {
                    div class="dept-empty" {
                        div class="dept-empty-illust" {
                            (icon::building_icon("w-9 h-9"))
                        }
                        h4 { "选择部门查看详情" }
                        p { "点击左侧组织架构中的部门节点" }
                    }
                }
            }
        }

        // ── Drawer container (Surreal.js open/close) ──
        div class="drawer-overlay" id="deptDrawer"
            tabindex="-1"
            _="on click[me is event.target] remove .open on keydown[event.key is 'Escape'] remove .open" {
            div class="drawer-panel" id="drawerPanel" {
                // Content loaded via HTMX
            }
        }
    }
}

fn tree_panel(departments: &[Department], selected_id: Option<i64>, can_create: bool) -> Markup {
    let count = departments.len();
    html! {
        div class="dept-tree-panel" {
            // ── Top bar ──
            div class="tree-top-bar" {
                h3 {
                    (icon::building_icon("w-[15px] h-[15px]"))
                    "组织架构"
                }
                @if can_create {
                    button class="tree-add-btn" title="新建部门"
                        hx-get=(DepartmentCreateDrawerPath::PATH)
                        hx-target="#drawerPanel"
                        hx-swap="innerHTML"
                        _="on 'htmx:afterRequest' add .open to #deptDrawer" {
                        (icon::plus_icon("w-[13px] h-[13px]"))
                    }
                }
            }

            // ── Search ──
            div class="tree-search-wrap" {
                (icon::search_icon("w-[13px] h-[13px]"))
                input type="text" class="tree-search" placeholder="搜索部门…" {}
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

            // ── Tree list ──
            div class="tree-list" id="deptTree" {
                @for dept in departments {
                    (tree_item(dept, selected_id == Some(dept.department_id)))
                }
                @if departments.is_empty() {
                    div class="empty-state-text" {
                        "暂无部门数据"
                    }
                }
            }

            // ── Footer ──
            div class="tree-foot" id="treeFoot" {
                "共 " (count) " 个部门"
            }
        }
    }
}

fn tree_item(dept: &Department, is_active: bool) -> Markup {
    let active_class = if is_active { " active" } else { "" };
    let code_color = dept_code_color_class(&dept.department_code);
    let detail_path = DepartmentDetailPath {
        id: dept.department_id,
    }
    .to_string();

    html! {
        div class={"tree-item" (active_class)}
            data-id=(dept.department_id)
            data-name=(dept.department_name)
            data-code=(dept.department_code)
            hx-get=(detail_path)
            hx-target="#deptDetail"
            hx-swap="innerHTML"
            _="on click take .active from .tree-item" {
            span class={"tree-code " (code_color)} {
                (dept.department_code.chars().take(2).collect::<String>())
            }
            span class="tree-name" {
                (dept.department_name)
            }
            @if dept.is_default {
                span class="tree-tag tag-default" { "默认" }
            }
            @if !dept.is_active {
                span class="tree-tag tag-off" { "停用" }
            }

        }
    }
}

fn detail_content_fragment(dept: &Department, members: &[UserWithRoles], can_create: bool, can_delete: bool) -> Markup {
    let code_color = dept_code_color_class(&dept.department_code);
    let member_count = members.len();
    let edit_path = DepartmentEditPath {
        id: dept.department_id,
    }
    .to_string();
    let delete_path = DepartmentDeletePath {
        id: dept.department_id,
    }
    .to_string();

    let description = match &dept.description {
        Some(d) => d.as_str(),
        None => "",
    };

    let status_text = if dept.is_active {
        "已激活"
    } else {
        "已停用"
    };
    let default_text = if dept.is_default { "默认" } else { "普通" };

    let status_class = if dept.is_active {
        "text-success"
    } else {
        "text-muted"
    };

    html! {
        // ── Hero ──
        div class="d-hero" {
            div class="d-hero-left" {
                div class={"d-hero-icon " (code_color)} {
                    (icon::building_icon("w-5 h-5"))
                }
                div class="d-hero-text" {
                    h2 { (dept.department_name) }
                    div class="d-hero-sub" {
                        span class="d-hero-code" { (dept.department_code) }
                        @if !description.is_empty() {
                            span class="d-hero-desc" { (description) }
                        }
                    }
                }
            }
            div class="d-hero-actions" {
                @if can_create {
                    button class="btn btn-default btn-sm"
                        hx-get=(edit_path)
                        hx-target="#drawerPanel"
                        hx-swap="innerHTML"
                        _="on 'htmx:afterRequest' add .open to #deptDrawer" {
                        (icon::edit_icon("w-[13px] h-[13px]"))
                        "编辑"
                    }
                }
                @if can_delete && !dept.is_default {
                    button class="btn btn-default btn-sm text-danger"
                        hx-confirm=(format!("确认删除部门「{}」？该操作不可恢复。", dept.department_name))
                        hx-post=(delete_path)
                        hx-target="body"
                        hx-swap="none" {
                        (icon::trash_icon("w-[13px] h-[13px]"))
                        "删除"
                    }
                }
            }
        }

        // ── Stats ──
        div class="d-stats" {
            div class="d-stat" {
                span class="d-stat-dot dot-blue" {}
                b { (member_count) } span { "名成员" }
            }
            div class="d-stat" {
                span class="d-stat-dot dot-green" {}
                b class=(status_class) { (status_text) }
            }
            div class="d-stat" {
                span class="d-stat-dot dot-amber" {}
                b { (default_text) } span { "部门" }
            }
        }

        // ── Body ──
        div class="d-body" {
            // Info section
            div class="d-section" {
                div class="d-section-head" {
                    span class="d-section-title" {
                        (icon::circle_alert_icon("w-[14px] h-[14px]"))
                        "基本信息"
                    }
                }
                div class="info-card" {
                    div class="info-row" {
                        span class="info-label" { "部门 ID" }
                        span class="info-val mono" { "#" (format!("{:03}", dept.department_id)) }
                    }
                    div class="info-row" {
                        span class="info-label" { "部门代码" }
                        span class="info-val mono" { (dept.department_code) }
                    }
                    div class="info-row" {
                        span class="info-label" { "创建时间" }
                        span class="info-val" { (dept.created_at.format("%Y-%m-%d %H:%M")) }
                    }
                    @if let Some(updated) = &dept.updated_at {
                        div class="info-row" {
                            span class="info-label" { "最后更新" }
                            span class="info-val" { (updated.format("%Y-%m-%d %H:%M")) }
                        }
                    }
                }
            }

            // Members section
            div class="d-section" {
                div class="d-section-head" {
                    span class="d-section-title" {
                        (icon::users_icon("w-[14px] h-[14px]"))
                        "部门成员"
                    }
                    span class="d-section-count" { (member_count) " 人" }
                }
                @if members.is_empty() {
                    div class="empty-state-text" {
                        "暂无成员"
                    }
                } @else {
                    div class="member-grid" {
                        @for (i, m) in members.iter().enumerate() {
                            @if i < 4 {
                                (member_card(m))
                            }
                        }
                        @if member_count > 4 {
                            div class="m-more" {
                                "还有 " (member_count - 4) " 人…"
                            }
                        }
                    }
                }
            }
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
        div class="m-card" {
            span class={"m-ava " (ava_color)} { (initials) }
            div class="m-text" {
                div class="m-name" { (display_name) }
                span class="m-role" { (role_display) }
            }
        }
    }
}

fn dept_drawer_fragment(is_edit: bool, dept: Option<&Department>) -> Markup {
    let title = if is_edit {
        "编辑部门"
    } else {
        "新建部门"
    };
    let subtitle = if let Some(d) = &dept {
        if is_edit {
            format!("修改「{}」的部门信息", d.department_name)
        } else {
            "填写部门信息后保存".to_string()
        }
    } else {
        "填写部门信息后保存".to_string()
    };

    let (action_path, name_val, code_val, desc_val, is_active_val, _is_default_val) = match &dept {
        Some(d) => (
            DepartmentEditPath {
                id: d.department_id,
            }
            .to_string(),
            d.department_name.as_str(),
            d.department_code.as_str(),
            d.description.as_deref().unwrap_or(""),
            d.is_active,
            d.is_default,
        ),
        None => (
            DepartmentCreateDrawerPath::PATH.to_string(),
            "",
            "",
            "",
            true,
            false,
        ),
    };

    html! {
        form id="deptForm" hx-post=(action_path) hx-swap="none" _="on 'htmx:afterRequest' remove .open from #deptDrawer" {
            div class="drawer-head" {
                div class="drawer-head-left" {
                    div class="drawer-head-icon" {
                        (icon::building_icon("w-[18px] h-[18px]"))
                    }
                    div {
                        h3 { (title) }
                        p { (subtitle) }
                    }
                }
                button class="drawer-close" type="button" _="on click remove .open from #deptDrawer" {
                    (icon::x_icon("w-[18px] h-[18px]"))
                }
            }
            div class="drawer-body" {
                div class="drawer-section" {
                    div class="drawer-label" { "基本信息" }
                    div class="form-row" {
                        label { "部门名称 " span class="req" { "*" } }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="department_name"
                            required placeholder="如：销售部" value=(name_val) {}
                    }
                    div class="form-row" {
                        label { "部门代码 " span class="req" { "*" } }
                        @if is_edit {
                            input class="form-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]-readonly" type="text" name="department_code"
                                required placeholder="如：SA"
                                value=(code_val)
                                readonly {}
                        } @else {
                            input class="form-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]-mono" type="text" name="department_code"
                                required placeholder="如：SA"
                                value=(code_val) {}
                        }
                    }
                    div class="form-row" {
                        label { "部门描述" }
                        textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="description"
                            placeholder="描述该部门的职责和业务范围…" {
                            (desc_val)
                        }
                    }
                }

                // ── Settings section ──
                div class="drawer-section" {
                    div class="drawer-label" { "设置" }
                    @if is_edit {
                        div class="form-row" {
                            label class="flex items-center gap-2 text-[13px] cursor-pointer py-1.5" {
                                input type="checkbox" name="is_active" value="true"
                                    checked[is_active_val] {}
                                "启用部门"
                            }
                        }
                    } @else {
                        input type="hidden" name="is_active" value="true" {}
                    }
                    @if let Some(d) = dept {
                        @if d.is_default {
                            div class="form-row" {
                                label class="flex items-center gap-2 text-[13px] cursor-pointer py-1.5" {
                                    input type="checkbox" checked disabled {}
                                    "默认部门"
                                    span class="flex items-center gap-2 text-[13px] cursor-pointer py-1.5-hint" { "（系统默认部门不可取消）" }
                                }
                            }
                        }
                    }
                }

                // ── Tip (shown in both create and edit mode) ──
                div class="drawer-section" {
                    div class="drawer-tip" {
                        (icon::circle_alert_icon("w-[15px] h-[15px]"))
                        div { "部门代码用于系统内部标识，创建后不可修改。建议使用大写英文字母缩写，如 "
                            strong { "SA" } "（销售部）、" strong { "PU" } "（采购部）。"
                        }
                    }
                }
            }
            div class="drawer-foot" {
                button class="btn btn-default" type="button" _="on click remove .open from #deptDrawer" { "取消" }
                button class="btn btn-primary" type="submit" {
                    (icon::check_circle_icon("w-[14px] h-[14px]"))
                    "保存"
                }
            }
        }
    }
}

// ── Helpers ──

/// Map department code to a color class for the tree badge.
fn dept_code_color_class(code: &str) -> &'static str {
    match code.to_uppercase().as_str() {
        "GO" | "GM" => "tc-purple",
        "SA" | "SL" => "tc-blue",
        "PU" | "PC" => "tc-teal",
        "WH" | "WM" => "tc-orange",
        "FI" | "FN" => "tc-green",
        "QC" | "QA" => "tc-red",
        _ => {
            // Deterministic color based on first char
            let first = code.chars().next().unwrap_or('A');
            match first.to_ascii_uppercase() {
                'A'..='D' => "tc-blue",
                'E'..='H' => "tc-green",
                'I'..='L' => "tc-teal",
                'M'..='P' => "tc-orange",
                'Q'..='T' => "tc-purple",
                _ => "tc-red",
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
        0 => "aa-blue",
        1 => "aa-green",
        2 => "aa-purple",
        3 => "aa-orange",
        4 => "aa-red",
        _ => "aa-teal",
    }
}
