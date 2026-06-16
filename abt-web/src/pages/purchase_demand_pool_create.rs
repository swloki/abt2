use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::demand_handler::{
    CreateOrderFromDemandsReq, DemandPoolQuery, DemandSummary, PurchaseDemandService,
};
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::OrderDetailPath;
use crate::routes::purchase_demand_pool::*;
use crate::routes::purchase_order::PODetailPath;
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

#[derive(Debug, Deserialize)]
pub struct SupplierDetailQueryParams {
    pub supplier_id: i64,
}

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct CreateOrderForm {
    pub supplier_id: i64,
    pub expected_delivery_date: Option<String>,
    pub remark: String,
    pub demand_ids: String, // comma-separated from hidden input
}

// ── Helpers ──

fn priority_label(p: i32) -> (&'static str, &'static str) {
    match p {
        1 => ("紧急", "background:#fee2e2;color:#dc2626"),
        2 => ("高", "background:#fef3c7;color:#d97706"),
        3 => ("中", "background:#f1f5f9;color:#475569"),
        4 => ("低", "background:#f1f5f9;color:#94a3b8"),
        _ => ("—", "background:#f1f5f9;color:#94a3b8"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "create")]
pub async fn get_demand_pool_create(
    _path: PurchaseDemandPoolCreatePath,
    ctx: RequestContext,
    Query(params): Query<DemandPoolCreateParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        claims,
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    // Load suppliers for dropdown
    let supplier_svc = state.supplier_service();
    let suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    // Load demands for the selected product
    let demand_svc = state.purchase_demand_service();
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
        &suppliers.items,
        &demands,
        &preselected_ids,
        params.product_id,
        product_name,
        product_code,
    );

    let page_html = admin_page(
        is_htmx,
        "创建采购订单",
        &claims,
        "purchase",
        PurchaseDemandPoolCreatePath::PATH,
        "采购管理",
        Some("创建采购订单"),
        content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: return supplier detail fragment when supplier is selected
#[require_permission("SUPPLIER", "read")]
pub async fn get_supplier_detail(
    _path: PurchaseDemandSupplierDetailPath,
    ctx: RequestContext,
    Query(params): Query<SupplierDetailQueryParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.supplier_service();

    let supplier = svc
        .get(&service_ctx, &mut conn, params.supplier_id)
        .await?;
    let contacts = svc
        .list_contacts(&service_ctx, &mut conn, params.supplier_id)
        .await
        .unwrap_or_default();

    let primary = contacts.iter().find(|c| c.is_primary);
    let contact_name = primary
        .map(|c| c.name.as_str())
        .unwrap_or("—");
    let contact_phone = primary
        .and_then(|c| c.phone.as_deref())
        .unwrap_or("—");

    let coop_years = {
        let created = supplier.created_at;
        let now = chrono::Utc::now();
        now.signed_duration_since(created).num_days() / 365
    };

    Ok(Html(
        supplier_detail_fragment(contact_name, contact_phone, coop_years).into_string(),
    ))
}

/// POST: create purchase order from selected demands
#[require_permission("PURCHASE_ORDER", "create")]
pub async fn create_order_from_demands(
    _path: PurchaseDemandPoolCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateOrderForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
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
        return Err(DomainError::validation("请至少选择一条采购需求").into());
    }

    let expected_delivery_date = form
        .expected_delivery_date
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| DomainError::validation(format!("无效预期交货日期格式: {e}")))
        })
        .transpose()?;

    let create_req = CreateOrderFromDemandsReq {
        demand_ids,
        supplier_id: form.supplier_id,
        expected_delivery_date,
        remark: form.remark,
    };

    // 整个流程必须在同一事务中：乐观锁(状态1→2) → 创建采购订单 → 更新 target_doc → 发布事件。
    // 任一步骤失败需整体回滚，避免需求成为孤儿状态（status=2 但无 target_doc）。
    let mut tx = state.pool.begin().await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let svc = state.purchase_demand_service();
    let result = svc
        .create_order_from_demands(&service_ctx, &mut tx, create_req)
        .await?;

    tx.commit().await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let redirect = PODetailPath {
        id: result.doc_id,
    }
    .to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Content ──

