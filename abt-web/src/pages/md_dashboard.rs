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
            abt_core::master_data::routing::model::RoutingQuery { keyword: None },
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
        content, &nav_filter,    );
    Ok(axum::response::Html(page_html.into_string()))
}

// ── Components ──

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
        }

        // ── Stat Cards (4 columns) ──
        div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4);margin-bottom:var(--space-8)" {
            (stat_card("产品总数", &product_count.to_string()))
            (stat_card("物料清单总数", &bom_count.to_string()))
            (stat_card("供应商总数", &supplier_count.to_string()))
            (stat_card("工艺路线总数", &routing_count.to_string()))
        }

        // ── Quick Entry Grid ──
        div {
            h2 class="section-title" style="margin-bottom:var(--space-4)" { "快捷入口" }
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4)" {
                (quick_entry_card(ProductListPath::PATH, &icon::package_icon("w-[28px] h-[28px]"), "产品管理"))
                (quick_entry_card(CategoryListPath::PATH, &icon::grid_icon("w-[28px] h-[28px]"), "产品分类"))
                (quick_entry_card(BomListPath::PATH, &icon::clipboard_list_icon("w-[28px] h-[28px]"), "物料清单"))
                (quick_entry_card(SupplierListPath::PATH, &icon::building_icon("w-[28px] h-[28px]"), "供应商管理"))
            }
        }
    }
}

fn stat_card(label: &str, value: &str) -> Markup {
    html! {
        div class="info-card-flat" {
            span class="info-label" { (label) }
            div style="display:flex;align-items:baseline;gap:var(--space-2);margin-top:var(--space-2)" {
                span class="amount-value text-2xl" { (value) }
            }
        }
    }
}

fn quick_entry_card(href: &str, icon: &Markup, title: &str) -> Markup {
    html! {
        a href=(href) class="quick-link" {
            span style="color:var(--accent)" { (icon) }
            span class="text-sm font-semibold text-fg" { (title) }
        }
    }
}
