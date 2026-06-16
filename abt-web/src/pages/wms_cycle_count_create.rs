use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::cycle_count::model::{CreateCycleCountReq, CreateCycleCountItemReq};
use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::*;
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

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct CycleCountItemWeb {
    product_id: String,
    bin_id: Option<String>,
    batch_no: Option<String>,
    system_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCycleCountForm {
    #[serde(deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub zone_id: Option<i64>,
    pub count_date: String,
    pub is_blind: Option<String>,
    pub remark: Option<String>,
    pub action: Option<String>,
    pub items_json: String,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_create(
    _path: CycleCountCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = cycle_count_create_page(&warehouses);
    let page_html = admin_page(
        is_htmx,
        "新建盘点",
        &claims,
        "inventory",
        CycleCountCreatePath::PATH,
        "库存管理",
        Some("新建盘点"),
        content, &nav_filter,    );

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

/// HTMX: return a single item row fragment
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

#[require_permission("INVENTORY", "create")]
pub async fn create_cycle_count(
    _path: CycleCountCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateCycleCountForm>,
) -> Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.cycle_count_service();

    let count_date = chrono::NaiveDate::parse_from_str(&form.count_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效日期格式: {e}")))?;

    let is_blind = form.is_blind.as_deref() == Some("on");
    let warehouse_id = form.warehouse_id
        .ok_or_else(|| DomainError::validation("请选择盘点仓库"))?;

    let web_items: Vec<CycleCountItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("物料数据无效: {e}")))?;

    let items: Vec<CreateCycleCountItemReq> = web_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse().unwrap_or(0);
        let bin_id: i64 = it.bin_id.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0);
        let system_qty: Decimal = it.system_qty.parse().unwrap_or(Decimal::ZERO);
        CreateCycleCountItemReq {
            bin_id,
            product_id,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
            system_qty,
        }
    }).collect();

    let req = CreateCycleCountReq {
        warehouse_id,
        zone_id: form.zone_id,
        count_date,
        is_blind,
        remark: form.remark,
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, req).await?;

    if form.action.as_deref() == Some("start") {
        svc.start_count(&service_ctx, &mut conn, id).await?;
    }

    let redirect = CycleCountListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())).into_response())
}

// ── Components ──

fn cycle_count_create_page(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div {
            a href=(format!("{}?restore=true", CycleCountListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回盘点列表"
            }

            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建盘点" }
            }

            div class="flex items-center" {
                div class="flex items-center gap-2 text-xs text-text-muted current" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "草稿" }
                div class="w-[48px] h-[2px] bg-border" {}
                div class="flex items-center gap-2 text-xs text-text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "盘点中" }
                div class="w-[48px] h-[2px] bg-border" {}
                div class="flex items-center gap-2 text-xs text-text-muted" { span class="w-[10px] h-[10px] rounded-full bg-border" {} "完成" }
            }

            form hx-post=(CycleCountCreatePath::PATH) hx-swap="none" id="cycleCountForm"
                onsubmit="return cycleCountCollectItems()" {

                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::building_icon("w-4 h-4"))
                        "盘点信息"
                    }
                    div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "仓库 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" {
                                option value="" { "全部库区" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "盘点日期 " span class="required" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="count_date" required {}
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "盲盘模式" }
                            label style="display:flex;align-items:center;gap:var(--space-2);cursor:pointer;padding-top:var(--space-2)" {
                                input type="checkbox" name="is_blind";
                                "开启盲盘（隐藏系统数量）"
                            }
                        }
                    }
                }

                // ── Line Items ──
                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::box_icon("w-4 h-4"))
                        "盘点物料"
                        span id="cc-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th style="width:40px" { "行号" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "规格" }
                                    th style="width:100px" { "储位" }
                                    th style="width:120px" { "批次号" }
                                    th style="width:100px" { "系统数量" }
                                    th style="width:40px" { }
                                }
                            }
                            tbody id="cc-item-tbody" { }
                        }
                    }
                    button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
                        _="on click add .is-open to #product-modal" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加物料"
                    }
                }

                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" rows="3" placeholder="可选备注…" style="resize:vertical;width:100%;min-height:80px" {}
                }

                input type="hidden" name="items_json" id="cc-items-json" value="[]" {}

                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" href=(format!("{}?restore=true", CycleCountListPath::PATH)) { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" name="action" value="draft" {
                        "保存草稿"
                    }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" name="action" value="start" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "开始盘点"
                    }
                }
            }
        }

        // ── Product Search Modal ──
        div id="product-modal" class="fixed z-[1000] grid place-items-center opacity-0"
            _="on click[me is event.target] remove .is-open" {
            div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-lg" onclick="event.stopPropagation()" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { "选择物料" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #product-modal" { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" hx-disinherit="hx-select" {
                    div class="flex gap-4 p-4 border-b" {
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-[12px] font-medium text-fg-2" { "产品名称" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(CycleCountProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#cc-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-[12px] font-medium text-fg-2" { "产品编码" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(CycleCountProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#cc-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
                            hx-get=(CycleCountProductsPath::PATH)
                            hx-target="#cc-product-results"
                            hx-swap="innerHTML"
                            _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
                            "清除"
                        }
                    }
                    div id="cc-product-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索物料" }
                        }
                    }
                }
            }
        }

        // ── JS ──
        (maud::PreEscaped(r#"<script>
        function ccCalcSummary() {
            var tbody = document.getElementById('cc-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            document.getElementById('cc-item-count').textContent = '共 ' + rows.length + ' 项';
        }

        function ccRenumber() {
            var tbody = document.getElementById('cc-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            rows.forEach(function(row, i) {
                row.querySelector('.line-num').textContent = i + 1;
            });
            ccCalcSummary();
        }

        function cycleCountCollectItems() {
            var tbody = document.getElementById('cc-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            var items = [];
            rows.forEach(function(row) {
                items.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    bin_id: row.querySelector('input[name="bin_id"]').value || null,
                    batch_no: row.querySelector('input[name="batch_no"]').value || null,
                    system_qty: row.querySelector('input[name="system_qty"]').value || '0'
                });
            });
            document.getElementById('cc-items-json').value = JSON.stringify(items);
            return true;
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
            div class="py-2" {
                @for p in products {
                    div class="flex items-center justify-between p-3 border-b" {
                        div class="product-select-info" {
                            div class="text-sm font-medium text-fg" { (p.pdt_name) }
                            div class="text-[12px] text-text-muted flex items-center gap-[6px] flex-wrap" {
                                span class="bg-surface rounded-sm" { (p.product_code) }
                                span class="text-border" { "·" }
                                span { (p.meta.specification) }
                                span class="text-border" { "·" }
                                span { (p.unit) }
                            }
                        }
                        button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-get=(format!("{}?product_id={}", CycleCountItemRowPath::PATH, p.product_id))
                            hx-target="#cc-item-tbody"
                            hx-swap="beforeend"
                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from #product-modal then wait 50ms then call ccRenumber()" {
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
        tr {
            td class="text-text-muted text-xs text-center" { }
            td class="font-mono tabular-nums" { (product.product_code) }
            td { (product.pdt_name) }
            td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="bin_id" placeholder="储位ID" style="width:80px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" style="width:100px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0" step="any" name="system_qty" placeholder="0" style="width:80px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="w-[28px] h-[28px] border-none text-text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
                _="on click remove closest <tr/> then call ccRenumber()" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
