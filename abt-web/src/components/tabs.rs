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
 status_tabs_with_oob(hx_get, hx_target, hx_include, "#status-tabs", tabs, active_value, param_name)
}

/// Like [`status_tabs_with_param`], but with a configurable `hx-select-oob`
/// (the default hard-codes `#status-tabs`).
///
/// Use this when switching tabs must also refresh extra regions — e.g. a
/// filter form that carries a hidden status input. Without re-rendering that
/// form on tab switch, the hidden status goes stale, so a later filter
/// re-submit (search, or an event-driven refresh after a row action such as
/// toggle) sends the old status and the view jumps back to the first tab.
pub fn status_tabs_with_oob(
 hx_get: &str,
 hx_target: &str,
 hx_include: &str,
 hx_select_oob: &str,
 tabs: &[TabItem],
 active_value: &str,
 param_name: &str,
) -> Markup {
 html! {
 div id="status-tabs" class="flex gap-1 mb-6 border-b border-border-soft" {
 @for tab in tabs {
 (status_tab(hx_get, hx_target, hx_include, hx_select_oob, tab, active_value, param_name))
 }
 }
 }
}

fn status_tab(hx_get: &str, hx_target: &str, hx_include: &str, hx_select_oob: &str, tab: &TabItem, active_value: &str, param_name: &str) -> Markup {
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
 hx-select-oob=(hx_select_oob)
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
