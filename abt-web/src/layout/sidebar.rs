use std::collections::HashSet;

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
    _Archive,
    Database,
    Wrench,
    Tag,
    _Link,
    Lock,
    Search,
    ArrowDown,
    ArrowUp,
    Switch,
    Refresh,
    Lightning,
    _Factory,
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
    /// (resource_code, action) — None means always visible
    permission: Option<(&'static str, &'static str)>,
}

struct NavModule {
    id: &'static str,
    name: &'static str,
    items: Vec<NavItem>,
}

// ── NavFilter ──

/// Pre-computed permission filter for sidebar rendering.
/// `None` inner means super admin — show everything.
pub struct NavFilter {
    permissions: Option<HashSet<String>>,
}

impl NavFilter {
    pub fn new(is_super_admin: bool, permissions: HashSet<String>) -> Self {
        if is_super_admin {
            Self { permissions: None }
        } else {
            Self {
                permissions: Some(permissions),
            }
        }
    }

    fn is_item_visible(&self, item: &NavItem) -> bool {
        match (&self.permissions, &item.permission) {
            (None, _) => true,
            (_, None) => true,
            (Some(perms), Some((r, a))) => perms.contains(&format!("{r}:{a}")),
        }
    }

    fn visible_items<'a>(&self, module: &'a NavModule) -> Vec<&'a NavItem> {
        module
            .items
            .iter()
            .filter(|i| self.is_item_visible(i))
            .collect()
    }

    fn has_visible_items(&self, module: &NavModule) -> bool {
        module.items.iter().any(|i| self.is_item_visible(i))
    }
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
                    permission: Some(("SALES_ORDER", "read")),
                },
                NavItem {
                    name: "客户管理",
                    path: "/admin/customers",
                    icon: NavIcon::Users,
                    permission: Some(("CUSTOMER", "read")),
                },
                NavItem {
                    name: "报价单",
                    path: "/admin/quotations",
                    icon: NavIcon::File,
                    permission: Some(("SALES_ORDER", "read")),
                },
                NavItem {
                    name: "销售订单",
                    path: "/admin/orders",
                    icon: NavIcon::Package,
                    permission: Some(("SALES_ORDER", "read")),
                },
                NavItem {
                    name: "销售退货",
                    path: "/admin/returns",
                    icon: NavIcon::Return,
                    permission: Some(("SHIPPING", "read")),
                },
                NavItem {
                    name: "月对账单",
                    path: "/admin/reconciliations",
                    icon: NavIcon::Check,
                    permission: Some(("SALES_ORDER", "read")),
                },
            ],
        },
        NavModule {
            id: "purchase",
            name: "采购管理",
            items: vec![
                NavItem {
                    name: "采购作业中心",
                    path: "/admin/purchase/work-center",
                    icon: NavIcon::Grid,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购总览",
                    path: "/admin/purchase",
                    icon: NavIcon::Home,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购需求池",
                    path: "/admin/purchase/demand-pool",
                    icon: NavIcon::Layers,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购报价",
                    path: "/admin/purchase/quotations",
                    icon: NavIcon::File,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购订单",
                    path: "/admin/purchase/orders",
                    icon: NavIcon::ClipboardDoc,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购退货",
                    path: "/admin/purchase/returns",
                    icon: NavIcon::Return,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购对账",
                    path: "/admin/purchase/reconciliations",
                    icon: NavIcon::Check,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "付款申请",
                    path: "/admin/purchase/payments",
                    icon: NavIcon::Payment,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "零星请购",
                    path: "/admin/purchase/misc-requests",
                    icon: NavIcon::Sliders,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "供应商价格目录",
                    path: "/admin/purchase/supplier-prices",
                    icon: NavIcon::Tag,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "供应商管理",
                    path: "/admin/md/suppliers",
                    icon: NavIcon::Building,
                    permission: Some(("SUPPLIER", "read")),
                },
                NavItem {
                    name: "采购审批规则",
                    path: "/admin/purchase/approval-rules",
                    icon: NavIcon::Lock,
                    permission: Some(("PURCHASE_ORDER", "read")),
                },
                NavItem {
                    name: "采购参数设置",
                    path: "/admin/purchase/settings",
                    icon: NavIcon::Wrench,
                    permission: Some(("SUPPLIER", "read")),
                },
            ],
        },
        NavModule {
            id: "inventory",
            name: "库存管理",
            items: vec![
                NavItem { name: "作业中心", path: "/admin/wms/work-center", icon: NavIcon::Grid, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "库存总览", path: "/admin/wms", icon: NavIcon::Home, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "仓库管理", path: "/admin/wms/warehouses", icon: NavIcon::Building, permission: Some(("WAREHOUSE", "read")) },
                NavItem { name: "库位管理", path: "/admin/wms/bins", icon: NavIcon::Database, permission: Some(("LOCATION", "read")) },
                NavItem { name: "库存查询", path: "/admin/wms/stock", icon: NavIcon::Search, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "低库存预警", path: "/admin/wms/low-stock", icon: NavIcon::AlertTriangle, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "入库管理", path: "/admin/wms/stock-in", icon: NavIcon::ArrowDown, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "出库管理", path: "/admin/wms/shipping", icon: NavIcon::ArrowUp, permission: Some(("SHIPPING", "read")) },
                NavItem { name: "库存调拨", path: "/admin/wms/transfers", icon: NavIcon::Switch, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "领料单", path: "/admin/wms/requisitions", icon: NavIcon::ClipboardDoc, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "形态转换", path: "/admin/wms/conversions", icon: NavIcon::Refresh, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "倒冲记录", path: "/admin/wms/backflushes", icon: NavIcon::Lightning, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "循环盘点", path: "/admin/wms/cycle-counts", icon: NavIcon::Check, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "库存锁定", path: "/admin/wms/locks", icon: NavIcon::Lock, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "策略管理", path: "/admin/wms/strategies", icon: NavIcon::Sliders, permission: Some(("WAREHOUSE", "read")) },
                NavItem { name: "事务日志", path: "/admin/wms/transactions", icon: NavIcon::File, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "级联查询", path: "/admin/wms/cascade", icon: NavIcon::Search, permission: Some(("INVENTORY", "read")) },
                NavItem { name: "WMS 设置", path: "/admin/wms/settings", icon: NavIcon::Sliders, permission: Some(("INVENTORY", "read")) },
            ],
        },
        NavModule {
            id: "production",
            name: "生产管理",
            items: vec![
                NavItem { name: "生产总览", path: "/admin/mes", icon: NavIcon::Home, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "生产作业中心", path: "/admin/mes/work-center", icon: NavIcon::Grid, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "生产需求池", path: "/admin/mes/demand-pool", icon: NavIcon::Layers, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "工单管理", path: "/admin/mes/orders", icon: NavIcon::ClipboardDoc, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "流转卡查询", path: "/admin/mes/cards", icon: NavIcon::Search, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "排程看板", path: "/admin/mes/schedule", icon: NavIcon::Grid, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "报工记录", path: "/admin/mes/reports", icon: NavIcon::Hammer, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "计件工资", path: "/admin/mes/wages", icon: NavIcon::DollarSign, permission: Some(("LABOR_COST", "read")) },
                NavItem { name: "生产报检", path: "/admin/mes/inspections", icon: NavIcon::Eye, permission: Some(("INSPECTION", "read")) },
                NavItem { name: "完工入库", path: "/admin/mes/receipts", icon: NavIcon::ArrowDown, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "生产异常", path: "/admin/mes/exceptions", icon: NavIcon::AlertTriangle, permission: Some(("WORK_ORDER", "read")) },
                NavItem { name: "工序字典", path: "/admin/md/process-dicts", icon: NavIcon::Database, permission: Some(("BOM", "read")) },
                NavItem { name: "工艺路线", path: "/admin/md/routings", icon: NavIcon::Wrench, permission: Some(("BOM", "read")) },
                NavItem { name: "工作中心", path: "/admin/md/work-centers", icon: NavIcon::Hammer, permission: Some(("BOM", "read")) },
                NavItem { name: "工作日历", path: "/admin/md/work-calendars", icon: NavIcon::Calendar, permission: Some(("BOM", "read")) },
            ],
        },
        NavModule {
            id: "outsourcing",
            name: "委外管理",
            items: vec![
                NavItem { name: "委外总览", path: "/admin/om", icon: NavIcon::Home, permission: Some(("PURCHASE_ORDER", "read")) },
                NavItem { name: "委外单管理", path: "/admin/om/outsourcing", icon: NavIcon::ClipboardDoc, permission: Some(("PURCHASE_ORDER", "read")) },
                NavItem { name: "追踪管理", path: "/admin/om/tracking", icon: NavIcon::Search, permission: Some(("PURCHASE_ORDER", "read")) },
            ],
        },
        NavModule {
            id: "quality",
            name: "质量管理",
            items: vec![
                NavItem { name: "质量总览", path: "/admin/qms", icon: NavIcon::Home, permission: Some(("INSPECTION", "read")) },
                NavItem { name: "检验规格", path: "/admin/qms/specs", icon: NavIcon::File, permission: Some(("INSPECTION", "read")) },
                NavItem { name: "检验结果", path: "/admin/qms/results", icon: NavIcon::Check, permission: Some(("INSPECTION", "read")) },
                NavItem { name: "MRB评审", path: "/admin/qms/mrb", icon: NavIcon::AlertTriangle, permission: Some(("INSPECTION", "read")) },
                NavItem { name: "RMA客诉", path: "/admin/qms/rma", icon: NavIcon::Return, permission: Some(("INSPECTION", "read")) },
            ],
        },
        NavModule {
            id: "finance",
            name: "财务管理",
            items: vec![
                NavItem { name: "财务总览", path: "/admin/fms", icon: NavIcon::Home, permission: Some(("FMS", "read")) },
                NavItem { name: "出纳日记账", path: "/admin/fms/journals", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "核销管理", path: "/admin/fms/writeoffs", icon: NavIcon::Check, permission: Some(("FMS", "read")) },
                NavItem { name: "应收台账", path: "/admin/fms/ar-ledger", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "应付台账", path: "/admin/fms/ap-ledger", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "应收调整", path: "/admin/fms/ar-adjustments", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "应付调整", path: "/admin/fms/ap-adjustments", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "应收账龄", path: "/admin/fms/ar-aging", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "应付账龄", path: "/admin/fms/ap-aging", icon: NavIcon::File, permission: Some(("FMS", "read")) },
                NavItem { name: "核销记录", path: "/admin/fms/settlement", icon: NavIcon::Check, permission: Some(("FMS", "read")) },
                NavItem { name: "成本核算", path: "/admin/fms/cost-analysis", icon: NavIcon::DollarSign, permission: Some(("COST", "read")) },
            ],
        },
        NavModule {
            id: "md",
            name: "工程",
            items: vec![
                NavItem {
                    name: "工程总览",
                    path: "/admin/md",
                    icon: NavIcon::Home,
                    permission: Some(("PRODUCT", "read")),
                },
                NavItem {
                    name: "产品管理",
                    path: "/admin/md/products",
                    icon: NavIcon::Package,
                    permission: Some(("PRODUCT", "read")),
                },
                NavItem {
                    name: "产品分类",
                    path: "/admin/md/categories",
                    icon: NavIcon::Tag,
                    permission: Some(("CATEGORY", "read")),
                },
                NavItem {
                    name: "BOM管理",
                    path: "/admin/md/boms",
                    icon: NavIcon::ClipboardDoc,
                    permission: Some(("BOM", "read")),
                },
                NavItem {
                    name: "电源BOM",
                    path: "/admin/md/boms?category_name=电源",
                    icon: NavIcon::ClipboardDoc,
                    permission: Some(("BOM", "read")),
                },
                NavItem {
                    name: "模组BOM",
                    path: "/admin/md/boms?category_name=模组",
                    icon: NavIcon::ClipboardDoc,
                    permission: Some(("BOM", "read")),
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
                    permission: Some(("USER", "read")),
                },
                NavItem {
                    name: "角色管理",
                    path: "/admin/system/roles",
                    icon: NavIcon::Lock,
                    permission: Some(("ROLE", "read")),
                },
                NavItem {
                    name: "部门管理",
                    path: "/admin/system/departments",
                    icon: NavIcon::Building,
                    permission: Some(("DEPARTMENT", "read")),
                },
                NavItem {
                    name: "权限配置",
                    path: "/admin/system/permissions",
                    icon: NavIcon::Sliders,
                    permission: Some(("ROLE", "read")),
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
        NavIcon::_Archive => icon::package_icon(""),
        NavIcon::Database => icon::grid_icon(""),
        NavIcon::Wrench => icon::sliders_icon(""),
        NavIcon::Tag => icon::file_text_icon(""),
        NavIcon::_Link => icon::link_icon(""),
        NavIcon::Lock => icon::lock_icon(""),
        NavIcon::Search => icon::search_icon(""),
        NavIcon::ArrowDown => icon::download_icon(""),
        NavIcon::ArrowUp => icon::upload_icon(""),
        NavIcon::Switch => icon::refresh_icon(""),
        NavIcon::Refresh => icon::refresh_icon(""),
        NavIcon::Lightning => icon::bolt_icon(""),
        NavIcon::_Factory => icon::box_icon(""),
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
        "finance" => icon::currency_icon(""),
        "gl" => icon::grid_icon(""),
        "system" => icon::lock_icon(""),
        _ => html! {},
    }
}

fn item_class(active: bool) -> &'static str {
    if active {
        "flex items-center gap-3 py-[9px] px-5 text-sm text-white transition-all duration-150 rounded-sm mx-3 cursor-pointer relative whitespace-nowrap font-semibold bg-[rgba(37,99,235,0.15)] before:content-[''] before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:bg-accent before:rounded-r-sm icon:w-4.5 icon:h-4.5 icon:shrink-0 icon:opacity-100 icon:text-accent"
    } else {
        "flex items-center gap-3 py-[9px] px-5 text-sm text-white/60 transition-all duration-150 rounded-sm mx-3 cursor-pointer relative whitespace-nowrap hover:bg-white/[0.06] hover:text-white/95 hover:icon:opacity-80 icon:w-4.5 icon:h-4.5 icon:shrink-0 icon:opacity-55 icon:transition-opacity"
    }
}

