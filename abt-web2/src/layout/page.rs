use abt_core::shared::identity::model::Claims;
use axum::http::HeaderMap;
use maud::{DOCTYPE, Markup, html};

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
                link rel="stylesheet" href="/app.css";
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/notyf@3/notyf.min.css";
                script src="/htmx.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/notyf@3/notyf.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js" defer {}
            }
            body {
                (body)
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
        div x-data=(format!("{{ collapsed: localStorage.getItem('sidebar-collapsed') === 'true', mobileOpen: false, activeModule: '{}' }}", active_module))
            x-effect="localStorage.setItem('sidebar-collapsed', collapsed)" {
            div class="app-shell" x-bind:class="{ 'sidebar-collapsed': collapsed }" {
                (sidebar::sidebar(claims, active_module, current_path))
                div class="main-content" {
                    (header::header(claims, module_name, page_name))
                    div class="page-content" {
                        (content)
                    }
                }
            }
            div class="mobile-sidebar-overlay" x-bind:class="{ 'open': mobileOpen }" x-on:click="mobileOpen = false" {}
            (sidebar::mobile_nav(active_module, current_path))
        }
    }
}

/// Renders a full admin page or just the content fragment, depending on whether
/// the request came from HTMX (checks `HX-Request` header).
pub fn admin_page(
    headers: &HeaderMap,
    title: &str,
    claims: &Claims,
    active_module: &str,
    current_path: &str,
    module_name: &str,
    page_name: Option<&str>,
    content: Markup,
) -> Markup {
    let is_htmx = headers.get("HX-Request").is_some();
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
