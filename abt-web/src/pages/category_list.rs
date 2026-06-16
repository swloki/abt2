use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::category::model::*;
use abt_core::master_data::category::CategoryService;

use crate::components::icon;
use crate::components::pagination::htmx_pagination_inherited;
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

#[derive(Debug, Deserialize)]
pub(crate) struct PanelQuery {
    #[serde(default = "default_page")]
    pub page: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub category_id: Option<i64>,
    #[serde(default = "default_page")]
    pub page: u32,
}

fn default_page() -> u32 { 1 }

// ── Handlers ──

#[require_permission("CATEGORY", "read")]
pub async fn get_category_list(
    _path: CategoryListPath,
    ctx: RequestContext,
    Query(query): Query<ListQuery>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("CATEGORY", "create").await;
    let can_update = ctx.has_permission("CATEGORY", "update").await;
    let can_delete = ctx.has_permission("CATEGORY", "delete").await;
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

    // Determine which category to show: explicit query param, or first in tree
    let selected_id = query.category_id.or_else(|| tree.first().map(|f| f.category_id));

    let first_panel = if let Some(cat_id) = selected_id {
        let category = svc.get(&service_ctx, &mut conn, cat_id).await.ok();
        if let Some(cat) = category {
            let parent_name = if cat.parent_id != 0 {
                svc.get(&service_ctx, &mut conn, cat.parent_id).await.map(|p| p.category_name.clone()).unwrap_or_else(|_| "—".into())
            } else {
                "—".to_string()
            };
            let update_url = CategoryUpdatePath { id: cat_id }.to_string();
            let delete_url = CategoryDeletePath { id: cat_id }.to_string();
            let mut child_tree = svc.get_tree(&service_ctx, &mut conn, Some(cat_id), None).await.unwrap_or_default();
            if !child_tree.is_empty() {
                let child_ids: Vec<i64> = child_tree.iter().map(|c| c.category_id).collect();
                if let Ok(counts) = svc.count_products_batch(&service_ctx, &mut conn, &child_ids).await {
                    for child in &mut child_tree {
                        if let Some(cnt) = counts.get(&child.category_id) {
                            child.meta.count = *cnt;
                        }
                    }
                }
            }
            let page = abt_core::shared::types::PageParams::new(query.page, 5);
            let products = svc.list_products(&service_ctx, &mut conn, cat_id, page).await
                .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult::empty(query.page, 5));
            Some((detail_panel(&cat, &parent_name, &update_url, &delete_url, &child_tree, &products, cat_id, can_update, can_delete), cat_id))
        } else {
            None
        }
    } else {
        None
    };

    let content = category_page(&tree, first_panel.as_ref().map(|(p, _)| p), first_panel.as_ref().map(|(_, id)| *id), can_create);
    let page_html = admin_page(
        is_htmx,
        "产品分类",
        &claims,
        "md",
        CategoryListPath::PATH,
        "主数据管理",
        Some("产品分类"),
        content, &nav_filter,    );
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

    Ok(Html(tree_fragment(&tree, None).into_string()))
}

