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
 div id="status-tabs" class="flex gap-1 mb-6 border-b border-border-soft" {
 @for tab in tabs {
 (status_tab(hx_get, hx_target, hx_include, tab, active_value, param_name))
 }
 }
 }
}

fn status_tab(hx_get: &str, hx_target: &str, hx_include: &str, tab: &TabItem, active_value: &str, param_name: &str) -> Markup {
 let is_active = tab.value == active_value;
 let class = if is_active {
 "status-tab active px-4 py-3 text-sm text-accent font-semibold cursor-pointer whitespace-nowrap relative [border-bottom:2px_solid_var(--accent)] -mb-px"
 } else {
 "status-tab px-4 py-3 text-sm text-muted cursor-pointer whitespace-nowrap relative [border-bottom:2px_solid_transparent] -mb-px hover:text-fg transition-colors"
 };
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
 hx-push-url="true"
 hx-vals=(vals)
 hx-include=(hx_include) {
 (tab.label)
 @if let Some(c) = tab.count {
 span class="text-[11px] bg-surface px-1.5 py-0.5 rounded-full text-muted font-medium ml-1" { (c) }
 }
 }
 }
}
