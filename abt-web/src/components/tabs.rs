use maud::{html, Markup};

pub struct TabItem {
    pub value: String,
    pub label: &'static str,
    pub count: Option<u64>,
}

pub fn status_tabs(
    hx_get: &str,
    hx_target: &str,
    hx_include: &str,
    tabs: &[TabItem],
    active_value: &str,
) -> Markup {
    status_tabs_with_param(hx_get, hx_target, hx_include, tabs, active_value, "status")
}

pub fn status_tabs_with_param(
    hx_get: &str,
    hx_target: &str,
    hx_include: &str,
    tabs: &[TabItem],
    active_value: &str,
    param_name: &str,
) -> Markup {
    html! {
        div id="status-tabs" class="status-tabs" {
            @for tab in tabs {
                (status_tab(hx_get, hx_target, hx_include, tab, active_value, param_name))
            }
        }
    }
}

fn status_tab(hx_get: &str, hx_target: &str, hx_include: &str, tab: &TabItem, active_value: &str, param_name: &str) -> Markup {
    let is_active = tab.value == active_value;
    let class = if is_active { "status-tab active" } else { "status-tab" };
    let vals = if tab.value.is_empty() {
        format!("{{\"{param_name}\": \"\"}}")
    } else {
        format!("{{\"{param_name}\": \"{}\"}}", tab.value)
    };

    html! {
        a class=(class)
            hx-get=(hx_get)
            hx-target=(hx_target)
            hx-select=(hx_target)
            hx-select-oob="#status-tabs"
            hx-swap="outerHTML"
            hx-vals=(vals)
            hx-include=(hx_include) {
            (tab.label)
            @if let Some(c) = tab.count {
                span class="count" { (c) }
            }
        }
    }
}
