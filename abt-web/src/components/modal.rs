use maud::{Markup, html};

/// Generic modal component (Hyperscript `_` attribute for open/close + HTMX body).
///
/// `modal_id` — HTML id of the overlay div; callers toggle `.is-open` via Hyperscript.
/// `title` — modal title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button in footer can reference it.
/// `body` — form content slot.
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, hx_post: &str, body: Markup) -> Markup {
    html! {
        div id=(modal_id) class="modal-overlay"
            _="on click[me is event.target] remove .is-open" {
            form id=(form_id) class="modal" hx-post=(hx_post) hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { (title) }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                    button type="button" class="btn btn-default"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "取消" }
                    button type="submit" class="btn btn-primary" { (submit_label) }
                }
            }
        }
    }
}
