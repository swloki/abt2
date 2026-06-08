use abt_core::shared::identity::model::Claims;
use maud::{DOCTYPE, Markup, PreEscaped, html};

use super::header;
use super::sidebar;

// ── Page Shell ──

/// Full HTML document shell with head, CSS, scripts.
fn document(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="zh-CN" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) " - ABT 管理系统" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href="/app.css?v=20260531";
                script src="/htmx.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.6/Sortable.min.js" {}
                script src="/surreal.js" {}
                script src="/app.js?v=20260603" {}
            }
            body {
                (body)
                (toast_container())
                (global_confirm_dialog())
            }
        }
    }
}

/// Admin layout: sidebar + header + content area.
fn admin_shell(
    claims: &Claims,
    active_module: &str,
    current_path: &str,
    module_name: &str,
    page_name: Option<&str>,
    content: Markup,
) -> Markup {
    html! {
        div id="app-wrapper" {
            div class="app-shell" {
                (sidebar::sidebar(claims, active_module, current_path))
                div class="main-content" {
                    (header::header(claims, module_name, page_name))
                    div class="page-content" {
                        (content)
                    }
                }
            }
            (PreEscaped("<script>if(localStorage.getItem('sidebar-collapsed')==='true')me('.app-shell').classAdd('sidebar-collapsed')</script>"))
            div class="mobile-sidebar-overlay"
                onclick="hsRemove(this,null,'open')" {}
            (sidebar::mobile_nav(active_module, current_path))
        }
    }
}

/// Renders a full admin page or just the content fragment, depending on whether
/// the request came from HTMX (checks `HX-Request` header).
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn admin_page(
    is_htmx: bool,
    title: &str,
    claims: &Claims,
    active_module: &str,
    current_path: &str,
    module_name: &str,
    page_name: Option<&str>,
    content: Markup,
) -> Markup {
    if is_htmx {
        content
    } else {
        document(
            title,
            admin_shell(
                claims,
                active_module,
                current_path,
                module_name,
                page_name,
                content,
            ),
        )
    }
}
/// Renders a standalone page (e.g. login) — no admin shell.
pub fn standalone_page(title: &str, body: Markup) -> Markup {
    document(title, body)
}

fn toast_container() -> Markup {
    html! {
        div class="toast-container" {}
    }
}

fn global_confirm_dialog() -> Markup {
    let icon = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-7 h-7"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>"#;
    html! {
        div id="global-confirm-dialog" {
            div class="dialog-overlay"
                onclick="hsRemove(this,null,'open')" {
                div class="dialog" onclick="event.stopPropagation()" {
                    div class="dialog-body" {
                        div class="dialog-icon-wrap" {
                            (PreEscaped(icon))
                        }
                        p class="dialog-desc" id="global-confirm-message" {}
                    }
                    div class="dialog-foot" {
                        button type="button" class="btn btn-default" onclick="hsRemoveClosest(this,'.dialog-overlay','open')" { "取消" }
                        button type="button" class="btn btn-danger" onclick="window._confirmIssueRequest();hsRemoveClosest(this,'.dialog-overlay','open')" { "确认" }
                    }
                }
            }
        }
    }
}
