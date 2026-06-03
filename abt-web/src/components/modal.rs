use maud::{Markup, html};

/// Generic modal component (Hyperscript open/close + HTMX body).
///
/// `modal_id` — HTML id of the overlay div; callers toggle `.is-open` on it via Hyperscript.
/// `title` — modal title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button in footer can reference it.
/// `body` — form content slot.
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, hx_post: &str, body: Markup) -> Markup {
    let close_hs = format!("on click remove .is-open from closest .modal-overlay then call #{}.reset()", form_id);
    html! {
        div id=(modal_id) class="modal-overlay" _="on click if event.target is me remove .is-open" {
            form id=(form_id) class="modal" hx-post=(hx_post) hx-swap="none"
                _={"on submit remove .is-open from closest .modal-overlay then call me.reset()"} {
                div class="modal-head" {
                    h2 { (title) }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _=(close_hs.clone()) { "×" }
                }
                div class="modal-body" {
                    (body)
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        _=(close_hs) { "取消" }
                    button type="submit" class="btn btn-primary" { (submit_label) }
                }
            }
        }
    }
}
