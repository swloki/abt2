//! MES 生产需求池 → 创建生产计划页面

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::mes::demand_handler::{
    CreatePlanFromDemandsReq, DemandPoolQuery, DemandSummary, MesDemandService, PlanDemandItemReq,
};
use abt_core::mes::enums::PlanStatus;
use abt_core::mes::production_plan::ProductionPlanService;
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_demand_pool::*;
use crate::routes::order::OrderDetailPath;
use crate::routes::mes_plan::PlanDetailPath;
use crate::utils::{fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandPoolCreateParams {
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub demand_ids: Option<String>,
}

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct CreatePlanForm {
    pub plan_type: i16,
    pub plan_date: String,
    pub remark: Option<String>,
    pub default_scheduled_start: Option<String>,
    pub default_scheduled_end: Option<String>,
    pub demand_ids: String,         // comma-separated from hidden input
    pub items_json: Option<String>, // JSON array of per-row scheduling params
    #[serde(default)]
    pub action: Option<String>, // "draft" (default) or "release"
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "create")]
pub async fn get_demand_pool_create(
    _path: MesDemandPoolCreatePath,
    ctx: RequestContext,
    Query(params): Query<DemandPoolCreateParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    // Load demands for the selected product
    let demand_svc = state.mes_demand_service();
    let demands = if let Some(product_id) = params.product_id {
        demand_svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    status: Some(1), // Pending only
                    product_id: Some(product_id),
                    order_id: None,
                    ..Default::default()
                },
                PageParams::new(1, 100),
            )
            .await?
            .items
    } else {
        vec![]
    };

    // Filter demands by pre-selected demand_ids if provided
    let preselected_ids: Vec<i64> = params
        .demand_ids
        .as_deref()
        .map(|s| {
            s.split(',')
                .filter_map(|id| id.trim().parse::<i64>().ok())
                .collect()
        })
        .unwrap_or_default();

    let product_name = params
        .product_name
        .as_deref()
        .or_else(|| demands.first().map(|d| d.product_name.as_str()))
        .unwrap_or("—");
    let product_code = params
        .product_code
        .as_deref()
        .or_else(|| demands.first().map(|d| d.product_code.as_str()))
        .unwrap_or("—");

    let content = create_page_content(
        &demands,
        &preselected_ids,
        params.product_id,
        product_name,
        product_code,
    );

    let page_html = admin_page(
        is_htmx,
        "创建生产计划",
        &claims,
        "production",
        MesDemandPoolCreatePath::PATH,
        "生产管理",
        Some("创建生产计划"),
        content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// POST: create production plan from selected demands
#[require_permission("WORK_ORDER", "create")]
pub async fn create_plan_from_demands(
    _path: MesDemandPoolCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreatePlanForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    // Parse demand_ids from comma-separated string
    let demand_ids: Vec<i64> = form
        .demand_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    if demand_ids.is_empty() {
        return Err(DomainError::validation("请至少选择一条生产需求").into());
    }

    let plan_date = chrono::NaiveDate::parse_from_str(&form.plan_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效计划日期格式: {e}")))?;

    let default_scheduled_start = form
        .default_scheduled_start
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| DomainError::validation(format!("无效默认排程开始日期: {e}")))
        })
        .transpose()?;

    let default_scheduled_end = form
        .default_scheduled_end
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| DomainError::validation(format!("无效默认排程结束日期: {e}")))
        })
        .transpose()?;

    // Parse per-row scheduling items from JSON
    let items: Option<Vec<PlanDemandItemReq>> = form
        .items_json
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|j| serde_json::from_str(j))
        .transpose()
        .map_err(|e| DomainError::validation(format!("无效排程参数JSON: {e}")))?;

    let create_req = CreatePlanFromDemandsReq {
        demand_ids,
        plan_type: form.plan_type,
        plan_date,
        remark: form.remark,
        items,
        default_scheduled_start,
        default_scheduled_end,
    };

    let svc = state.mes_demand_service();
    let result = svc
        .create_plan_from_demands(&service_ctx, &mut conn, create_req)
        .await?;

    // 创建并下达：自动确认 + 下达
    if form.action.as_deref() == Some("release") {
        let plan_svc = state.production_plan_service();
        let plan = plan_svc.find_by_id(&service_ctx, &mut conn, result.doc_id).await?;
        if plan.status == PlanStatus::Draft {
            plan_svc.confirm(&service_ctx, &mut conn, result.doc_id).await?;
        }
        plan_svc.release_to_work_orders(&service_ctx, &mut conn, result.doc_id).await?;
    }

    let redirect = PlanDetailPath { id: result.doc_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Content ──

fn create_page_content(
    demands: &[DemandSummary],
    preselected_ids: &[i64],
    product_id: Option<i64>,
    product_name: &str,
    product_code: &str,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_start = chrono::Local::now()
        .checked_add_days(chrono::Days::new(1))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let default_end = chrono::Local::now()
        .checked_add_days(chrono::Days::new(10))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    // Compute total quantity for summary bar
    let total_qty: rust_decimal::Decimal = demands
        .iter()
        .filter(|d| preselected_ids.is_empty() || preselected_ids.contains(&d.id))
        .map(|d| d.quantity)
        .sum();

    let selected_count = if preselected_ids.is_empty() && !demands.is_empty() {
        demands.len()
    } else {
        preselected_ids.len()
    };

    let preselected_str = if preselected_ids.is_empty() {
        demands.iter().map(|d| d.id.to_string()).collect::<Vec<_>>().join(",")
    } else {
        preselected_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                div {
                    a class="back-link" href=(MesDemandPoolListPath::PATH) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        "返回需求池"
                    }
                    h1 class="page-title" { "从需求创建生产计划" }
                    div style="font-size:13px;color:var(--muted);margin-top:4px;" {
                        span class="status-pill status-draft" style="font-size:11px;padding:2px 8px;margin-right:6px;background:#fef3c7;color:#d97706;" {
                            "生产需求池 · 按物料聚合"
                        }
                        "将生产需求池中的自制需求聚合为生产计划草稿"
                    }
                }
            }

            form id="demand-create-form"
                 hx-post=(MesDemandPoolCreatePath::PATH)
                 hx-swap="none" {
                input type="hidden" id="demand-ids-input" name="demand_ids" value=(preselected_str);
                input type="hidden" id="items-json-input" name="items_json";

                // ── Section 1: Plan Info ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::sliders_icon("w-[18px] h-[18px]"))
                        "计划信息"
                    }
                    div class="form-grid" {
                        div class="form-field" {
                            label class="form-label" { "物料名称 " span style="color:var(--danger)" { "*" } }
                            input class="form-input" type="text" readonly
                                value=(product_name)
                                style="background:var(--surface);" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "物料编码" }
                            input class="form-input mono" type="text" readonly
                                value=(product_code)
                                style="background:var(--surface);" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "计划类型 " span style="color:var(--danger)" { "*" } }
                            select class="form-select" name="plan_type" required {
                                option value="1" selected { "按单生产 (MTO)" }
                                option value="2" { "按库存备货 (MTS)" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "计划日期 " span style="color:var(--danger)" { "*" } }
                            input class="form-input" type="date" name="plan_date"
                                value=(today) required {}
                        }
                    }
                }

                // ── Section 2: Default Scheduling Parameters ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::clock_icon("w-[18px] h-[18px]"))
                        "默认排程参数"
                    }
                    div class="scheduling-hint" {
                        "以下参数将应用于所有未单独配置的需求行。可在需求明细中逐行修改排程日期。"
                    }
                    div class="form-grid" style="grid-template-columns:repeat(4,1fr)" {
                        div class="form-field" {
                            label class="form-label" { "默认排程开始" }
                            input class="form-input" type="date"
                                id="defaultStart"
                                name="default_scheduled_start"
                                value=(default_start) {}
                        }
                        div class="form-field" {
                            label class="form-label" { "默认排程结束" }
                            input class="form-input" type="date"
                                id="defaultEnd"
                                name="default_scheduled_end"
                                value=(default_end) {}
                        }
                        div class="form-field" {
                            label class="form-label" { "工作中心" }
                            select class="form-select" disabled title="待 work_centers 主数据建成" {
                                option value="" selected { "自动推断" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "优先级" }
                            select class="form-select" id="defaultPriority" name="default_priority" {
                                option value="2" selected { "普通 (2)" }
                                option value="1" { "高 (1)" }
                                option value="3" { "低 (3)" }
                            }
                        }
                    }
                    div class="form-grid" style="margin-top:var(--space-4)" {
                        div class="form-field" {
                            label class="form-label" { "备注" }
                            textarea class="form-input" name="remark"
                                placeholder="可选填写生产备注…"
                                rows="1" {}
                        }
                    }
                }

                // ── Section 3: Demand Details ──
                div class="form-section" {
                    div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:var(--space-3);" {
                        div class="form-section-title" style="margin:0;padding:0;border:none;" {
                            (icon::clipboard_list_icon("w-[18px] h-[18px]"))
                            "需求明细"
                            @if let Some(pid) = product_id {
                                span style="font-weight:400;color:var(--muted);margin-left:var(--space-2);" {
                                    "(物料 ID: " (pid) ")"
                                }
                            }
                        }
                        button type="button" class="btn btn-sm btn-default" id="applyDefaultBtn" {
                            "应用默认排程"
                            (PreEscaped(r#"<script>me().on('click',function(){
                                var start = document.getElementById('defaultStart').value;
                                var end = document.getElementById('defaultEnd').value;
                                var rows = document.querySelectorAll('#demand-tbody tr');
                                rows.forEach(function(row){
                                    var inputs = row.querySelectorAll('input[type=date]');
                                    if(inputs[0] && start) inputs[0].value = start;
                                    if(inputs[1] && end) inputs[1].value = end;
                                });
                            })</script>"#))
                        }
                    }

                    div class="data-card-scroll" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th style="width:40px;" { input type="checkbox" id="checkAll" title="全选"; }
                                    th { "需求ID" }
                                    th { "来源订单" }
                                    th class="num-right" { "需求数量" }
                                    th { "需求日期" }
                                    th { "排程开始" }
                                    th { "排程结束" }
                                    th { "操作" }
                                }
                            }
                            tbody id="demand-tbody" {
                                @for d in demands {
                                    (demand_row(d, preselected_ids))
                                }
                                @if demands.is_empty() {
                                    tr {
                                        td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted);" {
                                            "暂无待处理需求"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── Summary Bar ──
                    div class="amount-summary" {
                        div class="amount-row" {
                            span { "已选需求" }
                            span class="mono font-semibold" {
                                span id="selectedCount" { (selected_count) }
                                " 条"
                            }
                        }
                        div class="amount-row" {
                            span { "总数量" }
                            span class="mono font-semibold" {
                                span id="totalQty" { (fmt_qty(total_qty)) }
                            }
                        }
                        div class="amount-row" {
                            span { "聚合方式" }
                            span class="mono font-semibold" { "按物料" }
                        }
                    }
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(MesDemandPoolListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3);" {
                        button type="submit" name="action" value="draft" class="btn btn-default" {
                            (icon::save_icon("w-4 h-4"))
                            "保存草稿"
                        }
                        button type="submit" name="action" value="draft" class="btn btn-primary" {
                            (icon::send_icon("w-4 h-4"))
                            "创建草稿"
                        }
                        button type="submit" name="action" value="release" class="btn btn-primary"
                            style="background:linear-gradient(135deg,var(--accent),#6366f1)"
                            hx-confirm="创建后将自动确认并下达，生成工单（含工序、批次）。继续？"
                            hx-disabled-elt="this" {
                            (icon::rocket_icon("w-4 h-4"))
                            "创建并下达"
                        }
                    }
                }
            }

            // ── Checkbox, Summary & Form Collection Scripts ──
            (PreEscaped(r#"<script>
                // Check-all checkbox in header
                me('#checkAll').on('change', function(){
                    var checked = this.checked;
                    any('#demand-tbody input[type=checkbox]').forEach(function(c){
                        c.checked = checked;
                        c.closest('tr').classList.toggle('demand-row-selected', checked);
                    });
                    updateDemandSummary();
                });

                // Individual checkbox change
                document.addEventListener('change', function(e){
                    if(e.target.type === 'checkbox' && e.target.closest('#demand-tbody')){
                        e.target.closest('tr').classList.toggle('demand-row-selected', e.target.checked);
                        updateDemandSummary();
                        // Update check-all state
                        var all = any('#demand-tbody input[type=checkbox]');
                        var checked = any('#demand-tbody input[type=checkbox]:checked');
                        var checkAll = document.getElementById('checkAll');
                        if(checkAll){
                            checkAll.checked = all.length > 0 && all.length === checked.length;
                        }
                    }
                });

                function updateDemandSummary(){
                    var checked = any('#demand-tbody input[type=checkbox]:checked');
                    var ids = [];
                    var totalQty = 0;
                    checked.forEach(function(c){
                        ids.push(c.value);
                        var qtyEl = c.closest('tr').querySelector('.demand-qty');
                        if(qtyEl) totalQty += parseFloat(qtyEl.textContent.replace(/,/g,'')) || 0;
                    });
                    document.getElementById('selectedCount').textContent = checked.length;
                    document.getElementById('totalQty').textContent = totalQty % 1 === 0 ? totalQty : totalQty.toFixed(2);
                    document.getElementById('demand-ids-input').value = ids.join(',');
                }

                // Collect per-row scheduling items on form submit
                document.getElementById('demand-create-form').addEventListener('submit', function(){
                    var rows = document.querySelectorAll('#demand-tbody tr');
                    var items = [];
                    rows.forEach(function(row){
                        var cb = row.querySelector('input[type=checkbox]');
                        if(!cb || !cb.checked) return;
                        var inputs = row.querySelectorAll('input[type=date]');
                        var startVal = inputs[0] ? inputs[0].value : '';
                        var endVal = inputs[1] ? inputs[1].value : '';
                        var priEl = row.querySelector('.priority-val');
                        if(startVal && endVal){
                            items.push({
                                demand_id: parseInt(cb.value),
                                scheduled_start: startVal,
                                scheduled_end: endVal,
                                priority: priEl ? parseInt(priEl.textContent) : (parseInt((document.getElementById('defaultPriority')||{}).value) || 2)
                            });
                        }
                    });
                    document.getElementById('items-json-input').value = items.length > 0 ? JSON.stringify(items) : '';
                });
            </script>"#))
        }
    }
}

