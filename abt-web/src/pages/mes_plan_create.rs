use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::{ProductQuery, ProductService};
use abt_core::mes::production_plan::ProductionPlanService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_plan::{PlanCreatePath, PlanListPath, PlanItemRowPath, ProductSearchPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct PlanCreateForm {
 pub plan_type: String,
 pub plan_date: String,
 pub remark: Option<String>,
 pub items_json: Option<String>,
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "create")]
pub async fn get_plan_create(
 _path: PlanCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;

 let content = plan_create_page();
 let page_html = admin_page(
 is_htmx, "新建生产计划", &claims, "production", PlanCreatePath::PATH, "生产管理", Some(PlanListPath::PATH), content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

#[require_permission("WORK_ORDER", "create")]
pub async fn create_plan(
 _path: PlanCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<PlanCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.production_plan_service();

 let plan_type = match form.plan_type.as_str() {
 "Mto" => abt_core::mes::enums::PlanType::Mto,
 _ => abt_core::mes::enums::PlanType::Mts,
 };
 let plan_date: chrono::NaiveDate = form.plan_date.parse().map_err(|_| {
 abt_core::shared::types::DomainError::Validation("无效日期格式".into())
 })?;

 let items: Vec<abt_core::mes::production_plan::CreatePlanItemReq> = form
 .items_json
 .as_deref()
 .map(|j| serde_json::from_str(j).unwrap_or_default())
 .unwrap_or_default();

 let req = abt_core::mes::production_plan::CreatePlanReq {
 plan_type,
 plan_date,
 remark: form.remark,
 items,
 };

 let _id = svc.create(&service_ctx, &mut tx, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 Ok(axum::response::Response::builder()
 .header("HX-Redirect", PlanListPath::PATH)
 .body(axum::body::Body::empty())
 .unwrap())
}

pub async fn get_item_row(_path: PlanItemRowPath) -> Result<Html<String>> {
 Ok(Html(plan_item_row_html(0).into_string()))
}

// ── Product Search ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchQuery {
 pub name: Option<String>,
 pub code: Option<String>,
}

pub async fn search_products(
 _path: ProductSearchPath,
 ctx: RequestContext,
 axum::extract::Query(query): axum::extract::Query<ProductSearchQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();
 let name = query.name.unwrap_or_default().trim().to_string();
 let code = query.code.unwrap_or_default().trim().to_string();
 let filter = ProductQuery {
 name: if name.is_empty() { None } else { Some(name) },
 code: if code.is_empty() { None } else { Some(code) },
 ..Default::default()
 };
 let result = svc.list(
 &service_ctx,
 &mut conn,
 filter,
 PageParams { page: 1, page_size: 20 },
 ).await?;
 let rows = if result.items.is_empty() {
 html! {
    tr {
        td colspan="3" class="text-center text-muted p-6" { "未找到匹配的产品" }
    }
}
 } else {
 html! {
    @for p in &result.items {
        tr  class="cursor-pointer"
            _=({
                format!(
                    "on dblclick set window._selectedProduct to {{id: {}, name: '{}'}} then remove .is-open from #product-picker then send productSelected to #product-picker",
                    p.product_id,
                    p.pdt_name.replace('\'', "\\'"),
                )
            })
        {
            td class="font-mono tabular-nums" { (p.product_code) }
            td { (p.pdt_name) }
            td class="w-[60px]" { (p.unit) }
        }
    }
}
 };

 Ok(Html(rows.into_string()))
}

// ── Components ──

fn plan_create_page() -> Markup {
 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            href=(format!("{}?restore=true", PlanListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回计划列表" }
        // ── Page Header ──
        div class="flex items-center justify-between mb-5" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建生产计划" }
        }
        form id="plan-create-form" hx-post=(PlanCreatePath::PATH) hx-swap="none" {
            // ── Basic Info ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "排产类型 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="plan_type"
                            required
                        {
                            option value="Mto" { "按单生产 (MTO)" }
                            option value="Mts" { "按库存备货 (MTS)" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "计划日期 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="date"
                            name="plan_date"
                            required;
                    }
                    div class="form-field col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "备注"
                        }
                        textarea
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent resize-y"
                            name="remark"
                            rows="2"
                            placeholder="可选备注…" {}
                    }
                }
            }
            // ── Plan Items ──
            div class="form-section p-0 overflow-hidden" {
                div class="px-6 pt-6 pb-4" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3" {
                        (icon::box_icon("w-[18px] h-[18px]"))
                        "计划明细"
                        span id="plan-item-count" class="ml-auto text-xs font-normal text-muted" {
                            "共 0 项"
                        }
                    }
                }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-10 text-center" { "序号" }
                                th { "产品" }
                                th class="text-right text-[13px]" {
                                    "计划数量 "
                                    span class="required" { "*" }
                                }
                                th { "开始日期" }
                                th { "结束日期" }
                                th { "优先级" }
                                th class="w-10" {}
                            }
                        }
                        tbody id="plan-items-tbody" {}
                    }
                }
                div class="p-4" {
                    button
                        type="button"
                        class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
                        id="add-plan-item-btn"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加计划行" }
                }
            }
            input type="hidden" name="items_json" id="items-json-input";
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                div {}
                div class="flex gap-3" {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", PlanListPath::PATH))
                    { "取消" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::check_circle_icon("w-4 h-4")) "提交" }
                }
            }
        }
        // ── Product Picker Modal ──
        div id="product-picker"
            class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            _="on click[me is event.target] remove .is-open
 on productSelected
 if window._productPickerTarget
 set t to window._productPickerTarget
 remove .picker-placeholder from (t's querySelector('[data-field=\"product_name\"]'))
 put window._selectedProduct.name into (t's querySelector('[data-field=\"product_name\"]'))
 set (t's querySelector('[data-field=\"product_id\"]'))'s value to window._selectedProduct.id"
        {
            div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt"
            {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 { "选择产品" }
                    button
                        type="button"
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                        _="on click remove .is-open from #product-picker"
                    { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="flex gap-2 mb-2" {
                        input
                            type="text"
                            class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            placeholder="产品名称…"
                            id="product-search-name"
                            name="name"
                            hx-get=(ProductSearchPath::PATH)
                            hx-trigger="load, input changed delay:300ms"
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            hx-include="#product-search-name, #product-search-code";
                        input
                            type="text"
                            class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            placeholder="产品编码…"
                            id="product-search-code"
                            name="code"
                            hx-get=(ProductSearchPath::PATH)
                            hx-trigger="input changed delay:300ms"
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            hx-include="#product-search-name, #product-search-code";
                    }
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-[120px]" { "编码" }
                                th { "名称" }
                                th class="w-[60px]" { "单位" }
                            }
                        }
                        tbody id="product-search-results" {
                            tr {
                                td colspan="3" class="text-center text-muted p-6" { "正在加载..." }
                            }
                        }
                    }
                }
            }
        }
    }
    ({
        maud::PreEscaped(
            r#"<script>
 window.openProductPicker = function(tr) {
 window._productPickerTarget = tr;
 document.getElementById('product-picker').classList.add('is-open');
 };
 (function(){
 let idx = 0;
 const tbody = document.getElementById('plan-items-tbody');
 document.getElementById('add-plan-item-btn').addEventListener('click', function(){
 const tr = document.createElement('tr');
 const i = idx++;
 tr.innerHTML = `
 <td class="text-muted text-xs text-center">${i+1}</td>
 <td>
 <div class="flex items-center gap-[6px] border border-border rounded-sm bg-white cursor-pointer px-2 py-[5px]"
 onclick="window.openProductPicker(this.closest('tr'))">
 <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M9 3.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM2 9a7 7 0 1112.452 4.391l3.328 3.329a.75.75 0 11-1.06 1.06l-3.329-3.328A7 7 0 012 9z" clip-rule="evenodd"/></svg>
 <span data-field="product_name" class="text-muted text-[13px]">点击选择产品</span>
 <input type="hidden" data-field="product_id">
 </div>
 </td>
 <td><input class="w-full px-2 py-[5px] text-right text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="number" step="any" data-field="planned_qty" placeholder="0" required></td>
 <td><input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="date" data-field="scheduled_start" required></td>
 <td><input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" type="date" data-field="scheduled_end" required></td>
 <td><input class="w-full px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent w-[60px]" type="number" step="any" data-field="priority" value="1"></td>
 <td><button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" onclick="this.closest('tr').remove()">✕</button></td>
 `;
 tbody.appendChild(tr);
 });
 document.getElementById('plan-create-form').addEventListener('submit', function(e){
 const rows = tbody.querySelectorAll('tr');
 const items = [];
 rows.forEach(r => {
 const obj = {};
 r.querySelectorAll('[data-field]').forEach(inp => {
 const f = inp.getAttribute('data-field');
 let v = inp.value;
 if(f === 'planned_qty' || f === 'priority' || f === 'product_id') v = Number(v);
 if(f !== 'product_name') obj[f] = v;
 });
 if(obj.product_id) items.push(obj);
 });
 document.getElementById('items-json-input').value = JSON.stringify(items);
 });
 })();
 </script>"#,
        )
    })
}
}

fn plan_item_row_html(index: usize) -> Markup {
 html! {
    tr {
        td class="text-muted text-xs text-center" { (index + 1) }
        td {
            div class="flex items-center gap-[6px] border border-border rounded-sm bg-white cursor-pointer px-2 py-[5px]"
                _="on click set window._productPickerTarget to closest tr then add .is-open to #product-picker"
            {
                (icon::search_icon("w-3.5 h-3.5 text-muted"))
                span data-field="product_name" class="text-muted text-[13px]" { "点击选择产品" }
                input type="hidden" data-field="product_id";
            }
        }
        td {
            input
                class="w-full px-2 py-[5px] text-right text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="number"
                step="any"
                name=(format!("items[{index}].planned_qty"));
        }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="date"
                name=(format!("items[{index}].scheduled_start"));
        }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="date"
                name=(format!("items[{index}].scheduled_end"));
        }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="number"
                step="any"
                name=(format!("items[{index}].priority"))
                value="1"
                class="w-[60px]";
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
            { "✕" }
        }
    }
}
}
