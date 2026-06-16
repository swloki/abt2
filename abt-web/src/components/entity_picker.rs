use maud::{html, Markup};

/// Configuration for an entity picker modal.
pub struct EntityPickerConfig<'a> {
    /// Modal overlay element id, e.g. `"wo-picker"`.
    pub modal_id: &'a str,
    /// Modal title, e.g. `"选择工单"`.
    pub title: &'a str,
    /// Search field label, e.g. `"关键词"`.
    pub search_label: &'a str,
    /// Search input placeholder.
    pub search_placeholder: &'a str,
    /// HTMX search endpoint URL, e.g. `"/admin/mes/receipts/search-wo"`.
    pub search_path: &'a str,
    /// Query parameter name for the search keyword, usually `"q"`.
    pub search_param: &'a str,
    /// Hidden `<input>` element id that receives the selected entity id.
    pub target_id: &'a str,
    /// Display element id that shows the selected entity label.
    pub display_id: &'a str,
    /// Custom event name fired on selection (use `""` to skip).
    pub event_name: &'a str,
    /// Extra `hx-include` selector for cascade context, e.g. `Some("#work_order_id")`.
    pub extra_include: Option<&'a str>,
}

/// A single result item in the picker list.
pub struct EntityPickerItem {
    pub id: i64,
    pub label: String,
    pub sub_label: Option<String>,
    /// When `true`, the item is shown greyed-out and not clickable.
    pub disabled: bool,
}

impl EntityPickerItem {
    pub fn new(id: i64, label: impl Into<String>) -> Self {
        Self { id, label: label.into(), sub_label: None, disabled: false }
    }
    pub fn sub(mut self, s: impl Into<String>) -> Self {
        self.sub_label = Some(s.into());
        self
    }
    #[allow(dead_code)]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

// ── Field (hidden input + display + button) ──

/// Renders the form field portion: hidden input, clickable display area, and "选择" button.
///
/// Place `entity_picker_modal` elsewhere on the page to provide the actual modal.
pub fn entity_picker_field(
    name: &str,
    target_id: &str,
    display_id: &str,
    modal_id: &str,
    label: &str,
    required: bool,
    placeholder: &str,
) -> Markup {
    let open_hs = format!(
        "on click add .is-open to #{} then send openModal to #{}-results",
        modal_id, modal_id
    );
    html! {
        div class="form-field" {
            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                (label)
                @if required { span class="required" { "*" } }
            }
            div class="search-select" {
                input type="hidden" name=(name) id=(target_id);
                div class="search-select-display placeholder" id=(display_id)
                    _=(open_hs.as_str()) {
                    (placeholder)
                }
                button type="button" class="search-select-btn"
                    _=(open_hs.as_str()) {
                    "选择"
                }
            }
        }
    }
}

// ── Modal ──

/// Renders the search modal overlay. Embed once per picker on the page.
pub fn entity_picker_modal(cfg: &EntityPickerConfig) -> Markup {
    let open_hs = format!("on click[me is event.target] remove .is-open from #{}", cfg.modal_id);

    html! {
        div class="modal-overlay" id=(cfg.modal_id) _=(open_hs) {
            div class="modal modal-lg" _="on click halt" {
                div class="modal-head" {
                    h2 { (cfg.title) }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--text-muted);padding:4px"
                        _=(format!("on click remove .is-open from #{}", cfg.modal_id)) { "×" }
                }
                div class="modal-body" style="padding:0" {
                    // Hidden context for results fragment
                    input type="hidden" name="target_id" value=(cfg.target_id);
                    input type="hidden" name="display_id" value=(cfg.display_id);
                    input type="hidden" name="modal_id" value=(cfg.modal_id);
                    input type="hidden" name="event_name" value=(cfg.event_name);

                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { (cfg.search_label) }
                            input class="product-search-input" type="text"
                                name=(cfg.search_param)
                                placeholder=(cfg.search_placeholder)
                                autocomplete="off"
                                hx-get=(cfg.search_path)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target=(format!("#{}-results", cfg.modal_id))
                                hx-swap="innerHTML"
                                hx-include=(hx_include_expr(cfg)) {}
                        }
                    }
                    div id=(format!("{}-results", cfg.modal_id))
                        style="max-height:360px;overflow-y:auto"
                        hx-get=(cfg.search_path)
                        hx-trigger="openModal"
                        hx-swap="innerHTML"
                        hx-include=(hx_include_expr(cfg)) {
                        div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--text-muted)" {
                            "加载中…"
                        }
                    }
                }
            }
        }
    }
}

/// Build the hx-include expression: always include the modal itself
/// (for hidden context inputs), plus optional extra selectors for cascade.
fn hx_include_expr(cfg: &EntityPickerConfig) -> String {
    match cfg.extra_include {
        Some(sel) => format!("#{}, {}", cfg.modal_id, sel),
        None => format!("#{}", cfg.modal_id),
    }
}

// ── Results fragment ──


/// Renders the results list returned by the search endpoint.
///
/// Each item, when clicked:
/// 1. Sets the hidden input value to `item.id`
/// 2. Sets the display element text to `item.label`
/// 3. Closes the modal
/// 4. Fires the custom event (if configured)
pub fn entity_picker_results(items: &[EntityPickerItem]) -> Markup {
    html! {
        @if items.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--text-muted)" {
                p style="margin:0;font-size:var(--text-sm)" { "未找到匹配结果" }
            }
        } @else {
            div class="product-select-list" {
                @for item in items {
                    @if item.disabled {
                        div class="product-select-item"
                            style="opacity:0.45;cursor:not-allowed"
                            data-id=(item.id)
                            data-label=(item.label.as_str()) {
                            div class="product-select-info" {
                                div class="product-select-name" { (item.label.as_str()) }
                                @if let Some(ref sub) = item.sub_label {
                                    div class="product-select-meta" { (sub.as_str()) }
                                }
                            }
                        }
                    } @else {
                        div class="product-select-item"
                            data-id=(item.id)
                            data-label=(item.label.as_str())
                            _=(selection_hs()) {
                            div class="product-select-info" {
                                div class="product-select-name" { (item.label.as_str()) }
                                @if let Some(ref sub) = item.sub_label {
                                    div class="product-select-meta" { (sub.as_str()) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn selection_hs() -> &'static str {
    "on click call entityPickerSelect(me)"
}
