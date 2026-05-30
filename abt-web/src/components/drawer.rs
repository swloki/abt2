use maud::{Markup, html};

/// Generic drawer component (Alpine.js open/close + HTMX body).
///
/// Mirrors the `modal` component API: built-in footer with cancel/submit buttons.
/// The submit button targets a `<form>` inside body via HTML5 `form` attribute.
///
/// `open_var`    — Alpine reactive boolean controlling visibility.
/// `title`       — drawer title.
/// `submit_label` — text for the primary submit button.
/// `form_id`     — `id` of the `<form>` inside body, so the submit button can reference it.
/// `body`        — content slot (rendered inside drawer-body).
pub fn drawer(open_var: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div class="drawer-overlay"
            x-bind:class=(format!("{{ 'open': {} }}", open_var))
            x-on:click=(format!("if(event.target===this) {} = false", open_var)) {
            div class="drawer" x-on:click="event.stopPropagation()" {
                div class="drawer-head" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                        x-on:click=(format!("{} = false", open_var)) { "×" }
                }
                div class="drawer-body" {
                    (body)
                }
                div class="drawer-foot" {
                    button type="button" class="btn btn-default"
                        x-on:click=(format!("{} = false", open_var)) { "取消" }
                    button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}
