use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_plan::{ProductionPlanService, model::PlanFilter};
use abt_core::mes::work_order::WorkOrderService;
use abt_core::master_data::work_center::WorkCenterService;
use abt_core::sales::sales_order::{SalesOrderService, model::SalesOrderQuery};
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::product_picker;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderCreatePath, OrderListPath, SourceOrderSearchPath, SourcePlanSearchPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct OrderCreateForm {
    pub product_id: String,
    pub planned_qty: String,
    pub scheduled_start: String,
    pub scheduled_end: String,
    pub work_center_id: Option<String>,
    pub remark: Option<String>,
    /// 来源单据类型: None / "sales_order" / "production_plan"
    pub source_type: Option<String>,
    pub source_sales_order_id: Option<String>,
    pub source_plan_id: Option<String>,
}

// ── Search Params ──

#[derive(Debug, Deserialize)]
pub struct SourceSearchParams {
    pub keyword: Option<String>,
}

// ── Handlers ──

pub async fn get_order_create(
    _path: OrderCreatePath, ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let work_centers = state
        .work_center_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    let content = order_create_page(&work_centers);
    Ok(Html(admin_page(is_htmx, "新建工单", &claims, "production", OrderCreatePath::PATH, "生产管理", Some(OrderListPath::PATH), content, &nav_filter).into_string()))
}

#[require_permission("WORK_ORDER", "create")]
pub async fn create_order(
    _path: OrderCreatePath, ctx: RequestContext,
    axum::Form(form): axum::Form<OrderCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let product_id: i64 = form.product_id.parse()
        .map_err(|_| DomainError::Validation("无效产品ID".into()))?;
    let planned_qty = form.planned_qty.parse()
        .map_err(|_| DomainError::Validation("无效数量".into()))?;
    let scheduled_start = form.scheduled_start.parse()
        .map_err(|_| DomainError::Validation("无效开始日期".into()))?;
    let scheduled_end = form.scheduled_end.parse()
        .map_err(|_| DomainError::Validation("无效结束日期".into()))?;

    // 解析来源单据关联
    let (sales_order_id, plan_item_id) = resolve_source(
        &state, &service_ctx, &mut conn,
        form.source_type.as_deref(),
        form.source_sales_order_id.as_deref(),
        form.source_plan_id.as_deref(),
        product_id,
    ).await?;

    let svc = state.work_order_service();
    let req = abt_core::mes::work_order::CreateWorkOrderReq {
        plan_item_id,
        product_id,
        bom_snapshot_id: None,
        routing_id: None,
        planned_qty,
        scheduled_start,
        scheduled_end,
        work_center_id: form.work_center_id.and_then(|s| s.parse().ok()),
        sales_order_id,
        remark: form.remark,
    };
    let _id = svc.create(&service_ctx, &mut conn, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", OrderListPath::PATH).body(axum::body::Body::empty()).unwrap())
}
async fn resolve_source(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    source_type: Option<&str>,
    so_id_str: Option<&str>,
    plan_id_str: Option<&str>,
    product_id: i64,
) -> Result<(Option<i64>, Option<i64>)> {
    match source_type {
        Some("sales_order") => {
            let so_id = so_id_str
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<i64>().ok());
            Ok((so_id, None))
        }
        Some("production_plan") => {
            let plan_id = plan_id_str
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<i64>().ok());
            if let Some(pid) = plan_id {
                // 在生产计划项中查找匹配产品的 item
                let plan_svc = state.production_plan_service();
                let items = plan_svc.list_items(ctx, db, pid).await?;
                let matching = items.iter().find(|i| i.product_id == product_id);
                match matching {
                    Some(item) => Ok((None, Some(item.id))),
                    None => Err(DomainError::validation(
                        "所选生产计划中无匹配当前产品的计划项"
                    ).into()),
                }
            } else {
                Ok((None, None))
            }
        }
        _ => Ok((None, None)),
    }
}

// ── Source Search APIs ──

/// 搜索已确认的销售订单（来源单据选择器）
pub async fn search_source_orders(
    ctx: RequestContext,
    Query(params): Query<SourceSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();
    let result = svc.list(
        &service_ctx, &mut conn,
        SalesOrderQuery {
            keyword: params.keyword.filter(|s| !s.is_empty()),
            ..Default::default()
        },
        PageParams::new(1, 20),
    ).await?;
    Ok(Html(source_order_results(&result.items).into_string()))
}