fn mobile_class(active: bool) -> &'static str {
    if active {
        "mobile-nav-item active icon:w-5 icon:h-5 text-accent font-semibold"
    } else {
        "mobile-nav-item icon:w-5 icon:h-5"
    }
}

pub fn avatar_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 2 {
        return trimmed.to_uppercase();
    }
    chars[chars.len() - 2..].iter().collect()
}

fn _find_module(id: &str) -> Option<usize> {
    modules().iter().position(|m| m.id == id)
}

// ── Sidebar Body Fragment (used by HTMX endpoint) ──

pub fn sidebar_body_fragment(claims: &Claims, active_module: &str, filter: &NavFilter) -> Markup {
    let mods = modules();
    let Some(active_mod) = mods.iter().find(|m| m.id == active_module) else {
        return html! {};
    };
    let visible_items = filter.visible_items(active_mod);

    html! {
        div class="p-4 text-sm font-bold [border-bottom:1px_solid_rgba(255,255,255,0.06)] flex items-center gap-2 shrink-0 text-white/90"
        {
            span
                class="w-[18px] h-[18px] grid place-items-center icon:w-4 icon:h-4 icon:text-accent"
            { (render_module_icon(active_mod.id)) }
            span class="whitespace-nowrap overflow-hidden" { (active_mod.name) }
        }
        div class="flex-1 overflow-y-auto p-2" {
            @for item in &visible_items {
                a href=(item.path) class=(item_class(false)) {
                    (render_item_icon(item.icon))
                    span
                        class="flex items-center gap-3 text-sm rounded-sm cursor-pointer relative whitespace-nowrap"
                    { (item.name) }
                }
            }
        }
        div class="p-4 [border-top:1px_solid_rgba(255,255,255,0.06)] flex items-center gap-3 mt-auto"
        {
            div class="w-[34px] h-[34px] rounded-full bg-accent grid place-items-center text-[13px] font-bold text-white shrink-0"
            { (avatar_initials(&claims.display_name)) }
            div class="flex-1 min-w-0" {
                div class="text-sm font-semibold text-white truncate" {
                    (claims.display_name.as_str())
                }
                div class="text-[11px] text-white/40" { (claims.system_role.as_str()) }
            }
        }
    }
}

