use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::category::model::*;
use abt_core::master_data::category::CategoryService;

use crate::components::confirm_dialog;
use crate::components::icon;
use crate::components::modal;
use crate::layout::page::admin_page;
use crate::routes::category::{
    CategoryCreatePath, CategoryDeletePath, CategoryDetailPanelPath, CategoryListPath,
    CategoryUpdatePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCategoryForm {
    pub category_name: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateCategoryForm {
    pub category_name: String,
}

// ── Handlers ──

#[require_permission("CATEGORY", "read")]
pub async fn get_category_list(
    _path: CategoryListPath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> crate::errors::Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.category_service();
    let tree = svc
        .get_tree(&service_ctx, &mut conn, None, None)
        .await?;
    let content = category_page(&tree);
    let page_html = admin_page(
        &headers,
        "分类目录",
        &claims,
        "md-category",
        CategoryListPath::PATH,
        "主数据管理",
        Some("分类目录"),
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("CATEGORY", "read")]
pub async fn get_category_tree(ctx: RequestContext) -> crate::errors::Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.category_service();

    let tree = svc
        .get_tree(&service_ctx, &mut conn, None, None)
        .await?;

    Ok(Html(tree_fragment(&tree).into_string()))
}

#[require_permission("CATEGORY", "read")]
pub async fn get_category_detail_panel(
    path: CategoryDetailPanelPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.category_service();

    let category = svc.get(&service_ctx, &mut conn, path.id).await?;

    let parent_name = if category.parent_id != 0 {
        let parent = svc.get(&service_ctx, &mut conn, category.parent_id).await?;
        parent.category_name.clone()
    } else {
        "无 (顶级)".to_string()
    };

    let update_url = CategoryUpdatePath { id: path.id }.to_string();
    let delete_url = CategoryDeletePath { id: path.id }.to_string();

    Ok(Html(
        detail_panel(&category, &parent_name, &update_url, &delete_url).into_string(),
    ))
}

#[require_permission("CATEGORY", "create")]
pub async fn create_category(
    _path: CategoryCreatePath,
    ctx: RequestContext,
    Form(form): Form<CreateCategoryForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.category_service();

    let req = CreateCategoryReq {
        category_name: form.category_name,
        parent_id: form.parent_id.unwrap_or(0),
    };

    svc.create(&service_ctx, &mut conn, req).await?;

    Ok(([("HX-Redirect", CategoryListPath::PATH)], Html(String::new())))
}

#[require_permission("CATEGORY", "update")]
pub async fn update_category(
    path: CategoryUpdatePath,
    ctx: RequestContext,
    Form(form): Form<UpdateCategoryForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.category_service();

    let req = UpdateCategoryReq {
        category_name: Some(form.category_name),
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    Ok(([("HX-Redirect", CategoryListPath::PATH)], Html(String::new())))
}

#[require_permission("CATEGORY", "delete")]
pub async fn delete_category(
    path: CategoryDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.category_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", CategoryListPath::PATH)], Html(String::new())))
}

// ── Inline styles for split view ──

fn split_view_style() -> Markup {
    PreEscaped(
        r#"
        <style>
        .split-view { display: flex; gap: 0; height: calc(100vh - 140px); border: 1px solid var(--border-soft, #e5e7eb); border-radius: var(--radius-md, 8px); overflow: hidden; background: #fff; }
        .split-left { width: 300px; min-width: 300px; display: flex; flex-direction: column; border-right: 1px solid var(--border-soft, #e5e7eb); background: #fafbfc; }
        .split-left-header { padding: 16px 20px; border-bottom: 1px solid var(--border-soft, #e5e7eb); }
        .split-left-title { font-size: 16px; font-weight: 600; margin: 0; }
        .split-left-search { padding: 12px 16px; border-bottom: 1px solid var(--border-soft, #e5e7eb); }
        .split-left-search input { width: 100%; padding: 6px 10px 6px 32px; border: 1px solid var(--border-soft, #e5e7eb); border-radius: 6px; font-size: 13px; background: #fff url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' fill='none' stroke='%23999' stroke-width='2' viewBox='0 0 24 24'%3E%3Ccircle cx='11' cy='11' r='8'/%3E%3Cline x1='21' y1='21' x2='16.65' y2='16.65'/%3E%3C/svg%3E") 8px center no-repeat; }
        .split-left-body { flex: 1; overflow-y: auto; padding: 8px 0; }
        .split-left-footer { padding: 12px 16px; border-top: 1px solid var(--border-soft, #e5e7eb); }
        .split-left-footer .btn { width: 100%; justify-content: center; }
        .split-right { flex: 1; overflow-y: auto; padding: 24px; }
        .split-right-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: var(--muted, #999); }
        .split-right-empty svg { opacity: 0.3; margin-bottom: 16px; }
        .split-right-empty p { font-size: 14px; }

        /* Tree nodes */
        .tree-node-row { display: flex; align-items: center; padding: 6px 12px; cursor: pointer; font-size: 13px; color: #333; gap: 4px; user-select: none; }
        .tree-node-row:hover { background: #f0f5ff; }
        .tree-node-row.selected { background: #e8f0fe; color: var(--accent, #4f46e5); font-weight: 500; }
        .tree-toggle { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; flex-shrink: 0; transition: transform 0.15s; }
        .tree-toggle.collapsed { transform: rotate(-90deg); }
        .tree-node-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
        .tree-node-badge { font-size: 11px; background: #e5e7eb; color: #666; padding: 1px 6px; border-radius: 10px; flex-shrink: 0; }
        .tree-children { overflow: hidden; }

        /* Detail panel */
        .detail-title { font-size: 18px; font-weight: 600; margin: 0; }
        .detail-actions { display: flex; gap: 8px; }
        .detail-card { background: #fff; border: 1px solid var(--border-soft, #e5e7eb); border-radius: 8px; margin-bottom: 16px; }
        .detail-card-header { padding: 12px 16px; border-bottom: 1px solid var(--border-soft, #e5e7eb); font-weight: 600; font-size: 14px; }
        .detail-card-body { padding: 16px; display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
        .detail-card-empty { padding: 24px; text-align: center; color: var(--muted, #999); font-size: 13px; }
        .detail-field { }
        .detail-label { display: block; font-size: 12px; color: var(--muted, #999); margin-bottom: 4px; }
        .detail-value { display: block; font-size: 14px; color: #333; }
        .btn-danger-ghost { color: var(--danger, #ff4d4f) !important; border-color: var(--danger, #ff4d4f) !important; background: transparent !important; }
        .btn-danger-ghost:hover { background: #fff1f0 !important; }
        .btn-sm { padding: 4px 12px; font-size: 12px; }
        </style>
        "#.to_string(),
    )
}

// ── Page Component ──

fn category_page(tree: &[CategoryTree]) -> Markup {
    html! {
        div x-data="categorySplitView()" {
            (split_view_style())
            script { (category_split_view_script()) }

            // ── Split View Container ──
            div class="split-view" {
                // ── Left Panel: Tree ──
                div class="split-left" {
                    div class="split-left-header" {
                        h2 class="split-left-title" { "分类目录" }
                    }
                    div class="split-left-search" {
                        input type="text" placeholder="搜索分类..."
                            x-model="searchText"
                            x-on:input="filterTree()";
                    }
                    div class="split-left-body" id="category-tree" {
                        (tree_fragment(tree))
                    }
                    div class="split-left-footer" {
                        button class="btn btn-primary"
                            x-on:click="createModalOpen = true" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建分类"
                        }
                    }
                }

                // ── Right Panel: Detail ──
                div class="split-right" id="detail-panel" {
                    div class="split-right-empty" x-show="selectedId === null" {
                        (icon::grid_icon("w-12 h-12"))
                        p { "请从左侧选择一个分类" }
                    }
                }
            }

            // ── Create Modal ──
            (create_category_modal(tree))
        }
    }
}

// ── Alpine.js component ──

fn category_split_view_script() -> Markup {
    PreEscaped(
        r#"
        function categorySplitView() {
            return {
                selectedId: null,
                searchText: '',
                createModalOpen: false,
                selectCategory(id) {
                    this.selectedId = id;
                    htmx.ajax('GET', '/admin/md/categories/' + id + '/panel', '#detail-panel');
                },
                filterTree() {
                    var q = this.searchText.toLowerCase();
                    this.$el.querySelectorAll('.tree-node-row').forEach(function(el) {
                        var name = el.dataset.name || '';
                        el.style.display = (!q || name.toLowerCase().indexOf(q) >= 0) ? '' : 'none';
                    });
                },
            };
        }
        "#.to_string(),
    )
}

// ── Tree Fragment ──

fn tree_fragment(tree: &[CategoryTree]) -> Markup {
    html! {
        @for node in tree {
            (tree_node(node, 0))
        }
    }
}

fn tree_node(node: &CategoryTree, depth: usize) -> Markup {
    let has_children = !node.children.is_empty();
    let count = node.meta.count;
    let id = node.category_id;
    let name = &node.category_name;
    let name_lower = name.to_lowercase();
    let click_expr = format!("selectCategory({})", id);
    let class_expr = format!("{{'selected': selectedId === {}}}", id);
    let pad_with_toggle = format!("padding-left: {}px", depth * 20 + 8);
    let pad_leaf = format!("padding-left: {}px", depth * 20 + 32);

    html! {
        @if has_children {
            div x-data="{ open: true }" {
                div.tree-node-row data-name=(name_lower)
                    style=(pad_with_toggle)
                    x-on:click=(click_expr)
                    x-bind:class=(class_expr) {
                    span class="tree-toggle"
                        x-bind:class="{'collapsed': !open}"
                    onclick="event.stopPropagation()" x-on:click="open = !open" {
                        (icon::chevron_down_icon("w-3.5 h-3.5"))
                    }
                    span class="tree-node-name" { (name) }
                    @if count > 0 {
                        span class="tree-node-badge" { (count) }
                    }
                }
                div class="tree-children" x-show="open" x-transition="" {
                    @for child in &node.children {
                        (tree_node(child, depth + 1))
                    }
                }
            }
        } @else {
            div.tree-node-row data-name=(name_lower)
                style=(pad_leaf)
                x-on:click=(click_expr)
                x-bind:class=(class_expr) {
                span class="tree-node-name" { (name) }
                @if count > 0 {
                    span class="tree-node-badge" { (count) }
                }
            }
        }
    }
}

// ── Detail Panel ──

fn detail_panel(
    category: &Category,
    parent_name: &str,
    update_url: &str,
    delete_url: &str,
) -> Markup {
    html! {
        div x-data="{ editModalOpen: false, deleteDialogOpen: false }" {
            // ── Header ──
            div class="detail-header" {
                h2 class="detail-title" { (category.category_name) }
                div class="detail-actions" {
                    button class="btn btn-default btn-sm"
                        x-on:click="editModalOpen = true" {
                        (icon::edit_icon("w-4 h-4"))
                        "编辑"
                    }
                    button class="btn btn-danger-ghost btn-sm"
                        x-on:click="deleteDialogOpen = true" {
                        (icon::trash_icon("w-4 h-4"))
                        "删除"
                    }
                }
            }

            // ── Info Card ──
            div class="detail-card" {
                div class="detail-card-body" {
                    div class="detail-field" {
                        span class="detail-label" { "名称" }
                        span class="detail-value" { (category.category_name) }
                    }
                    div class="detail-field" {
                        span class="detail-label" { "路径" }
                        span class="detail-value" { (category.path) }
                    }
                    div class="detail-field" {
                        span class="detail-label" { "上级分类" }
                        span class="detail-value" { (parent_name) }
                    }
                    div class="detail-field" {
                        span class="detail-label" { "产品数" }
                        span class="detail-value" { (category.meta.count) }
                    }
                }
            }

            // ── Edit Modal ──
            (modal::modal(
                "editModalOpen",
                "编辑分类",
                "保存",
                "edit-category-form",
                html! {
                    form id="edit-category-form"
                        hx-post=(update_url) {
                        div class="form-field" {
                            label { "分类名称" }
                            input type="text" name="category_name"
                                value=(category.category_name) required;
                        }
                    }
                },
            ))

            // ── Delete Confirm ──
            (confirm_dialog::confirm_dialog(
                "deleteDialogOpen",
                "删除分类",
                &format!("确定要删除分类 <strong>{}</strong> 吗？此操作不可撤销。", category.category_name),
                "确认删除",
                "delete-category-form",
                html! {
                    form id="delete-category-form"
                        hx-post=(delete_url) {}
                }
            ))
        }
    }
}

// ── Create Category Modal ──

fn create_category_modal(tree: &[CategoryTree]) -> Markup {
    modal::modal(
        "createModalOpen",
        "新建分类",
        "创建",
        "create-category-form",
        html! {
            form id="create-category-form"
                hx-post=(CategoryCreatePath::PATH) {
                div class="form-field" {
                    label { "分类名称" }
                    input type="text" name="category_name"
                        placeholder="输入分类名称" required;
                }
                div class="form-field" {
                    label { "上级分类" }
                    select name="parent_id" {
                        option value="" { "无 (顶级分类)" }
                        @for node in tree {
                            (tree_option(node, 0))
                        }
                    }
                }
            }
        },
    )
}

fn tree_option(node: &CategoryTree, depth: usize) -> Markup {
    let prefix = "\u{3000}".repeat(depth);
    html! {
        option value=(node.category_id) { (prefix) (node.category_name) }
        @for child in &node.children {
            (tree_option(child, depth + 1))
        }
    }
}