#[require_permission("CATEGORY", "read")]
pub async fn get_category_detail_panel(
    path: CategoryDetailPanelPath,
    ctx: RequestContext,
    Query(query): Query<PanelQuery>,
) -> crate::errors::Result<Html<String>> {
    let can_update = ctx.has_permission("CATEGORY", "update").await;
    let can_delete = ctx.has_permission("CATEGORY", "delete").await;
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
        "—".to_string()
    };

    let update_url = CategoryUpdatePath { id: path.id }.to_string();
    let delete_url = CategoryDeletePath { id: path.id }.to_string();

    // Get child categories for sub-category cards
    let mut child_tree = svc
        .get_tree(&service_ctx, &mut conn, Some(path.id), None)
        .await
        .unwrap_or_default();

    // Enrich sub-category counts with actual product numbers
    if !child_tree.is_empty() {
        let child_ids: Vec<i64> = child_tree.iter().map(|c| c.category_id).collect();
        if let Ok(counts) = svc.count_products_batch(&service_ctx, &mut conn, &child_ids).await {
            for child in &mut child_tree {
                if let Some(cnt) = counts.get(&child.category_id) {
                    child.meta.count = *cnt;
                }
            }
        }
    }

    // Get associated products (paginated)
    let page = abt_core::shared::types::PageParams::new(query.page, 5);
    let products = svc
        .list_products(&service_ctx, &mut conn, path.id, page)
        .await
        .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult::empty(query.page, 5));

    Ok(Html(
        detail_panel(&category, &parent_name, &update_url, &delete_url, &child_tree, &products, path.id, can_update, can_delete).into_string(),
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

    Ok(([("HX-Refresh", "true")], Html(String::new())))
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
        /* ─── Split View Layout ─── */
        .split-view {
            display: flex;
            gap: var(--space-6);
            height: calc(100vh - var(--header-h) - var(--space-8) * 2);
            min-height: 600px;
        }

        /* ─── Left Panel: Tree ─── */
        .tree-panel {
            width: 320px;
            min-width: 320px;
            background: var(--bg);
            border: 1px solid var(--border-soft);
            border-radius: var(--radius-md);
            box-shadow: var(--shadow-xs);
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }
        .tree-panel-header {
            padding: var(--space-4) var(--space-4) var(--space-3);
            border-bottom: 1px solid var(--border-soft);
            flex-shrink: 0;
        }
        .tree-panel-header h3 {
            font-size: var(--text-base);
            font-weight: 600;
            color: var(--fg);
            margin: 0 0 var(--space-3);
        }
        .tree-search {
            position: relative;
        }
        .tree-search svg {
            position: absolute;
            left: 10px;
            top: 50%;
            transform: translateY(-50%);
            width: 15px;
            height: 15px;
            color: var(--muted);
        }
        .tree-search input {
            width: 100%;
            padding: 7px 12px 7px 32px;
            border: 1px solid var(--border);
            border-radius: var(--radius-sm);
            background: var(--surface);
            font-size: var(--text-sm);
            color: var(--fg);
            outline: none;
            transition: all var(--motion-fast) var(--ease-standard);
        }
        .tree-search input:focus {
            border-color: var(--accent);
            box-shadow: var(--shadow-focus);
            background: var(--bg);
        }
        .tree-search input::placeholder {
            color: var(--muted);
            opacity: 0.7;
        }

        .tree-scroll {
            flex: 1;
            overflow-y: auto;
            padding: var(--space-2) 0;
        }
        .tree-footer {
            padding: var(--space-3) var(--space-4);
            border-top: 1px solid var(--border-soft);
            flex-shrink: 0;
        }

        /* ─── Tree Nodes ─── */
        .tree-node {
            user-select: none;
        }
        .tree-node-row {
            display: flex;
            align-items: center;
            gap: 4px;
            padding: 6px var(--space-4);
            cursor: pointer;
            transition: all var(--motion-fast) var(--ease-standard);
            border-radius: 0;
            position: relative;
        }
        .tree-node-row:hover {
            background: var(--accent-bg);
        }
        .tree-node-row.active {
            background: var(--accent-bg);
        }
        .tree-node-row.active::before {
            content: '';
            position: absolute;
            left: 0;
            top: 0;
            bottom: 0;
            width: 3px;
            background: var(--accent);
            border-radius: 0 3px 3px 0;
        }
        .tree-node-row.active .tree-node-name {
            color: var(--accent);
            font-weight: 600;
        }
        .tree-arrow {
            width: 20px;
            height: 20px;
            display: grid;
            place-items: center;
            flex-shrink: 0;
            cursor: pointer;
            border-radius: var(--radius-sm);
            transition: all var(--motion-fast) var(--ease-standard);
        }
        .tree-arrow:hover {
            background: rgba(0, 0, 0, 0.06);
        }
        .tree-arrow svg {
            width: 14px;
            height: 14px;
            color: var(--muted);
            transition: transform var(--motion-fast) var(--ease-standard);
        }
        .tree-node.expanded > .tree-node-row > .tree-arrow svg {
            transform: rotate(90deg);
        }
        .tree-node:not(.expanded) > .tree-children {
            display: none;
        }
        .tree-arrow.leaf {
            visibility: hidden;
        }
        .tree-node-name {
            flex: 1;
            font-size: var(--text-sm);
            color: var(--fg);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            transition: color var(--motion-fast);
        }
        .tree-node-count {
            font-size: 11px;
            color: var(--muted);
            background: var(--surface);
            padding: 1px 8px;
            border-radius: var(--radius-pill);
            font-weight: 500;
            flex-shrink: 0;
            font-family: var(--font-mono);
            font-variant-numeric: tabular-nums;
        }
        .tree-node-row.active .tree-node-count {
            background: rgba(22, 119, 255, 0.12);
            color: var(--accent);
        }
        .tree-children {
            overflow: hidden;
        }

        /* ─── Right Panel ─── */
        .detail-panel {
            flex: 1;
            min-width: 0;
            overflow-y: auto;
        }

        /* ─── Empty State ─── */
        .empty-state {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            height: 100%;
            min-height: 400px;
            color: var(--muted);
        }
        .empty-state svg {
            width: 64px;
            height: 64px;
            color: var(--border);
            margin-bottom: var(--space-5);
        }
        .empty-state-text {
            font-size: var(--text-base);
            font-weight: 500;
            margin-bottom: var(--space-2);
        }
        .empty-state-hint {
            font-size: var(--text-sm);
            color: var(--muted);
        }

        /* ─── Category Info Card ─── */
        .info-card {
            background: var(--bg);
            border: 1px solid var(--border-soft);
            border-radius: var(--radius-md);
            padding: var(--space-5) var(--space-6);
            margin-bottom: var(--space-6);
        }
        .cat-info-header {
            display: flex;
            align-items: flex-start;
            justify-content: space-between;
            margin-bottom: var(--space-5);
        }
        .cat-info-title {
            font-size: var(--text-xl);
            font-weight: 700;
            color: var(--fg);
            letter-spacing: -0.01em;
        }
        .cat-info-path {
            font-size: var(--text-sm);
            color: var(--muted);
            font-family: var(--font-mono);
            font-variant-numeric: tabular-nums;
            margin-top: var(--space-1);
        }
        .cat-info-actions {
            display: flex;
            gap: var(--space-2);
        }
        .cat-meta-grid {
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: var(--space-5);
        }
        .cat-meta-item {
            display: flex;
            flex-direction: column;
            gap: 4px;
        }

        /* ─── Sub-category Cards ─── */
        .subcat-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
            gap: var(--space-3);
        }
        .subcat-card {
            background: var(--bg);
            border: 1px solid var(--border-soft);
            border-radius: var(--radius-md);
            padding: var(--space-4);
            display: flex;
            align-items: center;
            justify-content: space-between;
            cursor: pointer;
            transition: all var(--motion-fast) var(--ease-standard);
            box-shadow: var(--shadow-xs);
        }
        .subcat-card:hover {
            border-color: var(--accent);
            box-shadow: var(--shadow-sm);
            transform: translateY(-1px);
        }
        .subcat-card-name {
            font-size: var(--text-sm);
            font-weight: 500;
            color: var(--fg);
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
            min-width: 0;
        }
        .subcat-card-count {
            font-size: 12px;
            color: var(--muted);
            background: var(--surface);
            padding: 2px 10px;
            border-radius: var(--radius-pill);
            font-family: var(--font-mono);
            font-variant-numeric: tabular-nums;
        }

        /* ─── Section header in detail panel ─── */
        .detail-section {
            margin-bottom: var(--space-6);
        }
        .detail-section-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: var(--space-4);
        }
        .detail-section-title {
            display: inline;
            font-size: var(--text-base);
            font-weight: 600;
            color: var(--fg);
        }
        .detail-section-count {
            display: inline;
            font-size: 12px;
            color: var(--muted);
            margin-left: var(--space-2);
            font-weight: 400;
        }

        /* ─── Responsive ─── */
        @media (max-width: 768px) {
            .split-view {
                flex-direction: column;
                height: auto;
            }
            .tree-panel {
                width: 100%;
                min-width: 0;
                max-height: 50vh;
            }
            .cat-meta-grid {
                grid-template-columns: 1fr 1fr;
            }
            .subcat-grid {
                grid-template-columns: 1fr 1fr;
            }
        }
        .data-card-empty {
            padding: var(--space-8);
            text-align: center;
            color: var(--muted);
            font-size: var(--text-sm);
            background: var(--surface);
            border: 1px solid var(--border-soft);
            border-radius: var(--radius-md);
        }
        </style>
        "#.to_string(),
    )
}

