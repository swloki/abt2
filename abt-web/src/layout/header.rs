use abt_core::shared::identity::model::Claims;
use maud::{html, Markup};

use crate::components::icon;

pub fn header(claims: &Claims, module_name: &str, page_name: Option<&str>) -> Markup {
    let initials = crate::layout::sidebar::avatar_initials(&claims.display_name);
    html! {
        header class="h-[var(--header-h)] bg-bg border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10 shadow-xs" {
            div class="h-[var(--header-h)] bg-bg border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10 shadow-xs-left" {
                button class="hidden w-[38px] h-[38px] border-none rounded-sm place-items-center cursor-pointer shrink-0" _="on click add .open to .mobile-sidebar-overlay" aria-label="菜单" {
                    (icon::menu_icon(""))
                }
                div class="flex items-center gap-2 text-sm text-text-muted" {
                    @if let Some(page) = page_name {
                        span style="font-weight:600;color:var(--fg)" { (module_name) }
                        span class="flex items-center gap-2 text-sm text-text-muted-sep" { "/" }
                        span { (page) }
                    } @else {
                        span style="font-weight:600;color:var(--fg)" { (module_name) }
                    }
                }
            }
            div class="h-[var(--header-h)] bg-bg border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10 shadow-xs-right" {
                button class="w-9 h-9 rounded-sm border border-border-soft bg-bg grid place-items-center relative cursor-pointer transition-colors duration-150 hover:bg-surface hover:border-border" title="通知" {
                    (icon::bell_icon(""))
                    div class="absolute top-[7px] right-[7px] w-[7px] h-[7px] rounded-full bg-danger border-2 border-bg" {}
                }
                button class="w-9 h-9 rounded-sm border border-border-soft bg-bg grid place-items-center relative cursor-pointer transition-colors duration-150 hover:bg-surface hover:border-border" title="帮助" {
                    (icon::question_icon(""))
                }
                div class="relative" _="on click from elsewhere remove .is-open" {
                    button class="relative-trigger" aria-label="用户菜单" _="on click toggle .is-open on .user-menu" {
                        div class="inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" { (initials) }
                    }
                    div class="relative-dropdown" {
                        div class="relative-header" {
                            div class="inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" { (initials) }
                            div class="relative-info" {
                                div class="relative-name" { (claims.display_name.as_str()) }
                                div class="relative-email" { (claims.username.as_str()) }
                            }
                        }
                        a class="relative-item" href="/admin/users" {
                            (icon::user_icon("w-4 h-4"))
                            "个人中心"
                        }
                        a class="relative-item" href="/admin/users" {
                            (icon::tool_icon("w-4 h-4"))
                            "账号设置"
                        }
                        a class="relative-item" href="/admin/notifications" {
                            (icon::bell_icon("w-4 h-4"))
                            "通知中心"
                        }
                        div class="relative-divider" {}
                        form class="relative-form" hx-post="/logout" hx-swap="none" {
                            button class="flex items-center gap-2 px-3 py-2 text-sm text-fg-2 hover:text-fg hover:bg-surface rounded-sm cursor-pointer transition-colors relative-logout" type="submit" {
                                (icon::log_out_icon("w-4 h-4"))
                                "退出登录"
                            }
                        }
                    }

                }
            }
        }
    }
}
