use maud::{Markup, html};

/// Generic drawer component (Hyperscript `_` attribute for open/close + HTMX body).
///
/// Mirrors the `modal` component API: built-in footer with cancel/submit buttons.
/// The submit button targets a `<form>` inside body via HTML5 `form` attribute.
///
/// `drawer_id` — HTML id of the overlay div; callers toggle `.open` via Hyperscript.
/// `title` — drawer title.
/// `submit_label` — text for the primary submit button.
/// `form_id` — `id` of the `<form>` inside body, so the submit button can reference it.
/// `body` — content slot (rendered inside drawer-body).
pub fn drawer(drawer_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
 html! {
 div id=(drawer_id) class="drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]"
 _="on click[me is event.target] remove .open from me" {
 div class="drawer-panel bg-white h-full w-[420px] flex flex-col" _="on click js(event) event.stopPropagation() end" {
 div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
 h2 { (title) }
 button type="button" class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
 _="on click remove .open from closest .drawer-overlay" { "×" }
 }
 div class="flex-1 overflow-y-auto p-6" {
 (body)
 }
 div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from closest .drawer-overlay" { "取消" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" _=(format!("on click trigger submit on #{}", form_id)) { (submit_label) }
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
 div id=(drawer_id) class="drawer-overlay fixed inset-0 z-[1000] flex justify-end bg-[rgba(0,0,0,0.35)]"
 _="on click[me is event.target] remove .open from me" {
 div class="drawer-panel bg-white h-full w-[420px] flex flex-col" _="on click js(event) event.stopPropagation() end" {
 div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
 h2 { (title) }
 button type="button" class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
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
