use maud::{Markup, html};

use super::icon;

/// Confirm dialog with warning icon, title, description, and cancel/confirm buttons.
///
/// `open_var` — Alpine reactive boolean (e.g. `"deleteOpen"`).
/// `title` — dialog heading.
/// `desc` — body text (may contain `<strong>` via `maud::PreEscaped`).
/// `confirm_label` — text for the danger button (e.g. "确认删除").
/// `form_id` — `id` of the hidden `<form>` the confirm button triggers.
/// `form` — the HTMX form markup (hidden, with `hx-post`, `hx-swap`, etc.).
pub fn confirm_dialog(
    open_var: &str,
    title: &str,
    desc: &str,
    confirm_label: &str,
    form_id: &str,
    form: Markup,
) -> Markup {
    html! {
        template x-teleport="body" {
            div class="dialog-overlay"
                x-bind:class=(format!("{{ 'open': {} }}", open_var))
                x-on:click=(format!("{} = false", open_var)) {
                div class="dialog" x-on:click="event.stopPropagation()" {
                    div class="dialog-body" {
                        div class="dialog-icon-wrap" {
                            (icon::circle_alert_icon("w-7 h-7"))
                        }
                        div class="dialog-title" { (title) }
                        p class="dialog-desc" { (maud::PreEscaped(desc)) }
                    }
                    div class="dialog-foot" {
                        button type="button" class="btn btn-default"
                            x-on:click=(format!("{} = false", open_var)) { "取消" }
                        button type="button" class="btn btn-danger"
                            x-on:click=(format!("{} = false; htmx.trigger(document.getElementById('{}'), 'submit')", open_var, form_id))
                            { (confirm_label) }
                    }
                }
                (form)
            }
        }
    }
}
