use abt_core::shared::identity::model::Claims;
use maud::{DOCTYPE, Markup, PreEscaped, html};

use super::header;
use super::sidebar::{self, NavFilter};

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
                link rel="stylesheet" href=(cache_url("/app.css")) {}
                script src="/htmx.min.js" {}
                script src="/Sortable.min.js" {}
                script src="/hyperscript.min.js" {}
                script src=(cache_url("/app.js")) {}
            }
            body { (body) (toast_container()) (global_confirm_dialog()) }
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
    nav_filter: &NavFilter,
) -> Markup {
    html! {
        div id="app-wrapper" {
            div class="app-shell" _="on load if localStorage.getItem('sidebar-collapsed') is 'true' add .sidebar-collapsed" {
                (sidebar::sidebar(claims, active_module, current_path, nav_filter))
                div class="flex flex-col bg-surface" {
                    (header::header(claims, module_name, page_name))
                    div class="flex-1 p-8" { (content) }
                }
            }
            div class="hidden fixed z-[50]" _="on click remove .open" {}
            (sidebar::mobile_nav(active_module, current_path, nav_filter))
        }
    }
}

/// Renders a full admin page or just the content fragment, depending on whether
/// the request came from HTMX (checks `HX-Request` header).
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
    nav_filter: &NavFilter,
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
                nav_filter,
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
        div class="fixed z-[99999] flex flex-col gap-[10px]"
            hx-get="/api/toast"
            hx-trigger="showToast from:body"
            hx-swap="innerHTML" {}
    }
}

fn global_confirm_dialog() -> Markup {
    let icon = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-7 h-7"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>"#;
    html! {
        div id="global-confirm-dialog" {
            div class="hidden fixed z-[1100] place-items-center" _="on click remove .open" {
                div class="bg-bg rounded-lg w-[480px]" onclick="event.stopPropagation()" {
                    div class="bg-bg rounded-lg w-[480px]-body" {
                        div class="bg-bg rounded-lg w-[480px]-icon-wrap" { (PreEscaped(icon)) }
                        p class="text-sm text-muted text-center leading-relaxed" id="global-confirm-message" {}
                    }
                    div class="bg-bg rounded-lg w-[480px]-foot" {
                        button
                            type="button"
                            class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface"
                            _="on click remove .open from closest .dialog-overlay"
                        { "取消" }
                        button
                            type="button"
                            class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                            _="on click call window._confirmIssueRequest() then remove .open from closest .dialog-overlay"
                        { "确认" }
                    }
                }
            }
        }
    }
}

/// Cache-busting URL: appends `?v=<startup_unix_secs>` so static assets refresh every server restart.
fn cache_url(path: &str) -> String {
    use std::sync::OnceLock;
    static TS: OnceLock<String> = OnceLock::new();
    let ts = TS.get_or_init(|| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        secs.to_string()
    });
    format!("{path}?v={ts}")
}
