use maud::{DOCTYPE, Markup, html};

pub fn base_html(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="zh-CN" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) " - ABT 管理系统" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href="/base.css";
                link rel="stylesheet" href="/app.css";
                script src="/htmx.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js" defer {}
            }
            body class="min-h-screen bg-slate-50 text-slate-900" {
                (body)
            }
        }
    }
}

/// Returns only the partial content for HTMX requests (no layout shell).
pub fn maybe_full_page(title: &str, is_htmx: bool, content: Markup) -> Markup {
    if is_htmx {
        content
    } else {
        base_html(title, content)
    }
}
