use maud::{html, Markup};

/// 列表分页组件（`status_tabs` 同款 HTMX 局部刷新）。分页/搜索/筛选统一走列表端点。
///
/// 每个链接自带 `hx-get` + `hx-target` + `hx-select` + `hx-vals={"page":N}` +
/// `hx-include=form_sel`：page 经 `hx-vals` 传（htmx 原生传参），筛选由 `hx-include`
/// 携带，**无需 hidden page input、无 hyperscript**。点击 → HTMX 局部替换 `target_sel`。
///
/// - `target_sel`：替换目标选择器（如 `"#order-data-card"`、`".transaction-panel"`）
/// - `form_sel`：携带筛选的 form 选择器（如 `"#filter-form"`）；同页多 form 各传各的
pub fn pagination(
    base_path: &str,
    target_sel: &str,
    form_sel: &str,
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
                @if current_page > 1 {
                    (page_link(base_path, target_sel, form_sel, current_page - 1, "«"))
                }
                @for p in page_range(current_page, total_pages) {
                    @if p == 0 {
                        (ellipsis())
                    } @else if p == current_page {
                        (active_page(p))
                    } @else {
                        (page_link(base_path, target_sel, form_sel, p, &p.to_string()))
                    }
                }
                @if current_page < total_pages {
                    (page_link(base_path, target_sel, form_sel, current_page + 1, "»"))
                }
            }
        }
    }
}

fn page_link(base_path: &str, target_sel: &str, form_sel: &str, page: u32, label: &str) -> Markup {
    // href 仅作无 JS 降级（丢筛选可接受；正常路径走 hx-get + hx-include 携带筛选）。
    let vals = format!("{{\"page\":\"{page}\"}}");
    html! {
        a class=(link_class())
          href=(format!("{base_path}?page={page}"))
          hx-get=(base_path)
          hx-target=(target_sel)
          hx-select=(target_sel)
          hx-swap="outerHTML"
          hx-vals=(vals)
          hx-include=(form_sel)
        { (label) }
    }
}

fn ellipsis() -> Markup {
    html! {
        button class=(link_class()) disabled { "…" }
    }
}

fn active_page(p: u32) -> Markup {
    html! {
        button
            class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-accent text-white text-sm font-semibold cursor-pointer"
            disabled
        { (p) }
    }
}

fn link_class() -> &'static str {
    "w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-white text-fg-2 text-sm cursor-pointer hover:bg-surface hover:text-fg transition-colors"
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
