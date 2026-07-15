use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use rust_decimal::Decimal;

use std::collections::HashMap;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::Product;
use abt_core::wms::enums::TransactionType;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::inventory::model::TransactionDetailView;
use abt_core::wms::stock_ledger::StockLedgerService;
use abt_core::wms::stock_ledger::model::{StockFilter, StockLedger};
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::warehouse::model::*;

use crate::components::icon;
use crate::errors::{Result, error_page};
use crate::layout::page::admin_page;
use crate::pages::wms_bin_list::{bin_status_class, bin_status_label};
use crate::routes::wms_bin::{BinDetailPath, BinListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("LOCATION", "read")]
pub async fn get_bin_detail(path: BinDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.warehouse_service();

    let bww = match svc
        .get_bin_with_warehouse(&service_ctx, &mut conn, path.id)
        .await
    {
        Ok(bww) => bww,
        Err(e) => {
            if matches!(e, abt_core::shared::types::DomainError::NotFound(_)) {
                let content = error_page(
                    "库位未找到",
                    &format!("库位 ID {} 不存在或已被删除", path.id),
                );
                let page_html = admin_page(
                    is_htmx,
                    "库位未找到",
                    &claims,
                    "inventory",
                    &BinListPath.to_string(),
                    "库存管理",
                    Some("库位未找到"),
                    content,
                    &nav_filter,
                );
                return Ok(Html(page_html.into_string()));
            }
            return Err(e.into());
        }
    };
    let zones = svc
        .list_zones(&service_ctx, &mut conn, bww.warehouse_id)
        .await?;
    let zone = zones.iter().find(|z| z.id == bww.bin.zone_id);
    let stats = svc
        .get_bin_inventory_stats(&service_ctx, &mut conn, path.id)
        .await
        .ok();

    // 库存明细（批次级台账，按 bin 过滤；SSR 全量渲染）
    let stock_rows = state
        .stock_ledger_service()
        .query(
            &service_ctx,
            &mut conn,
            StockFilter {
                bin_id: Some(path.id),
                ..Default::default()
            },
            1,
            200,
        )
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    // 批量补产品编码/名称（避免 N+1）
    let product_ids: Vec<i64> = {
        let mut ids: Vec<i64> = stock_rows.iter().map(|s| s.product_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let product_map: HashMap<i64, Product> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        state
            .product_service()
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect()
    };
    // 操作历史（该 bin 的事务流水）
    let logs = state
        .inventory_service()
        .list_logs_by_bin(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();

    let content = bin_detail_page(&bww, zone, stats.as_ref(), &stock_rows, &product_map, &logs);
    let detail_path_str = BinDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 库位详情", bww.bin.code),
        &claims,
        "inventory",
        &detail_path_str,
        "库存管理",
        Some(&bww.bin.code),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn temperature_label(req: &str) -> &str {
    match req {
        "ambient" => "常温",
        "cool" => "冷藏 (2~8°C)",
        "freeze" => "冷冻 (-18°C以下)",
        "constant" => "恒温",
        _ => "无要求",
    }
}

fn product_type_label(t: &str) -> &str {
    match t {
        "raw_material" => "原材料",
        "semi_finished" => "半成品",
        "finished" => "成品",
        "packaging" => "包材",
        "consumable" => "耗材",
        _ => t,
    }
}

fn product_type_color(t: &str) -> (&str, &str) {
    match t {
        "raw_material" => ("rgba(22,119,255,0.06)", "#1677ff"),
        "semi_finished" => ("rgba(82,196,26,0.06)", "#52c41a"),
        "finished" => ("rgba(114,46,209,0.06)", "#722ed1"),
        "packaging" => ("rgba(250,173,20,0.06)", "#d48806"),
        "consumable" => ("rgba(255,77,79,0.06)", "#ff4d4f"),
        _ => ("rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

fn capacity_percent(stats: &BinInventoryStats, limit: Option<Decimal>) -> Option<Decimal> {
    limit
        .filter(|l| *l > Decimal::ZERO)
        .map(|l| (stats.total_quantity / l * Decimal::from(100)).min(Decimal::from(100)))
}

fn txn_type_label(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::PurchaseReceipt => "采购入库",
        TransactionType::ProductionReceipt => "生产入库",
        TransactionType::SalesShipment => "销售出库",
        TransactionType::MaterialIssue => "生产领料",
        TransactionType::MaterialReturn => "生产退料",
        TransactionType::Backflush => "系统倒冲",
        TransactionType::Transfer => "调拨",
        TransactionType::FormConversion => "形态转换",
        TransactionType::Adjustment => "盘点调整",
        TransactionType::Lock => "锁库",
        TransactionType::Unlock => "解锁",
        TransactionType::Scrap => "报废",
        TransactionType::RoutingOutput => "工序产出",
    }
}

fn txn_type_class(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::PurchaseReceipt
        | TransactionType::ProductionReceipt
        | TransactionType::MaterialReturn
        | TransactionType::Unlock
        | TransactionType::RoutingOutput => "txn-type-in",
        TransactionType::SalesShipment
        | TransactionType::MaterialIssue
        | TransactionType::Backflush
        | TransactionType::Scrap => "txn-type-out",
        TransactionType::Transfer => "txn-type-move",
        TransactionType::Adjustment => "txn-type-adjust",
        TransactionType::Lock => "txn-type-lock",
        TransactionType::FormConversion => "txn-type-convert",
    }
}

fn source_type_label(s: &str) -> &str {
    match s {
        "manual" => "手工录入",
        "purchase" => "采购",
        "sales" => "销售",
        "production" => "生产",
        "transfer" | "inventory_transfer" => "调拨",
        "conversion" | "form_conversion" => "形态转换",
        "cycle_count" | "adjustment" => "盘点调整",
        "lock" => "锁库",
        "unlock" => "解锁",
        "backflush" => "倒冲",
        "requisition" => "领料",
        "arrival" => "来料",
        "scrap" => "报废",
        _ => s,
    }
}

/// 库存状态：按预留量（InventoryLock 冻结）占用量派生
fn stock_status(reserved: &Decimal, qty: &Decimal) -> (&'static str, &'static str) {
    if *qty > Decimal::ZERO && *reserved >= *qty {
        ("全冻结", "status-defect")
    } else if *reserved > Decimal::ZERO {
        ("部分冻结", "status-partial")
    } else {
        ("可用", "status-active")
    }
}

// ── Components ──

fn bin_detail_page(
    bww: &BinWithWarehouse,
    zone: Option<&Zone>,
    stats: Option<&BinInventoryStats>,
    stock_rows: &[StockLedger],
    product_map: &HashMap<i64, Product>,
    logs: &[TransactionDetailView],
) -> Markup {
    let bin = &bww.bin;
    let status_label = bin_status_label(&bin.status);
    let status_class = bin_status_class(&bin.status);
    let _detail_path = BinDetailPath { id: bin.id };

    let zone_name = zone.map(|z| z.name.as_str()).unwrap_or("—");
    let _zone_code = zone.map(|z| z.code.as_str()).unwrap_or("—");

    let (used_qty, capacity_pct) = match stats {
        Some(s) => {
            let pct = capacity_percent(s, bin.capacity_limit);
            (format!("{:.2}", s.total_quantity), pct)
        }
        None => ("—".to_string(), None),
    };

    html! {
        div {
            // ── Back Link ──
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(format!("{}?restore=true", BinListPath::PATH))
            { (icon::arrow_left_icon("w-4 h-4")) "返回库位管理列表" }
            // ── Detail Header ──
            div class="block bg-bg border border-border-soft rounded-lg p-6 flex justify-between items-start mb-5"
            {
                div {
                    div class="flex items-center gap-3" {
                        h1 class="text-xl font-bold m-0 font-mono" { (bin.code) }
                        span
                            class=({
                                format!(
                                    "status-pill {}",
                                    crate::utils::status_color(status_class),
                                )
                            })
                        { (status_label) }
                    }
                    div class="text-[13px] text-muted mt-2" { (bww.warehouse_name) " · " (zone_name) }
                }
            }
            // ── Tabs ──
            div class="flex gap-1 mb-5 border-b border-border-soft pb-0" {
                button
                    class="detail-tab py-2 px-4 text-sm font-medium cursor-pointer border-none bg-transparent border-b-2 whitespace-nowrap text-muted border-transparent [&.active]:text-accent [&.active]:border-accent active"
                    _="on click take .active from .detail-tab then add .hidden to .tab-panel then remove .hidden from #tab-info"
                { "基本信息" }
                button
                    class="detail-tab py-2 px-4 text-sm font-medium cursor-pointer border-none bg-transparent border-b-2 whitespace-nowrap text-muted border-transparent [&.active]:text-accent [&.active]:border-accent"
                    _="on click take .active from .detail-tab then add .hidden to .tab-panel then remove .hidden from #tab-stock"
                { "库存明细" }
                button
                    class="detail-tab py-2 px-4 text-sm font-medium cursor-pointer border-none bg-transparent border-b-2 whitespace-nowrap text-muted border-transparent [&.active]:text-accent [&.active]:border-accent"
                    _="on click take .active from .detail-tab then add .hidden to .tab-panel then remove .hidden from #tab-history"
                { "操作历史" }
            }
            // ── Tab: 基本信息 ──
            div.tab-panel id="tab-info" {
                // Info card
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-sm" {
                    div class="font-bold text-fg mb-4" { "库位信息" }
                    div class="grid grid-cols-2 md:grid-cols-3 gap-x-6 gap-y-4" {
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "库位编码" }
                            span class="text-sm text-fg font-medium font-mono" { (bin.code) }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "库位名称" }
                            span class="text-sm text-fg font-medium" { (bin.name) }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "所属仓库" }
                            span class="text-sm text-fg font-medium" { (bww.warehouse_name) }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "所属库区" }
                            span class="text-sm text-fg font-medium" { (zone_name) }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "库位状态" }
                            span class="text-sm text-fg font-medium" {
                                span
                                    class=({
                                        format!(
                                            "status-pill {}",
                                            crate::utils::status_color(status_class),
                                        )
                                    })
                                { (status_label) }
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "容量上限" }
                            span class="text-sm text-fg font-medium font-mono" {
                                @if let Some(cap) = &bin.capacity_limit { (format!("{:.2}", cap)) } @else {
                                    "—"
                                }
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "已用容量" }
                            span class="text-sm text-fg font-medium font-mono text-warn" {
                                (used_qty)
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "温控要求" }
                            span class="text-sm text-fg font-medium" {
                                ({
                                    bin.temperature_req
                                        .as_deref()
                                        .map(temperature_label)
                                        .unwrap_or("无要求")
                                })
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "允许物料类型" }
                            span class="text-sm text-fg font-medium flex flex-wrap gap-1.5" {
                                @if let Some(types) = &bin.allowed_product_types {
                                    @for t in types {
                                        @let (bg, fg) = product_type_color(t);
                                        span
                                            class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap"
                                            style=(format!("background:{bg};color:{fg}"))
                                        { (product_type_label(t)) }
                                    }
                                } @else { "—" }
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "创建时间" }
                            span class="text-sm text-fg font-medium font-mono" {
                                (bin.created_at.format("%Y-%m-%d %H:%M"))
                            }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "最后更新" }
                            span class="text-sm text-fg font-medium font-mono" {
                                (bin.updated_at.format("%Y-%m-%d %H:%M"))
                            }
                        }
                    }
                }
                // Coordinates card
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-sm mt-4" {
                    div class="font-bold text-fg mb-4" { "库位坐标" }
                    div class="flex gap-4 mt-3" {
                        div class="text-center flex-1 bg-surface rounded-md p-4 border border-border-soft"
                        {
                            div class="font-bold font-mono text-fg text-xl" {
                                (bin.row_no.as_deref().unwrap_or("—"))
                            }
                            div class="text-muted text-xs mt-1" { "行号 (Row)" }
                        }
                        div class="text-center flex-1 bg-surface rounded-md p-4 border border-border-soft"
                        {
                            div class="font-bold font-mono text-fg text-xl" {
                                (bin.column_no.as_deref().unwrap_or("—"))
                            }
                            div class="text-muted text-xs mt-1" { "列号 (Column)" }
                        }
                        div class="text-center flex-1 bg-surface rounded-md p-4 border border-border-soft"
                        {
                            div class="font-bold font-mono text-fg text-xl" {
                                (bin.layer_no.as_deref().unwrap_or("—"))
                            }
                            div class="text-muted text-xs mt-1" { "层号 (Layer)" }
                        }
                        div class="text-center flex-1 bg-surface rounded-md p-4 border border-border-soft"
                        {
                            div class="font-bold font-mono text-fg text-xl" {
                                @if let Some(pct) = capacity_pct { (format!("{}%", pct.round())) } @else {
                                    "—"
                                }
                            }
                            div class="text-muted text-xs mt-1" { "容量使用率" }
                        }
                    }
                    @if let Some(pct) = capacity_pct {
                        div class="mt-4 max-w-[400px]" {
                            div class="overflow-hidden h-2 bg-border-soft rounded" {
                                div class="h-full rounded bg-warn transition-all duration-300"
                                    style=(format!("width:{}%", pct.round())) {}
                            }
                        }
                    }
                }
            }
            // ── Tab: 库存明细 ──
            div.tab-panel.hidden id="tab-stock" {
                div class="data-card" {
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "批次号" }
                                    th class="text-right text-[13px]" { "数量" }
                                    th class="text-right text-[13px]" { "单位成本" }
                                    th { "入库日期" }
                                    th { "有效期" }
                                    th { "状态" }
                                }
                            }
                            tbody {
                                @if stock_rows.is_empty() {
                                    tr {
                                        td colspan="8" class="text-center text-muted py-8" {
                                            "暂无库存数据"
                                        }
                                    }
                                } @else {
                                    @for s in stock_rows {
                                        @let p = product_map.get(&s.product_id);
                                        @let (st_label, st_key) = stock_status(
                                            &s.reserved_qty,
                                            &s.quantity,
                                        );
                                        tr {
                                            td class="font-mono tabular-nums text-[13px]" {
                                                (p.map(|x| x.product_code.as_str()).unwrap_or("—"))
                                            }
                                            td class="text-[13px]" {
                                                (p.map(|x| x.pdt_name.as_str()).unwrap_or("—"))
                                            }
                                            td class="font-mono text-[13px]" {
                                                (s.batch_no.as_deref().unwrap_or("—"))
                                            }
                                            td  class="text-right text-[13px] font-mono tabular-nums"
                                            { (crate::utils::fmt_qty(s.quantity)) }
                                            td  class="text-right text-[13px] font-mono tabular-nums"
                                            {
                                                ({
                                                    s.unit_cost
                                                        .map(crate::utils::fmt_qty)
                                                        .unwrap_or_else(|| "—".into())
                                                })
                                            }
                                            td class="text-[13px] font-mono tabular-nums" {
                                                ({
                                                    s.received_date
                                                        .map(|d| d.format("%Y-%m-%d").to_string())
                                                        .unwrap_or_else(|| "—".into())
                                                })
                                            }
                                            td class="text-[13px] font-mono tabular-nums" {
                                                ({
                                                    s.expiry_date
                                                        .map(|d| d.format("%Y-%m-%d").to_string())
                                                        .unwrap_or_else(|| "—".into())
                                                })
                                            }
                                            td {
                                                span
                                                    class=({
                                                        format!(
                                                            "status-pill {}",
                                                            crate::utils::status_color(st_key),
                                                        )
                                                    })
                                                { (st_label) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // ── Tab: 操作历史 ──
            div.tab-panel.hidden id="tab-history" {
                div class="data-card" {
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "时间" }
                                    th { "事务类型" }
                                    th { "关联单号" }
                                    th { "产品" }
                                    th class="text-right text-[13px]" { "变动数量" }
                                    th { "操作员" }
                                    th { "备注" }
                                }
                            }
                            tbody {
                                @if logs.is_empty() {
                                    tr {
                                        td colspan="7" class="text-center text-muted py-8" {
                                            "暂无操作历史"
                                        }
                                    }
                                } @else {
                                    @for t in logs {
                                        @let label = txn_type_label(&t.transaction_type);
                                        @let css_class = txn_type_class(&t.transaction_type);
                                        tr {
                                            td class="font-mono tabular-nums text-[13px]" {
                                                (t.created_at.format("%Y-%m-%d %H:%M"))
                                            }
                                            td {
                                                span
                                                    class=({
                                                        format!(
                                                            "inline-flex items-center rounded-full text-[11px] font-medium px-2.5 py-0.5 whitespace-nowrap {}",
                                                            css_class,
                                                        )
                                                    })
                                                { (label) }
                                            }
                                            td class="text-[13px]" {
                                                (source_type_label(&t.source_type))
                                                @if t.source_id > 0 { " #" (t.source_id) }
                                            }
                                            td class="text-[13px]" {
                                                div class="font-mono tabular-nums" {
                                                    (t.product_code)
                                                }
                                                div class="text-xs text-fg-2 truncate max-w-[180px]"
                                                    title=(t.product_name)
                                                { (t.product_name) }
                                            }
                                            td  class="text-right text-[13px] font-mono tabular-nums font-semibold"
                                            {
                                                @if t.quantity >= Decimal::ZERO {
                                                    span class="text-success" {
                                                        "+"
                                                        (crate::utils::fmt_qty(t.quantity))
                                                    }
                                                } @else {
                                                    span class="text-danger" {
                                                        (crate::utils::fmt_qty(t.quantity))
                                                    }
                                                }
                                            }
                                            td class="text-[13px]" { (t.operator_name) }
                                            td class="text-[13px] text-muted" {
                                                (t.remark.as_deref().unwrap_or("—"))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
