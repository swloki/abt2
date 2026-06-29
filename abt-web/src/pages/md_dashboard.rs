use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::bom::BomListPath;
use crate::routes::category::CategoryListPath;
use crate::routes::md_dashboard::MdDashboardPath;
use crate::routes::product::ProductListPath;
use crate::routes::supplier::SupplierListPath;
use crate::utils::RequestContext;

// ── Handler ──

pub async fn get_md_dashboard(
    _path: MdDashboardPath,
    ctx: RequestContext,
) -> crate::errors::Result<axum::response::Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let db = &mut conn;
    let svc_ctx = &service_ctx;
    let page = PageParams::new(1, 1);
    let product_svc = state.product_service();
    let supplier_svc = state.supplier_service();
    let bom_svc = state.bom_query_service();
    let routing_svc = state.routing_service();

    let product_count = product_svc
        .list(svc_ctx, db, Default::default(), page.clone())
        .await
        .map(|r| r.total)
        .unwrap_or(0);
    let bom_count = bom_svc
        .list(
            svc_ctx,
            db,
            abt_core::master_data::bom::model::BomQuery {
                name: None,
                status: None,
 bom_category_id: None,
 date_from: None,
 date_to: None,
 no_labor_cost: false,
 no_material_cost: false,
            },
            page.clone(),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let supplier_count = supplier_svc
        .list(svc_ctx, db, Default::default(), page.clone())
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let routing_count = routing_svc
        .list(
            svc_ctx,
            db,
            abt_core::master_data::routing::model::RoutingQuery { keyword: None, bom_keyword: None },
            page,
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let content = md_dashboard_content(product_count, bom_count, supplier_count, routing_count);
    let page_html = admin_page(
        is_htmx,
        "主数据管理",
        &claims,
        "md",
        MdDashboardPath::PATH,
        "主数据管理",
        None,
        content, &nav_filter, );
    Ok(axum::response::Html(page_html.into_string()))
}

// ── Page ──

fn md_dashboard_content(
    product_count: u64,
    bom_count: u64,
    supplier_count: u64,
    routing_count: u64,
) -> Markup {
    html! {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "主数据管理概览" }
            span class="text-xs text-muted" { "数据截至 " (chrono::Utc::now().format("%Y-%m-%d")) }
        }
        // ── Stat Cards ──
        div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8" {
            ({
                stat_card(
                    "产品总数",
                    &format!("{}", product_count),
                    "产品主数据",
                    icon::package_icon("w-5 h-5"),
                    "bg-accent-bg text-accent",
                )
            })
            ({
                stat_card(
                    "供应商",
                    &format!("{}", supplier_count),
                    "客商主数据",
                    icon::users_icon("w-5 h-5"),
                    "bg-success-bg text-success",
                )
            })
            ({
                stat_card(
                    "BOM 清单",
                    &format!("{}", bom_count),
                    "物料清单",
                    icon::clipboard_list_icon("w-5 h-5"),
                    "bg-warn-100 text-warn",
                )
            })
            ({
                stat_card(
                    "工艺路线",
                    &format!("{}", routing_count),
                    "生产工序",
                    icon::link_icon("w-5 h-5"),
                    "bg-danger-100 text-danger",
                )
            })
        }
        // ── Quick Entry Grid ──
        div class="mb-8" {
            h2 class="text-lg font-semibold text-fg mb-4" { "快捷入口" }
            div class="grid grid-cols-2 lg:grid-cols-3 gap-4" {
                ({
                    quick_entry_card(
                        ProductListPath::PATH,
                        icon::package_icon("w-5 h-5"),
                        "产品管理",
                        &format!("{} 条", product_count),
                        "bg-accent-bg text-accent",
                    )
                })
                ({
                    quick_entry_card(
                        CategoryListPath::PATH,
                        icon::grid_icon("w-5 h-5"),
                        "产品分类",
                        "分类体系",
                        "bg-success-bg text-success",
                    )
                })
                ({
                    quick_entry_card(
                        SupplierListPath::PATH,
                        icon::building_icon("w-5 h-5"),
                        "供应商管理",
                        &format!("{} 位", supplier_count),
                        "bg-warn-100 text-warn",
                    )
                })
                ({
                    quick_entry_card(
                        BomListPath::PATH,
                        icon::clipboard_list_icon("w-5 h-5"),
                        "物料清单",
                        &format!("{} 份", bom_count),
                        "bg-danger-100 text-danger",
                    )
                })
                ({
                    quick_entry_card(
                        crate::routes::routing::RoutingListPath::PATH,
                        icon::link_icon("w-5 h-5"),
                        "工艺路线",
                        &format!("{} 条", routing_count),
                        "bg-accent-bg text-accent",
                    )
                })
            }
        }
        // ── Data Flow ──
        div {
            h2 class="text-lg font-semibold text-fg mb-4" { "数据关系流" }
            div class="data-card flex items-center gap-0 p-8 overflow-x-auto" {
                ({
                    flow_node(
                        CategoryListPath::PATH,
                        icon::grid_icon("w-5 h-5"),
                        "产品分类",
                        "分类体系",
                        "bg-success-bg text-success",
                    )
                })
                (flow_arrow())
                ({
                    flow_node(
                        ProductListPath::PATH,
                        icon::package_icon("w-5 h-5"),
                        "产品",
                        "产品主数据",
                        "bg-accent-bg text-accent",
                    )
                })
                (flow_arrow())
                ({
                    flow_node(
                        BomListPath::PATH,
                        icon::clipboard_list_icon("w-5 h-5"),
                        "BOM",
                        "物料清单",
                        "bg-warn-100 text-warn",
                    )
                })
                (flow_arrow())
                ({
                    flow_node(
                        crate::routes::routing::RoutingListPath::PATH,
                        icon::link_icon("w-5 h-5"),
                        "工艺路线",
                        "生产工序",
                        "bg-danger-100 text-danger",
                    )
                })
            }
        }
    }
}

