use maud::{html, Markup};

pub fn pagination(
    base_path: &str,
    query: &str,
    total: u64,
    current_page: u32,
    total_pages: u32,
) -> Markup {
    if total_pages == 0 {
        return html! {};
    }

    html! {
        div class="pagination" {
            span { "共 " (total) " 条记录，第 " (current_page) "/" (total_pages) " 页" }
            div class="pagination-pages" {
                @if current_page > 1 {
                    (page_link(base_path, query, current_page - 1, "«"))
                }
                @for p in page_range(current_page, total_pages) {
                    @if p == 0 {
                        button class="page-btn" disabled { "…" }
                    } @else if p == current_page {
                        button class="page-btn active" disabled { (p) }
                    } @else {
                        (page_link(base_path, query, p, &p.to_string()))
                    }
                }
                @if current_page < total_pages {
                    (page_link(base_path, query, current_page + 1, "»"))
                }
            }
        }
    }
}

fn page_link(base_path: &str, query: &str, page: u32, label: &str) -> Markup {
    let qs = if query.is_empty() {
        format!("page={page}")
    } else {
        format!("{query}&page={page}")
    };

    html! {
        a class="page-btn" href=(format!("{base_path}?{qs}")) { (label) }
    }
}

fn page_range(current: u32, total: u32) -> Vec<u32> {
    if total <= 5 {
        (1..=total).collect()
    } else if current <= 3 {
        let mut r: Vec<u32> = (1..=4).collect();
        r.push(0);
        r.push(total);
        r
    } else if current >= total - 2 {
        let mut r = vec![1u32, 0];
        r.extend((total - 3)..=total);
        r
    } else {
        let mut r = vec![1u32, 0];
        r.extend((current - 1)..=(current + 1));
        r.push(0);
        r.push(total);
        r
    }
}
