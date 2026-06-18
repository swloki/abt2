use maud::{Markup, html};

use super::icon;

/// Confirm dialog with warning icon, title, description, and cancel/confirm buttons.
///
/// `dialog_id` — unique id for the dialog overlay element (e.g. `"bom-delete-dialog"`).
/// `title` — dialog heading.
/// `desc` — body text (may contain `<strong>` via `maud::PreEscaped`).
/// `confirm_label` — text for the danger button (e.g. "确认删除").
/// `form_id` — `id` of the hidden `<form>` the confirm button triggers.
/// `form` — the HTMX form markup (hidden, with `hx-post`, `hx-swap`, etc.).
pub fn confirm_dialog(
 dialog_id: &str,
 title: &str,
 desc: &str,
 confirm_label: &str,
 form_id: &str,
 form: Markup,
) -> Markup {
 html! {
 div id=(dialog_id) class="hidden fixed z-[1100] place-items-center"
 _=(format!("on click[me is event.target] remove .open")) {
 div class="bg-bg rounded-lg w-[480px]" _="on click halt the event" {
 div class="bg-bg rounded-lg w-[480px]-body" {
 div class="bg-bg rounded-lg w-[480px]-icon-wrap" {
 (icon::circle_alert_icon("w-7 h-7"))
 }
 div class="text-lg font-semibold text-fg text-center mb-2" { (title) }
 p class="text-sm text-muted text-center leading-relaxed" { (maud::PreEscaped(desc)) }
 }
 div class="bg-bg rounded-lg w-[480px]-foot" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 _="on click remove .open from closest .dialog-overlay" { "取消" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-white border-none hover:opacity-90 text-sm font-medium cursor-pointer transition-all duration-150"
 _=(format!("on click remove .open from closest .dialog-overlay then trigger submit on #{}", form_id))
 { (confirm_label) }
 }
 }
 (form)
 }
 }
}
