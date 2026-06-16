use abt_core::master_data::customer::model::CustomerContact;
use maud::{html, Markup};


/// HTMX query params for customer contacts endpoint
#[derive(Debug, serde::Deserialize)]
pub struct CustomerContactsParams {
    pub customer_id: Option<i64>,
}

/// Self-contained HTMX component: customer selector with auto-filling contacts.
/// `hx-target="this" + hx-swap="outerHTML"` replaces the entire data-card on change.
pub fn customer_info_panel(
    customers: &[abt_core::master_data::customer::model::Customer],
    contacts: &[CustomerContact],
    selected_customer_id: Option<i64>,
    contacts_endpoint: &str,
) -> Markup {
    let selected = selected_customer_id.map(|id| id.to_string()).unwrap_or_default();
    let phone_value = contacts.first().and_then(|c| c.phone.as_deref()).unwrap_or("");

    html! {
        div class="form-bg-bg border border-border-soft rounded-lg overflow-hidden" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                (crate::components::icon::users_icon("w-[18px] h-[18px]"))
                "客户信息"
            }
            div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                div class="form-field span-2" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "客户名称" span class="required" { "*" } }
                    select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="customer_id" id="f-customer-id"
                        hx-get=(contacts_endpoint)
                        hx-trigger="change"
                        hx-target="closest .form-section-card"
                        hx-swap="outerHTML"
                        hx-include="this" {
                        option value="0" { "请选择客户" }
                        @for c in customers {
                            option value=(c.id) selected[selected == c.id.to_string()] { (c.name) }
                        }
                    }
                }
                div class="form-field" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系人" }
                    select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="contact_id" id="f-contact-id" {
                        option value="0" { "请选择联系人" }
                        @for ct in contacts {
                            option value=(ct.id) { (ct.name) }
                        }
                    }
                }
                div class="form-field" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系电话" }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" id="f-contact-phone"
                        value=(phone_value)
                        placeholder="自动填充" readonly {}
                }
            }
        }
    }
}

/// Shared handler logic: fetch contacts for a customer and re-render the panel.
/// Each page's handler calls this after querying its own customer list.
#[allow(dead_code)]
pub fn render_contacts_response(
    customers: &[abt_core::master_data::customer::model::Customer],
    contacts: &[CustomerContact],
    customer_id: Option<i64>,
    contacts_endpoint: &str,
) -> crate::errors::Result<Markup> {
    Ok(customer_info_panel(customers, contacts, customer_id, contacts_endpoint))
}
