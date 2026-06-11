use maud::{Markup, html};

/// Generic modal component (Surreal.js open/close + HTMX body).
///
/// `modal_id` — HTML id of the overlay div; callers toggle `.is-open` on it via Surreal.js helpers.
/// `title` — modal title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button in footer can reference it.
/// `body` — form content slot.
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, hx_post: &str, body: Markup) -> Markup {
    let close_click = format!("hsRemoveClosest(this,'.modal-overlay','is-open');me('#{}').reset()", form_id);
    let after_request = "if(event.detail.xhr.status < 400){hsRemoveClosest(this,'.modal-overlay','is-open');this.reset()}".to_string();
    html! {
        div id=(modal_id) class="modal-overlay" onclick="hsBackdropClose(this,event,'is-open')" {
            form id=(form_id) class="modal" hx-post=(hx_post) hx-swap="none"
                hx-on::after-request=(after_request) {
                div class="modal-head" {
                    h2 { (title) }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        onclick=(close_click.clone()) { "×" }
                }
                div class="modal-body" {
                    (body)
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        onclick=(close_click) { "取消" }
                    button type="submit" class="btn btn-primary" { (submit_label) }
                }
            }
        }
    }
}