// ── Page Component ──

fn category_page(tree: &[CategoryTree], initial_panel: Option<&Markup>, first_id: Option<i64>, can_create: bool) -> Markup {

    html! {
        div {
            (split_view_style())
            script { (category_split_view_script()) }

            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "产品分类" }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" {
                        (icon::upload_icon("w-4 h-4"))
                        "导出"
                    }
                }
            }

            // ── Split View Container ──
            div class="split-view" {
                // ── Left Panel: Tree ──
                div class="tree-panel" {
                    div class="tree-panel-header" {
                        h3 { "分类目录" }
                        div class="w-full border border-border-soft rounded-sm text-[12px] bg-surface text-fg" {
                            (icon::search_icon("search-icon"))
                            input type="text" placeholder="搜索分类…"
                                oninput="filterTree(this.value)";
                        }
                    }
                    div class="tree-scroll" id="category-tree" {
                        (tree_fragment(tree, first_id))
                    }
                    @if can_create {
                        div class="tree-footer" {
                            button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" style="width: 100%; justify-content: center;"
                                _="on click add .is-open to #create-modal" {
                                (icon::plus_icon("w-4 h-4"))
                                "新建分类"
                            }
                        }
                    }
                }

                // ── Right Panel: Detail ──
                div class="detail-panel" id="detail-panel" {
                    @if let Some(panel) = initial_panel {
                        (panel)
                    } @else {
                        div class="text-center p-6 text-text-muted text-sm" {
                            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" {
                                path d="M4 20h16M8 16h8M6 12h12M10 8h4M12 4v16" {}
                            }
                            div class="text-center p-6 text-text-muted text-sm-text" { "请从左侧选择一个分类" }
                            div class="text-center p-6 text-text-muted text-sm-hint" { "选择分类查看详情和管理关联产品" }
                        }
                    }
                }
            }

            // ── Create Modal ──
            (create_category_modal(tree, can_create))
        }
    }
}

