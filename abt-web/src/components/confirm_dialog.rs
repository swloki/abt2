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
            _={ "on click remove .open from #" (dialog_id) } {
            div class="dialog" _="on click halt the event" {
                div class="dialog-body" {
                    div class="dialog-icon-wrap" {
                        (icon::circle_alert_icon("w-7 h-7"))
                    }
                    div class="dialog-title" { (title) }
                    p class="dialog-desc" { (maud::PreEscaped(desc)) }
                }
                div class="dialog-foot" {
                    button type="button" class="btn btn-default"
                        _={ "on click remove .open from #" (dialog_id) } { "取消" }
                    button type="button" class="btn btn-danger"
                        _={ "on click remove .open from #" (dialog_id) " then htmx.trigger(document.getElementById('" (form_id) "'), 'submit')" }
                        { (confirm_label) }
                }
            }
            (form)
        }
    }
}
