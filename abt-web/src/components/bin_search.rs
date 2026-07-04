//! 库位选择弹窗组件（左仓库列表 + 右库位列表）。
//!
//! 由两件事拼成：
//! 1. 行内按钮 `warehouse_bin_cell` —— 点击 → JS `binPickerOpen` 记当前行 + 写 product_id/mode → 开弹窗
//! 2. 页面级弹窗壳 `bin_picker_modal` —— 左仓库项 hx-get `/api/bin-picker` 加载右侧库位 fragment
//!
//! `picker_bins` 按 `mode` 分两种语义：
//! - inbound（入库）：排除被其他物料占用的库位（一库位一物料），该物料已有库存的库位排前推荐，空位可选
//! - outbound（出库）：仅该物料在该仓库有实物存量的库位，按实物量降序（出库不能选空位/他物料位）
//!
//! hidden input 同时带 `name`（入库创建页 `wmsStockInCollectItems` 读）+ `data-k`（作业中心 drawer
//! `wcCollectItems` 读）——两个收集器各取所需，不冲突。
use std::collections::{HashMap, HashSet};

use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::errors::Result;
use crate::utils::RequestContext;
use abt_core::master_data::product::ProductService;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::warehouse::model::{Bin, Warehouse};

// ── 弹窗式库位选择：端点 + 弹窗壳 ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/bin-picker")]
pub struct BinPickerPath;

#[derive(Debug, Deserialize)]
pub struct BinPickerParams {
    #[serde(default)]
    pub product_id: i64,
    #[serde(default)]
    pub warehouse_id: i64,
    /// inbound（默认）：排除他物料占用 + 同物料排前 + 空位可选
    /// outbound：仅该物料有实物存量的库位，按实物量降序
    #[serde(default)]
    pub mode: String,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route(BinPickerPath::PATH, get(picker_bins))
        .route(BinPickerProductInfoPath::PATH, get(product_info))
}

/// 仓库 + 库位选择按钮（弹窗式）。
///
/// 渲染：一个按钮（显示当前仓库名 / 占位文字）+ hidden `warehouse_id` + hidden `bin_id`。
/// 点击按钮 → JS `binPickerOpen(me)` 记录当前行 + 写 product_id/mode → 打开 `#bin-picker-modal`。
///
/// - `bid`: 唯一 key（拼入 hidden 的 `data-bin-key`）
/// - `product_id`: 库位推荐用（查该物料已有库存的库位）
/// - `warehouses`: 仓库列表（弹窗左侧 + 单仓库时按钮回显名）
/// - `auto_wh`: 单仓库自动选中值；空串不预选（按钮显示占位）
/// - `mode`: `"inbound"`（入库）/ `"outbound"`（出库），写入按钮 `data-mode`，决定 picker 端过滤语义
pub fn warehouse_bin_cell(
    bid: &str,
    product_id: i64,
    warehouses: &[Warehouse],
    auto_wh: &str,
    mode: &str,
) -> Markup {
    let label = if auto_wh.is_empty() {
        "选择仓库 / 库位".to_string()
    } else {
        warehouses
            .iter()
            .find(|w| w.id.to_string() == auto_wh)
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "选择仓库 / 库位".to_string())
    };
    html! {
        button type="button"
            class="bin-cell-btn w-full px-2 py-1.5 border border-border rounded-sm text-xs bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
            data-bin-key=(bid)
            data-product-id=(product_id)
            data-mode=(mode)
            _="on click call binPickerOpen(me)"
        { (label) }
        input type="hidden" name="warehouse_id" data-k="warehouse_id" data-bin-key=(bid) value=(auto_wh) {}
        input type="hidden" name="bin_id" data-k="bin_id" data-bin-key=(bid) value="" {}
    }
}

