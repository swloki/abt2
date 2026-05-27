use axum::http::HeaderMap;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use crate::auth::session::CURRENT_USER_KEY;
use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::dashboard::DashboardPath;

// ── Handler ──

pub async fn get_dashboard(
    _path: DashboardPath,
    session: Session,
    headers: HeaderMap,
) -> axum::response::Html<String> {
    let claims = session
        .get::<abt_core::shared::identity::model::Claims>(CURRENT_USER_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| abt_core::shared::identity::model::Claims {
            sub: 0,
            username: "未知用户".into(),
            display_name: "未知用户".into(),
            system_role: "user".into(),
            role_ids: vec![],
            role_codes: vec![],
            department_ids: vec![],
            iss: String::new(),
            exp: 0,
            iat: 0,
        });

    let content = dashboard_content(&claims);
    let page = admin_page(
        &headers, "销售总览", &claims, "sales", DashboardPath::PATH, "销售管理", None, content,
    );
    axum::response::Html(page.into_string())
}

// ── Component ──

fn dashboard_content(claims: &abt_core::shared::identity::model::Claims) -> Markup {
    html! {
        // ── Page Header ──
        div class="page-header" {
            h1 class="page-title" { "销售管理概览" }
            div class="page-actions" {
                span class="text-muted text-[13px]" {
                    "欢迎回来, " (claims.display_name.as_str())
                }
            }
        }

        // ── Stat Cards (4 columns) ──
        div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4);margin-bottom:var(--space-8)" {
            (stat_card("本月报价", "8", "份", "+3 vs 上月", "text-success"))
            (stat_card("进行中订单", "17", "笔", "¥ 1.2M 待发货", "text-warn"))
            (stat_card_with_color("待处理退货", "3", "笔", "¥ 11,020 待处理", "text-danger"))
            (stat_card_accent("本月营收", "¥ 780K", "+12% vs 上月", "text-success"))
        }

        // ── 2-column: 待办事项 + 快捷入口 ──
        div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-6);margin-bottom:var(--space-8)" {
            // 待办事项
            div {
                div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)" {
                    h2 class="section-title" { "待办事项" }
                }
                div class="data-card" {
                    (todo_item("status-progress", "拣货中", "发货申请 SR-2026-0018 待确认发货", "今天"))
                    (todo_item("status-progress", "质检中", "退货单 RT-2026-0009 待质检判定", "今天"))
                    (todo_item("status-info", "已确认", "退货单 RT-2026-0007 待收货确认", "昨天"))
                    (todo_item_last("status-info", "已发送", "报价单 QT-2026-0041 客户未回复", "3天前"))
                }
            }
            // 快捷入口
            div {
                div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)" {
                    h2 class="section-title" { "快捷入口" }
                }
                div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-3)" {
                    (quick_link_card("#", &icon::file_text_icon("w-[28px] h-[28px]"), "报价单", "24 份"))
                    (quick_link_card("#", &icon::box_icon("w-[28px] h-[28px]"), "销售订单", "31 笔"))
                    (quick_link_card("#", &icon::truck_icon("w-[28px] h-[28px]"), "发货申请", "18 单"))
                    (quick_link_card("#", &icon::clipboard_list_icon("w-[28px] h-[28px]"), "月对账单", "14 份"))
                }
            }
        }

        // ── 销售流程 ──
        div style="margin-bottom:var(--space-8)" {
            h2 class="section-title" style="margin-bottom:var(--space-4)" { "销售流程" }
            div style="display:flex;align-items:center;gap:0;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-md);padding:var(--space-6) var(--space-8);overflow-x:auto" {
                (flow_step(&icon::file_text_icon("w-5 h-5"), "报价单", "客户报价", "bg-[color-mix(in_srgb,var(--info)_10%,transparent)]", "text-info"))
                (arrow_right_svg())
                (flow_step(&icon::box_icon("w-5 h-5"), "销售订单", "确认订单", "bg-[color-mix(in_srgb,var(--success)_10%,transparent)]", "text-success"))
                (arrow_right_svg())
                (flow_step(&icon::truck_icon("w-5 h-5"), "发货申请", "拣货发货", "bg-[color-mix(in_srgb,var(--warn)_10%,transparent)]", "text-warn"))
                (arrow_right_svg())
                (flow_step(&icon::return_arrow_icon("w-5 h-5"), "销售退货", "退货处理", "bg-[color-mix(in_srgb,var(--danger)_10%,transparent)]", "text-danger"))
                (arrow_right_svg())
                (flow_step(&icon::clipboard_list_icon("w-5 h-5"), "月对账单", "月度结算", "bg-[color-mix(in_srgb,var(--accent)_10%,transparent)]", "text-accent"))
            }
        }

        // ── 最近活动 ──
        div {
            h2 class="section-title" style="margin-bottom:var(--space-4)" { "最近活动" }
            div class="data-card" {
                (activity_item("status-progress", "订单", "SO-2026-0038 状态变更为 ", "生产中", "10 分钟前"))
                (activity_item("status-progress", "发货", "发货申请 SR-2026-0018 开始拣货", "", "2 小时前"))
                (activity_item("status-progress", "退货", "退货单 RT-2026-0009 进入质检阶段", "", "昨天"))
                (activity_item("status-info", "对账", "对账单 RC-2026-005 已发送给客户", "", "昨天"))
                (activity_item_last("status-success", "报价", "报价单 QT-2026-0042 客户已接受，已转订单", "", "3 天前"))
            }
        }
    }
}

