use abt_core::master_data::category::model::CategoryTree;
use maud::{html, Markup};
use serde::Serialize;

/// A reusable searchable, indented tree-select component for categories.
///
/// Embeds category data as JSON in a `data-ct` attribute.
/// The vanilla JS component reads it via `this.$el.dataset.ct` and handles
/// all rendering and filtering client-side.
///
/// The hidden `<input>` fires a `change` event on selection — works with
/// HTMX form-based triggers (`hx-trigger="change"` on the parent form).
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

    html! {
        // TODO: Rewrite categoryTreeSelect to vanilla JS (was Alpine.js x-data component)
        // Currently non-functional — the tree-select component needs a vanilla JS rewrite.
        // The data is embedded in data-ct; the rendering/search/selection logic needs to be
        // reimplemented without Alpine.js.
        div class="tree-select" data-ct=(json) {

            input type="hidden" name=(input_name)
                value=(selected_id.map(|id| id.to_string()).unwrap_or_default());

            button type="button" class="tree-select-trigger"
                id="tree-select-trigger" {
                span class="tree-select-value" {
                    (selected_id.and_then(|id| {
                        categories.iter().find(|c| c.category_id == id)
                            .map(|c| c.category_name.as_str())
                    }).unwrap_or(all_label))
                }
                span class="tree-select-arrow" {}
            }

            div class="tree-select-backdrop" style="display:none" {}

            div class="tree-select-dropdown" style="display:none" {
                div class="tree-select-search" {
                    input type="text"
                        placeholder="搜索分类…"
                        class="tree-select-search-input" {}
                }
                div class="tree-select-list" {
                    button type="button" class="tree-select-option"
                        style="padding-left:var(--space-3)" {
                        span { (all_label) }
                    }

                    @for cat in flatten_tree(categories, 0) {
                        button type="button"
                            class="tree-select-option"
                            data-id=(cat.id)
                            style=(format!("padding-left: calc({} * 20px + var(--space-3))", cat.depth)) {
                            span { (cat.name) }
                        }
                    }
                }
            }
        }
    }
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
