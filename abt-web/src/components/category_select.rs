use abt_core::master_data::category::model::CategoryTree;
use maud::{Markup, html};
use serde::Serialize;

/// A reusable searchable, indented tree-select component for categories.
///
/// Renders as a button that looks like a `<select>`, with a dropdown panel
/// containing a search box and indented category options. Vanilla JS handles
/// show/hide, search filtering, and selection. The hidden `<input>` fires a
/// `change` event on selection — works with HTMX form-based triggers.
pub fn category_tree_select(
    categories: &[CategoryTree],
    selected_id: Option<i64>,
    input_name: &str,
    all_label: &str,
) -> Markup {
    let payload = CategorySelectPayload {
        items: flatten_tree(categories, 0),
        selected_id,
        all_label: all_label.to_string(),
    };
    let json = serde_json::to_string(&payload).unwrap_or_default();

    let current_label = selected_id
        .and_then(|id| find_category_name(categories, id))
        .unwrap_or_else(|| all_label.to_string());

    html! {
        div class="cat-select relative" data-ct=(json) {

            input
                type="hidden"
                name=(input_name)
                value=(selected_id.map(|id| id.to_string()).unwrap_or_default());
            // Trigger button (looks like a select)
            button
                type="button"
                class="cat-trigger w-40 flex items-center justify-between gap-2 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer hover:border-[rgba(37,99,235,0.3)]"
                _="on click if next .cat-dropdown's style's display is 'none' then show next .cat-dropdown then show next .cat-backdrop then call focus(first <input/> in next .cat-dropdown) else hide next .cat-dropdown then hide next .cat-backdrop"
            {
                span class="cat-label truncate flex-1 text-left" { (current_label) }
                svg class="w-4 h-4 text-muted shrink-0"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                {
                    path d="M19 9l-7 7-7-7" {}
                }
            }
            // Backdrop
            div class="cat-backdrop fixed inset-0 z-[999]"
                style="display:none"
                _="on click hide next .cat-dropdown then hide me" {}
            // Dropdown panel
            div class="cat-dropdown absolute top-full left-0 mt-1 w-80 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-[1000]"
                style="display:none"
            {
                div class="p-2 border-b border-border-soft" {
                    input
                        type="text"
                        placeholder="搜索分类…"
                        class="cat-search w-full px-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        _="on input call filterCatOptions(me)" {}
                }
                // Options list
                div class="cat-list max-h-[280px] overflow-y-auto py-1" {
                    button
                        type="button"
                        class="cat-option w-full text-left px-3 py-1.5 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors cursor-pointer border-none bg-transparent"
                        data-id=""
                        _="on click call selectCat(me)"
                    { (all_label) }

                    @for cat in &payload.items {
                        button
                            type="button"
                            class="cat-option w-full text-left py-1.5 pr-3 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors cursor-pointer border-none bg-transparent"
                            data-id=(cat.id)
                            data-name=(cat.name)
                            style=(format!("padding-left: {}px", cat.depth * 20 + 12))
                            _="on click call selectCat(me)"
                        { (cat.name) }
                    }
                }
            }
        }
    }
}

fn find_category_name(categories: &[CategoryTree], id: i64) -> Option<String> {
    for cat in categories {
        if cat.category_id == id {
            return Some(cat.category_name.clone());
        }
        if let Some(name) = find_category_name(&cat.children, id) {
            return Some(name);
        }
    }
    None
}

#[derive(Serialize)]
struct CategorySelectPayload {
    items: Vec<CategoryItem>,
    selected_id: Option<i64>,
    all_label: String,
}

#[derive(Serialize)]
struct CategoryItem {
    id: i64,
    name: String,
    depth: u32,
}

fn flatten_tree(categories: &[CategoryTree], depth: u32) -> Vec<CategoryItem> {
    let mut result = Vec::new();
    for cat in categories {
        result.push(CategoryItem {
            id: cat.category_id,
            name: cat.category_name.clone(),
            depth,
        });
        result.extend(flatten_tree(&cat.children, depth + 1));
    }
    result
}
