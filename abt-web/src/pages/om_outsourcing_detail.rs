use axum::response::{Html, IntoResponse};
use std::collections::HashMap;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::om::enums::{OutsourcingStatus, OutsourcingType, TrackingNodeType};
use abt_core::om::outsourcing_order::OutsourcingOrderService;
use abt_core::om::outsourcing_tracking::OutsourcingTrackingService;
use abt_core::shared::identity::UserService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::types::pagination::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{
    OmOutsourcingDetailPath, OmOutsourcingListPath, OmOutsourcingSendPath,
    OmOutsourcingReceivePath, OmOutsourcingConvertPath, OmOutsourcingCancelPath,
    OmRecordNodePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: &OutsourcingStatus) -> (&'static str, &'static str) {
    match s {
        OutsourcingStatus::Draft => ("草稿", "status-draft"),
        OutsourcingStatus::Sent => ("已发出", "status-sent"),
        OutsourcingStatus::InProduction => ("生产中", "status-progress"),
        OutsourcingStatus::Delivered => ("已发货", "status-shipped"),
        OutsourcingStatus::Received => ("已收货", "status-received"),
        OutsourcingStatus::Closed => ("已关闭", "status-completed"),
        OutsourcingStatus::ConvertedToInternal => ("转自制", "status-confirmed"),
        OutsourcingStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

fn type_label(t: &OutsourcingType) -> &'static str {
    match t {
        OutsourcingType::Full => "整体委外",
        OutsourcingType::Process => "工序委外",
        OutsourcingType::Material => "物料委外",
        OutsourcingType::Rework => "返工委外",
    }
}

fn node_type_label(t: &TrackingNodeType) -> &'static str {
    match t {
        TrackingNodeType::SendMaterial => "发料",
        TrackingNodeType::CarrierPickup => "承运商取件",
        TrackingNodeType::SupplierReceived => "供应商收料",
        TrackingNodeType::InProduction => "生产中",
        TrackingNodeType::Shipped => "已发货",
        TrackingNodeType::IqcInspected => "IQC检验",
        TrackingNodeType::Warehoused => "已入库",
    }
}
fn format_amount(d: rust_decimal::Decimal) -> String {
    let f: f64 = d.try_into().unwrap_or(0.0);
    if f == 0.0 { return "0".to_string(); }
    let abs = f.abs();
    if abs >= 1_000_000.0 {
        format!("{:.1}M", f / 1_000_000.0)
    } else {
        let formatted = format!("{:.2}", f);
        let parts: Vec<&str> = formatted.split('.').collect();
        let int_str = parts[0];
        let mut result = String::new();
        for (i, c) in int_str.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { result.insert(0, ','); }
            result.insert(0, c);
        }
        let dec = parts[1].trim_end_matches('0');
        if dec.is_empty() { result } else { format!("{result}.{dec}") }
    }
}

fn status_pill(label: &str, class: &str) -> Markup {
    html! { span class=(format!("status-pill {class}")) { (label) } }
}

// ── Handlers ──

#[require_permission("OM", "read")]
pub async fn get_detail(
    path: OmOutsourcingDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.outsourcing_order_service();
    let tracking_svc = state.outsourcing_tracking_service();
    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, order.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let product_name = product_svc
        .get(&service_ctx, &mut conn, order.product_id)
        .await
        .map(|p| p.pdt_name)
        .unwrap_or_else(|_| "—".into());

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, order.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let warehouse_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, order.virtual_warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    // Tracking nodes
    let tracking = tracking_svc
        .list_by_outsourcing(&service_ctx, &mut conn, path.id, PageParams::new(1, 100))
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    // Note: materials not loaded — OutsourcingOrderService trait doesn't expose materials listing

    let content = detail_page(
        &order, &supplier_name, &product_name, &operator_name, &warehouse_name, &tracking,
    );

    let page_html = admin_page(
        is_htmx, "委外单详情", &claims, "outsourcing",
        &OmOutsourcingDetailPath { id: path.id }.to_string(),
        "委外管理", Some(OmOutsourcingListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("OM", "update")]
pub async fn send_order(
    path: OmOutsourcingSendPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.outsourcing_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    svc.send(&service_ctx, &mut conn, abt_core::om::outsourcing_order::SendOutsourcingReq {
        id: path.id,
        expected_version: order.version,
        remark: form.remark,
    }).await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
        .body(axum::body::Body::empty())
        .unwrap())
}

#[require_permission("OM", "update")]
pub async fn receive_order(
    path: OmOutsourcingReceivePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReceiveForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.outsourcing_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let received_qty: Decimal = form.received_qty.parse()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效收货数量"))?;
    svc.receive(&service_ctx, &mut conn, abt_core::om::outsourcing_order::ReceiveOutsourcingReq {
        id: path.id,
        expected_version: order.version,
        received_qty,
        warehouse_id: form.warehouse_id,
        iqc_passed_qty: None,
        remark: form.remark,
    }).await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
        .body(axum::body::Body::empty())
        .unwrap())
}