// Vanilla JS globals for tree interaction (filterTree).

fn category_split_view_script() -> Markup {
    PreEscaped(
        r#"
        function filterTree(q) {
            q = (q || '').trim().toLowerCase();
            var container = document.querySelector('#category-tree');
            if (!container) return;
            var allNodes = container.querySelectorAll('.tree-node');
            if (!q) {
                for (var i = 0; i < allNodes.length; i++) {
                    allNodes[i].style.display = '';
                    allNodes[i].classList.add('expanded');
                }
                return;
            }
            for (var i = 0; i < allNodes.length; i++) {
                var name = (allNodes[i].getAttribute('data-name') || '').toLowerCase();
                allNodes[i]._matches = (name.indexOf(q) >= 0);
            }
            for (var i = 0; i < allNodes.length; i++) {
                if (allNodes[i]._matches) {
                    var ancestor = allNodes[i].parentElement;
                    while (ancestor && ancestor !== container) {
                        if (ancestor.classList && ancestor.classList.contains('tree-node')) {
                            ancestor._matches = true;
                        }
                        ancestor = ancestor.parentElement;
                    }
                }
            }
            for (var i = 0; i < allNodes.length; i++) {
                allNodes[i].style.display = allNodes[i]._matches ? '' : 'none';
                if (allNodes[i]._matches) allNodes[i].classList.add('expanded');
                delete allNodes[i]._matches;
            }
        }
        "#.to_string(),
    )
}

// ── Tree Fragment ──

