use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_macros::require_permission;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::dashboard::DashboardPath;
use crate::utils::RequestContext;

// ── Handler ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_dashboard(
 _path: DashboardPath,
 ctx: RequestContext,
) -> crate::errors::Result<axum::response::Html<String>> {
 let content = dashboard_content(&ctx.claims);
 let nav_filter = ctx.nav_filter().await;
 let page = admin_page(
 false, "销售总览", &ctx.claims, "sales", DashboardPath::PATH, "销售管理", None, content, &nav_filter,
 );
 Ok(axum::response::Html(page.into_string()))
}

// ── Component ──

fn dashboard_content(claims: &abt_core::shared::identity::model::Claims) -> Markup {
 html! {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "销售管理概览" }
 span class="text-[13px] text-muted" {
 "欢迎回来, " (claims.display_name.as_str())
 }
 }

 // ── Stat Cards ──
 div class="grid grid-cols-4 gap-4 mb-8" {
 (stat_card("本月报价", "8", "份", "+3 vs 上月", "text-success", None))
 (stat_card("进行中订单", "17", "笔", "¥ 1.2M 待发货", "text-warn", None))
 (stat_card("待处理退货", "3", "笔", "¥ 11,020 待处理", "text-danger", Some("text-danger")))
 (stat_card("本月营收", "¥ 780K", "", "+12% vs 上月", "text-success", Some("text-accent")))
 }

 // ── 2-column: 待办事项 + 快捷入口 ──
 div class="grid grid-cols-2 gap-6 mb-8" {
 // 待办事项
 div {
 h2 class="text-lg font-semibold text-fg mb-4" { "待办事项" }
 div class="data-card" {
 (todo_item("status-progress", "拣货中", "发货申请 SR-2026-0018 待确认发货", "今天", false))
 (todo_item("status-progress", "质检中", "退货单 RT-2026-0009 待质检判定", "今天", false))
 (todo_item("status-info", "已确认", "退货单 RT-2026-0007 待收货确认", "昨天", false))
 (todo_item("status-info", "已发送", "报价单 QT-2026-0041 客户未回复", "3天前", true))
 }
 }
 // 快捷入口
 div {
 h2 class="text-lg font-semibold text-fg mb-4" { "快捷入口" }
 div class="grid grid-cols-2 gap-3" {
 (quick_link_card("/admin/quotations", &icon::file_text_icon("w-7 h-7"), "报价单", "24 份"))
 (quick_link_card("/admin/orders", &icon::box_icon("w-7 h-7"), "销售订单", "31 笔"))
 (quick_link_card("/admin/shipping", &icon::truck_icon("w-7 h-7"), "发货申请", "18 单"))
 (quick_link_card("/admin/reconciliations", &icon::clipboard_list_icon("w-7 h-7"), "月对账单", "14 份"))
 }
 }
 }

 // ── 销售流程 ──
 div class="mb-8" {
 h2 class="text-lg font-semibold text-fg mb-4" { "销售流程" }
 div class="flex items-center overflow-x-auto rounded-md border border-border bg-bg px-8 py-6" {
 (flow_step(&icon::file_text_icon("w-5 h-5"), "报价单", "客户报价", "bg-[#e8f4ff]", "text-info"))
 (arrow_right_svg())
 (flow_step(&icon::box_icon("w-5 h-5"), "销售订单", "确认订单", "bg-success-bg", "text-success"))
 (arrow_right_svg())
 (flow_step(&icon::truck_icon("w-5 h-5"), "发货申请", "拣货发货", "bg-warn-bg", "text-warn"))
 (arrow_right_svg())
 (flow_step(&icon::return_arrow_icon("w-5 h-5"), "销售退货", "退货处理", "bg-danger-bg", "text-danger"))
 (arrow_right_svg())
 (flow_step(&icon::clipboard_list_icon("w-5 h-5"), "月对账单", "月度结算", "bg-[#eff6ff]", "text-accent"))
 }
 }

 // ── 最近活动 ──
 div {
 h2 class="text-lg font-semibold text-fg mb-4" { "最近活动" }
 div class="data-card" {
 (activity_item("status-progress", "订单", "SO-2026-0038 状态变更为 ", "生产中", "10 分钟前", false))
 (activity_item("status-picking", "发货", "发货申请 SR-2026-0018 开始拣货", "", "2 小时前", false))
 (activity_item("status-inspecting", "退货", "退货单 RT-2026-0009 进入质检阶段", "", "昨天", false))
 (activity_item("status-sent", "对账", "对账单 RC-2026-005 已发送给客户", "", "昨天", false))
 (activity_item("status-accepted", "报价", "报价单 QT-2026-0042 客户已接受，已转订单", "", "3 天前", true))
 }
 }
 }
}

