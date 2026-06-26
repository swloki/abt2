use maud::{Markup, html};

use super::overlay::modal_shell;

/// Input dialog: a modal with a single input field and configurable confirm action.
///
/// Uses `modal-overlay` + `modal modal-sm` structure, toggled via `.is-open` class.
/// `confirm_action` is a Hyperscript string on the confirm button — supports both
/// pure-frontend actions (`call doSplit()`) and HTMX submissions (`trigger submit on #form`).
///
/// `dialog_id` — HTML id of the overlay div.
/// `title` — dialog heading.
/// `desc` — description markup (shown above input).
/// `input_id` — HTML id of the input field.
/// `input_label` — label text.
/// `input_type` — "number" or "text".
/// `input_placeholder` — placeholder text.
/// `confirm_label` — confirm button text.
/// `confirm_action` — Hyperscript action for confirm button.
pub fn input_dialog(
 dialog_id: &str,
 title: &str,
 desc: Markup,
 input_id: &str,
 input_label: &str,
 input_type: &str,
 input_placeholder: &str,
 confirm_label: &str,
 confirm_action: &str,
) -> Markup {
 modal_shell(dialog_id, "z-[1000]", html! {
        div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0"
        {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
            {
                h2 { (title) }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-6" {
                p class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" {
                    (desc)
                }
                div class="form-field" {
                    label { (input_label) }
                    input
                        id=(input_id)
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                        type=(input_type)
                        placeholder=(input_placeholder);
                }
            }
            div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    _=(format!("on click remove .is-open from closest .modal-overlay"))
                { "取消" }
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    _=(format!("on click {confirm_action}"))
                { (confirm_label) }
            }
        }
    })
}

