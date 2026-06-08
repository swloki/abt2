use abt_core::shared::identity::model::Claims;
use maud::{Markup, html};

use crate::components::icon;

// ── Navigation Data ──

#[derive(Clone, Copy)]
enum NavIcon {
    Home,
    Users,
    File,
    Package,
    Truck,
    Return,
    Check,
    Grid,
    Building,
    ClipboardDoc,
    Payment,
    Sliders,
    Archive,
    Database,
    Wrench,
    Tag,
    Link,
    Lock,
    Search,
    ArrowDown,
    ArrowUp,
    Switch,
    Refresh,
    Lightning,
    Factory,
    Calendar,
    Layers,
    Hammer,
    Eye,
    DollarSign,
    AlertTriangle,
}
struct NavItem {
    name: &'static str,
    path: &'static str,
    icon: NavIcon,
}

struct NavModule {
    id: &'static str,
    name: &'static str,
    items: Vec<NavItem>,
}

fn modules() -> Vec<NavModule> {
    vec![
        NavModule {
            id: "sales",
            name: "销售管理",
            items: vec![
                NavItem {
                    name: "销售总览",
                    path: "/admin",
                    icon: NavIcon::Home,
                },
                NavItem {
                    name: "客户管理",
                    path: "/admin/customers",
                    icon: NavIcon::Users,
                },
                NavItem {
                    name: "报价单",
                    path: "/admin/quotations",
                    icon: NavIcon::File,
                },
                NavItem {
                    name: "销售订单",
                    path: "/admin/orders",
                    icon: NavIcon::Package,
                },
                NavItem {
                    name: "发货申请",
                    path: "/admin/shipping",
                    icon: NavIcon::Truck,
                },
                NavItem {
                    name: "销售退货",
                    path: "/admin/returns",
                    icon: NavIcon::Return,
                },
                NavItem {
                    name: "月对账单",
                    path: "/admin/reconciliations",
                    icon: NavIcon::Check,
                },
            ],
        },
        NavModule {
            id: "purchase",
            name: "采购管理",
            items: vec![
                NavItem {
                    name: "采购总览",
                    path: "/admin/purchase",
                    icon: NavIcon::Home,
                },
                NavItem {
                    name: "采购报价",
                    path: "/admin/purchase/quotations",
                    icon: NavIcon::File,
                },
                NavItem {
                    name: "采购订单",
                    path: "/admin/purchase/orders",
                    icon: NavIcon::ClipboardDoc,
                },
                NavItem {
                    name: "采购退货",
                    path: "/admin/purchase/returns",
                    icon: NavIcon::Return,
                },
                NavItem {
                    name: "采购对账",
                    path: "/admin/purchase/reconciliations",
                    icon: NavIcon::Check,
                },
                NavItem {
                    name: "付款申请",
                    path: "/admin/purchase/payments",
                    icon: NavIcon::Payment,
                },
                NavItem {
                    name: "零星请购",
                    path: "/admin/purchase/misc-requests",
                    icon: NavIcon::Sliders,
                },
            ],
        },
        NavModule {
            id: "inventory",
            name: "库存管理",
            items: vec![
                NavItem { name: "库存总览", path: "/admin/wms", icon: NavIcon::Home },
                NavItem { name: "仓库管理", path: "/admin/wms/warehouses", icon: NavIcon::Building },
                NavItem { name: "储位管理", path: "/admin/wms/bins", icon: NavIcon::Database },
                NavItem { name: "库存查询", path: "/admin/wms/stock", icon: NavIcon::Search },
                NavItem { name: "入库管理", path: "/admin/wms/stock-in", icon: NavIcon::ArrowDown },
                NavItem { name: "出库管理", path: "/admin/wms/stock-out", icon: NavIcon::ArrowUp },
                NavItem { name: "来料通知", path: "/admin/wms/arrivals", icon: NavIcon::Truck },
                NavItem { name: "库存调拨", path: "/admin/wms/transfers", icon: NavIcon::Switch },
                NavItem { name: "领料单", path: "/admin/wms/requisitions", icon: NavIcon::ClipboardDoc },
                NavItem { name: "形态转换", path: "/admin/wms/conversions", icon: NavIcon::Refresh },
                NavItem { name: "倒冲记录", path: "/admin/wms/backflushes", icon: NavIcon::Lightning },
                NavItem { name: "循环盘点", path: "/admin/wms/cycle-counts", icon: NavIcon::Check },
                NavItem { name: "库存锁定", path: "/admin/wms/locks", icon: NavIcon::Lock },
                NavItem { name: "策略管理", path: "/admin/wms/strategies", icon: NavIcon::Sliders },
                NavItem { name: "事务日志", path: "/admin/wms/transactions", icon: NavIcon::File },
                NavItem { name: "级联查询", path: "/admin/wms/cascade", icon: NavIcon::Search },
            ],
        },
        NavModule {
            id: "production",
            name: "生产管理",
            items: vec![
                NavItem { name: "生产总览", path: "/admin/mes", icon: NavIcon::Home },
                NavItem { name: "生产计划", path: "/admin/mes/plans", icon: NavIcon::Calendar },
                NavItem { name: "工单管理", path: "/admin/mes/orders", icon: NavIcon::ClipboardDoc },
                NavItem { name: "生产批次", path: "/admin/mes/batches", icon: NavIcon::Layers },
                NavItem { name: "流转卡查询", path: "/admin/mes/cards", icon: NavIcon::Search },
                NavItem { name: "排程看板", path: "/admin/mes/schedule", icon: NavIcon::Grid },
                NavItem { name: "报工记录", path: "/admin/mes/reports", icon: NavIcon::Hammer },
                NavItem { name: "计件工资", path: "/admin/mes/wages", icon: NavIcon::DollarSign },
                NavItem { name: "生产报检", path: "/admin/mes/inspections", icon: NavIcon::Eye },
                NavItem { name: "完工入库", path: "/admin/mes/receipts", icon: NavIcon::ArrowDown },
                NavItem { name: "物料消耗", path: "/admin/mes/material-usage", icon: NavIcon::Package },
                NavItem { name: "生产异常", path: "/admin/mes/exceptions", icon: NavIcon::AlertTriangle },
            ],
        },
        NavModule {
            id: "outsourcing",
            name: "委外管理",
            items: vec![
                NavItem { name: "委外总览", path: "/admin/om", icon: NavIcon::Home },
                NavItem { name: "委外单管理", path: "/admin/om/outsourcing", icon: NavIcon::ClipboardDoc },
                NavItem { name: "追踪管理", path: "/admin/om/tracking", icon: NavIcon::Search },
            ],
        },
        NavModule {
            id: "quality",
            name: "质量管理",
            items: vec![
                NavItem { name: "质量总览", path: "/admin/qms", icon: NavIcon::Home },
                NavItem { name: "检验规格", path: "/admin/qms/specs", icon: NavIcon::File },
                NavItem { name: "检验结果", path: "/admin/qms/results", icon: NavIcon::Check },
                NavItem { name: "MRB评审", path: "/admin/qms/mrb", icon: NavIcon::AlertTriangle },
                NavItem { name: "RMA客诉", path: "/admin/qms/rma", icon: NavIcon::Return },
            ],
        },
        NavModule {
            id: "md",
            name: "主数据",
            items: vec![
                NavItem {
                    name: "主数据总览",
                    path: "/admin/md",
                    icon: NavIcon::Home,
                },
                NavItem {
                    name: "产品管理",
                    path: "/admin/md/products",
                    icon: NavIcon::Package,
                },
                NavItem {
                    name: "产品分类",
                    path: "/admin/md/categories",
                    icon: NavIcon::Tag,
                },
                NavItem {
                    name: "BOM管理",
                    path: "/admin/md/boms",
                    icon: NavIcon::ClipboardDoc,
                },
                NavItem {
                    name: "电源BOM",
                    path: "/admin/md/boms?category_name=电源",
                    icon: NavIcon::ClipboardDoc,
                },
                NavItem {
                    name: "模组BOM",
                    path: "/admin/md/boms?category_name=模组",
                    icon: NavIcon::ClipboardDoc,
                },
                NavItem {
                    name: "工序字典",
                    path: "/admin/md/process-dicts",
                    icon: NavIcon::Database,
                },
                NavItem {
                    name: "工艺路线",
                    path: "/admin/md/routings",
                    icon: NavIcon::Wrench,
                },
                NavItem {
                    name: "供应商管理",
                    path: "/admin/md/suppliers",
                    icon: NavIcon::Building,
                },
            ],
        },
        NavModule {
            id: "system",
            name: "系统管理",
            items: vec![
                NavItem {
                    name: "用户管理",
                    path: "/admin/system/users",
                    icon: NavIcon::Users,
                },
                NavItem {
                    name: "角色管理",
                    path: "/admin/system/roles",
                    icon: NavIcon::Lock,
                },
                NavItem {
                    name: "部门管理",
                    path: "/admin/system/departments",
                    icon: NavIcon::Building,
                },

            ],
        },
    ]
}

