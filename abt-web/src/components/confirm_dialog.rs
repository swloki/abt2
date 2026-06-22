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
        div id=(dialog_id)
            class="dialog-overlay fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-md"
            style="display:none"
            _="on click[me is event.target] hide me"
        {
            div class="bg-bg rounded-lg w-[480px] max-w-[92vw] shadow-[0_25px_60px_rgba(15,23,42,0.18)]"
                _="on click halt the event"
            {
                div class="p-8 pb-6 flex flex-col items-center" {
                    div class="w-14 h-14 rounded-full bg-danger/10 flex items-center justify-center mb-5 icon:w-7 icon:h-7 icon:text-danger"
                    { (icon::circle_alert_icon("w-7 h-7")) }
                    div class="text-lg font-semibold text-fg text-center mb-2" { (title) }
                    p class="text-sm text-muted text-center leading-relaxed" {
                        (maud::PreEscaped(desc))
                    }
                }
                div class="py-4 border-t border-border-soft flex justify-center gap-3" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 px-5 py-2 rounded-sm text-sm font-medium cursor-pointer bg-white text-fg border border-border hover:bg-surface transition-all duration-150"
                        _="on click hide closest .dialog-overlay"
                    { "取消" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 px-5 py-2 rounded-sm text-sm font-medium cursor-pointer bg-danger text-white border-none hover:opacity-90 transition-opacity min-w-[100px] justify-center"
                        _=({
                            format!(
                                "on click hide closest .dialog-overlay then trigger submit on #{}",
                                form_id,
                            )
                        })
                    { (confirm_label) }
                }
            }
            (form)
        }
    }
}
