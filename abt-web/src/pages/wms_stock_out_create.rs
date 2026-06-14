use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::enums::TransactionType;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::{StockOutCreatePath, StockOutListPath, StockOutProductsPath, StockOutItemRowPath};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_stock_out_create(
    _path: StockOutCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = stock_out_create_content(&warehouses, &claims.display_name);
    let page_html = admin_page(
        is_htmx, "新建出库单", &claims, "inventory", StockOutCreatePath::PATH, "库存管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// HTMX: search products for the modal
#[require_permission("PRODUCT", "read")]
pub async fn get_products(
    ctx: RequestContext,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let filter = ProductQuery {
        name: params.name.filter(|s| !s.is_empty()),
        code: params.code.filter(|s| !s.is_empty()),
        status: None,
        owner_department_id: None,
        category_id: None,
    };
    let result = svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("INVENTORY", "create")]
pub async fn get_item_row(
    ctx: RequestContext,
    Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();
    let product = svc.get(&service_ctx, &mut conn, params.product_id).await?;
    Ok(Html(item_row_fragment(&product).into_string()))
}

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct StockOutCreateForm {
    pub source_type: String,
    pub source_ref: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub zone_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub bin_id: Option<i64>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct StockOutItemWeb {
    product_id: String,
    quantity: String,
    unit_cost: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_stock_out(
    _path: StockOutCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<StockOutCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_transaction_service();

    let warehouse_id = form.warehouse_id
        .ok_or_else(|| DomainError::validation("请选择来源仓库"))?;

    let web_items: Vec<StockOutItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个物料").into());
    }

    let transaction_type = match form.source_type.as_str() {
        "shipping" | "sales" => TransactionType::SalesShipment,
        "requisition" | "material" => TransactionType::MaterialIssue,
        _ => TransactionType::SalesShipment,
    };

    let source_type = form.source_type.as_str();

    let remark = form.remark.filter(|s| !s.is_empty());

    // Record one transaction per line item
    for item in &web_items {
        let product_id: i64 = item.product_id.parse()
            .map_err(|_| DomainError::validation("无效产品ID"))?;
        let quantity: Decimal = item.quantity.parse()
            .map_err(|_| DomainError::validation("无效数量"))?;
        let unit_cost: Option<Decimal> = item.unit_cost.as_ref()
            .and_then(|s| s.parse().ok());

        if quantity <= Decimal::ZERO {
            return Err(DomainError::validation("出库数量必须大于0").into());
        }

        // Check available stock
        let available = svc.query_available(&service_ctx, &mut conn, product_id, Some(warehouse_id)).await?;
        if quantity > available {
            return Err(DomainError::business_rule(
                format!("库存不足：产品ID {} 需要 {}，可用 {}", product_id, quantity, available),
            ).into());
        }

        let req = RecordTransactionReq {
            doc_number: None,
            delivery_no: None,
            source_doc_number: None,
            transaction_type,
            product_id,
            warehouse_id,
            zone_id: form.zone_id,
            bin_id: form.bin_id,
            batch_no: None,
            quantity,
            unit_cost,
            source_type: source_type.to_string(),
            source_id: 0,
            remark: remark.clone(),
        };

        svc.record(&service_ctx, &mut conn, req).await?;
    }

    let redirect = StockOutListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn stock_out_create_content(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    operator_name: &str,
) -> Markup {
    html! {
        div {
            // ── Back Link ──
            a href="/admin/wms/stock-out" class="back-link" style="display:inline-flex;align-items:center;gap:var(--space-2);color:var(--fg-2);font-size:var(--text-sm);margin-bottom:var(--space-4);text-decoration:none" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回出库列表"
            }

            // ── Page Header ──
            div class="page-header" style="margin-bottom:var(--space-6)" {
                h1 class="page-title" { "新建出库单" }
            }

            // ── Type Switch ──
            div style="display:flex;gap:var(--space-3);margin-bottom:var(--space-6)" {
                div id="type-card-sales" onclick="wmsStockOutSelectType('sales')" style="flex:1;display:flex;flex-direction:column;align-items:center;gap:var(--space-2);padding:var(--space-5) var(--space-4);border:2px solid var(--danger);border-radius:var(--radius-lg);background:var(--danger-bg);cursor:pointer" {
                    (icon::upload_icon("w-7 h-7"))
                    span style="font-weight:600;font-size:var(--text-base);color:var(--fg)" { "销售出库" }
                    span style="font-size:var(--text-xs);color:var(--muted);text-align:center" { "SALES_SHIPMENT\n关联发货申请 / 销售订单\n消耗 SOFT 预留" }
                }
                div id="type-card-material" onclick="wmsStockOutSelectType('material')" style="flex:1;display:flex;flex-direction:column;align-items:center;gap:var(--space-2);padding:var(--space-5) var(--space-4);border:2px solid var(--border);border-radius:var(--radius-lg);background:var(--bg);cursor:pointer" {
                    (icon::clipboard_document_icon("w-7 h-7"))
                    span style="font-weight:600;font-size:var(--text-base);color:var(--fg)" { "生产领料" }
                    span style="font-size:var(--text-xs);color:var(--muted);text-align:center" { "MATERIAL_ISSUE\n关联工单 / 领料单\n消耗 HARD 预留" }
                }
            }

            form id="stockOutForm" hx-post=(StockOutCreatePath::PATH) hx-swap="none"
                onsubmit="return wmsStockOutCollectItems()" {
                // ── Source Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::link_icon("w-4 h-4"))
                        "来源关联"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源类型" }
                            select class="form-select" name="source_type" {
                                option value="shipping" { "发货申请 (SH)" }
                                option value="requisition" { "领料单 (MR)" }
                                option value="manual" { "手工录入" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "来源单号 " span style="color:var(--danger)" { "*" } }
                            input class="form-input" type="text" name="source_ref" placeholder="选择来源单号" readonly;
                        }
                        div class="form-group" {
                            label class="form-label" { "客户/工单" }
                            input class="form-input" type="text" placeholder="选择来源后自动填充" readonly style="background:var(--surface)";
                        }
                        div class="form-group" {
                            label class="form-label" { "预留类型" }
                            input id="reservation-type-input" class="form-input" type="text" value="SOFT 预留（发货消耗）" readonly style="background:var(--surface);color:var(--danger)";
                        }
                    }
                }

                // ── Warehouse Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::building_icon("w-4 h-4"))
                        "出库信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源仓库 " span style="color:var(--danger)" { "*" } }
                            select class="form-select" name="warehouse_id" {
                                option value="" { "请选择仓库" }
                                @for wh in warehouses {
                                    option value=(wh.id) { (wh.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "来源库区" }
                            select class="form-select" name="zone_id" {
                                option value="" { "按拣货策略分配" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "拣货策略" }
                            select class="form-select" name="pick_strategy" {
                                option value="fifo" selected { "FIFO 先进先出" }
                                option value="fefo" { "FEFO 先到期先出" }
                                option value="shortest" { "最短路径" }
                                option value="full_pallet" { "整托优先" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "操作员" }
                            input class="form-input" type="text" value=(operator_name) readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Pick Strategy Tip ──
                div style="padding:var(--space-3) var(--space-4);background:rgba(250,173,20,0.05);border:1px solid rgba(250,173,20,0.15);border-radius:var(--radius-md);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-3)" {
                    (icon::circle_alert_icon("w-4 h-4"))
                    span style="font-size:var(--text-sm);color:var(--fg-2)" {
                        "拣货策略："
                        strong { "FIFO 先进先出" }
                        " — 系统优先拣选最早入库批次的物料，确保库存周转。对于有效期管理物料建议使用 FEFO。"
                    }
                }

                // ── Line Items ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "出库物料明细"
                        span id="stockout-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    table class="detail-table" {
                        thead {
                            tr {
                                th style="width:40px" { "序号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格型号" }
                                th style="width:100px" { "出库数量 " span style="color:var(--danger)" { "*" } }
                                th style="width:90px" { "单位" }
                                th style="width:110px" { "单位成本" }
                                th style="width:110px" { "小计" }
                                th style="width:40px" { }
                            }
                        }
                        tbody id="stockout-item-tbody" {
                            // JS-managed dynamic rows
                        }
                    }
                    div style="margin-top:var(--space-4)" {
                        button type="button" class="add-row-btn"
                            _="on click add .is-open to #stockout-product-modal" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加物料"
                        }
                    }
                }

                // ── Reservation Info ──
                div style="margin-top:var(--space-4);padding:var(--space-4);background:linear-gradient(135deg,rgba(250,173,20,0.04),rgba(255,77,79,0.04));border:1px solid var(--border-soft);border-radius:var(--radius-md)" {
                    h4 style="font-size:var(--text-sm);font-weight:600;color:var(--fg-2);margin-bottom:var(--space-3);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::lock_icon("w-4 h-4"))
                        "库存预留 & 可用性检查"
                    }
                    div style="display:grid;grid-template-columns:repeat(3,1fr);gap:var(--space-4)" {
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "预留类型" }
                            div id="reservation-type-badge" style="font-size:var(--text-base);font-weight:600;font-family:var(--font-mono);color:var(--danger)" { "SOFT" }
                        }
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "已预留量" }
                            div style="font-size:var(--text-lg);font-weight:600;font-family:var(--font-mono);color:var(--warn)" { "—" }
                        }
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "出库后释放" }
                            div style="font-size:var(--text-base);font-weight:600;font-family:var(--font-mono);color:var(--success)" { "→ available_qty" }
                        }
                    }
                }

                // ── Summary ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::clipboard_list_icon("w-4 h-4"))
                        "出库汇总"
                    }
                    div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-6)" {
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "物料种类" }
                            div id="stockout-summary-kinds" style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono)" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "出库总量" }
                            div id="stockout-summary-qty" style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono)" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--danger-bg);border-radius:var(--radius-md);border:1px solid rgba(255,77,79,0.15)" {
                            div style="font-size:11px;color:var(--danger);margin-bottom:var(--space-1)" { "出库总金额" }
                            div id="stockout-summary-amount" style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono);color:var(--danger)" { "¥0.00" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "拣货策略" }
                            div style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { "FIFO" }
                        }
                    }
                }

                // ── Remark ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    textarea class="form-input" name="remark" placeholder="输入备注信息…" rows="3" style="width:100%;min-height:80px;padding:var(--space-2) var(--space-3);resize:vertical" { }
                }

                // hidden input for items JSON
                input type="hidden" name="items_json" id="stockout-items-json" value="[]" {}
                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href="/admin/wms/stock-out" { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="button" class="btn btn-default" { "保存草稿" }
                        button type="submit" class="btn btn-primary" style="background:var(--danger);border-color:var(--danger)" {
                            (icon::upload_icon("w-4 h-4"))
                            "确认出库"
                        }
                    }
                }
            }
        }

        // ── Product Search Modal ──
        div id="stockout-product-modal" class="modal-overlay"
            _="on click[me is event.target] remove .is-open" {
            div class="modal modal-lg" onclick="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "选择物料" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #stockout-product-modal" { "×" }
                }
                div class="modal-body" style="padding:0" hx-disinherit="hx-select" {
                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { "产品名称" }
                            input class="product-search-input" type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(StockOutProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#stockout-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(StockOutProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#stockout-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(StockOutProductsPath::PATH)
                            hx-target="#stockout-product-results"
                            hx-swap="innerHTML"
                            _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
                            "清除"
                        }
                    }
                    div id="stockout-product-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索物料" }
                        }
                    }
                }
            }
        }

        // ── Line Item JS ──
        (maud::PreEscaped(r#"<script>
        // Line item calculations
        function wmsStockOutCalcRow(row) {
            var qtyInput = row.querySelector('input[name="quantity"]');
            var costInput = row.querySelector('input[name="unit_cost"]');
            var totalCell = row.querySelector('.line-subtotal');
            var qty = parseFloat(qtyInput.value) || 0;
            var cost = parseFloat(costInput.value) || 0;
            var subtotal = qty * cost;
            totalCell.textContent = subtotal > 0 ? '¥' + subtotal.toFixed(2) : '—';
            wmsStockOutCalcSummary();
        }

        function wmsStockOutCalcSummary() {
            var tbody = document.getElementById('stockout-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            var kinds = rows.length;
            var totalQty = 0;
            var totalAmount = 0;
            rows.forEach(function(row) {
                var qty = parseFloat(row.querySelector('input[name="quantity"]').value) || 0;
                var cost = parseFloat(row.querySelector('input[name="unit_cost"]').value) || 0;
                totalQty += qty;
                totalAmount += qty * cost;
            });
            document.getElementById('stockout-summary-kinds').textContent = kinds;
            document.getElementById('stockout-summary-qty').textContent = totalQty;
            document.getElementById('stockout-summary-amount').textContent = '¥' + totalAmount.toFixed(2);
            document.getElementById('stockout-item-count').textContent = '共 ' + kinds + ' 项';
        }

        // Collect items for form submission
        function wmsStockOutCollectItems() {
            var tbody = document.getElementById('stockout-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            var items = [];
            rows.forEach(function(row) {
                items.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    quantity: row.querySelector('input[name="quantity"]').value || '0',
                    unit_cost: row.querySelector('input[name="unit_cost"]').value || null
                });
            });
            document.getElementById('stockout-items-json').value = JSON.stringify(items);
            if (items.length === 0) {
                alert('请至少添加一个物料');
                return false;
            }
            return true;
        }

        // Renumber rows
        function wmsStockOutRenumber() {
            var tbody = document.getElementById('stockout-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            rows.forEach(function(row, i) {
                row.querySelector('.line-num').textContent = i + 1;
            });
            wmsStockOutCalcSummary();
        }

        // Type switch: toggle card visual state and update form fields
        function wmsStockOutSelectType(type) {
            var sales = document.getElementById('type-card-sales');
            var material = document.getElementById('type-card-material');
            var selectEl = document.querySelector('select[name=source_type]');
            var resInput = document.getElementById('reservation-type-input');
            var resBadge = document.getElementById('reservation-type-badge');
            if (type === 'sales') {
                sales.style.border = '2px solid var(--danger)';
                sales.style.background = 'var(--danger-bg)';
                material.style.border = '2px solid var(--border)';
                material.style.background = 'var(--bg)';
                selectEl.value = 'shipping';
                resInput.value = 'SOFT 预留（发货消耗）';
                resBadge.textContent = 'SOFT';
            } else {
                material.style.border = '2px solid var(--danger)';
                material.style.background = 'var(--danger-bg)';
                sales.style.border = '2px solid var(--border)';
                sales.style.background = 'var(--bg)';
                selectEl.value = 'requisition';
                resInput.value = 'HARD 预留（生产领料）';
                resBadge.textContent = 'HARD';
            }
        }
        </script>"#))
    }
}