// ── Helpers ──

fn render_item_icon(ni: NavIcon) -> Markup {
    match ni {
        NavIcon::Home => icon::home_icon(""),
        NavIcon::Users => icon::users_icon(""),
        NavIcon::File => icon::file_text_icon(""),
        NavIcon::Package => icon::box_icon(""),
        NavIcon::Truck => icon::truck_icon(""),
        NavIcon::Return => icon::return_arrow_icon(""),
        NavIcon::Check => icon::clipboard_list_icon(""),
        NavIcon::Grid => icon::grid_icon(""),
        NavIcon::Building => icon::building_icon(""),
        NavIcon::ClipboardDoc => icon::clipboard_document_icon(""),
        NavIcon::Payment => icon::payment_icon(""),
        NavIcon::Sliders => icon::sliders_icon(""),
        NavIcon::Archive => icon::package_icon(""),
        NavIcon::Database => icon::grid_icon(""),
        NavIcon::Wrench => icon::sliders_icon(""),
        NavIcon::Tag => icon::file_text_icon(""),
        NavIcon::Link => icon::link_icon(""),
        NavIcon::Lock => icon::lock_icon(""),
        NavIcon::Search => icon::search_icon(""),
        NavIcon::ArrowDown => icon::download_icon(""),
        NavIcon::ArrowUp => icon::upload_icon(""),
        NavIcon::Switch => icon::refresh_icon(""),
        NavIcon::Refresh => icon::refresh_icon(""),
        NavIcon::Lightning => icon::bolt_icon(""),
        NavIcon::Factory => icon::box_icon(""),
        NavIcon::Calendar => icon::file_text_icon(""),
        NavIcon::Layers => icon::grid_icon(""),
        NavIcon::Hammer => icon::sliders_icon(""),
        NavIcon::Eye => icon::search_icon(""),
        NavIcon::DollarSign => icon::payment_icon(""),
        NavIcon::AlertTriangle => icon::circle_alert_icon(""),
    }
}