// ── Stat Card ──

fn stat_card(label: &str, value: &str, sub: &str, icon_svg: Markup, icon_cls: &str) -> Markup {
    html! {
        div class="data-card flex items-center gap-4 p-5" {
            div class=({
                    format!(
                        "w-11 h-11 rounded-md grid place-items-center shrink-0 {}",
                        icon_cls,
                    )
                })
            { (icon_svg) }
            div {
                div class="text-2xl font-bold font-mono tabular-nums text-fg" { (value) }
                div class="text-sm text-muted mt-1" { (label) }
                div class="text-xs text-muted mt-0.5" { (sub) }
            }
        }
    }
}

// ── Quick Entry Card ──

fn quick_entry_card(href: &str, icon_svg: Markup, title: &str, count: &str, icon_cls: &str) -> Markup {
    html! {
        a   href=(href)
            class="data-card flex flex-col items-center gap-3 p-6 text-center no-underline hover:shadow-[var(--shadow-card-hover)] transition-shadow duration-200"
        {
            div class=(format!("w-11 h-11 rounded-md grid place-items-center {}", icon_cls)) {
                (icon_svg)
            }
            span class="text-sm font-semibold text-fg" { (title) }
            span class="text-xs text-muted" { (count) }
        }
    }
}

// ── Flow Node ──

fn flow_node(href: &str, icon_svg: Markup, label: &str, sub: &str, icon_cls: &str) -> Markup {
    html! {
        a href=(href) class="flex flex-col items-center gap-3 min-w-[100px] no-underline" {
            div class=(format!("w-12 h-12 rounded-md grid place-items-center {}", icon_cls)) {
                (icon_svg)
            }
            span class="text-sm font-semibold text-fg" { (label) }
            span class="text-xs text-muted" { (sub) }
        }
    }
}

fn flow_arrow() -> Markup {
    html! {
        svg class="shrink-0 mx-3 text-border"
            width="48"
            height="20"
            viewBox="0 0 48 20"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
        {
            path d="M0 10h40M34 5l6 5-6 5" {}
        }
    }
}