fn create_page_content(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    demands: &[DemandSummary],
    preselected_ids: &[i64],
    product_id: Option<i64>,
    product_name: &str,
    product_code: &str,
) -> Markup {
    let default_delivery = chrono::Local::now()
        .checked_add_days(chrono::Days::new(15))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    // Compute total quantity for summary bar
    let total_qty: rust_decimal::Decimal = demands
        .iter()
        .filter(|d| preselected_ids.contains(&d.id) || preselected_ids.is_empty())
        .map(|d| d.quantity)
        .sum();

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
                    a class="back-link" href=(format!("{}?restore=true", PurchaseDemandPoolListPath::PATH)) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        "返回需求池"
                    }
                    h1 class="page-title" { "从需求创建采购订单" }
                    div style="font-size:13px;color:var(--muted);margin-top:4px;" {
                        span class="status-pill status-draft" style="font-size:11px;padding:2px 8px;margin-right:6px;" {
                            "采购需求池 · 按物料聚合"
                        }
                        "选择待处理的需求，指定供应商后创建采购订单草稿"
                    }
                }
            }

            form id="demand-create-form"
                 hx-post=(PurchaseDemandPoolCreatePath::PATH)
                 hx-sync="this:drop"
                 hx-swap="none" {
                input type="hidden" id="demand-ids-input" name="demand_ids" value=(preselected_str);

                // ── Section 1: Basic Info ──
                div class="data-card" style="margin-bottom:var(--space-4);" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "物料名称" }
                            input class="form-input" type="text" readonly
                                value=(product_name)
                                style="background:var(--bg-muted);" {}
                        }
                        div class="form-field" {
                            label { "物料编码" }
                            input class="form-input" type="text" readonly
                                value=(product_code)
                                style="background:var(--bg-muted);" {}
                        }
                        div class="form-field" {
                            label { "供应商" span style="color:var(--danger)" { "*" } }
                            select class="form-select" name="supplier_id" required
                                hx-get=(PurchaseDemandSupplierDetailPath::PATH)
                                hx-trigger="change"
                                hx-target="#supplier-detail"
                                hx-swap="innerHTML"
                                hx-include="this" {
                                option value="" disabled selected { "请选择供应商…" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label { "预期交货日期" }
                            input class="form-input" type="date"
                                name="expected_delivery_date"
                                value=(default_delivery) {}
                        }
                        div class="form-field span-2" {
                            label { "备注" }
                            textarea class="form-input" name="remark"
                                placeholder="输入订单相关备注信息…"
                                style="width:100%;min-height:80px;resize:vertical;font-family:inherit;" {}
                        }
                    }

                    // ── Supplier Info Bar ──
                    div id="supplier-detail" style="margin-top:var(--space-3);" {}
                }

                // ── Section 2: Demand Details ──
                div class="data-card" style="margin-bottom:var(--space-4);padding:0;overflow:hidden;" {
                    div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center;" {
                        span class="form-section-title" style="margin:0;padding:0;border:none;" {
                            "需求明细"
                            @if let Some(pid) = product_id {
                                span style="font-weight:400;color:var(--muted);margin-left:var(--space-2);" {
                                    "(物料 ID: " (pid) ")"
                                }
                            }
                        }
                        div style="display:flex;gap:var(--space-2);align-items:center;" {
                            button type="button" class="btn btn-sm btn-default" id="selectAllBtn" {
                                "全选"
                                (PreEscaped(r#"<script>document.currentScript.parentElement.addEventListener('click',function(){
                                    var cbs = Array.from(document.querySelectorAll('#demand-tbody input[type=checkbox]'));
                                    var allChecked = cbs.every(function(c){return c.checked});
                                    cbs.forEach(function(c){c.checked = !allChecked});
                                    updateDemandSummary();
                                })</script>"#))
                            }
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
                                    th { "优先级" }
                                    th { "操作" }
                                }
                            }
                            tbody id="demand-tbody" {
                                @for d in demands {
                                    (demand_row(d, preselected_ids))
                                }
                                @if demands.is_empty() {
                                    tr {
                                        td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted);" {
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
                            span class="mono" style="font-weight:600;" {
                                span id="selectedCount" {
                                    @if preselected_ids.is_empty() && !demands.is_empty() {
                                        (demands.len())
                                    } @else {
                                        (preselected_ids.len())
                                    }
                                }
                                " 条"
                            }
                        }
                        div class="amount-row" {
                            span { "总数量" }
                            span class="mono" style="font-weight:600;" {
                                span id="totalQty" { (fmt_qty(total_qty)) }
                            }
                        }
                    }
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(format!("{}?restore=true", PurchaseDemandPoolListPath::PATH)) { "取消" }
                    div style="display:flex;gap:var(--space-3);" {
                        button type="submit" name="action" value="draft" class="btn btn-default" {
                            (icon::save_icon("w-4 h-4"))
                            "保存草稿"
                        }
                        button type="submit" class="btn btn-primary" {
                            (icon::send_icon("w-4 h-4"))
                            "创建采购订单草稿"
                        }
                    }
                }
            }

            // ── Checkbox & Summary Scripts ──
            (PreEscaped(r#"<script>
                // Check-all checkbox in header
                var checkAllEl = document.querySelector('#checkAll');
                if(checkAllEl){
                    checkAllEl.addEventListener('change', function(){
                        var checked = this.checked;
                        document.querySelectorAll('#demand-tbody input[type=checkbox]').forEach(function(c){
                            c.checked = checked;
                        });
                        updateDemandSummary();
                    });
                }

                // Individual checkbox change
                document.addEventListener('change', function(e){
                    if(e.target.type === 'checkbox' && e.target.closest('#demand-tbody')){
                        updateDemandSummary();
                        // Update check-all state
                        var all = document.querySelectorAll('#demand-tbody input[type=checkbox]');
                        var checked = document.querySelectorAll('#demand-tbody input[type=checkbox]:checked');
                        var checkAll = document.getElementById('checkAll');
                        if(checkAll){
                            checkAll.checked = all.length > 0 && all.length === checked.length;
                        }
                    }
                });

                function updateDemandSummary(){
                    var checked = document.querySelectorAll('#demand-tbody input[type=checkbox]:checked');
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
            </script>"#))
        }
    }
}