fn render_module_icon(module_id: &str) -> Markup {
    match module_id {
        "sales" => icon::trending_up_icon(""),
        "purchase" => icon::clipboard_module_icon(""),
        "inventory" => icon::package_icon(""),
        "production" => icon::box_icon(""),
        "outsourcing" => icon::truck_icon(""),
        "md" => icon::grid_icon(""),
        "quality" => icon::check_circle_icon(""),
        "system" => icon::lock_icon(""),
        _ => html! {},
    }
}

fn item_class(active: bool) -> &'static str {
    if active {
        "sidebar-item active"
    } else {
        "sidebar-item"
    }
}

fn mobile_class(active: bool) -> &'static str {
    if active {
        "mobile-nav-item active"
    } else {
        "mobile-nav-item"
    }
}

pub fn avatar_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "?".into();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 2 {
        return trimmed.to_uppercase();
    }
    chars[chars.len() - 2..].iter().collect()
}

fn find_module(id: &str) -> Option<usize> {
    modules().iter().position(|m| m.id == id)
}

// ── Sidebar Body Fragment (used by HTMX endpoint) ──

pub fn sidebar_body_fragment(claims: &Claims, active_module: &str) -> Markup {
    let mods = modules();
    let active_mod = &mods[find_module(active_module).unwrap_or(0)];

    html! {
        div class="sidebar-module-header" {
            span class="module-header-icon" { (render_module_icon(active_mod.id)) }
            span class="module-header-name" { (active_mod.name) }
        }
        div class="sidebar-nav" {
            @for item in &active_mod.items {
                a href=(item.path) class=(item_class(false)) {
                    (render_item_icon(item.icon))
                    span class="sidebar-item-text" { (item.name) }
                }
            }
        }
        div class="sidebar-user" {
            div class="sidebar-user-avatar" { (avatar_initials(&claims.display_name)) }
            div class="sidebar-user-info" {
                div class="sidebar-user-name" { (claims.display_name.as_str()) }
                div class="sidebar-user-role" { (claims.system_role.as_str()) }
            }
        }
    }
}

// ── Full Sidebar (initial page load) ──

