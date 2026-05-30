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
                NavItem {
                    name: "产品管理",
                    path: "#",
                    icon: NavIcon::Package,
                },
                NavItem {
                    name: "库存管理",
                    path: "#",
                    icon: NavIcon::Archive,
                },
                NavItem {
                    name: "仓库管理",
                    path: "#",
                    icon: NavIcon::Building,
                },
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
                    name: "物料清单",
                    path: "/admin/md/boms",
                    icon: NavIcon::ClipboardDoc,
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
        NavIcon::Database => icon::package_icon(""),
        NavIcon::Wrench => icon::sliders_icon(""),
        NavIcon::Tag => icon::file_text_icon(""),
        NavIcon::Link => icon::link_icon(""),
    }
}

fn render_module_icon(module_id: &str) -> Markup {
    match module_id {
        "sales" => icon::trending_up_icon(""),
        "purchase" => icon::clipboard_module_icon(""),
        "inventory" => icon::package_icon(""),
        "md" => icon::grid_icon(""),
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
        nav id="sidebar" x-bind:class="{ 'sidebar-collapsed': collapsed }" {
            // ── Icon Rail ──
            div class="sidebar-rail" {
                div class="rail-brand" title="ABT ERP" {
                    (icon::box_icon(""))
                }
                div class="rail-modules" {
                    @for m in &mods {
                        @let is_initial_active = m.id == active_mod.id;
                        @let bind_expr = format!("{{ 'active': activeModule === '{}' }}", m.id);
                        @let hx_url = format!("/sidebar/body/{}", m.id);
                        @let click_expr = format!("activeModule = '{}'", m.id);
                        button class=(if is_initial_active { "rail-item active" } else { "rail-item" })
                           x-bind:class=(bind_expr)
                           hx-get=(hx_url)
                           hx-target=".sidebar-body"
                           hx-swap="innerHTML"
                           x-on:click=(click_expr)
                           title=(m.name) {
                            span class="rail-icon" { (render_module_icon(m.id)) }
                            span class="rail-label" { (m.name.replace("管理", "")) }
                        }
                    }
                }
                div class="rail-bottom" {
                    button class="rail-item rail-collapse"
                            x-on:click="collapsed = !collapsed"
                            x-bind:title="collapsed ? '展开侧栏' : '收起侧栏'" {
                        (icon::sidebar_toggle_icon(""))
                        span class="rail-label" x-text="collapsed ? '展开' : '收起'" { "收起" }
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

/// Renders module header + nav items (for initial page load, with active item highlight).
fn sidebar_body_fragment_inner(active_mod: &NavModule, current_path: &str) -> Markup {
    html! {
        div class="sidebar-module-header" {
            span class="module-header-icon" { (render_module_icon(active_mod.id)) }
            span class="module-header-name" { (active_mod.name) }
        }
        div class="sidebar-nav" {
            @for item in &active_mod.items {
                a href=(item.path) class=(item_class(item.path == current_path)) {
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

    html! {
        nav class="mobile-nav" {
            div class="mobile-nav-scroll" {
                div class="mobile-nav-inner" {
                    @for item in &active_mod.items {
                        a href=(item.path) class=(mobile_class(item.path == current_path)) {
                            (render_item_icon(item.icon))
                            span { (item.name) }
                        }
                    }
                }
            }
        }
    }
}