// ── Components ──

fn demand_row(d: &DemandSummary, preselected_ids: &[i64]) -> Markup {
    let (pri_text, pri_style) = priority_label(d.priority);
    let req_date = d
        .required_date
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let is_checked = preselected_ids.is_empty() || preselected_ids.contains(&d.id);
    let is_pending = d.demand_status == 1;

    html! {
        tr {
            td {
                @if is_pending {
                    input type="checkbox" value=(d.id)
                        checked[is_checked];
                } @else {
                    input type="checkbox" disabled;
                }
            }
            td class="mono" style="font-size:12px;" { (d.id) }
            td {
                a class="link-cell" href=(OrderDetailPath { id: d.order_id }.to_string()) { (d.order_no.as_ref().map(|s| s.as_str()).unwrap_or("—")) }
            }
            td class="num-right mono demand-qty" { (fmt_qty(d.quantity)) }
            td class="mono" { (req_date) }
            td {
                span class="tag-chip" style=(pri_style) { (pri_text) }
            }
            td {
                button type="button" class="btn-remove-row" title="移除" _="on click remove closest <tr/> then call updateDemandSummary()" {
                    (icon::x_icon("w-3.5 h-3.5"))
                }
            }
        }
    }
}

fn supplier_detail_fragment(contact_name: &str, contact_phone: &str, coop_years: i64) -> Markup {
    html! {
        div class="supplier-info-bar" style="display:flex;gap:var(--space-6);padding:var(--space-3) var(--space-4);background:var(--bg-muted);border-radius:var(--radius-sm);font-size:var(--text-sm);color:var(--text-secondary);" {
            span { "联系人: " strong { (contact_name) } }
            span { "电话: " strong { (contact_phone) } }
            span { "合作年限: " strong { (coop_years) " 年" } }
        }
    }
}
