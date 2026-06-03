use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::category::model::*;
use abt_core::master_data::category::CategoryService;

use crate::components::icon;
use crate::components::pagination::htmx_pagination;
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

fn default_page() -> u32 { 1 }

// ── Handlers ──

#[require_permission("CATEGORY", "read")]
pub async fn get_category_list(
    _path: CategoryListPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
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

    // Pre-render the first category's detail panel
    let first_panel = if let Some(first) = tree.first() {
        let first_id = first.category_id;
        let category = svc.get(&service_ctx, &mut conn, first_id).await.ok();
        if let Some(cat) = category {
            let parent_name = if cat.parent_id != 0 {
                svc.get(&service_ctx, &mut conn, cat.parent_id).await.map(|p| p.category_name.clone()).unwrap_or_else(|_| "—".into())
            } else {
                "—".to_string()
            };
            let update_url = CategoryUpdatePath { id: first_id }.to_string();
            let delete_url = CategoryDeletePath { id: first_id }.to_string();
            let mut child_tree = svc.get_tree(&service_ctx, &mut conn, Some(first_id), None).await.unwrap_or_default();
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
            let page = abt_core::shared::types::PageParams::new(1, 5);
            let products = svc.list_products(&service_ctx, &mut conn, first_id, page).await
                .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult::empty(1, 5));
            Some((detail_panel(&cat, &parent_name, &update_url, &delete_url, &child_tree, &products, first_id), first_id))
        } else {
            None
        }
    } else {
        None
    };

    let content = category_page(&tree, first_panel.as_ref().map(|(p, _)| p), first_panel.as_ref().map(|(_, id)| *id));
    let page_html = admin_page(
        is_htmx,
        "产品分类",
        &claims,
        "md",
        CategoryListPath::PATH,
        "主数据管理",
        Some("产品分类"),
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
    Query(query): Query<PanelQuery>,
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
        detail_panel(&category, &parent_name, &update_url, &delete_url, &child_tree, &products, path.id).into_string(),
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
        .tree-arrow.expanded svg {
            transform: rotate(90deg);
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

fn category_page(tree: &[CategoryTree], initial_panel: Option<&Markup>, first_id: Option<i64>) -> Markup {
    let selected_expr = first_id.map_or("null".to_string(), |id| id.to_string());
    html! {
        div x-data=(format!("categorySplitView()")) x-init=(format!("selectedId = {}", selected_expr)) {
            (split_view_style())
            script { (category_split_view_script()) }

            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "产品分类" }
                div class="page-actions" {
                    button class="btn btn-default" {
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
                        div class="tree-search" {
                            (icon::search_icon("search-icon"))
                            input type="text" placeholder="搜索分类…"
                                x-model="searchText"
                                x-on:input="filterTree($event)";
                        }
                    }
                    div class="tree-scroll" id="category-tree" {
                        (tree_fragment(tree))
                    }
                    div class="tree-footer" {
                        button class="btn btn-primary" style="width: 100%; justify-content: center;"
                            _="on click add .is-open to #create-modal" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建分类"
                        }
                    }
                }

                // ── Right Panel: Detail ──
                div class="detail-panel" id="detail-panel" {
                    @if let Some(panel) = initial_panel {
                        (panel)
                    } @else {
                        div class="empty-state" {
                            svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" {
                                path d="M4 20h16M8 16h8M6 12h12M10 8h4M12 4v16" {}
                            }
                            div class="empty-state-text" { "请从左侧选择一个分类" }
                            div class="empty-state-hint" { "选择分类查看详情和管理关联产品" }
                        }
                    }
                }
            }

            // ── Create Modal ──
            (create_category_modal(tree))
        }
    }
}