/// 搜索生产计划（来源单据选择器）
pub async fn search_source_plans(
    ctx: RequestContext,
    Query(params): Query<SourceSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_plan_service();
    let keyword = params.keyword.filter(|s| !s.is_empty());
    let result = svc.list(
        &service_ctx, &mut conn,
        PlanFilter {
            status: None,
            plan_type: None,
            keyword,
            date_from: None,
            date_to: None,
        },
        1, 20,
    ).await?;
    Ok(Html(source_plan_results(&result.items).into_string()))
}

// ── Page ──

fn order_create_page(work_centers: &[abt_core::master_data::work_center::WorkCenter]) -> Markup {
    html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(format!("{}?restore=true", OrderListPath::PATH)) { "\u{2190} 返回列表" } h1 class="page-title" { "新建工单" } }
        }
        form hx-post=(OrderCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label class="form-label" { "产品" }
                        div style="display:flex;gap:var(--space-2)" {
                            input type="hidden" name="product_id" id="product_id" required;
                            div class="form-input" id="product-display" style="flex:1;cursor:pointer;color:var(--muted)"
                                _="on click add .is-open to #product-modal" {
                                "点击选择产品…"
                            }
                            button type="button" class="btn btn-default"
                                _="on click add .is-open to #product-modal" { "选择" }
                        }
                    }
                    div class="form-field" { label class="form-label" { "计划数量" } input class="form-input" type="number" step="0.01" name="planned_qty" required; }
                    div class="form-field" { label class="form-label" { "开始日期" } input class="form-input" type="date" name="scheduled_start" required; }
                    div class="form-field" { label class="form-label" { "结束日期" } input class="form-input" type="date" name="scheduled_end" required; }
                    div class="form-field" {
                        label class="form-label" { "工作中心" }
                        select class="form-select" name="work_center_id" {
                            option value="" { "— 不指定 —" }
                            @for wc in work_centers {
                                option value=(wc.id) { (format!("{} - {}", wc.code, wc.name)) }
                            }
                        }
                    }
                    // ── 来源单据关联 ──
                    div class="form-field span-2" {
                        label class="form-label" { "来源单据（可选）" }
                        select class="form-select" name="source_type"
                            _="on change hide #source-order-field then hide #source-plan-field then if my value is 'sales_order' show #source-order-field else if my value is 'production_plan' show #source-plan-field" {
                            option value="" { "无" }
                            option value="sales_order" { "销售订单" }
                            option value="production_plan" { "生产计划" }
                        }
                    }
                    div class="form-field span-2" id="source-order-field" style="display:none" {
                        label class="form-label" { "关联销售订单" }
                        div style="display:flex;gap:var(--space-2)" {
                            input type="hidden" name="source_sales_order_id" id="source_sales_order_id";
                            div class="form-input" id="so-display" style="flex:1;cursor:pointer;color:var(--muted)"
                                _="on click add .is-open to #so-modal" {
                                "点击选择销售订单…"
                            }
                            button type="button" class="btn btn-default"
                                _="on click add .is-open to #so-modal" { "选择" }
                            button type="button" class="btn btn-default"
                                _="on click set #source_sales_order_id's value to '' then put '点击选择销售订单…' into #so-display then set #so-display's style.color to 'var(--muted)'" { "清除" }
                        }
                    }
                    div class="form-field span-2" id="source-plan-field" style="display:none" {
                        label class="form-label" { "关联生产计划（自动匹配同产品的计划项）" }
                        div style="display:flex;gap:var(--space-2)" {
                            input type="hidden" name="source_plan_id" id="source_plan_id";
                            div class="form-input" id="pp-display" style="flex:1;cursor:pointer;color:var(--muted)"
                                _="on click add .is-open to #pp-modal" {
                                "点击选择生产计划…"
                            }
                            button type="button" class="btn btn-default"
                                _="on click add .is-open to #pp-modal" { "选择" }
                            button type="button" class="btn btn-default"
                                _="on click set #source_plan_id's value to '' then put '点击选择生产计划…' into #pp-display then set #pp-display's style.color to 'var(--muted)'" { "清除" }
                        }
                    }
                    div class="form-field span-2" { label class="form-label" { "备注" } textarea class="form-input" name="remark" rows="2" {}; }
                }
            }
            div class="create-action-bar" {
                a class="btn btn-default" href=(format!("{}?restore=true", OrderListPath::PATH)) { "取消" }
                button type="submit" class="btn btn-primary" { "提交" }
            }
        }
        // ── 弹窗组件 ──
        (product_picker::product_picker_modal("product-modal", "product_id", "product-display"))
        (source_order_modal())
        (source_plan_modal())
    }}
}