// ── Full Sidebar (initial page load) ──

pub fn sidebar(claims: &Claims, active_module: &str, current_path: &str, filter: &NavFilter) -> Markup {
    let mods = modules();
    let active_mod = mods.iter().find(|m| m.id == active_module);

    html! {
        nav id="sidebar"
            class="bg-sidebar-bg text-white/85 flex flex-row sticky top-0 h-screen overflow-hidden z-20"
        {
            // ── Icon Rail ──
            div class="sidebar-rail w-[56px] min-w-[56px] bg-sidebar-rail flex flex-col items-center py-3 [border-right:1px_solid_rgba(255,255,255,0.04)] shrink-0"
            {
                div class="w-[36px] h-[36px] rounded-md bg-accent grid place-items-center mb-5 shadow-[var(--shadow-accent)]"
                    title="ABT ERP"
                {
                    span class="icon:w-4.5 icon:h-4.5 icon:text-white" { (icon::box_icon("")) }
                }
                div class="rail-modules flex-1 flex flex-col items-center gap-[2px] w-full overflow-y-auto"
                {
                    @for m in &mods {
                        @if filter.has_visible_items(m) {
                            @let is_initial_active = active_mod
                                .is_some_and(|am| m.id == am.id);
                            @let hx_url = format!("/sidebar/body/{}", m.id);
                            // 激活态视觉统一由 preflight `.rail-item.active` 驱动（背景+竖条+文字/icon 联动），
                            // 这样初始渲染（带 active）与点击切换（hyperscript take .active）走同一条路径，
                            // 避免旧双分支下「未激活分支无 act: 样式 → 点击后无激活态」的根因。
                            @let rail_cls = format!(
                                "rail-item{} w-[44px] flex flex-col items-center gap-[3px] py-2 px-0 pb-[6px] border-none bg-transparent rounded-sm text-white/40 cursor-pointer transition-all duration-150 relative hover:text-white/85 hover:bg-white/[0.06]",
                                if is_initial_active { " active" } else { "" }
                            );
                            button
                                class=(rail_cls)
                                hx-get=(hx_url)
                                hx-target=".sidebar-body"
                                hx-swap="innerHTML"
                                _="on click take .active from .rail-item"
                                title=(m.name)
                            {
                                span
                                    class="w-[20px] h-[20px] grid place-items-center icon:w-4.5 icon:h-4.5"
                                { (render_module_icon(m.id)) }
                                span class="text-[10px] leading-none whitespace-nowrap" {
                                    (m.name.replace("管理", ""))
                                }
                            }
                        }
                    }
                }
                div class="rail-bottom flex flex-col items-center w-full pt-3 [border-top:1px_solid_rgba(255,255,255,0.06)] mt-2"
                {
                    button
                        class="w-[44px] flex flex-col items-center gap-[3px] border-none bg-transparent rounded-sm cursor-pointer relative text-white/25 hover:text-white/60 transition-colors icon:w-4 icon:h-4 icon:opacity-70 hover:icon:opacity-100"
                        _="on click toggle .sidebar-collapsed on .app-shell then if .app-shell matches .sidebar-collapsed call localStorage.setItem('sidebar-collapsed','true') else call localStorage.removeItem('sidebar-collapsed')"
                        title="收起侧栏"
                    {
                        (icon::sidebar_toggle_icon(""))
                        span class="text-[10px] leading-none whitespace-nowrap" { "收起" }
                    }
                }
            }
            // ── Sidebar Body ──
            div class="sidebar-body flex-1 min-w-0 flex flex-col overflow-y-auto" {
                @if let Some(active_mod) = active_mod {
                    (sidebar_body_fragment_inner(active_mod, current_path, filter))
                }
                div class="mt-auto p-4 [border-top:1px_solid_rgba(255,255,255,0.06)] flex items-center gap-3"
                {
                    div class="w-[34px] h-[34px] rounded-full bg-accent grid place-items-center text-[13px] font-bold text-white shrink-0"
                    { (avatar_initials(&claims.display_name)) }
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-semibold text-white truncate" {
                            (claims.display_name.as_str())
                        }
                        div class="text-[11px] text-white/40" { (claims.system_role.as_str()) }
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
fn find_active_path<'a>(items: &[&'a NavItem], current_path: &str) -> Option<&'a str> {
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
fn sidebar_body_fragment_inner(active_mod: &NavModule, current_path: &str, filter: &NavFilter) -> Markup {
    let visible_items = filter.visible_items(active_mod);
    let active_path = find_active_path(&visible_items, current_path);
    html! {
        div class="p-4 text-sm font-bold [border-bottom:1px_solid_rgba(255,255,255,0.06)] flex items-center gap-2 shrink-0"
        {
            span class="w-[18px] h-[18px] grid place-items-center" {
                (render_module_icon(active_mod.id))
            }
            span class="whitespace-nowrap overflow-hidden" { (active_mod.name) }
        }
        div class="flex-1 overflow-y-auto p-2" {
            @for item in &visible_items {
                a href=(item.path) class=(item_class(active_path == Some(item.path))) {
                    (render_item_icon(item.icon))
                    span
                        class="flex items-center gap-3 text-sm rounded-sm cursor-pointer relative whitespace-nowrap-text"
                    { (item.name) }
                }
            }
        }
    }
}

// ── Mobile Nav ──

pub fn mobile_nav(active_module: &str, current_path: &str, filter: &NavFilter) -> Markup {
    let mods = modules();
    let Some(active_mod) = mods.iter().find(|m| m.id == active_module) else {
        return html! {};
    };
    let visible_items = filter.visible_items(active_mod);
    let active_path = find_active_path(&visible_items, current_path);

    html! {
        nav class="hidden fixed h-[60px] bg-bg border-t z-[30]" {
            div class="hidden fixed h-[60px] bg-bg border-t z-[30]-scroll" {
                div class="hidden fixed h-[60px] bg-bg border-t z-[30]-inner" {
                    @for item in &visible_items {
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
