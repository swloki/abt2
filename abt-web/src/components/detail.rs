use maud::{Markup, html};

/// A label-value row used in detail pages.
pub fn detail_row(label: &str, value: Markup) -> Markup {
    html! {
        div class="detail-row" {
            span class="detail-label" { (label) }
            span class="detail-value" { (value) }
        }
    }
}