// ── Source Picker Modals ──

fn source_order_modal() -> Markup {
    html! {
        div class="modal-overlay" id="so-modal"
            _="on click remove .is-open from #so-modal" {
            div class="modal modal-lg" _="on click halt" {
                div class="modal-head" {
                    h2 { "选择销售订单" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #so-modal" { "\u{00d7}" }
                }
                div class="modal-body" style="padding:0" {
                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { "订单编号 / 关键词" }
                            input class="product-search-input" type="text" name="keyword" placeholder="输入订单编号搜索…"
                                hx-get=(SourceOrderSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#so-search-results"
                                hx-swap="innerHTML" {}
                        }
                    }
                    div id="so-search-results" style="max-height:320px;overflow-y:auto"
                        hx-get=(SourceOrderSearchPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML" {
                        div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                            "加载中…"
                        }
                    }
                }
            }
        }
    }
}

fn source_plan_modal() -> Markup {
    html! {
        div class="modal-overlay" id="pp-modal"
            _="on click remove .is-open from #pp-modal" {
            div class="modal modal-lg" _="on click halt" {
                div class="modal-head" {
                    h2 { "选择生产计划" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #pp-modal" { "\u{00d7}" }
                }
                div class="modal-body" style="padding:0" {
                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { "计划编号 / 关键词" }
                            input class="product-search-input" type="text" name="keyword" placeholder="输入计划编号搜索…"
                                hx-get=(SourcePlanSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#pp-search-results"
                                hx-swap="innerHTML" {}
                        }
                    }
                    div id="pp-search-results" style="max-height:320px;overflow-y:auto"
                        hx-get=(SourcePlanSearchPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML" {
                        div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                            "加载中…"
                        }
                    }
                }
            }
        }
    }
}

// ── Search Result Fragments ──

fn source_order_results(orders: &[abt_core::sales::sales_order::model::SalesOrder]) -> Markup {
    let click_hs = "on click set #source_sales_order_id's value to my @data-oid then put my @data-label into #so-display then set #so-display's style.color to 'inherit' then remove .is-open from #so-modal";
    html! {
        @if orders.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                p style="margin:0;font-size:var(--text-sm)" { "未找到匹配的销售订单" }
            }
        } @else {
            div class="product-select-list" {
                @for o in orders {
                    div class="product-select-item"
                        data-oid=(o.id)
                        data-label=(format!("{} ({})", o.doc_number, o.order_date.format("%Y-%m-%d")))
                        _=(click_hs) {
                        div class="product-select-info" {
                            div class="product-select-name" { (o.doc_number) }
                            div class="product-select-meta" {
                                span { (o.order_date.format("%Y-%m-%d")) }
                                span class="product-select-sep" { "\u{00b7}" }
                                span { (format!("{:?}", o.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn source_plan_results(plans: &[abt_core::mes::production_plan::model::ProductionPlan]) -> Markup {
    let click_hs = "on click set #source_plan_id's value to my @data-pid then put my @data-label into #pp-display then set #pp-display's style.color to 'inherit' then remove .is-open from #pp-modal";
    html! {
        @if plans.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                p style="margin:0;font-size:var(--text-sm)" { "未找到匹配的生产计划" }
            }
        } @else {
            div class="product-select-list" {
                @for p in plans {
                    div class="product-select-item"
                        data-pid=(p.id)
                        data-label=(format!("{} ({})", p.doc_number, p.plan_date.format("%Y-%m-%d")))
                        _=(click_hs) {
                        div class="product-select-info" {
                            div class="product-select-name" { (p.doc_number) }
                            div class="product-select-meta" {
                                span { (p.plan_date.format("%Y-%m-%d")) }
                                span class="product-select-sep" { "\u{00b7}" }
                                span { (format!("{:?}", p.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}