/// Product search results fragment
fn product_list_fragment(products: &[abt_core::master_data::product::model::Product]) -> Markup {
    html! {
        @if products.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::package_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
            }
        } @else {
            div class="product-select-list" {
                @for p in products {
                    div class="product-select-item" {
                        div class="product-select-info" {
                            div class="product-select-name" { (p.pdt_name) }
                            div class="product-select-meta" {
                                span class="product-select-code" { (p.product_code) }
                                span class="product-select-sep" { "·" }
                                span { (p.meta.specification) }
                                span class="product-select-sep" { "·" }
                                span { (p.unit) }
                            }
                        }
                        button type="button" class="btn btn-sm btn-primary"
                            hx-get=(format!("{}?product_id={}", StockOutItemRowPath::PATH, p.product_id))
                            hx-target="#stockout-item-tbody"
                            hx-swap="beforeend"
                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from #stockout-product-modal then wait 50ms then call wmsStockOutRenumber()" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
    html! {
        tr oninput="wmsStockOutCalcRow(this)" {
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
            td { input class="form-input num-input" type="number" min="0.01" step="any" name="quantity" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td style="text-align:center;font-size:var(--text-sm);color:var(--fg-2)" { (product.unit) }
            td { input class="form-input num-input" type="number" step="any" name="unit_cost" placeholder="0.00" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td class="line-subtotal" style="text-align:right;font-family:var(--font-mono);font-weight:600;white-space:nowrap" { "—" }
            td { button type="button" class="btn-remove-row" title="删除行"
                _="on click remove closest <tr/> then call wmsStockOutRenumber()" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