/// HTMX: 按产品 + 仓库 + 模式返回可选库位列表（HTML fragment，填弹窗右侧）。
///
/// - inbound：仓库下所有可用库位，**排除被其他物料占用的**（一库位一物料），该物料已有库存的排前推荐，空位可选
/// - outbound：**仅该物料在该仓库有实物存量（qty>0）的库位**，按实物量降序（出库不能选空位/他物料位）
pub async fn picker_bins(
    ctx: RequestContext,
    Query(p): Query<BinPickerParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouse_svc = state.warehouse_service();
    let inv_svc = state.inventory_service();
    let is_outbound = p.mode == "outbound";

    // 仓库下所有可用库位
    let bins: Vec<Bin> = if p.warehouse_id <= 0 {
        Vec::new()
    } else {
        warehouse_svc
            .list_bins_by_warehouse(
                &service_ctx, &mut conn,
                abt_core::wms::warehouse::model::ListBinsByWarehouseParams {
                    warehouse_id: p.warehouse_id,
                    keyword: None,
                    is_active: Some(true),
                    page: 1, page_size: 500,
                },
            )
            .await
            .map(|r| r.items)
            .unwrap_or_default()
    };

    // 该仓库下所有库存记录（实物存量）
    let all_inv = if p.warehouse_id <= 0 {
        Vec::new()
    } else {
        inv_svc
            .query(
                &service_ctx, &mut conn,
                abt_core::wms::inventory::model::InventoryQueryFilter {
                    product_id: None,
                    keyword: None,
                    warehouse_id: Some(p.warehouse_id),
                    bin_id: None,
                },
                1, 500,
            )
            .await
            .map(|r| r.items)
            .unwrap_or_default()
    };

    // bin_id → 该物料在该 bin 的实物存量；occupied = 有其他物料存量的 bin
    let mut my_stock: HashMap<i64, Decimal> = HashMap::new();
    let mut occupied_bins: HashSet<i64> = HashSet::new();
    for v in &all_inv {
        if v.product_id == p.product_id {
            *my_stock.entry(v.bin_id).or_insert(Decimal::ZERO) += v.quantity;
        } else if v.quantity > Decimal::ZERO {
            occupied_bins.insert(v.bin_id);
        }
    }

    let rows: Vec<(Bin, Option<Decimal>)> = if is_outbound {
        // 出库：仅该物料有存量的 bin，按实物量降序
        let mut rows: Vec<(Bin, Option<Decimal>)> = bins
            .into_iter()
            .filter_map(|b| {
                let q = my_stock.get(&b.id).copied().filter(|q| *q > Decimal::ZERO)?;
                Some((b, Some(q)))
            })
            .collect();
        rows.sort_by(|a, b| {
            b.1.unwrap_or(Decimal::ZERO).cmp(&a.1.unwrap_or(Decimal::ZERO))
        });
        rows
    } else {
        // 入库：该物料已有库存的 bin 一律保留（同物料合并，即使 bin 混放他物料）；
        // 其余 bin 排除被它物料占用的；该物料有库存的排前，空位可选。
        let mut rows: Vec<(Bin, Option<Decimal>)> = bins
            .into_iter()
            .filter(|b| {
                let mine = my_stock.get(&b.id).copied().unwrap_or(Decimal::ZERO) > Decimal::ZERO;
                mine || !occupied_bins.contains(&b.id)
            })
            .map(|b| {
                let q = my_stock.get(&b.id).copied().filter(|q| *q > Decimal::ZERO);
                (b, q)
            })
            .collect();
        rows.sort_by_key(|(_, q)| q.is_none());
        rows
    };

    Ok(Html(bin_picker_results(&rows, is_outbound).into_string()))
}

// 物料在某 bin 的库存聚合（product_info 内部用）
struct BinStock {
    warehouse_id: i64,
    warehouse_name: String,
    bin_id: i64,
    bin_code: String,
    qty: Decimal,
}

// ── 该物料产品信息 + 库存分布 + 之前存储位置（弹窗顶部展示 + 仓库标注 + 一键选中）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/bin-picker/product-info")]
pub struct BinPickerProductInfoPath;

