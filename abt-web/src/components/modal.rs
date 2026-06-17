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
        div id=(modal_id) class="fixed z-[1000] grid place-items-center opacity-0"
            _="on click[me is event.target] remove .is-open" {
            form id=(form_id) class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" hx-post=(hx_post) hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me" {
                div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
                    h2 { (title) }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { (submit_label) }
                }
            }
        }
    }
}
