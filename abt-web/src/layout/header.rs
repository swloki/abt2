use abt_core::shared::identity::model::Claims;
use maud::{html, Markup};

use crate::components::icon;

pub fn header(claims: &Claims, module_name: &str, page_name: Option<&str>) -> Markup {
    let initials = crate::layout::sidebar::avatar_initials(&claims.display_name);
    html! {
        header class="top-header" {
            div class="top-header-left" {
                button class="mobile-menu-btn" onclick="hsAdd(null,'.mobile-sidebar-overlay','open')" aria-label="菜单" {
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
                button class="header-icon-btn" title="通知" {
                    (icon::bell_icon(""))
                    div class="header-dot" {}
                }
                button class="header-icon-btn" title="帮助" {
                    (icon::question_icon(""))
                }
                div class="user-menu" {
                    button class="user-menu-trigger" aria-label="用户菜单" {
                        div class="avatar" { (initials) }
                    }
                    div class="user-menu-dropdown" {
                        div class="user-menu-header" {
                            div class="avatar" { (initials) }
                            div class="user-menu-info" {
                                div class="user-menu-name" { (claims.display_name.as_str()) }
                                div class="user-menu-email" { (claims.username.as_str()) }
                            }
                        }
                        a class="user-menu-item" href="/admin/users" {
                            (icon::user_icon("w-4 h-4"))
                            "个人中心"
                        }
                        a class="user-menu-item" href="/admin/users" {
                            (icon::tool_icon("w-4 h-4"))
                            "账号设置"
                        }
                        a class="user-menu-item" href="/admin/notifications" {
                            (icon::bell_icon("w-4 h-4"))
                            "通知中心"
                        }
                        div class="user-menu-divider" {}
                        form class="user-menu-form" hx-post="/logout" hx-swap="none" {
                            button class="user-menu-item user-menu-logout" type="submit" {
                                (icon::log_out_icon("w-4 h-4"))
                                "退出登录"
                            }
                        }
                    }
                    (maud::PreEscaped(r#"<script>
                        me('.user-menu-trigger').on('click', function(e) {
                            e.stopPropagation();
                            me('.user-menu').classToggle('is-open');
                        });
                        document.addEventListener('click', function(e) {
                            if (!e.target.closest('.user-menu')) {
                                me('.user-menu').classRemove('is-open');
                            }
                        });
                    </script>"#))
                }
            }
        }
    }
}
