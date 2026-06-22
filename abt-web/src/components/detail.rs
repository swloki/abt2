use maud::{Markup, html};

/// A label-value row used in detail pages.
pub fn detail_row(label: &str, value: Markup) -> Markup {
    html! {
        div class="flex py-2 text-sm" {
            span class="w-[90px] shrink-0 text-muted" { (label) }
            span class="detail-value" { (value) }
        }
    }
}

/// 详情页 Tab 栏（纯前端切换）。`tabs`: `&[(id, label)]`，`active` 为默认激活 id。
/// 与 `tab_panel` 配合使用，CSS 类：`detail-tabs`/`detail-tab`/`tab-panel`。
pub fn detail_tabs(active: &str, tabs: &[(&str, &str)]) -> Markup {
    html! {
        div class="flex gap-0 mb-6 border-b border-border-soft" {
            @for (id, label) in tabs {
                @let cls = if *id == active { "detail-tab active" } else { "detail-tab" };
                button
                    class=({
                        format!(
                            "{} px-5 py-3 text-sm font-medium text-muted cursor-pointer border-b-2 border-transparent -mb-px transition-colors duration-150 hover:text-fg act:text-accent act:border-accent act:font-semibold",
                            cls,
                        )
                    })
                    type="button"
                    onclick=(format!("switchDetailTab('{id}', this)"))
                { (label) }
            }
        }
        ({
            maud::PreEscaped(
                r#"<script>function switchDetailTab(t,b){document.querySelectorAll('.tab-panel').forEach(function(p){p.style.display='none'});document.querySelectorAll('.detail-tab').forEach(function(x){x.classList.remove('active')});var e=document.getElementById('tab-'+t);if(e)e.style.display='';if(b)b.classList.add('active')};setTimeout(function(){var p=new URLSearchParams(location.search);var t=p.get('tab');if(t){var b=document.querySelector('.detail-tab[onclick*="'+t+'"]');if(b)switchDetailTab(t,b)}},0);</script>"#,
            )
        })
    }
}

/// 详情页 Tab 内容面板。`active` 为 true 时默认显示。
pub fn tab_panel(id: &str, active: bool, content: Markup) -> Markup {
    let style = if active { "" } else { "display:none" };
    html! {
        div class="tab-panel" id=(format!("tab-{id}")) style=(style) { (content) }
    }
}