fn tree_fragment(tree: &[CategoryTree], selected_id: Option<i64>) -> Markup {
    let expand_ids: Vec<i64> = selected_id.map_or(Vec::new(), |sid| {
        fn find_path(nodes: &[CategoryTree], target: i64, path: &mut Vec<i64>) -> bool {
            for n in nodes {
                path.push(n.category_id);
                if n.category_id == target { return true; }
                if find_path(&n.children, target, path) { return true; }
                path.pop();
            }
            false
        }
        let mut p = Vec::new();
        find_path(tree, sid, &mut p);
        p
    });

    html! {
        @for node in tree {
            (tree_node(node, 0, selected_id, &expand_ids))
        }
    }
}

fn tree_node(node: &CategoryTree, depth: usize, selected_id: Option<i64>, expand_ids: &[i64]) -> Markup {
    let has_children = !node.children.is_empty();
    let count = node.meta.count;
    let id = node.category_id;
    let name = &node.category_name;
    let name_lower = name.to_lowercase();
    let detail_url = format!("/admin/md/categories?category_id={}", id);
    let pad = format!("padding-left: {}px", depth * 24 + 16);
    let is_active = selected_id == Some(id);
    let should_expand = is_active || expand_ids.contains(&id);

    html! {
        @if has_children {
            div.tree-node.expanded[should_expand] data-name=(name_lower) {
                div.tree-node-row.active[is_active]
                    style=(pad)
                    hx-get=(detail_url)
                    hx-select="#detail-panel" hx-target="#detail-panel" hx-swap="innerHTML"
                    hx-push-url="true"
                    _="on click take .active from .tree-node-row" {
                    span.tree-arrow _="on click halt the event then toggle .expanded on closest .tree-node" {
                        (icon::chevron_down_icon(""))
                    }
                    span class="tree-node-name" { (name) }
                    @if count > 0 {
                        span class="tree-node-count" { (count) }
                    }
                }
                div class="tree-children" {
                    @for child in &node.children {
                        (tree_node(child, depth + 1, selected_id, expand_ids))
                    }
                }
            }
        } @else {
            div.tree-node data-name=(name_lower) {
                div.tree-node-row.active[is_active]
                    style=(pad)
                    hx-get=(detail_url)
                    hx-select="#detail-panel" hx-target="#detail-panel" hx-swap="innerHTML"
                    hx-push-url="true"
                    _="on click take .active from .tree-node-row" {
                    span class="tree-arrow leaf" {
                        (icon::chevron_down_icon(""))
                    }
                    span class="tree-node-name" { (name) }
                    @if count > 0 {
                        span class="tree-node-count" { (count) }
                    }
                }
            }
        }
}
}

