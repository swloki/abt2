use abt_core::shared::identity::model::Claims;
use axum::http::HeaderMap;
use maud::{DOCTYPE, Markup, html, PreEscaped};

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
                script src="/htmx.min.js" {}
                script src="/app.js" {}
                script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.6/Sortable.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js" defer {}
            }
            body {
                (body)
                (toast_container())
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
#[allow(clippy::too_many_arguments)]
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

fn toast_container() -> Markup {
    let alpine_data = r#"{ toasts: [], init() { var self = this; window.addEventListener('show-toast', function(e) { var t = { id: Date.now(), message: e.detail.message, type: e.detail.type || 'success' }; self.toasts.push(t); setTimeout(function() { self.toasts = self.toasts.filter(function(x) { return x.id !== t.id; }) }, 4000); }) }, removeToast(id) { this.toasts = this.toasts.filter(function(x) { return x.id !== id; }) } }"#;
    let bind_class = r#"'toast-' + toast.type"#;
    let icon_success = r#"<span x-show="toast.type !== 'error' && toast.type !== 'warning'" style="display:none"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg></span>"#;
    let icon_error = r#"<span x-show="toast.type === 'error'" style="display:none"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg></span>"#;
    let icon_warning = r#"<span x-show="toast.type === 'warning'" style="display:none"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg></span>"#;
    html! {
        div
            x-data=(alpine_data)
            class="toast-container" {
            template x-for="toast in toasts" {
                div
                    class="toast toast-show"
                    x-bind:class=(bind_class) {
                    (PreEscaped(icon_success))
                    (PreEscaped(icon_error))
                    (PreEscaped(icon_warning))
                    span class="toast-message" x-text="toast.message" {}
                    button class="toast-close" x-on:click="removeToast(toast.id)" {
                        "×"
                    }
                }
            }
        }
    }
}
