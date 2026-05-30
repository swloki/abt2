use abt_core::master_data::category::model::CategoryTree;
use maud::{html, Markup};
use serde::Serialize;

/// A reusable searchable, indented tree-select component for categories.
///
/// Embeds category data as JSON in a `data-ct` attribute. The Alpine.js
/// component in `app.js` reads it via `this.$el.dataset.ct` and handles
/// all rendering and filtering client-side with `x-for`.
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
        div class="tree-select" x-data="categoryTreeSelect()" data-ct=(json) {

            input type="hidden" name=(input_name)
                value=(selected_id.map(|id| id.to_string()).unwrap_or_default());

            button type="button" class="tree-select-trigger"
                x-on:click="toggle()" {
                span class="tree-select-value" x-text="selectedName" { }
                span class="tree-select-arrow" {}
            }

            div class="tree-select-backdrop" x-show="open" x-cloak
                x-on:click="close()" {}

            div class="tree-select-dropdown" x-show="open && items.length > 0" x-cloak
                x-transition {

                div class="tree-select-search" {
                    input type="text" x-model="search"
                        placeholder="搜索分类…"
                        class="tree-select-search-input" {}
                }

                div class="tree-select-list" {
                    button type="button" class="tree-select-option"
                        x-show="!search"
                        x-on:click="select('')" {
                        span x-text="allLabel" { }
                    }

                    template x-for="item in filteredItems" {
                        button type="button"
                            class="tree-select-option"
                            x-bind:class="{ 'is-selected': item.id == selectedId }"
                            x-on:click="select(item.id)"
                            x-bind:style="'padding-left: calc(' + item.depth + ' * 20px + var(--space-3))'" {
                            span x-text="item.name" { }
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