fn detail_panel(
    category: &Category,
    parent_name: &str,
    update_url: &str,
    delete_url: &str,
    child_tree: &[CategoryTree],
    products: &abt_core::shared::types::PaginatedResult<abt_core::master_data::category::ProductSummary>,
    category_id: i64,
    can_update: bool,
    can_delete: bool,
) -> Markup {
    use abt_core::master_data::product::ProductStatus;

    let has_children = !child_tree.is_empty();
    let has_products = !products.items.is_empty();
    let total_products = products.total;
    let total_pages = products.total_pages;
    let current_page = products.page;
    html! {
        div {
            // ── Category Info Card ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="cat-info-header" {
                    div {
                        div class="cat-info-title" { (category.category_name) }
                        div class="cat-info-path" {
                            "路径: " (category.path) " \u{00a0}·\u{00a0} 上级: " (parent_name)
                        }
                    }
                    div class="cat-info-actions" {
                        @if can_update {
                            button class="btn bg-white text-fg border border-border hover:bg-surface inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm [&_svg]:w-4 [&_svg]:h-4"
                                _="on click add .is-open to #edit-category-modal" {
                                (icon::edit_icon("w-4 h-4"))
                                "编辑"
                            }
                        }
                        @if can_delete {
                            button class="btn bg-white text-fg border border-border hover:bg-surface inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm [&_svg]:w-4 [&_svg]:h-4" style="color: var(--danger); border-color: var(--border);"
                                hx-post=(delete_url)
                                hx-confirm="确定要删除此分类吗？此操作不可撤销。"
                                hx-swap="none" {
                                (icon::trash_icon("w-4 h-4"))
                                "删除"
                            }
                        }
                    }
                }
                div class="cat-meta-grid" {
                    div class="cat-meta-item" {
                        span class="text-xs text-text-muted font-medium" { "分类名称" }
                        span class="text-sm text-fg font-medium" { (category.category_name) }
                    }
                    div class="cat-meta-item" {
                        span class="text-xs text-text-muted font-medium" { "分类路径" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (category.path) }
                    }
                    div class="cat-meta-item" {
                        span class="text-xs text-text-muted font-medium" { "上级分类" }
                        span class="text-sm text-fg font-medium" { (parent_name) }
                    }
                    div class="cat-meta-item" {
                        span class="text-xs text-text-muted font-medium" { "关联产品数" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (category.meta.count) }
                    }
                }
            }

            // ── Sub-categories ──
            @if has_children {
                div class="mb-5" {
                    div class="mb-5-header" {
                        div {
                            span class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" { "子分类" }
                            span class="mb-5-count" { "(" (child_tree.len()) ")" }
                        }
                    }
                    div class="subcat-grid" {
                        @for child in child_tree {
                            div class="subcat-card"
                                onclick=(format!("htmx.ajax('GET', '/admin/md/categories/{}/panel', '#detail-panel')", child.category_id)) {
                                span class="subcat-card-name" { (child.category_name) }
                                span class="subcat-card-count" { (child.meta.count) }
                            }
                        }
                    }
                }
            }

            // ── Associated Products ──
            div class="mb-5" id="products-section"
                hx-select="#products-section" hx-target="#products-section"
                hx-swap="outerHTML" hx-push-url="true" {
                div class="mb-5-header" {
                    div {
                        span class="text-[13px] font-semibold text-fg flex items-center gap-[6px]" { "关联产品" }
                        span class="mb-5-count" { "(" (total_products) ")" }
                    }
                }
                @if has_products {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="border: 1px solid var(--border-soft); border-radius: var(--radius-md);" {
                        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                            table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" style="min-width: 0;" {
                                thead {
                                    tr {
                                        th { "产品编码" }
                                        th { "产品名称" }
                                        th { "状态" }
                                    }
                                }
                                tbody {
                                    @for p in &products.items {
                                        tr {
                                            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" { (p.product_code) }
                                            td style="max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title=(p.pdt_name) { strong { (p.pdt_name) } }
                                            td {
                                                @match p.status {
                                                    ProductStatus::Active => {
                                                        span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#f0fff0] text-[#389e0d]" { "在用" }
                                                    }
                                                    ProductStatus::Inactive => {
                                                        span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-surface text-text-muted" { "停用" }
                                                    }
                                                    ProductStatus::Obsolete => {
                                                        span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff2f0] text-[#cf1322]" { "淘汰" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        @if total_pages > 1 {
                            (htmx_pagination_inherited(
                                &format!("/admin/md/categories?category_id={}", category_id),
                                total_products, current_page, total_pages,
                            ))
                        }
                    }
                } @else {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-empty" {
                        "暂无关联产品"
                    }
                }
            }

            // ── Edit Modal ──
            (modal::modal(
                "edit-category-modal",
                "编辑分类",
                "保存",
                "edit-category-form",
                update_url,
                html! {
                    div class="form-field" {
                        label { "分类名称" }
                        input type="text" name="category_name"
                            value=(category.category_name) required;
                    }
                },
            ))

        }
    }
}


// ── Create Category Modal ──

fn create_category_modal(tree: &[CategoryTree], can_create: bool) -> Markup {
    if !can_create {
        return html! {};
    }
    // TODO: Surreal.js migration - modal tied to Alpine categorySplitView component
    modal::modal(
        "create-modal",
        "新建分类",
        "保存分类",
        "create-category-form",
        CategoryCreatePath::PATH,
        html! {
            div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                div class="form-field" {
                    label { "分类名称 " span style="color:var(--danger)" { "*" } }
                    input type="text" name="category_name"
                        placeholder="请输入分类名称" required;
                }
                div class="form-field" {
                    label { "上级分类" }
                    select name="parent_id" {
                        option value="0" { "无 (顶级分类)" }
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
