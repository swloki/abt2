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
    div class="flex items-center justify-between py-4 px-5" {
        span class="text-[13px] text-muted" {
            "共 "
            (total)
            " 条记录，第 "
            (current_page)
            "/"
            (total_pages)
            " 页"
        }
        div class="flex gap-1" {
            @if current_page > 1 { (page_link(base_path, query, current_page - 1, "«")) }
            @for p in page_range(current_page, total_pages) {
                @if p == 0 {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
                        disabled
                    { "…" }
                } @else if p == current_page {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-accent text-white text-sm font-semibold cursor-pointer"
                        disabled
                    { (p) }
                } @else { (page_link(base_path, query, p, &p.to_string())) }
            }
            @if current_page < total_pages { (page_link(base_path, query, current_page + 1, "»")) }
        }
    }
}
}

/// HTMX-aware pagination: page links use hx-get with the given hx-target/hx-swap.
/// `query` carries filter params (e.g. `category_id=3`) so page links preserve the active filter;
/// pass "" when the filter is encoded in the path itself (e.g. `/customers/{id}/transactions`).
pub fn htmx_pagination(
 base_path: &str,
 query: &str,
 total: u64,
 current_page: u32,
 total_pages: u32,
 hx_target: &str,
 hx_swap: &str,
) -> Markup {
 if total_pages == 0 {
 return html! {};
 }

 html! {
    div class="flex items-center justify-between py-4 px-5" {
        span class="text-[13px] text-muted" {
            "共 "
            (total)
            " 条记录，第 "
            (current_page)
            "/"
            (total_pages)
            " 页"
        }
        div class="flex gap-1" {
            @if current_page > 1 {
                ({
                    htmx_page_link(
                        base_path,
                        query,
                        current_page - 1,
                        "«",
                        Some((hx_target, hx_swap)),
                    )
                })
            }
            @for p in page_range(current_page, total_pages) {
                @if p == 0 {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
                        disabled
                    { "…" }
                } @else if p == current_page {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-accent text-white text-sm font-semibold cursor-pointer"
                        disabled
                    { (p) }
                } @else {
                    ({
                        htmx_page_link(
                            base_path,
                            query,
                            p,
                            &p.to_string(),
                            Some((hx_target, hx_swap)),
                        )
                    })
                }
            }
            @if current_page < total_pages {
                ({
                    htmx_page_link(
                        base_path,
                        query,
                        current_page + 1,
                        "»",
                        Some((hx_target, hx_swap)),
                    )
                })
            }
        }
    }
}
}

/// Lightweight HTMX pagination: links only have `hx-get`, inheriting hx-target/hx-swap
/// from an ancestor container that declares them.
/// `query` carries filter params (e.g. `category_id=3`) so page links preserve the active filter;
/// pass "" when the filter is encoded in the path itself.
pub fn htmx_pagination_inherited(
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
    div class="flex items-center justify-between py-4 px-5" {
        span class="text-[13px] text-muted" {
            "共 "
            (total)
            " 条记录，第 "
            (current_page)
            "/"
            (total_pages)
            " 页"
        }
        div class="flex gap-1" {
            @if current_page > 1 { (htmx_page_link(base_path, query, current_page - 1, "«", None)) }
            @for p in page_range(current_page, total_pages) {
                @if p == 0 {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
                        disabled
                    { "…" }
                } @else if p == current_page {
                    button
                        class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-accent text-white text-sm font-semibold cursor-pointer"
                        disabled
                    { (p) }
                } @else { (htmx_page_link(base_path, query, p, &p.to_string(), None)) }
            }
            @if current_page < total_pages {
                (htmx_page_link(base_path, query, current_page + 1, "»", None))
            }
        }
    }
}
}

fn htmx_page_link(base_path: &str, query: &str, page: u32, label: &str, target_swap: Option<(&str, &str)>) -> Markup {
 // Combine the filter `query` (e.g. "category_id=3") with page=N so paginated
 // requests preserve the active filter. When `query` is empty the filter is
 // assumed to be encoded in the path itself.
 let qs = if query.is_empty() {
 format!("page={page}")
 } else {
 format!("{query}&page={page}")
 };
 let sep = if base_path.contains('?') { '&' } else { '?' };
 let url = format!("{base_path}{sep}{qs}");
 match target_swap {
 Some((t, s)) => html! {
    a   class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
        href=(url)
        hx-get=(url)
        hx-target=(t)
        hx-swap=(s)
    { (label) }
},
 None => html! {
    a   class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
        href=(url)
        hx-get=(url)
    { (label) }
},
 }
}

fn page_link(base_path: &str, query: &str, page: u32, label: &str) -> Markup {
 let qs = if query.is_empty() {
 format!("page={page}")
 } else {
 format!("{query}&page={page}")
 };

 html! {
    a   class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg border border-border-soft transition-colors"
        href=(format!("{base_path}?{qs}"))
    { (label) }
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