// TODO: hyperscript migration - complex Alpine pattern (categorySplitView with x-init, x-model, filterTree)

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
                filterTree(ev) {
                    var q = (ev ? ev.target.value : this.searchText).trim().toLowerCase();
                    var container = document.getElementById('category-tree');
                    if (!container) return;
                    var allNodes = container.querySelectorAll('.tree-node');
                    if (!q) {
                        // Reset: show all nodes
                        for (var i = 0; i < allNodes.length; i++) {
                            allNodes[i].style.display = '';
                            var ch = allNodes[i].querySelector(':scope > .tree-children');
                            if (ch) ch.style.display = '';
                            var arr = allNodes[i].querySelector(':scope > .tree-node-row > .tree-arrow');
                            if (arr) arr.classList.add('expanded');
                        }
                        return;
                    }
                    // First pass: mark each node as matching or not
                    for (var i = 0; i < allNodes.length; i++) {
                        var name = (allNodes[i].getAttribute('data-name') || '').toLowerCase();
                        allNodes[i]._matches = (name.indexOf(q) >= 0);
                    }
                    // Second pass: propagate child matches upward
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
                    // Third pass: apply visibility
                    for (var i = 0; i < allNodes.length; i++) {
                        allNodes[i].style.display = allNodes[i]._matches ? '' : 'none';
                        if (allNodes[i]._matches) {
                            var ch = allNodes[i].querySelector(':scope > .tree-children');
                            if (ch) ch.style.display = '';
                            var arr = allNodes[i].querySelector(':scope > .tree-node-row > .tree-arrow');
                            if (arr) arr.classList.add('expanded');
                        }
                        delete allNodes[i]._matches;
                    }
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
    let class_expr = format!("{{'active': selectedId === {}}}", id);
    let pad = format!("padding-left: {}px", depth * 24 + 16);

    html! {
        @if has_children {
            div.tree-node data-name=(name_lower) {
                div.tree-node-row
                    style=(pad)
                    x-on:click=(click_expr)
                    x-bind:class=(class_expr) {
                    span.tree-arrow
                        onclick="event.stopPropagation()"
                        _="on click toggle .expanded on me then if (me matches .expanded) set (closest .tree-node .tree-children).style.display to '' else set (closest .tree-node .tree-children).style.display to 'none'" {
                        (icon::chevron_down_icon(""))
                    }
                    span class="tree-node-name" { (name) }
                    @if count > 0 {
                        span class="tree-node-count" { (count) }
                    }
                }
                div class="tree-children" style="display:none" {
                    @for child in &node.children {
                        (tree_node(child, depth + 1))
                    }
                }
            }
        } @else {
            div.tree-node data-name=(name_lower) {
                div.tree-node-row
                    style=(pad)
                    x-on:click=(click_expr)
                    x-bind:class=(class_expr) {
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
) -> Markup {
    use abt_core::master_data::product::ProductStatus;

    let has_children = !child_tree.is_empty();
    let has_products = !products.items.is_empty();
    let total_products = products.total;
    let total_pages = products.total_pages;
    let current_page = products.page;
    let panel_url = format!("/admin/md/categories/{}/panel", category_id);
    html! {
        div {
            // ── Category Info Card ──
            div class="info-card" {
                div class="cat-info-header" {
                    div {
                        div class="cat-info-title" { (category.category_name) }
                        div class="cat-info-path" {
                            "路径: " (category.path) " \u{00a0}·\u{00a0} 上级: " (parent_name)
                        }
                    }
                    div class="cat-info-actions" {
                        button class="btn btn-default btn-sm"
                            _="on click add .is-open to #edit-category-modal" {
                            (icon::edit_icon("w-4 h-4"))
                            "编辑"
                        }
                        button class="btn btn-default btn-sm" style="color: var(--danger); border-color: var(--border);"
                            _="on click add .open to #delete-category-dialog" {
                            (icon::trash_icon("w-4 h-4"))
                            "删除"
                        }
                    }
                }
                div class="cat-meta-grid" {
                    div class="cat-meta-item" {
                        span class="info-label" { "分类名称" }
                        span class="info-value" { (category.category_name) }
                    }
                    div class="cat-meta-item" {
                        span class="info-label" { "分类路径" }
                        span class="info-value mono" { (category.path) }
                    }
                    div class="cat-meta-item" {
                        span class="info-label" { "上级分类" }
                        span class="info-value" { (parent_name) }
                    }
                    div class="cat-meta-item" {
                        span class="info-label" { "关联产品数" }
                        span class="info-value mono" { (category.meta.count) }
                    }
                }
            }

            // ── Sub-categories ──
            @if has_children {
                div class="detail-section" {
                    div class="detail-section-header" {
                        div {
                            span class="detail-section-title" { "子分类" }
                            span class="detail-section-count" { "(" (child_tree.len()) ")" }
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
            div class="detail-section" {
                div class="detail-section-header" {
                    div {
                        span class="detail-section-title" { "关联产品" }
                        span class="detail-section-count" { "(" (total_products) ")" }
                    }
                }
                @if has_products {
                    div class="data-card" style="border: 1px solid var(--border-soft); border-radius: var(--radius-md);" {
                        div class="data-card-scroll" {
                            table class="data-table" style="min-width: 0;" {
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
                                            td class="link-cell mono" { (p.product_code) }
                                            td style="max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title=(p.pdt_name) { strong { (p.pdt_name) } }
                                            td {
                                                @match p.status {
                                                    ProductStatus::Active => {
                                                        span class="status-pill status-success" { "在用" }
                                                    }
                                                    ProductStatus::Inactive => {
                                                        span class="status-pill status-draft" { "停用" }
                                                    }
                                                    ProductStatus::Obsolete => {
                                                        span class="status-pill status-danger" { "淘汰" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        @if total_pages > 1 {
                            (htmx_pagination(&panel_url, total_products, current_page, total_pages, "#detail-panel", "innerHTML"))
                        }
                    }
                } @else {
                    div class="data-card-empty" {
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

            // ── Delete Confirm Dialog (inline, no x-teleport) ──
            div class="dialog-overlay" id="delete-category-dialog"
                _="on click remove .open" {
                div class="dialog" _="on click halt the event" {
                    div class="dialog-body" {
                        div class="dialog-icon-wrap" {
                            (icon::circle_alert_icon("w-7 h-7"))
                        }
                        div class="dialog-title" { "删除分类" }
                        p class="dialog-desc" {
                            "确定要删除分类 "
                            strong { (category.category_name) }
                            " 吗？此操作不可撤销。"
                        }
                    }
                    div class="dialog-foot" {
                        button type="button" class="btn btn-default"
                            _="on click remove .open from #delete-category-dialog" { "取消" }
                        button type="submit" class="btn btn-danger"
                            form="delete-category-form" { "确认删除" }
                    }
                }
                form id="delete-category-form" style="display:none"
                    hx-post=(delete_url) {}
            }
        }
    }
}


// ── Create Category Modal ──

fn create_category_modal(tree: &[CategoryTree]) -> Markup {
    // TODO: hyperscript migration - modal tied to Alpine categorySplitView component
    modal::modal(
        "create-modal",
        "新建分类",
        "保存分类",
        "create-category-form",
        html! {
            form id="create-category-form"
                hx-post=(CategoryCreatePath::PATH) {
                div class="form-grid" {
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