#[derive(Debug, Deserialize)]
pub struct ProductInfoParams {
    pub product_id: i64,
}

/// 返回该物料的：
/// - 产品编码/名称（弹窗顶部展示）
/// - `stock_by_warehouse`：各仓库库存量（左侧仓库标「库存 X」+ 排前）
/// - `suggested`：库存量最大的 bin（「之前存储位置」，一键选中用）
pub async fn product_info(
    ctx: RequestContext,
    Query(p): Query<ProductInfoParams>,
) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    // 产品信息（无库存也能取到）
    let (code, name) = match state
        .product_service()
        .get(&service_ctx, &mut conn, p.product_id)
        .await
    {
        Ok(prod) => (prod.product_code, prod.pdt_name),
        Err(_) => (String::new(), String::new()),
    };
    // 库存按 bin 聚合
    let inv = state
        .inventory_service()
        .get_by_product(&service_ctx, &mut conn, p.product_id)
        .await
        .unwrap_or_default();
    let mut by_bin: HashMap<i64, BinStock> = HashMap::new();
    for v in inv {
        let e = by_bin.entry(v.bin_id).or_insert(BinStock {
            warehouse_id: v.warehouse_id,
            warehouse_name: v.warehouse_name.clone(),
            bin_id: v.bin_id,
            bin_code: v.bin_code.clone(),
            qty: Decimal::ZERO,
        });
        e.qty += v.quantity;
    }
    // 各仓库库存量
    let mut wh_qty: HashMap<i64, Decimal> = HashMap::new();
    for e in by_bin.values() {
        *wh_qty.entry(e.warehouse_id).or_insert(Decimal::ZERO) += e.qty;
    }
    let stock_by_warehouse: Vec<serde_json::Value> = wh_qty
        .into_iter()
        .filter(|(_, q)| *q > Decimal::ZERO)
        .map(|(wid, q)| serde_json::json!({ "warehouse_id": wid, "qty": q.to_string() }))
        .collect();
    // 之前存储位置：所有有库存的 bin，按库存量降序（前端折叠列表，每行点击回填）
    let mut bins: Vec<BinStock> = by_bin
        .into_values()
        .filter(|e| e.qty > Decimal::ZERO)
        .collect();
    bins.sort_by(|a, b| b.qty.cmp(&a.qty));
    let stocks: Vec<serde_json::Value> = bins
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "warehouse_id": e.warehouse_id,
                "warehouse_name": e.warehouse_name,
                "bin_id": e.bin_id,
                "bin_code": e.bin_code,
                "qty": e.qty.to_string(),
            })
        })
        .collect();
    Ok(axum::Json(serde_json::json!({
        "product_code": code,
        "product_name": name,
        "stock_by_warehouse": stock_by_warehouse,
        "stocks": stocks,
    })))
}

/// 右侧库位列表 fragment。`is_outbound` 决定空列表提示与副文案。
fn bin_picker_results(rows: &[(Bin, Option<Decimal>)], is_outbound: bool) -> Markup {
    use crate::components::icon;
    html! {
        @if rows.is_empty() {
            div class="text-center text-muted py-10" {
                p class="text-sm" {
                    @if is_outbound { "该物料在此仓库无库存" } @else { "该仓库暂无可用库位" }
                }
                p class="text-xs mt-1" { "请选择其他仓库，或在仓库管理中创建库位" }
            }
        } @else {
            @for (bin, qty) in rows {
                @let suggested = qty.is_some();
                button type="button"
                    class=( format!(
                        "w-full flex items-center justify-between gap-3 px-4 py-2.5 border-b border-border-soft last:border-b-0 text-left transition-colors {}",
                        if suggested { "bg-accent-bg/40 hover:bg-accent-bg" } else { "hover:bg-surface" }
                    ) )
                    data-bin-id=(bin.id)
                    data-bin-code=(bin.code.as_str())
                    data-bin-name=(bin.name.as_str())
                    _="on click call binPickerSelect(@data-bin-id, @data-bin-code, @data-bin-name)"
                {
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-medium text-fg font-mono truncate" { (bin.code.as_str()) }
                        div class="text-xs text-muted truncate mt-0.5" { (bin.name.as_str()) }
                        @if let Some(q) = qty {
                            div class="text-xs text-success flex items-center gap-1 mt-0.5" {
                                (icon::check_circle_icon("w-3 h-3"))
                                @if is_outbound {
                                    "实物存量 " (crate::utils::fmt_qty(*q))
                                } @else {
                                    "已有该物料库存 " (crate::utils::fmt_qty(*q)) " · 推荐同物料合并"
                                }
                            }
                        } @else if !is_outbound {
                            div class="text-xs text-muted mt-0.5" { "空库位" }
                        }
                    }
                }
            }
        }
    }
}

