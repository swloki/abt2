use abt_core::shared::identity::model::Claims;
use maud::{html, Markup};

use crate::components::icon;

pub fn header(claims: &Claims, module_name: &str, page_name: Option<&str>) -> Markup {
    let initials = crate::layout::sidebar::avatar_initials(&claims.display_name);
    html! {
        header class="h-[var(--header-h)] bg-white/82 backdrop-blur-md border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10" {
            // ── Left: mobile menu + breadcrumb ──
            div class="flex items-center gap-4" {
                button class="hidden md:grid w-[38px] h-[38px] border-none rounded-sm place-items-center cursor-pointer shrink-0 hover:bg-surface transition-colors [&_[class*=i-lucide]]:w-[22px] [&_[class*=i-lucide]]:h-[22px] [&_[class*=i-lucide]]:text-fg"
                        _="on click add .mobile-open to #sidebar then add .is-open to .mobile-sidebar-overlay"
                        aria-label="菜单" {
                    (icon::menu_icon(""))
                }
                div class="flex items-center gap-2 text-sm text-muted" {
                    @if let Some(page) = page_name {
                        span class="font-semibold text-fg" { (module_name) }
                        span class="text-border text-xs" { "/" }
                        span { (page) }
                    } @else {
                        span class="font-semibold text-fg" { (module_name) }
                    }
                }
            }
            // ── Right: icon buttons + user menu ──
            div class="flex items-center gap-4" {
                button class="w-9 h-9 rounded-md border border-border-soft bg-white/60 grid place-items-center relative cursor-pointer transition-all duration-150 hover:bg-bg hover:border-border hover:shadow-sm [&_[class*=i-lucide]]:w-4.5 [&_[class*=i-lucide]]:h-4.5 [&_[class*=i-lucide]]:text-muted"
                        title="通知" {
                    (icon::bell_icon(""))
                    div class="absolute top-[7px] right-[7px] w-[7px] h-[7px] rounded-full bg-danger border-2 border-bg" {}
                }
                button class="w-9 h-9 rounded-md border border-border-soft bg-white/60 grid place-items-center relative cursor-pointer transition-all duration-150 hover:bg-bg hover:border-border hover:shadow-sm [&_[class*=i-lucide]]:w-4.5 [&_[class*=i-lucide]]:h-4.5 [&_[class*=i-lucide]]:text-muted"
                        title="帮助" {
                    (icon::question_icon(""))
                }
                // ── User Menu ──
                div class="user-menu group relative" _="on click toggle .is-open then on click from elsewhere remove .is-open" {
                    button class="flex items-center gap-2 py-px px-1.5 rounded-full border border-transparent bg-transparent cursor-pointer transition-all duration-150 hover:bg-bg hover:border-border-soft group-[.is-open]:bg-bg group-[.is-open]:border-border-soft"
                            aria-label="用户菜单" {
                        div class="w-8 h-8 rounded-full bg-accent grid place-items-center text-xs font-bold text-white shrink-0" { (initials) }
                        div class="flex flex-col items-start leading-tight max-md:hidden" {
                            span class="text-sm font-semibold text-fg" { (claims.display_name.as_str()) }
                            span class="text-[11px] text-muted" { (claims.system_role.as_str()) }
                        }
                    }
                    div class="absolute top-[calc(100%+10px)] right-0 min-w-[252px] bg-surface border border-border rounded-md shadow-lg p-2 opacity-0 invisible -translate-y-2 transition-all duration-150 group-[.is-open]:opacity-100 group-[.is-open]:visible group-[.is-open]:translate-y-0 z-[60]" {
                        div class="flex items-center gap-3 py-2 px-2 pb-3 border-b border-border-soft mb-2" {
                            div class="w-[42px] h-[42px] rounded-full bg-accent grid place-items-center text-sm font-bold text-white shrink-0" { (initials) }
                            div class="flex flex-col min-w-0" {
                                span class="font-semibold text-fg text-sm whitespace-nowrap" { (claims.display_name.as_str()) }
                                span class="text-xs text-muted whitespace-nowrap overflow-hidden text-ellipsis" { (claims.username.as_str()) }
                            }
                        }
                        a class="flex items-center gap-3 w-full py-2 px-3 border-none bg-transparent rounded-sm text-sm text-fg-2 cursor-pointer text-left transition-colors hover:bg-bg hover:text-fg [&_[class*=i-lucide]]:w-[17px] [&_[class*=i-lucide]]:h-[17px] [&_[class*=i-lucide]]:text-muted [&_[class*=i-lucide]]:shrink-0 hover:[&_[class*=i-lucide]]:text-accent" href="/admin/users" {
                            (icon::user_icon(""))
                            "个人中心"
                        }
                        a class="flex items-center gap-3 w-full py-2 px-3 border-none bg-transparent rounded-sm text-sm text-fg-2 cursor-pointer text-left transition-colors hover:bg-bg hover:text-fg [&_[class*=i-lucide]]:w-[17px] [&_[class*=i-lucide]]:h-[17px] [&_[class*=i-lucide]]:text-muted [&_[class*=i-lucide]]:shrink-0 hover:[&_[class*=i-lucide]]:text-accent" href="/admin/users" {
                            (icon::tool_icon(""))
                            "账号设置"
                        }
                        a class="flex items-center gap-3 w-full py-2 px-3 border-none bg-transparent rounded-sm text-sm text-fg-2 cursor-pointer text-left transition-colors hover:bg-bg hover:text-fg [&_[class*=i-lucide]]:w-[17px] [&_[class*=i-lucide]]:h-[17px] [&_[class*=i-lucide]]:text-muted [&_[class*=i-lucide]]:shrink-0 hover:[&_[class*=i-lucide]]:text-accent" href="/admin/notifications" {
                            (icon::bell_icon(""))
                            "通知中心"
                        }
                        div class="h-px bg-border-soft mx-2 my-1" {}
                        form class="m-0" hx-post="/logout" hx-swap="none" {
                            button class="flex items-center gap-3 w-full py-2 px-3 border-none bg-transparent rounded-sm text-sm text-danger cursor-pointer text-left transition-colors hover:bg-danger/9 [&_[class*=i-lucide]]:w-[17px] [&_[class*=i-lucide]]:h-[17px] [&_[class*=i-lucide]]:text-danger [&_[class*=i-lucide]]:shrink-0" type="submit" {
                                (icon::log_out_icon(""))
                                "退出登录"
                            }
                        }
                    }
                }
            }
        }
    }
}