#[require_permission("OM", "update")]
pub async fn convert_to_internal(
    path: OmOutsourcingConvertPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.outsourcing_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    svc.convert_to_internal(&service_ctx, &mut conn, abt_core::om::outsourcing_order::ConvertToInternalReq {
        id: path.id,
        expected_version: order.version,
        remark: form.remark,
    }).await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
        .body(axum::body::Body::empty())
        .unwrap())
}

#[require_permission("OM", "update")]
pub async fn cancel_order(
    path: OmOutsourcingCancelPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ActionForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.outsourcing_order_service();
    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    svc.cancel(&service_ctx, &mut conn, abt_core::om::outsourcing_order::CancelOutsourcingReq {
        id: path.id,
        expected_version: order.version,
        remark: form.remark,
    }).await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
        .body(axum::body::Body::empty())
        .unwrap())
}

#[require_permission("OM", "update")]
pub async fn record_node(
    path: OmRecordNodePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RecordNodeForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let tracking_svc = state.outsourcing_tracking_service();
    let node_type = TrackingNodeType::from_i16(form.node_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效节点类型"))?;
    tracking_svc.record_node(&service_ctx, &mut conn, abt_core::om::outsourcing_tracking::RecordNodeReq {
        outsourcing_id: path.id,
        node_type,
        tracked_at: None,
        remark: form.remark,
    }).await?;
    Ok(axum::response::Response::builder()
        .header("HX-Redirect", &OmOutsourcingDetailPath { id: path.id }.to_string())
        .body(axum::body::Body::empty())
        .unwrap())
}

// ── Forms ──

#[derive(Debug, Deserialize)]
pub struct ActionForm {
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveForm {
    pub received_qty: String,
    pub warehouse_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordNodeForm {
    pub node_type: i16,
    pub remark: Option<String>,
}

// ── Components ──

fn detail_page(
    order: &abt_core::om::outsourcing_order::OutsourcingOrder,
    supplier_name: &str,
    product_name: &str,
    operator_name: &str,
    warehouse_name: &str,
    tracking: &[abt_core::om::outsourcing_tracking::OutsourcingTracking],
) -> Markup {
    let (sl, sc) = status_label(&order.status);
    let tl = type_label(&order.outsourcing_type);
    let type_tag_cls = match order.outsourcing_type {
        OutsourcingType::Full => "type-tag full",
        OutsourcingType::Process => "type-tag process",
        OutsourcingType::Material => "type-tag material",
        OutsourcingType::Rework => "type-tag rework",
    };

    // Progress ring calculation
    let pct: f64 = if order.planned_qty > Decimal::ZERO {
        let ratio = order.completed_qty / order.planned_qty;
        (ratio * Decimal::ONE_HUNDRED).to_string().parse::<f64>().unwrap_or(0.0).min(100.0)
    } else {
        0.0
    };
    let r: f64 = 22.0;
    let circumference = 2.0 * std::f64::consts::PI * r;
    let offset = circumference * (1.0 - pct / 100.0);

    // Build tracking set: which node types have been recorded
    let tracked_nodes: HashMap<TrackingNodeType, &abt_core::om::outsourcing_tracking::OutsourcingTracking> =
        tracking.iter().map(|t| (t.node_type, t)).collect();
    let all_node_types = [
        TrackingNodeType::SendMaterial,
        TrackingNodeType::CarrierPickup,
        TrackingNodeType::SupplierReceived,
        TrackingNodeType::InProduction,
        TrackingNodeType::Shipped,
        TrackingNodeType::IqcInspected,
        TrackingNodeType::Warehoused,
    ];
    let _completed_count = all_node_types.iter().filter(|nt| tracked_nodes.contains_key(nt)).count();
    let active_index = all_node_types.iter().position(|nt| !tracked_nodes.contains_key(nt)).unwrap_or(all_node_types.len());

    html! { div {
        // ── Back link ──
        a class="back-link" href=(format!("{}?restore=true", OmOutsourcingListPath::PATH)) {
            (icon::chevron_left_icon("w-4 h-4"))
            "返回委外单列表"
        }

        // ═══ Detail Hero Card ═══
        div class="detail-hero" {
            div class="detail-hero-accent" {}
            div class="detail-hero-body" {

                // Title + Actions
                div class="detail-title-row" {
                    div {
                        div class="detail-doc-no" {
                            div class="doc-icon" {
                                (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
                            }
                            (order.doc_number)
                        }
                        div class="detail-meta" {
                            (status_pill(sl, sc))
                            span class=(type_tag_cls) { (tl) }
                            span style="font-size:12px;color:var(--muted)" { "v" (order.version) }
                        }
                    }
                    div class="detail-actions" {
                        button class="btn btn-default" _="on click add .is-open to #record-node-modal" {
                            (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:15px;height:15px"><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></svg>"#))
                            "记录节点"
                        }
                        button class="btn btn-default" _="on click add .is-open to #receive-modal" {
                            (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:15px;height:15px"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
                            "收货登记"
                        }
                        button class="btn btn-default" _="on click add .is-open to #convert-modal" {
                            (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:15px;height:15px"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
                            "转自制"
                        }
                        button class="btn btn-default" _="on click add .is-open to #cancel-modal" style="color:var(--danger);border-color:rgba(220,38,38,0.3)" {
                            (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:15px;height:15px"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
                            "取消"
                        }
                    }
                }

                // Info Split: Key fields + Progress ring
                div class="detail-info-split" {
                    div {
                        div class="info-key-grid" {
                            div class="info-key-item" {
                                span class="info-key-label" { "供应商" }
                                span class="info-key-value" { (supplier_name) }
                            }
                            div class="info-key-item" {
                                span class="info-key-label" { "产品" }
                                span class="info-key-value" { (product_name) }
                            }
                            div class="info-key-item" {
                                span class="info-key-label" { "关联工单" }
                                span class="info-key-value" {
                                    (order.work_order_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
                                }
                            }
                            div class="info-key-item" {
                                span class="info-key-label" { "关联工序" }
                                span class="info-key-value" {
                                    (order.routing_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
                                }
                            }
                            div class="info-key-item" {
                                span class="info-key-label" { "虚拟仓库" }
                                span class="info-key-value" { (warehouse_name) }
                            }
                            div class="info-key-item" {
                                span class="info-key-label" { "预计交期" }
                                span class="info-key-value mono" {
                                    (order.scheduled_date.map(|d| d.to_string()).unwrap_or_else(|| "—".into()))
                                }
                            }
                        }
                        // Detail row — secondary meta
                        div class="info-detail-row" {
                            span class="info-detail-chip" { "计划数量 " strong class="mono" { (crate::utils::fmt_qty(order.planned_qty)) } }
                            span class="info-detail-chip" { "完成数量 " strong class="mono" style="color:var(--success)" { (crate::utils::fmt_qty(order.completed_qty)) } }
                            span class="info-detail-chip" { "单价 " strong class="mono" { (crate::utils::fmt_qty(order.unit_price)) } }
                            span class="info-detail-chip" { "总金额 " strong class="mono" style="color:var(--accent)" { (format_amount(order.planned_qty * order.unit_price)) } }
                            span class="info-detail-chip" { "创建人 " strong { (operator_name) } }
                            span class="info-detail-chip" { "创建 " strong class="mono" { (order.created_at.format("%Y-%m-%d %H:%M")) } }
                            span class="info-detail-chip" { "更新 " strong class="mono" { (order.updated_at.format("%Y-%m-%d %H:%M")) } }
                        }
                    }
                    // Progress Ring
                    div class="info-progress" {
                        div class="progress-ring-wrap" {
                            div class="progress-ring" {
                                svg viewBox="0 0 56 56" {
                                    circle class="progress-ring-bg" cx="28" cy="28" r="22";
                                    circle class="progress-ring-fill" cx="28" cy="28" r="22"
                                        stroke-dasharray=(format!("{circumference:.1}"))
                                        stroke-dashoffset=(format!("{offset:.1}"));
                                }
                                span class="progress-ring-text" { (format!("{:.0}%", pct)) }
                            }
                            span style="font-size:12px;color:var(--muted);font-weight:500" { "完成进度" }
                        }
                    }
                }

                // Remark inside hero
                @if !order.remark.is_empty() {
                    div style="margin-top:20px;padding-top:16px;border-top:1px dashed var(--border-soft)" {
                        span style="font-size:12px;color:var(--muted);font-weight:600" { "备注" }
                        p style="color:var(--fg-2);font-size:13px;margin-top:6px;line-height:1.6" { (&order.remark) }
                    }
                }
            }
        }

        // ═══ Tracking Timeline ═══
        div class="tracking-section" {
            div class="tracking-head" {
                div class="tracking-title" {
                    div class="tracking-icon-wrap" {
                        (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>"#))
                    }
                    "追踪节点"
                }
                div class="tracking-hint" {
                    span class="hint-dot" {}
                    (format!("实时追踪 · 7 个节点 · 当前第 {} 步", active_index + 1))
                }
            }
            div class="tracking-timeline" {
                @for (i, nt) in all_node_types.iter().enumerate() {
                    @let tracked = tracked_nodes.get(nt);
                    @let is_completed = tracked.is_some();
                    @let is_active = !is_completed && i > 0 && tracked_nodes.contains_key(&all_node_types[i - 1]);
                    @let dot_cls = if is_completed { "track-dot completed" } else if is_active { "track-dot active" } else { "track-dot pending" };
                    @let label = node_type_label(nt);

                    div class="track-node" {
                        div class=(dot_cls) {}
                        div class=(if is_active { "track-content active-content" } else { "track-content" }) {
                            div class="track-info" {
                                div class=(if is_active || is_completed { "track-label" } else { "track-label muted" }) {
                                    (label)
                                    @if is_active {
                                        span style="font-size:11px;font-weight:500;padding:2px 10px;border-radius:var(--radius-pill);background:rgba(37,99,235,0.1);color:var(--accent)" { "当前" }
                                    }
                                }
                                @if let Some(t) = tracked {
                                    @if let Some(at) = t.tracked_at {
                                        div class="track-time" { (at.format("%Y-%m-%d %H:%M")) }
                                    }
                                    @if let Some(remark) = &t.remark {
                                        div class="track-remark" { (remark) }
                                    }
                                } @else {
                                    @if let Some(t) = tracked_nodes.get(&all_node_types[if i > 0 { i - 1 } else { 0 }]) {
                                        @if let Some(planned) = &t.planned_at {
                                            div class="track-time" { "计划 " (planned.format("%m-%d")) }
                                        }
                                    }
                                }
                            }
                            div class="track-status" {
                                @if is_completed {
                                    (status_pill("已完成", "status-completed"))
                                } @else if is_active {
                                    (status_pill("进行中", "status-progress"))
                                } @else {
                                    span style="font-size:11px;color:var(--muted)" { "待完成" }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ═══ Transaction Records ═══
        @if !tracking.is_empty() {
            div class="sub-section" {
                div class="sub-section-title" {
                    div class="section-icon-wrap" {
                        (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 7h12M8 12h12M8 17h12M4 7h.01M4 12h.01M4 17h.01"/></svg>"#))
                    }
                    "收发记录"
                    span class="section-count" { (tracking.len()) " 条记录" }
                }
                div class="sub-section-body" {
                    div class="data-card-scroll" {
                        table class="data-table" style="width:100%" {
                            thead {
                                tr {
                                    th { "时间" }
                                    th { "类型" }
                                    th { "描述" }
                                    th { "状态" }
                                }
                            }
                            tbody {
                                @for t in tracking {
                                    tr {
                                        td class="time-cell" {
                                            (t.tracked_at.map(|at| at.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into()))
                                        }
                                        td { (node_type_label(&t.node_type)) }
                                        td { (t.remark.as_deref().unwrap_or("—")) }
                                        td {
                                            @if t.tracked_at.is_some() {
                                                (status_pill("已完成", "status-completed"))
                                            } @else {
                                                (status_pill("计划中", "status-draft"))
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

        // ═══ Modals ═══
        // All modals use hyperscript (`_=`) for open/close, matching prototype structure.
        // Backdrop click: `_="on click[me is event.target] remove .is-open"` on modal-overlay div.
        // Close buttons: `_="on click remove .is-open from #X-modal"` on the button.

        // ── Record Node Modal ──
        div id="record-node-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" {
            div class="modal" style="width:520px" {
                div class="modal-head" {
                    h2 style="display:flex;align-items:center;gap:var(--space-2)" {
                        (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></svg>"#))
                        "记录追踪节点"
                    }
                    button class="btn btn-text btn-sm" type="button" _="on click remove .is-open from #record-node-modal" {
                        "✕"
                    }
                }
                form hx-post=(OmRecordNodePath { id: order.id }.to_string()) hx-swap="none"
                    hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#record-node-modal').classList.remove('is-open');this.reset()}" {
                    div class="modal-body" {
                        div style="background:linear-gradient(135deg,rgba(22,163,74,0.04),rgba(22,163,74,0.08));padding:var(--space-4) var(--space-5);border-radius:var(--radius-md);margin-bottom:var(--space-6);font-size:13px;color:var(--fg-2);border:1px solid rgba(22,163,74,0.08)" {
                            "当前已完成节点："
                            strong style="color:var(--success)" {
                                @if let Some(last) = tracking.last() {
                                    (node_type_label(&last.node_type))
                                } @else {
                                    "无"
                                }
                            }
                            "，下一可记录节点："
                            strong style="color:var(--accent)" {
                                @if let Some(last) = tracking.last() {
                                    @if let Some(next) = all_node_types.iter().find(|nt| nt.as_i16() > last.node_type.as_i16()) {
                                        (node_type_label(next))
                                    } @else {
                                        "已全部完成"
                                    }
                                } @else {
                                    "发料"
                                }
                            }
                        }
                        div class="form-grid" {
                            div class="form-field" {
                                label { "节点类型" }
                                select name="node_type" class="form-select" style="width:100%" {
                                    @for nt in all_node_types.iter() {
                                        @let label = node_type_label(nt);
                                        option value=(nt.as_i16()) { (label) }
                                    }
                                }
                            }
                            div class="form-field" {
                                label { "实际时间" }
                                input type="datetime-local" name="actual_time" class="form-input" style="width:100%" {}
                            }
                            div class="form-field field-full" {
                                label { "备注" }
                                textarea name="remark" class="form-input" rows="2" placeholder="节点备注…" style="width:100%;resize:vertical" {}
                            }
                        }
                    }
                    div class="modal-foot" {
                        button type="button" class="btn btn-default" _="on click remove .is-open from #record-node-modal" {
                            "取消"
                        }
                        button type="submit" class="btn btn-primary" { "确认记录" }
                    }
                }
            }
        }

        // ── Receive Modal ──
        div id="receive-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" {
            div class="modal" {
                div class="modal-head" {
                    h2 style="display:flex;align-items:center;gap:var(--space-2)" {
                        (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
                        "收货登记"
                    }
                    button class="btn btn-text btn-sm" type="button" _="on click remove .is-open from #receive-modal" {
                        "✕"
                    }
                }
                form hx-post=(OmOutsourcingReceivePath { id: order.id }.to_string()) hx-swap="none"
                    hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#receive-modal').classList.remove('is-open');this.reset()}" {
                    div class="modal-body" {
                        div style="background:linear-gradient(135deg,var(--accent-bg),rgba(37,99,235,0.06));padding:var(--space-4) var(--space-5);border-radius:var(--radius-md);margin-bottom:var(--space-6);font-size:13px;color:var(--fg-2);border:1px solid rgba(37,99,235,0.08)" {
                            div style="display:flex;align-items:center;gap:var(--space-4);flex-wrap:wrap" {
                                span { "委外单 " strong style="color:var(--fg)" { (order.doc_number) } }
                                span style="color:var(--border)" { "|" }
                                span { (product_name) }
                                span style="color:var(--border)" { "|" }
                                span { (supplier_name) }
                                span style="color:var(--border)" { "|" }
                                span { "计划 " span class="mono" style="font-weight:700" { (order.planned_qty.to_string()) } " · 已收 " span class="mono text-success" style="font-weight:700" { (order.completed_qty.to_string()) } }
                            }
                        }
                        div class="form-grid" {
                            div class="form-field" {
                                label { "本次收货数量 " span style="color:var(--danger)" { "*" } }
                                input type="number" name="received_qty" class="form-input" placeholder="请输入数量" min="1" style="width:100%" required {}
                            }
                            div class="form-field" {
                                label { "入库仓库" }
                                select name="warehouse_id" class="form-select" style="width:100%" {
                                    option value="" { "成品仓（默认）" }
                                    option value="1" { "待检仓" }
                                }
                            }
                            div class="form-field" {
                                label { "IQC 合格数量" }
                                input type="number" name="qualified_qty" class="form-input" placeholder="自动填充" style="width:100%" {}
                            }
                            div class="form-field" {
                                label { "IQC 不合格数量" }
                                input type="number" name="unqualified_qty" class="form-input" placeholder="0" style="width:100%" {}
                            }
                            div class="form-field field-full" {
                                label { "备注" }
                                textarea name="remark" class="form-input" rows="2" placeholder="收货备注…" style="width:100%;resize:vertical" {}
                            }
                        }
                    }
                    div class="modal-foot" {
                        button type="button" class="btn btn-default" _="on click remove .is-open from #receive-modal" {
                            "取消"
                        }
                        button type="submit" class="btn btn-primary" { "确认收货" }
                    }
                }
            }
        }

        // ── Convert Modal ──
        div id="convert-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" {
            div class="modal" style="width:520px" {
                div class="modal-head" {
                    h2 style="display:flex;align-items:center;gap:var(--space-2)" {
                        (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><path d="M12 9v4M12 17h.01"/></svg>"#))
                        "转自制确认"
                    }
                    button class="btn btn-text btn-sm" type="button" _="on click remove .is-open from #convert-modal" {
                        "✕"
                    }
                }
                form hx-post=(OmOutsourcingConvertPath { id: order.id }.to_string()) hx-swap="none"
                    hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#convert-modal').classList.remove('is-open');this.reset()}" {
                    div class="modal-body" style="text-align:center;padding:var(--space-8)" {
                        div style="width:64px;height:64px;border-radius:50%;background:linear-gradient(135deg,rgba(217,119,6,0.08),rgba(217,119,6,0.15));display:grid;place-items:center;margin:0 auto var(--space-5)" {
                            (maud::PreEscaped(r#"<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4"/></svg>"#))
                        }
                        p style="font-size:var(--text-lg);font-weight:700;color:var(--fg);margin:0 0 var(--space-2)" { "将委外单转为内部生产？" }
                        p style="font-size:var(--text-sm);color:var(--muted);margin:0 0 var(--space-6);line-height:1.7" { "系统将自动创建新的内部工单，" br {} "并将已发物料从委外虚拟仓调回。" }
                        div style="text-align:left" {
                            div class="form-field" {
                                label { "备注（可选）" }
                                textarea name="remark" class="form-input" rows="2" placeholder="转自制原因…" style="width:100%;resize:vertical" {}
                            }
                        }
                    }
                    div class="modal-foot" {
                        button type="button" class="btn btn-default" _="on click remove .is-open from #convert-modal" {
                            "取消"
                        }
                        button type="submit" class="btn btn-primary" style="background:linear-gradient(135deg,var(--warn),#f59e0b)" { "确认转自制" }
                    }
                }
            }
        }

        // ── Cancel Modal ──
        div id="cancel-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" {
            div class="modal" style="width:480px" {
                div class="modal-head" {
                    h2 style="display:flex;align-items:center;gap:var(--space-2)" {
                        (maud::PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
                        "取消委外单"
                    }
                    button class="btn btn-text btn-sm" type="button" _="on click remove .is-open from #cancel-modal" {
                        "✕"
                    }
                }
                form hx-post=(OmOutsourcingCancelPath { id: order.id }.to_string()) hx-swap="none"
                    hx-on::after-request="if(event.detail.xhr.status<400){document.querySelector('#cancel-modal').classList.remove('is-open');this.reset()}" {
                    div class="modal-body" style="text-align:center;padding:var(--space-8)" {
                        div style="width:64px;height:64px;border-radius:50%;background:linear-gradient(135deg,rgba(220,38,38,0.08),rgba(220,38,38,0.15));display:grid;place-items:center;margin:0 auto var(--space-5)" {
                            (maud::PreEscaped(r#"<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="var(--danger)" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>"#))
                        }
                        p style="font-size:var(--text-lg);font-weight:700;color:var(--fg);margin:0 0 var(--space-2)" { "确认取消此委外单？" }
                        p style="font-size:var(--text-sm);color:var(--muted);margin:0 0 var(--space-6);line-height:1.7" { "仅草稿状态可取消。取消后不可恢复。" }
                        div style="text-align:left" {
                            div class="form-field" {
                                label { "取消原因 " span style="color:var(--danger)" { "*" } }
                                textarea name="remark" class="form-input" rows="2" placeholder="请填写取消原因…" style="width:100%;resize:vertical" required {}
                            }
                        }
                    }
                    div class="modal-foot" {
                        button type="button" class="btn btn-default" _="on click remove .is-open from #cancel-modal" {
                            "返回"
                        }
                        button type="submit" class="btn btn-danger" { "确认取消" }
                    }
                }
            }
        }
    }}
}