/// 库位选择弹窗壳（左仓库列表 + 右库位列表）。
///
/// 仓库项点击 → HTMX 加载右侧库位（`hx-vals` 携带 product_id + warehouse_id + mode）。
/// `product_id` / `mode` 由弹窗内 hidden input 携带，JS `binPickerOpen` 在打开时写入。
pub fn bin_picker_modal(modal_id: &str, warehouses: &[Warehouse]) -> Markup {
    use crate::components::overlay::modal_shell;
    let wh_list_id = format!("{modal_id}-wh-list");
    let bins_id = format!("{modal_id}-bins");
    let pid_id = format!("{modal_id}-product-id");
    let mode_id = format!("{modal_id}-mode");
    let info_id = format!("{modal_id}-product-info");
    modal_shell(modal_id, "z-[1001]", html! {
        div class="modal bg-bg rounded-xl w-[680px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl" {
            // header
            div class="px-6 py-4 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="font-bold text-base text-fg" { "选择仓库 / 库位" }
                button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                    _=(format!("on click remove .is-open from #{}", modal_id))
                { "×" }
            }
            // hidden：product_id + mode（JS binPickerOpen 写入；mode 默认 inbound）
            input type="hidden" id=(pid_id) value="" {}
            input type="hidden" id=(mode_id) value="inbound" {}
            // 产品信息条（JS 填充：编码/名称 + 之前存储位置 + 一键选中）
            div id=(info_id) class="px-4 py-2 border-b border-border-soft shrink-0 text-xs text-muted" {
                "加载产品信息…"
            }
            // body: 左仓库 + 右库位
            div class="flex flex-1 min-h-0" {
                // 左边仓库列表
                div class="w-[200px] border-r border-border-soft overflow-y-auto shrink-0" id=(wh_list_id) {
                    @for w in warehouses {
                        button type="button"
                            class="wh-item w-full text-left px-4 py-2.5 text-sm border-b border-border-soft hover:bg-surface transition-colors text-fg-2 act:bg-accent-bg act:text-accent act:font-medium"
                            data-warehouse-id=(w.id)
                            data-warehouse-name=(w.name.as_str())
                            hx-get=(BinPickerPath::PATH)
                            hx-target=(format!("#{bins_id}"))
                            hx-swap="innerHTML"
                            hx-vals=(format!(
                                "js:{{product_id: document.getElementById('{}').value, warehouse_id: {}, mode: document.getElementById('{}').value}}",
                                pid_id, w.id, mode_id
                            ))
                            _="on click take .active from .wh-item"
                        { (w.name.as_str()) }
                    }
                }
                // 右边库位列表（搜索框 + 列表）
                div class="flex-1 flex flex-col min-h-0" {
                    div class="p-2 border-b border-border-soft shrink-0" {
                        input type="text"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-xs bg-white text-fg outline-none focus:border-accent"
                            placeholder="搜索库位编码 / 名称"
                            _="on input call binPickerFilterBins(me)" {}
                    }
                    div class="flex-1 overflow-y-auto" id=(bins_id) {
                        div class="text-center text-muted py-10 text-sm" { "选择左侧仓库后加载库位列表" }
                    }
                }
            }
        }
    })
}
