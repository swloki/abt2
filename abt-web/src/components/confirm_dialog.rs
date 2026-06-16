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
        div id=(dialog_id) class="dialog-overlay"
            _=(format!("on click[me is event.target] remove .open")) {
            div class="dialog" onclick="event.stopPropagation()" {
                div class="dialog-body" {
                    div class="dialog-icon-wrap" {
                        (icon::circle_alert_icon("w-7 h-7"))
                    }
                    div class="text-lg font-semibold text-fg text-center mb-2" { (title) }
                    p class="text-sm text-muted text-center leading-relaxed" { (maud::PreEscaped(desc)) }
                }
                div class="dialog-foot" {
                    button type="button" class="btn bg-white text-fg border border-border hover:bg-surface"
                        _="on click remove .open from closest .dialog-overlay" { "取消" }
                    button type="button" class="btn bg-danger text-white border-none hover:opacity-90"
                        _=(format!("on click remove .open from closest .dialog-overlay then trigger submit on #{}", form_id))
                        { (confirm_label) }
                }
            }
            (form)
        }
    }
}
