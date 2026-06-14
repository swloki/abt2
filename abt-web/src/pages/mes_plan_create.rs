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
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
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

    let _id = svc.create(&service_ctx, &mut conn, req).await?;

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
    pub q: Option<String>,
}

pub async fn search_products(
    _path: ProductSearchPath,
    ctx: RequestContext,
    axum::extract::Query(query): axum::extract::Query<ProductSearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();
    let keyword = query.q.unwrap_or_default().trim().to_string();
    let filter = ProductQuery {
        name: if keyword.is_empty() { None } else { Some(keyword) },
        ..Default::default()
    };
    let result = svc.list(
        &service_ctx,
        &mut conn,
        filter,
        PageParams { page: 1, page_size: 20 },
    ).await?;
    let rows = if result.items.is_empty() {
        html! { tr { td colspan="4" style="text-align:center;color:var(--muted);padding:24px" { "未找到匹配的产品" } } }
    } else {
        html! {
            @for p in &result.items {
                tr {
                    td class="mono" { (p.product_code) }
                    td { (p.pdt_name) }
                    td style="width:60px" { (p.unit) }
                    td style="width:60px;text-align:center" {
                        button type="button" class="btn btn-primary btn-sm"
                            _=(format!("on click set window._selectedProduct to {{id: {}, name: '{}'}} then remove .is-open from #product-picker then send productSelected to #product-picker", p.product_id, p.pdt_name.replace('\'', "\\'"))) { "选择" }
                    }
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
            div class="page-header" {
                div class="page-header-left" {
                    a class="back-link" href=(PlanListPath::PATH) {
                        "← 返回列表"
                    }
                    h1 class="page-title" { "新建生产计划" }
                }
            }

            form id="plan-create-form" hx-post=(PlanCreatePath::PATH) hx-swap="none" {
                // ── Basic Info ──
                div class="form-section" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label class="form-label" { "排产类型" }
                            select class="form-select" name="plan_type" required {
                                option value="Mto" { "按单生产 (MTO)" }
                                option value="Mts" { "按库存备货 (MTS)" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "计划日期" }
                            input class="form-input" type="date" name="plan_date" required;
                        }
                        div class="form-field span-2" {
                            label class="form-label" { "备注" }
                            textarea class="form-input" name="remark" rows="2" {}
                        }
                    }
                }

                // ── Plan Items ──
                div class="form-section" {
                    div class="form-section-title" { "计划明细" }
                    div class="data-card" {
                        div class="data-card-scroll" {
                            table class="data-table" {
                                thead {
                                    tr {
                                        th style="width:40px" { "序号" }
                                        th { "产品" }
                                        th class="num-right" { "计划数量" }
                                        th { "开始日期" }
                                        th { "结束日期" }
                                        th { "优先级" }
                                        th style="width:40px" { }
                                    }
                                }
                                tbody id="plan-items-tbody" {
                                    // Dynamic rows added via JS
                                }
                            }
                        }
                    }
                    div class="add-row-bar" {
                        button type="button" class="btn-add-row" id="add-plan-item-btn" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加计划行"
                        }
                    }
                    input type="hidden" name="items_json" id="items-json-input";
                }

                // ── Actions ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(PlanListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        "提交"
                    }
                }
            }

            // ── Product Picker Modal ──
            div id="product-picker" class="modal-overlay"
                _="on click[me is event.target] remove .is-open
                   on productSelected
                     if window._productPickerTarget
                       set t to window._productPickerTarget
                       put window._selectedProduct.name into (t's querySelector('[data-field=\"product_name\"]'))
                       set (t's querySelector('[data-field=\"product_id\"]'))'s value to window._selectedProduct.id" {
                div class="modal modal-lg" _="on click halt" {
                    div class="modal-head" {
                        h2 { "选择产品" }
                        button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            _="on click remove .is-open from #product-picker" { "×" }
                    }
                    div class="modal-body" {
                        input type="text" class="search-input" placeholder="搜索产品名称或编码…"
                            id="product-search-input"
                            name="q"
                            hx-get=(ProductSearchPath::PATH)
                            hx-trigger="input changed delay:300ms"
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            hx-include="this";
                        table class="data-table" style="margin-top:12px" {
                            thead {
                                tr {
                                    th style="width:120px" { "编码" }
                                    th { "名称" }
                                    th style="width:60px" { "单位" }
                                    th style="width:60px" { }
                                }
                            }
                            tbody id="product-search-results" {
                                tr { td colspan="4" style="text-align:center;color:var(--muted);padding:24px" { "请输入关键词搜索" } }
                            }
                        }
                    }
                }
            }
        }
        (maud::PreEscaped(r#"<script>
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
                    <td class="line-num">${i+1}</td>
                    <td>
                      <div class="product-cell" style="cursor:pointer;padding:4px 8px;border:1px dashed var(--border);border-radius:4px"
                           onclick="window.openProductPicker(this.closest('tr'))">
                        <span data-field="product_name" style="color:var(--muted)">点击选择产品</span>
                        <input type="hidden" data-field="product_id">
                      </div>
                    </td>
                    <td><input class="form-input num-right" type="number" step="0.01" data-field="planned_qty" placeholder="数量" required></td>
                    <td><input class="form-input" type="date" data-field="scheduled_start" required></td>
                    <td><input class="form-input" type="date" data-field="scheduled_end" required></td>
                    <td><input class="form-input" type="number" data-field="priority" value="1" style="width:60px"></td>
                    <td><button type="button" class="btn-remove-row" onclick="this.closest('tr').remove()">✕</button></td>
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
        </script>"#))
    }
}

fn plan_item_row_html(index: usize) -> Markup {
    html! {
        tr {
            td class="line-num" { (index + 1) }
            td {
                div class="product-cell" style="cursor:pointer;padding:4px 8px;border:1px dashed var(--border);border-radius:4px"
                    _="on click set window._productPickerTarget to closest tr then add .is-open to #product-picker" {
                    span data-field="product_name" style="color:var(--muted)" { "点击选择产品" }
                    input type="hidden" data-field="product_id";
                }
            }
            td { input class="form-input num-right" type="number" step="0.01" name=(format!("items[{index}].planned_qty")); }
            td { input class="form-input" type="date" name=(format!("items[{index}].scheduled_start")); }
            td { input class="form-input" type="date" name=(format!("items[{index}].scheduled_end")); }
            td { input class="form-input" type="number" name=(format!("items[{index}].priority")) value="1" style="width:60px"; }
            td { button type="button" class="btn-remove-row" { "✕" } }
        }
    }
}
