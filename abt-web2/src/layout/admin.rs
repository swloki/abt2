use abt_core::shared::identity::model::Claims;
use maud::{html, Markup};

use super::header::header;
use super::sidebar::sidebar;

pub fn admin_layout(claims: &Claims, current_path: &str, content: Markup) -> Markup {
    html! {
        div class="hidden md:flex h-screen overflow-hidden bg-white" {
            (sidebar(current_path))
            div class="flex flex-1 flex-col overflow-hidden" {
                (header(claims, current_path))
                main class="flex-1 overflow-y-auto bg-slate-50 px-4 py-4 lg:px-6" {
                    (content)
                }
            }
        }
    }
}
