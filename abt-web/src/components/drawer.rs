use maud::{Markup, html};

/// Generic drawer component (Hyperscript open/close + HTMX body).
///
/// Mirrors the `modal` component API: built-in footer with cancel/submit buttons.
/// The submit button targets a `<form>` inside body via HTML5 `form` attribute.
///
/// `drawer_id`   — HTML id of the overlay div; callers toggle `.open` on it via Hyperscript.
/// `title`       — drawer title.
/// `submit_label` — text for the primary submit button.
/// `form_id`     — `id` of the `<form>` inside body, so the submit button can reference it.
/// `body`        — content slot (rendered inside drawer-body).
pub fn drawer(drawer_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div id=(drawer_id) class="drawer-overlay" _="on click if event.target is me remove .open" {
            div class="drawer" _="on click halt the event" {
                div class="drawer-head" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                        _="on click remove .open from closest .drawer-overlay" { "×" }
                }
                div class="drawer-body" {
                    (body)
                }
                div class="drawer-foot" {
                    button type="button" class="btn btn-default"
                        _="on click remove .open from closest .drawer-overlay" { "取消" }
                    button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}

/// Drawer variant with custom footer content.
///
/// `drawer_id` — HTML id of the overlay div; callers toggle `.open` on it via Hyperscript.
/// `title` — drawer title.
/// `body` — content slot (rendered inside drawer-body).
/// `footer` — custom footer content.
pub fn drawer_with_footer(drawer_id: &str, title: &str, body: Markup, footer: Markup) -> Markup {
    html! {
        div id=(drawer_id) class="drawer-overlay" _="on click if event.target is me remove .open" {
            div class="drawer" _="on click halt the event" {
                div class="drawer-head" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                        _="on click remove .open from closest .drawer-overlay" { "×" }
                }
                div class="drawer-body" {
                    (body)
                }
                div class="drawer-foot" {
                    (footer)
                }
            }
        }
    }
}
