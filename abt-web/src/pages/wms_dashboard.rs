use axum::response::Html;
use maud::{html, Markup};

use abt_core::shared::types::PageParams;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::stock_ledger::model::StockFilter;
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use chrono::Datelike;
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_dashboard::WmsDashboardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
// ── Handler ──

#[require_permission("WMS", "read")]
pub async fn get_wms_dashboard(
    _path: WmsDashboardPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let wh_svc = state.warehouse_service();
    let inv_svc = state.inventory_service();
    let txn_svc = state.inventory_transaction_service();

    let page1 = PageParams::new(1, 1);

    // 仓库总数
    let warehouse_count = wh_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), page1.page, page1.page_size)
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 库存品类数（query_stock total）
    let stock_sku_count = txn_svc
        .query_stock(
            &service_ctx,
            &mut conn,
            StockFilter::default(),
            page1.page,
            page1.page_size,
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 低库存预警
    let low_stock_count = inv_svc
        .list_low_stock(&service_ctx, &mut conn)
        .await
        .map(|v| v.len() as u64)
        .unwrap_or(0);

    // 本月入库/出库 — 使用 query_logs 按月过滤
    let now = chrono::Utc::now();
    let month_start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
        .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
        .unwrap_or(now);

    let mut in_filter = abt_core::wms::inventory::model::TransactionLogFilter::default();
    in_filter.start_date = Some(month_start);
    in_filter.transaction_type = Some("PURCHASE_RECEIPT".into());

    let month_in_count = inv_svc
        .query_logs(&service_ctx, &mut conn, in_filter, page1.page, page1.page_size)
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let mut out_filter = abt_core::wms::inventory::model::TransactionLogFilter::default();
    out_filter.start_date = Some(month_start);
    out_filter.transaction_type = Some("SALES_SHIPMENT".into());

    let month_out_count = inv_svc
        .query_logs(&service_ctx, &mut conn, out_filter, page1.page, page1.page_size)
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // 按仓库分组统计
    let warehouse_list = wh_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 100)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let mut warehouses = Vec::new();
    for wh in &warehouse_list {
        let mut filter = StockFilter::default();
        filter.warehouse_id = Some(wh.id);
        let sku = txn_svc
            .query_stock(&service_ctx, &mut conn, filter, 1, 1)
            .await
            .map(|r| r.total)
            .unwrap_or(0);
        let type_label = match wh.warehouse_type {
            abt_core::wms::enums::WarehouseType::RawMaterial => "原材料",
            abt_core::wms::enums::WarehouseType::FinishedGoods => "成品",
            abt_core::wms::enums::WarehouseType::SemiFinished => "半成品",
            abt_core::wms::enums::WarehouseType::Consumable => "辅料",
            abt_core::wms::enums::WarehouseType::VirtualOutsource => "虚拟委外",
        };
        warehouses.push(WarehouseGroup {
            name: wh.name.clone(),
            code: wh.code.clone(),
            wh_type: type_label.to_string(),
            sku_count: sku,
        });
    }

    let stats = DashboardStats {
        warehouse_count,
        stock_sku_count,
        month_in_count,
        month_out_count,
        low_stock_count,
        warehouses,
    };

    let content = wms_dashboard_content(&stats);
    let page_html = admin_page(
        is_htmx,
        "库存管理总览",
        &claims,
        "inventory",
        "/admin/wms",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

// ── Data ──

struct DashboardStats {
    warehouse_count: u64,
    stock_sku_count: u64,
    month_in_count: u64,
    month_out_count: u64,
    low_stock_count: u64,
    warehouses: Vec<WarehouseGroup>,
}

struct WarehouseGroup {
    name: String,
    code: String,
    wh_type: String,
    sku_count: u64,
}

// ── Main content (matches prototype 03-index.html) ──

fn wms_dashboard_content(stats: &DashboardStats) -> Markup {
    html! {
        // ── Page Header ──
        div class="page-header" {
            h1 class="page-title" { "库存管理总览" }
            div class="page-actions" {
                button class="btn btn-default" {
                    (icon::download_icon("w-4 h-4"))
                    " 导出报表"
                }
            }
        }

        // ── Stat Cards (5 columns) ──
        div style="display:grid;grid-template-columns:repeat(5,1fr);gap:var(--space-5);margin-bottom:var(--space-8)" {
            // 仓库总数
            div class="stat-card" {
                div class="stat-icon blue" {
                    (icon::building_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="stat-value" { (stats.warehouse_count.to_string()) }
                    div class="stat-label" { "仓库总数" }
                }
            }
            // 库存品类
            div class="stat-card" {
                div class="stat-icon green" {
                    (icon::box_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="stat-value" { (format_number(stats.stock_sku_count)) }
                    div class="stat-label" { "库存品类" }
                }
            }
            // 本月入库
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#e6f7ff,#bae7ff);color:var(--accent)" {
                    (icon::download_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="stat-value" { (stats.month_in_count.to_string()) }
                    div class="stat-label" { "本月入库" }
                }
            }
            // 本月出库
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#fff1f0,#ffccc7);color:var(--danger)" {
                    (icon::upload_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="stat-value" { (stats.month_out_count.to_string()) }
                    div class="stat-label" { "本月出库" }
                }
            }
            // 低库存预警
            div class="stat-card" {
                div class="stat-icon red" {
                    (icon::circle_alert_icon("w-[22px] h-[22px]"))
                }
                div {
                    div class="stat-value" { (stats.low_stock_count.to_string()) }
                    div class="stat-label" { "低库存预警" }
                }
            }
        }

        // ── Quick Entry Grid (4 columns, 14 cards) ──
        div style="margin-bottom:var(--space-8)" {
            div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)" {
                h2 class="section-title" { "快捷入口" }
            }
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4)" {
                (quick_entry_card("/admin/wms/warehouses", "#e6f4ff", "#d6e8ff", "var(--accent)", &icon::building_icon("w-[22px] h-[22px]"), "仓库管理", "仓库主数据与分区配置"))
                (quick_entry_card("/admin/wms/bins", "#f0fff0", "#e0ffe0", "var(--success)", &icon::grid_icon("w-[22px] h-[22px]"), "储位管理", "库位规划与容量管理"))
                (quick_entry_card("/admin/wms/stock", "#e6f4ff", "#d6e8ff", "var(--accent)", &icon::search_icon("w-[22px] h-[22px]"), "库存查询", "实时库存数量与批次"))
                (quick_entry_card("/admin/wms/arrivals", "#fff8eb", "#fff0d6", "var(--warn)", &icon::truck_icon("w-[22px] h-[22px]"), "来料通知", "供应商送货到货登记"))
                (quick_entry_card("/admin/wms/stock-in", "#e6f7ff", "#bae7ff", "var(--accent)", &icon::download_icon("w-[22px] h-[22px]"), "入库管理", "采购入库 / 生产入库"))
                (quick_entry_card("/admin/wms/stock-out", "#fff1f0", "#ffccc7", "var(--danger)", &icon::upload_icon("w-[22px] h-[22px]"), "出库管理", "销售出库 / 生产领料"))
                (quick_entry_card("/admin/wms/requisitions", "#f0fff0", "#e0ffe0", "var(--success)", &icon::clipboard_module_icon("w-[22px] h-[22px]"), "领料单", "生产领料与发料管理"))
                (quick_entry_card("/admin/wms/cycle-counts", "#fff8eb", "#fff0d6", "var(--warn)", &icon::clipboard_list_icon("w-[22px] h-[22px]"), "循环盘点", "定期盘点与差异处理"))
                (quick_entry_card("/admin/wms/transfers", "#e6f4ff", "#d6e8ff", "var(--accent)", &icon::arrow_right_icon("w-[22px] h-[22px]"), "库存调拨", "跨仓调拨与在途管理"))
                (quick_entry_card("/admin/wms/conversions", "#fff2f0", "#ffe8e6", "var(--danger)", &icon::refresh_icon("w-[22px] h-[22px]"), "形态转换", "物料形态与单位转换"))
                (quick_entry_card("/admin/wms/backflushes", "#f0fff0", "#e0ffe0", "var(--success)", &icon::refresh_icon("w-[22px] h-[22px]"), "倒冲记录", "生产完工自动扣料"))
                (quick_entry_card("/admin/wms/locks", "#fff2f0", "#ffe8e6", "var(--danger)", &icon::lock_icon("w-[22px] h-[22px]"), "库存锁定", "质检与预留库存冻结"))
                (quick_entry_card("/admin/wms/transactions", "#fff8eb", "#fff0d6", "var(--warn)", &icon::file_text_icon("w-[22px] h-[22px]"), "事务日志", "全量库存事务流水"))
                (quick_entry_card("/admin/wms/strategies", "#e6f4ff", "#d6e8ff", "var(--accent)", &icon::sliders_icon("w-[22px] h-[22px]"), "策略管理", "上架与拣货策略配置"))
            }
        }

        // ── Per-Warehouse Statistics ──
        div style="margin-bottom:var(--space-8)" {
            h2 style="font-size:var(--text-lg);font-weight:600;margin-bottom:var(--space-4)" { "按仓库统计" }
            div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:var(--space-4)" {
                @for wh in &stats.warehouses {
                    div class="stat-card" style="cursor:pointer" onclick=(format!("location.href='/admin/wms/stock?warehouse_id='")) {
                        div style="display:flex;align-items:center;gap:var(--space-3);margin-bottom:var(--space-3)" {
                            div class="stat-icon" style="background:linear-gradient(135deg,#e6f4ff,#d6e8ff);color:var(--accent);width:36px;height:36px" {
                                (icon::building_icon("w-4 h-4"))
                            }
                            div {
                                div style="font-weight:600;font-size:var(--text-sm)" { (wh.name) }
                                div style="font-size:var(--text-xs);color:var(--muted)" { (wh.code) " · " (wh.wh_type) }
                            }
                        }
                        div style="display:flex;justify-content:space-between;align-items:center" {
                            span style="font-size:var(--text-xs);color:var(--fg-2)" { "库存品类" }
                            span style="font-size:var(--text-lg);font-weight:700;font-variant-numeric:tabular-nums;color:var(--accent)" { (wh.sku_count) }
                        }
                    }
                }
                @if stats.warehouses.is_empty() {
                    div style="text-align:center;padding:var(--space-6);color:var(--muted)" { "暂无仓库数据" }
                }
            }
        }
        // ── Recent Operations ──
        div {
            h2 style="font-size:var(--text-lg);font-weight:600;margin-bottom:var(--space-4)" { "最近操作" }
            div class="data-card" style="overflow:hidden" {
                table class="data-table" style="width:100%" {
                    thead {
                        tr {
                            th { "时间" }
                            th { "操作类型" }
                            th { "单号" }
                            th { "仓库" }
                            th { "操作人" }
                        }
                    }
                    tbody {
                        tr {
                            td style="color:var(--muted);font-size:12px" { "—" }
                            td { span class="status-pill status-confirmed" { "来料接收" } }
                            td { a href="/admin/wms/arrivals" style="color:var(--accent)" { "—" } }
                            td { "—" }
                            td { "—" }
                        }
                        tr {
                            td style="color:var(--muted);font-size:12px" { "—" }
                            td { span class="status-pill status-progress" { "领料出库" } }
                            td { a href="/admin/wms/requisitions" style="color:var(--accent)" { "—" } }
                            td { "—" }
                            td { "—" }
                        }
                        tr {
                            td style="color:var(--muted);font-size:12px" { "—" }
                            td { span class="status-pill status-completed" { "库存调拨" } }
                            td { a href="/admin/wms/transfers" style="color:var(--accent)" { "—" } }
                            td { "—" }
                            td { "—" }
                        }
                        tr {
                            td style="color:var(--muted);font-size:12px" { "—" }
                            td { span class="status-pill status-draft" { "循环盘点" } }
                            td { a href="/admin/wms/cycle-counts" style="color:var(--accent)" { "—" } }
                            td { "—" }
                            td { "—" }
                        }
                        tr {
                            td style="color:var(--muted);font-size:12px" { "—" }
                            td { span class="status-pill status-inspecting" { "库存锁定" } }
                            td { a href="/admin/wms/locks" style="color:var(--accent)" { "—" } }
                            td { "—" }
                            td { "—" }
                        }
                    }
                }
            }
        }
    }
}

// ── Sub-components ──

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn quick_entry_card(
    href: &str,
    bg_from: &str,
    bg_to: &str,
    icon_color: &str,
    svg_icon: &Markup,
    title: &str,
    desc: &str,
) -> Markup {
    let bg_style = format!("linear-gradient(135deg,{bg_from},{bg_to})");
    html! {
        a href=(href) style="display:flex;flex-direction:column;align-items:center;gap:var(--space-3);padding:var(--space-6) var(--space-4);background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);transition:all var(--motion-fast) var(--ease-standard);text-align:center;box-shadow:var(--shadow-xs)" {
            div style=(format!("width:44px;height:44px;border-radius:var(--radius-md);background:{};display:grid;place-items:center", bg_style)) {
                span style=(format!("color:{}", icon_color)) { (svg_icon) }
            }
            span style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { (title) }
            span style="font-size:12px;color:var(--muted)" { (desc) }
        }
    }
}
