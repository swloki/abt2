use maud::{Markup, html};

/// Generic modal component (Alpine.js open/close + HTMX body).
///
/// `open_var` — Alpine reactive boolean controlling visibility (e.g. `"createModalOpen"`).
/// `title` — modal title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button in footer can reference it.
/// `body` — form content slot.
pub fn modal(open_var: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div class="modal-overlay"
            x-bind:class=(format!("{{ 'is-open': {} }}", open_var))
            x-on:click=(format!("{} = false", open_var)) {
            div class="modal" x-on:click="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        x-on:click=(format!("{} = false", open_var)) { "×" }
                }
                div class="modal-body" {
                    (body)
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        x-on:click=(format!("{} = false", open_var)) { "取消" }
                    button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}
