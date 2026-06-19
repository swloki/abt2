use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, PreEscaped, html};

use crate::layout::page::standalone_page;

/// Friendly 404 page body — standalone (works without auth), design tokens only.
fn not_found_body() -> Markup {
    // Magnifier-with-x icon: "looked, not found"
    let icon = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.65" y2="16.65"/><line x1="8" y1="8" x2="14" y2="14"/><line x1="14" y1="8" x2="8" y2="14"/></svg>"#;
    let home_icon = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4"><path d="M3 9.5L12 3l9 6.5V20a1 1 0 01-1 1h-5v-6H9v6H4a1 1 0 01-1-1V9.5z"/></svg>"#;
    let back_icon = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4"><path d="M19 12H5"/><path d="M12 19l-7-7 7-7"/></svg>"#;

    html! {
        div class="min-h-screen flex flex-col bg-surface" {
            // ── Brand bar ──
            div class="flex items-center gap-2 px-8 py-5" {
                span class="w-8 h-8 rounded-md bg-accent text-white grid place-items-center font-bold text-sm" { "A" }
                span class="text-fg font-semibold text-sm" { "ABT 管理系统" }
            }

            // ── Centered content ──
            main class="flex-1 grid place-items-center px-6 py-10" {
                div class="text-center max-w-md" {
                    // Icon badge
                    div class="w-14 h-14 mx-auto rounded-full bg-accent/10 flex items-center justify-center text-accent [&_svg]:w-7 [&_svg]:h-7" {
                        (PreEscaped(icon))
                    }
                    // Big 404
                    div class="mt-6 text-[120px] leading-none font-extrabold text-accent tracking-tight font-mono select-none" {
                        "404"
                    }
                    // Heading
                    h1 class="text-xl font-semibold text-fg mt-6" { "页面未找到" }
                    // Subtext
                    p class="text-sm text-muted mt-2 leading-relaxed" {
                        "您访问的页面不存在或已被移动。请检查网址是否正确，或返回首页继续操作。"
                    }
                    // Buttons
                    div class="flex items-center justify-center gap-3 mt-8" {
                        a href="/" class="inline-flex items-center gap-2 px-5 py-2.5 rounded-sm bg-accent text-white text-sm font-medium no-underline hover:bg-accent-hover transition-colors" {
                            (PreEscaped(home_icon))
                            "返回首页"
                        }
                        button type="button"
                            class="inline-flex items-center gap-2 px-5 py-2.5 rounded-sm bg-bg border border-border text-fg-2 text-sm font-medium cursor-pointer hover:bg-surface hover:text-fg transition-colors"
                            _="on click call history.back()"
                        {
                            (PreEscaped(back_icon))
                            "返回上一页"
                        }
                    }
                    // Error code footer
                    p class="text-[11px] text-muted mt-12 font-mono tracking-wider" {
                        "ERROR 404 · PAGE NOT FOUND"
                    }
                }
            }
        }
    }
}

/// Renders the full 404 HTML document.
pub fn not_found_page() -> Markup {
    standalone_page("404 页面未找到", not_found_body())
}

/// Axum handler: returns the friendly 404 page with the correct status code.
pub async fn not_found_handler() -> Response {
    let body = not_found_page().into_string();
    (StatusCode::NOT_FOUND, Html(body)).into_response()
}
