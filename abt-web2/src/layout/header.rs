use abt_core::shared::identity::model::Claims;
use maud::{html, Markup};

use crate::components::icon;

pub fn header(claims: &Claims, module_name: &str, page_name: Option<&str>) -> Markup {
    let initials = crate::layout::sidebar::avatar_initials(&claims.display_name);
    html! {
        header class="top-header" {
            div class="top-header-left" {
                button class="mobile-menu-btn" x-on:click="mobileOpen = true" aria-label="菜单" {
                    (icon::menu_icon(""))
                }
                div class="breadcrumb" {
                    @if let Some(page) = page_name {
                        span style="font-weight:600;color:var(--fg)" { (module_name) }
                        span class="breadcrumb-sep" { "/" }
                        span { (page) }
                    } @else {
                        span style="font-weight:600;color:var(--fg)" { (module_name) }
                    }
                }
            }
            div class="top-header-right" {
                span class="text-muted" style="font-size:13px" {
                    "操作员：" (claims.display_name.as_str())
                }
                button class="header-icon-btn" title="通知" {
                    (icon::bell_icon(""))
                    div class="header-dot" {}
                }
                button class="header-icon-btn" title="帮助" {
                    (icon::question_icon(""))
                }
                div class="avatar" { (initials) }
            }
        }
    }
}
