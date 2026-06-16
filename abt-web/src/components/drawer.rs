use maud::{Markup, html};

/// Generic drawer component (Hyperscript `_` attribute for open/close + HTMX body).
///
/// Mirrors the `modal` component API: built-in footer with cancel/submit buttons.
/// The submit button targets a `<form>` inside body via HTML5 `form` attribute.
///
/// `drawer_id`   — HTML id of the overlay div; callers toggle `.open` via Hyperscript.
/// `title`       — drawer title.
/// `submit_label` — text for the primary submit button.
/// `form_id`     — `id` of the `<form>` inside body, so the submit button can reference it.
/// `body`        — content slot (rendered inside drawer-body).
pub fn drawer(drawer_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div id=(drawer_id) class="drawer-overlay"
            _="on click[me is event.target] remove .open" {
            div class="drawer-panel" onclick="event.stopPropagation()" {
                div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                        _="on click remove .open from closest .drawer-overlay" { "×" }
                }
                div class="flex-1 overflow-y-auto p-6" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
                    button type="button" class="btn bg-white text-fg border border-border hover:bg-surface"
                        _="on click remove .open from closest .drawer-overlay" { "取消" }
                    button type="submit" class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}

/// Drawer variant with custom footer content.
///
/// `drawer_id` — HTML id of the overlay div; callers toggle `.open` via Hyperscript.
/// `title` — drawer title.
/// `body` — content slot (rendered inside drawer-body).
/// `footer` — custom footer content.
pub fn drawer_with_footer(drawer_id: &str, title: &str, body: Markup, footer: Markup) -> Markup {
    html! {
        div id=(drawer_id) class="drawer-overlay"
            _="on click[me is event.target] remove .open" {
            div class="drawer-panel" onclick="event.stopPropagation()" {
                div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
                    h2 { (title) }
                    button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                        _="on click remove .open from closest .drawer-overlay" { "×" }
                }
                div class="flex-1 overflow-y-auto p-6" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
                    (footer)
                }
            }
        }
    }
}
