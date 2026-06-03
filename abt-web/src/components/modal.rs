use maud::{Markup, html};

/// Generic modal component (Hyperscript open/close + HTMX body).
///
/// `modal_id` — HTML id of the overlay div; callers toggle `.is-open` on it via Hyperscript.
/// `title` — modal title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button in footer can reference it.
/// `body` — form content slot.
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div id=(modal_id) class="modal-overlay" _="on click remove .is-open" {
            div class="modal" _="on click halt the event" {
                div class="modal-head" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from closest .modal-overlay" { "×" }
                }
                div class="modal-body" {
                    (body)
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        _="on click remove .is-open from closest .modal-overlay" { "取消" }
                    button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}