// ── Sub-components ──

fn stat_card(label: &str, value: &str, unit: &str, trend: &str, trend_color: &str, value_color: Option<&str>) -> Markup {
 let vcls = value_color.unwrap_or("text-fg");
 html! {
 div class="bg-bg border border-border-soft rounded-md p-5 shadow-sm" {
 span class="text-xs font-medium text-muted" { (label) }
 div class="flex items-baseline gap-2 mt-2" {
 span class=(format!("text-2xl font-bold {}", vcls)) { (value) }
 @if !unit.is_empty() {
 span class="text-xs text-muted" { (unit) }
 }
 }
 div class=(format!("text-xs mt-2 {}", trend_color)) { (trend) }
 }
 }
}

fn todo_item(status_class: &str, status_text: &str, desc: &str, time: &str, last: bool) -> Markup {
 let border = if last { "" } else { " border-b border-border-soft" };
 html! {
 div class=(format!("flex items-center gap-3 px-5 py-4 cursor-pointer hover:bg-accent-bg transition-colors{border}")) {
 span class=(format!("status-pill {} text-[11px]", crate::utils::status_color(status_class))) { (status_text) }
 span class="flex-1 text-sm text-fg" { (desc) }
 span class="text-xs text-muted" { (time) }
 }
 }
}

fn quick_link_card(href: &str, icon: &Markup, title: &str, count: &str) -> Markup {
 html! {
 a href=(href) class="flex flex-col gap-1 p-4 bg-bg border border-border-soft rounded-md no-underline cursor-pointer hover:border-accent hover:bg-accent-bg transition-colors" {
 span class="text-accent" { (icon) }
 span class="text-sm font-semibold text-fg" { (title) }
 span class="text-xs text-muted" { (count) }
 }
 }
}

fn flow_step(icon: &Markup, label: &str, desc: &str, icon_bg: &str, icon_color: &str) -> Markup {
 html! {
 div class="flex-1 flex flex-col items-center" {
 div class=(format!("w-12 h-12 rounded-full flex items-center justify-center mb-2 {}", icon_bg)) {
 span class=(icon_color) { (icon) }
 }
 div class="text-sm font-semibold text-fg" { (label) }
 span class="text-[11px] text-muted" { (desc) }
 }
 }
}

fn arrow_right_svg() -> Markup {
 html! {
 svg class="shrink-0 mx-2 text-border" viewBox="0 0 40 20" width="40" height="20" {
 path d="M0 10h32M26 5l6 5-6 5" fill="none" stroke="currentColor" stroke-width="2" {}
 }
 }
}

fn activity_item(
 status_class: &str,
 status_text: &str,
 desc: &str,
 highlight: &str,
 time: &str,
 last: bool,
) -> Markup {
 let border = if last { "" } else { " border-b border-border-soft" };
 html! {
 div class=(format!("flex items-center gap-4 px-5 py-4{border}")) {
 span class=(format!("status-pill {} text-[11px] min-w-[56px] justify-center", crate::utils::status_color(status_class))) { (status_text) }
 span class="flex-1 text-sm text-fg" {
 (desc)
 @if !highlight.is_empty() {
 span class="font-semibold" { " " (highlight) }
 }
 }
 span class="text-xs text-muted" { (time) }
 }
 }
}
