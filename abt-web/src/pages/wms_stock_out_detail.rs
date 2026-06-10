use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::inventory_transaction::repo::InventoryTransactionRepo;
use abt_core::wms::inventory_transaction::model::InventoryTransaction;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn transaction_type_label(t: &abt_core::wms::enums::TransactionType) -> &'static str {
    use abt_core::wms::enums::TransactionType::*;
    match t {
        PurchaseReceipt => "采购入库",
        ProductionReceipt => "生产入库",
        SalesShipment => "销售出库",
        MaterialIssue => "领料出库",
        MaterialReturn => "退料入库",
        Backflush => "倒冲出入库",
        Transfer => "调拨",
        FormConversion => "形态转换",
        Adjustment => "盘点调整",
        Lock => "锁定",
        Unlock => "解锁",
        Scrap => "报废",
    }
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_stock_out_detail(
    path: StockOutDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let txn = InventoryTransactionRepo::get_by_id(&mut conn, path.id)
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("出库记录不存在"))?;

    let wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, txn.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let product_name = state.product_service()
        .get(&service_ctx, &mut conn, txn.product_id)
        .await
        .map(|p| format!("{} ({})", p.pdt_name, p.product_code))
        .unwrap_or_else(|_| format!("产品 #{}", txn.product_id));

    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, txn.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| format!("用户 #{}", txn.operator_id));

    let zone_name = if let Some(zid) = txn.zone_id {
        state.warehouse_service()
            .get_zone(&service_ctx, &mut conn, zid)
            .await
            .map(|z| z.name)
            .unwrap_or_else(|_| format!("库区 #{}", zid))
    } else {
        "—".into()
    };

    let bin_name = if let Some(bid) = txn.bin_id {
        state.warehouse_service()
            .get_bin_with_warehouse(&service_ctx, &mut conn, bid)
            .await
            .map(|b| format!("{} ({})", b.bin.name, b.bin.code))
            .unwrap_or_else(|_| format!("储位 #{}", bid))
    } else {
        "—".into()
    };

    let content = stock_out_detail_page(&txn, &wh_name, &product_name, &zone_name, &bin_name, &operator_name);
    let detail_path = StockOutDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 出库详情", txn.doc_number.as_deref().unwrap_or("—")),
        &claims,
        "inventory",
        &detail_path,
        "库存管理",
        txn.doc_number.as_deref(),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stock_out_detail_page(
    txn: &InventoryTransaction,
    wh_name: &str,
    product_name: &str,
    zone_name: &str,
    bin_name: &str,
    operator_name: &str,
) -> Markup {
    let type_label = transaction_type_label(&txn.transaction_type);

    html! {
        div {
            a href=(StockOutListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回出库列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (txn.doc_number.as_deref().unwrap_or("—")) }
                        span class="status-pill status-completed" { "已出库" }
                    }
                }
            }

            // ── 基本信息 ──
            div class="info-card" {
                div class="info-card-title" { "基本信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "单据编号" }
                        span class="info-value mono" { (txn.doc_number.as_deref().unwrap_or("—")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "出库类型" }
                        span class="info-value" { (type_label) }
                    }
                    div class="info-item" {
                        span class="info-label" { "产品" }
                        span class="info-value" { (product_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "来源仓库" }
                        span class="info-value" { (wh_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "库区" }
                        span class="info-value" { (zone_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "储位" }
                        span class="info-value" { (bin_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "批次号" }
                        span class="info-value mono" { (txn.batch_no.as_deref().unwrap_or("—")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "数量" }
                        span class="info-value mono" { (format!("{:.2}", txn.quantity)) }
                    }
                    div class="info-item" {
                        span class="info-label" { "单位成本" }
                        span class="info-value mono" {
                            (txn.unit_cost.map(|c| format!("¥{:.2}", c)).unwrap_or_else(|| "—".into()))
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "来源类型" }
                        span class="info-value" { (txn.source_type) }
                    }
                    div class="info-item" {
                        span class="info-label" { "来源单号" }
                        span class="info-value mono" { (txn.source_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "备注" }
                        span class="info-value" { (if txn.remark.as_deref().unwrap_or("").is_empty() { "—" } else { txn.remark.as_deref().unwrap_or("—") }) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "创建时间" }
                        span class="info-value mono" { (txn.created_at.format("%Y-%m-%d %H:%M:%S")) }
                    }
                }
            }
        }
    }
}