// ── Sub-components ──

fn stat_card(label: &str, value: &str, unit: &str, trend: &str, trend_color: &str) -> Markup {
    html! {
        div class="info-card-flat" {
            span class="info-label" { (label) }
            div style="display:flex;align-items:baseline;gap:var(--space-2);margin-top:var(--space-2)" {
                span class="amount-value text-2xl" { (value) }
                span class="text-muted text-xs" { (unit) }
            }
            div style="margin-top:var(--space-2);font-size:12px" class=(trend_color) { (trend) }
        }
    }
}

fn stat_card_with_color(
    label: &str,
    value: &str,
    unit: &str,
    trend: &str,
    trend_color: &str,
) -> Markup {
    html! {
        div class="info-card-flat" {
            span class="info-label" { (label) }
            div style="display:flex;align-items:baseline;gap:var(--space-2);margin-top:var(--space-2)" {
                span class="amount-value text-2xl text-danger" { (value) }
                span class="text-muted text-xs" { (unit) }
            }
            div style="margin-top:var(--space-2);font-size:12px" class=(trend_color) { (trend) }
        }
    }
}

fn stat_card_accent(label: &str, value: &str, trend: &str, trend_color: &str) -> Markup {
    html! {
        div class="info-card-flat" {
            span class="info-label" { (label) }
            div style="display:flex;align-items:baseline;gap:var(--space-2);margin-top:var(--space-2)" {
                span class="amount-value-accent text-2xl" { (value) }
            }
            div style="margin-top:var(--space-2);font-size:12px" class=(trend_color) { (trend) }
        }
    }
}

fn todo_item(status_class: &str, status_text: &str, desc: &str, time: &str) -> Markup {
    html! {
        div class="activity-row" {
            span class=(status_class) style="font-size:11px" { (status_text) }
            span style="flex:1" { (desc) }
            span class="text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn todo_item_last(status_class: &str, status_text: &str, desc: &str, time: &str) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);display:flex;align-items:center;gap:var(--space-3);cursor:pointer" {
            span class=(status_class) style="font-size:11px" { (status_text) }
            span style="flex:1" { (desc) }
            span class="text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn quick_link_card(href: &str, icon: &Markup, title: &str, count: &str) -> Markup {
    html! {
        a href=(href) class="quick-link" {
            span style="color:var(--accent)" { (icon) }
            span class="text-sm font-semibold text-fg" { (title) }
            span class="text-xs text-muted" { (count) }
        }
    }
}

fn flow_step(icon: &Markup, label: &str, desc: &str, icon_bg: &str, icon_color: &str) -> Markup {
    html! {
        div class="flow-step" {
            div class=(format!("flow-step-icon {}", icon_bg)) {
                span class=(icon_color) { (icon) }
            }
            a href="#" class="text-sm font-semibold text-fg" { (label) }
            span class="text-[11px] text-muted" { (desc) }
        }
    }
}

fn arrow_right_svg() -> Markup {
    html! {
        svg viewBox="0 0 40 20" style="flex-shrink:0;margin:0 var(--space-2)" width="40" height="20" {
            path d="M0 10h32M26 5l6 5-6 5" fill="none" stroke="var(--border)" stroke-width="2" {}
        }
    }
}

fn activity_item(
    status_class: &str,
    status_text: &str,
    desc: &str,
    highlight: &str,
    time: &str,
) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);border-bottom:1px solid var(--border-soft);display:flex;align-items:center;gap:var(--space-4)" {
            span class=(status_class) style="font-size:11px;min-width:56px;justify-content:center" { (status_text) }
            span style="flex:1" {
                (desc)
                @if !highlight.is_empty() {
                    span style="font-weight:600" { " " (highlight) }
                }
            }
            span class="text-muted" style="font-size:12px" { (time) }
        }
    }
}

fn activity_item_last(
    status_class: &str,
    status_text: &str,
    desc: &str,
    highlight: &str,
    time: &str,
) -> Markup {
    html! {
        div style="padding:var(--space-4) var(--space-5);display:flex;align-items:center;gap:var(--space-4)" {
            span class=(status_class) style="font-size:11px;min-width:56px;justify-content:center" { (status_text) }
            span style="flex:1" {
                (desc)
                @if !highlight.is_empty() {
                    span style="font-weight:600" { " " (highlight) }
                }
            }
            span class="text-muted" style="font-size:12px" { (time) }
        }
    }
}