pub fn sidebar(claims: &Claims, active_module: &str, current_path: &str) -> Markup {
    let mods = modules();
    let active_mod = &mods[find_module(active_module).unwrap_or(0)];

    html! {
        nav id="sidebar" {
            // ── Icon Rail ──
            div class="sidebar-rail" {
                div class="rail-brand" title="ABT ERP" {
                    (icon::box_icon(""))
                }
                div class="rail-modules" {
                    @for m in &mods {
                        @let is_initial_active = m.id == active_mod.id;
                        @let hx_url = format!("/sidebar/body/{}", m.id);
                        button class=(if is_initial_active { "rail-item active" } else { "rail-item" })
                           hx-get=(hx_url)
                           hx-target=".sidebar-body"
                           hx-swap="innerHTML"
                           onclick="hsTake(this,'.rail-item','active')"
                           title=(m.name) {
                            span class="rail-icon" { (render_module_icon(m.id)) }
                            span class="rail-label" { (m.name.replace("管理", "")) }
                        }
                    }
                }
                div class="rail-bottom" {
                    button class="rail-item rail-collapse"
                            onclick="hsToggleSidebar()"
                            title="收起侧栏" {
                        (icon::sidebar_toggle_icon(""))
                        span class="rail-label" { "收起" }
                    }
                }
            }

            // ── Sidebar Body ──
            div class="sidebar-body" {
                (sidebar_body_fragment_inner(active_mod, current_path))
                div class="sidebar-user" {
                    div class="sidebar-user-avatar" { (avatar_initials(&claims.display_name)) }
                    div class="sidebar-user-info" {
                        div class="sidebar-user-name" { (claims.display_name.as_str()) }
                        div class="sidebar-user-role" { (claims.system_role.as_str()) }
                    }
                }
            }
        }
    }
}

/// Checks if a menu item should be highlighted for the given current path.
/// Exact match first, then longest-prefix match (item path must be a path segment prefix).
fn is_active(item_path: &str, current_path: &str) -> bool {
    if item_path == current_path {
        return true;
    }
    // current_path must start with item_path followed by '/' or '?'
    // e.g. "/admin/md/boms" matches "/admin/md/boms/new" and "/admin/md/boms/123/edit"
    current_path.starts_with(item_path)
        && current_path.len() > item_path.len()
        && matches!(current_path.as_bytes()[item_path.len()], b'/' | b'?')
}

/// Find the best matching nav item path: exact match preferred, then longest prefix.
fn find_active_path<'a>(items: &'a [NavItem], current_path: &str) -> Option<&'a str> {
    let mut best: Option<(&'a str, bool)> = None;
    for item in items {
        let exact = item.path == current_path;
        let prefix = !exact && is_active(item.path, current_path);
        if exact || prefix {
            let better = match best {
                None => true,
                Some((prev, prev_exact)) => {
                    // Exact match always beats prefix; longer match wins within same tier
                    if exact && !prev_exact { true }
                    else if exact == prev_exact { item.path.len() > prev.len() }
                    else { false }
                }
            };
            if better {
                best = Some((item.path, exact));
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Renders module header + nav items (for initial page load, with active item highlight).
fn sidebar_body_fragment_inner(active_mod: &NavModule, current_path: &str) -> Markup {
    let active_path = find_active_path(&active_mod.items, current_path);
    html! {
        div class="sidebar-module-header" {
            span class="module-header-icon" { (render_module_icon(active_mod.id)) }
            span class="module-header-name" { (active_mod.name) }
        }
        div class="sidebar-nav" {
            @for item in &active_mod.items {
                a href=(item.path) class=(item_class(active_path == Some(item.path))) {
                    (render_item_icon(item.icon))
                    span class="sidebar-item-text" { (item.name) }
                }
            }
        }
    }
}

// ── Mobile Nav ──

pub fn mobile_nav(active_module: &str, current_path: &str) -> Markup {
    let mods = modules();
    let active_mod = &mods[find_module(active_module).unwrap_or(0)];
    let active_path = find_active_path(&active_mod.items, current_path);

    html! {
        nav class="mobile-nav" {
            div class="mobile-nav-scroll" {
                div class="mobile-nav-inner" {
                    @for item in &active_mod.items {
                        a href=(item.path) class=(mobile_class(active_path == Some(item.path))) {
                            (render_item_icon(item.icon))
                            span { (item.name) }
                        }
                    }
                }
            }
        }
    }
}