// ── Components ──

fn demand_row(d: &DemandSummary, preselected_ids: &[i64]) -> Markup {
    let req_date = d
        .required_date
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let is_checked = preselected_ids.is_empty() || preselected_ids.contains(&d.id);
    let is_pending = d.demand_status == 1;

    // Per-row default schedule dates
    let row_start = d
        .required_date
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let row_end = d
        .required_date
        .and_then(|dt| dt.checked_add_days(chrono::Days::new(7)))
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    html! {
        tr class=@if is_checked { "demand-row-selected" } {
            td {
                @if is_pending {
                    input type="checkbox" value=(d.id)
                        checked[is_checked];
                    span class="priority-val" style="display:none;" { (d.priority) }
                } @else {
                    input type="checkbox" disabled;
                }
            }
            td class="mono" style="font-size:12px;" { (d.id) }
            td {
                a class="link-cell" href=(OrderDetailPath { id: d.order_id }.to_string()) { (d.order_no) }
            }
            td class="num-right mono demand-qty" { (fmt_qty(d.quantity)) }
            td class="mono" { (req_date) }
            td {
                input class="form-input" type="date"
                    value=(row_start)
                    style="width:130px;font-size:12px;padding:4px 6px;" {}
            }
            td {
                input class="form-input" type="date"
                    value=(row_end)
                    style="width:130px;font-size:12px;padding:4px 6px;" {}
            }
            td {
                button type="button" class="btn-remove-row" title="移除" {
                    (icon::x_icon("w-3.5 h-3.5"))
                    (PreEscaped(r#"<script>me().on('click',function(){me().closest('tr').remove();updateDemandSummary()})</script>"#))
                }
            }
        }
    }
}
