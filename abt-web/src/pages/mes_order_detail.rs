use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::{DefectReason, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{
    ProductionBatchService, SplitReq, StepConfirmationReq,
};
use abt_core::mes::work_order::{
    HubAuditLog, HubMaterial, HubReceipts, HubReports, HubRoutingMatrix,
    MaterialAvailabilityLevel, RoutingCellStatus, WorkOrder, WorkOrderHubSummary,
    WorkOrderService,
};

use crate::components::{disclosure, drawer, icon, material_badge, status_step_bar};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{
    OrderCancelPath, OrderClosePath, OrderDetailPath, OrderListPath, OrderReportPath,
    OrderReleasePath, OrderSplitPath, OrderUnreleasePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

/// 状态 → (中文标签, UnoCSS 语义色 token)
fn wo_status_meta(s: &WorkOrderStatus) -> (&'static str, &'static str) {
    use WorkOrderStatus::*;
    match s {
        Draft => ("待计划", "muted"),
        Planned => ("已计划", "accent"),
        Released => ("已下达", "success"),
        InProduction => ("生产中", "warn"),
        Closed => ("已关闭", "accent"),
        Cancelled => ("已取消", "danger"),
    }
}

/// 状态药丸（语义色 token）
fn status_pill(label: &str, token: &str) -> Markup {
    html! {
        span class=({
            format!(
                "inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{}-bg text-{}",
                token, token
            )
        }) {
            span class=({ format!("inline-block w-1.5 h-1.5 rounded-full bg-{}", token) }) {}
            (label)
        }
    }
}

fn fmt_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// RoutingCellStatus → (CSS cell class, 文本前缀)
fn cell_meta(s: RoutingCellStatus) -> (&'static str, &'static str) {
    match s {
        RoutingCellStatus::Done => ("done", "✓"),
        RoutingCellStatus::Active => ("active", "▶"),
        RoutingCellStatus::Pending => ("pending", ""),
    }
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_detail(
    path: OrderDetailPath,
    ctx: RequestContext,
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
    let wo_svc = state.work_order_service();

    // 工作台聚合：一次取全（detail-header + 摘要带 + 6 disclosure 全部数据）
    let summary = wo_svc
        .get_hub_summary(&service_ctx, &mut conn, path.id)
        .await?;

    let detail_path = OrderDetailPath { id: path.id }.to_string();
    let content = hub_page(&summary, &detail_path);
    let page_html = admin_page(
        is_htmx,
        "工单工作台",
        &claims,
        "production",
        &format!("/admin/mes/orders/{}", path.id),
        "生产管理",
        Some(OrderListPath::PATH),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn release_order(
    path: OrderReleasePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;

    if order.status == WorkOrderStatus::Released {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.release(&service_ctx, &mut tx, path.order_id, order.version)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn unrelease_order(
    path: OrderUnreleasePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;

    // 幂等：已是草稿则直接重定向
    if order.status == WorkOrderStatus::Draft {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.unrelease(&service_ctx, &mut tx, path.order_id, order.version)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn close_order(
    path: OrderClosePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;

    if order.status == WorkOrderStatus::Closed {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.close(&service_ctx, &mut tx, path.order_id, order.version)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn cancel_order(
    path: OrderCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;

    if order.status == WorkOrderStatus::Cancelled {
        let redirect = OrderDetailPath { id: path.order_id }.to_string();
        return Ok(([("HX-Redirect", redirect)], Html(String::new())));
    }

    svc.cancel(&service_ctx, &mut tx, path.order_id, order.version)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let redirect = OrderDetailPath { id: path.order_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct SplitForm {
    pub split_qty: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub team_id: Option<i64>,
}

/// 拆批：从工单创建额外的生产批次（事务包裹 + HX-Trigger 局部刷新）。
///
/// 成功不 redirect：返回 `HX-Trigger: batchChanged`，由批次 disclosure + 摘要带各自
/// `hx-trigger="batchChanged from:body"` 自刷新（单端点 + 事件联动范式）。
#[require_permission("WORK_ORDER", "update")]
pub async fn split_order(
    path: OrderSplitPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SplitForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();

    let split_qty = form
        .split_qty
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| abt_core::shared::types::DomainError::validation("数量格式错误"))?;

    if split_qty <= rust_decimal::Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("拆批数量必须大于 0").into());
    }
    batch_svc
        .split_work_order(
            &service_ctx,
            &mut tx,
            path.order_id,
            vec![SplitReq {
                batch_qty: split_qty,
                team_id: form.team_id,
            }],
        )
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 广播 batchChanged：批次 disclosure + 摘要带各自监听并自刷新
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct ReportForm {
    pub batch_id: i64,
    pub step_no: i32,
    pub worker_id: i64,
    pub shift: ShiftType,
    pub completed_qty: String,
    #[serde(default)]
    pub defect_qty: String,
    /// 缺陷原因（i16 → DefectReason；空串/缺省 → None）
    #[serde(default, deserialize_with = "deserialize_defect_reason")]
    pub defect_reason: Option<DefectReason>,
    #[serde(default)]
    pub work_hours: String,
    pub report_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 反序列化 DefectReason：接收 i16 数字（"1".."4"）或空值。DefectReason 未实现 FromStr，
/// 故不能用 `empty_as_none`；此处用 `from_i16` 显式转换。
fn deserialize_defect_reason<'de, D>(de: D) -> std::result::Result<Option<DefectReason>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = Option::deserialize(de)?;
    match raw.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => s
            .parse::<i16>()
            .ok()
            .and_then(DefectReason::from_i16)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid defect_reason: {s}"))),
    }
}

/// 工序报工（事务包裹 + HX-Trigger 局部刷新）。
///
/// 调 `ProductionBatchService::confirm_routing_step`：累加批次工序进度 + 插报工记录 +
/// 触发质检 + 累计计件工资（单事务）。成功广播 `reportChanged`，报工 disclosure + 矩阵 +
/// 摘要带各自监听并自刷新。
#[require_permission("WORK_ORDER", "update")]
pub async fn report_routing_step(
    _path: OrderReportPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReportForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();

    let parse_qty = |s: &str| -> Result<rust_decimal::Decimal> {
        s.parse::<rust_decimal::Decimal>()
            .map_err(|_| abt_core::shared::types::DomainError::validation("数量格式错误").into())
    };
    let completed_qty = parse_qty(&form.completed_qty)?;
    let defect_qty = if form.defect_qty.trim().is_empty() {
        rust_decimal::Decimal::ZERO
    } else {
        parse_qty(&form.defect_qty)?
    };
    let work_hours = if form.work_hours.trim().is_empty() {
        rust_decimal::Decimal::ZERO
    } else {
        parse_qty(&form.work_hours)?
    };

    if completed_qty < rust_decimal::Decimal::ZERO || defect_qty < rust_decimal::Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("数量不可为负").into());
    }

    let req = StepConfirmationReq {
        step_no: form.step_no,
        worker_id: form.worker_id,
        shift: form.shift,
        completed_qty,
        defect_qty,
        defect_reason: form.defect_reason,
        work_hours,
        report_date: form.report_date,
        remark: form.remark,
    };
    batch_svc
        .confirm_routing_step(&service_ctx, &mut tx, form.batch_id, form.step_no, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 广播 reportChanged：报工 disclosure + 批次矩阵 + 摘要带各自监听并自刷新
    Ok(([("HX-Trigger", "reportChanged")], Html(String::new())))
}

// ============================================================================
// 工作台渲染（hub_page + 各 disclosure body + drawer）
// ============================================================================

/// 工作台整页：detail-header（状态步骤条 + 物料徽章 + 来源链 + 进度条）+ 摘要带 +
/// 6 个 disclosure + 拆批/报工 drawer。
///
/// `detail_path` — `OrderDetailPath{id}` 字符串，供 disclosure 局部刷新 hx-get 复用。
fn hub_page(summary: &WorkOrderHubSummary, detail_path: &str) -> Markup {
    let order = &summary.order;
    html! {
        div class="hub-page" {
            (back_link())
            (detail_header(summary, detail_path))
            (stat_strip(summary, detail_path))
            p class="text-xs text-muted my-2 px-1" { "需要时点击展开 · 每个区块可就地发起操作" }
            // ① 工单信息
            (disclosure::disclosure(
                "d-info",
                "工单信息",
                icon::file_text_icon("w-4 h-4"),
                Some(&format!(
                    "{} · {} 工序 · {}",
                    summary.info.routing_doc.as_deref().unwrap_or("—"),
                    summary.info.routing_step_count,
                    summary.info.consumption_mode_label
                )),
                false,
                None,
                body_info(summary),
                detail_path,
                None,
            ))
            // ② 物料 & 领料（缺料时红点 + 异常摘要）
            (disclosure::disclosure(
                "d-mat",
                "物料 & 领料",
                icon::cube_icon("w-4 h-4"),
                Some(&material_summary(&summary.material)),
                matches!(
                    summary.material.availability.level,
                    MaterialAvailabilityLevel::Unavailable | MaterialAvailabilityLevel::Late
                ),
                None, // P2: 领料 drawer（留待接入 MaterialRequisitionService）
                body_material(&summary.material),
                detail_path,
                Some("requisitionChanged"),
            ))
            // ③ 批次 × 工序矩阵
            (disclosure::disclosure(
                "d-matrix",
                "批次进度",
                icon::grid_4_icon("w-4 h-4"),
                Some(&batch_summary(&summary.matrix)),
                false,
                can_split_action(order),
                body_matrix(&summary.matrix, order.id),
                detail_path,
                Some("batchChanged"),
            ))
            // ④ 报工记录（报废异常染色）
            (disclosure::disclosure(
                "d-report",
                "报工记录",
                icon::clipboard_list_icon("w-4 h-4"),
                Some(&report_summary(&summary.reports)),
                summary.reports.total_defect > rust_decimal::Decimal::ZERO,
                report_action(order),
                body_reports(&summary.reports),
                detail_path,
                Some("reportChanged"),
            ))
            // ⑤ 入库 & 质检
            (disclosure::disclosure(
                "d-rcpt",
                "入库 & 质检",
                icon::package_icon("w-4 h-4"),
                Some(&receipt_summary(&summary.receipts)),
                false,
                None, // P2: 入库 drawer（留待接入 ProductionReceiptService）
                body_receipts(&summary.receipts),
                detail_path,
                Some("receiptChanged"),
            ))
            // ⑥ 操作日志
            (disclosure::disclosure(
                "d-log",
                "操作日志",
                icon::clock_icon("w-4 h-4"),
                summary.audit_logs.first().map(|l| l.title.as_str()),
                false,
                None,
                body_audit(&summary.audit_logs),
                detail_path,
                None,
            ))
            // 拆批 drawer（新建批次）
            (split_drawer(order, detail_path))
            // 报工 drawer
            (report_drawer(&summary.matrix, order, detail_path))
        }
    }
}

/// 返回链接（带 restore=true，恢复列表筛选状态）
fn back_link() -> Markup {
    html! {
        a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            href=(format!("{}?restore=true", OrderListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回工单列表" }
    }
}

/// detail-header：title-row（状态驱动按钮）+ sub-row + 状态步骤条 + 物料徽章/来源链 + 进度条
fn detail_header(summary: &WorkOrderHubSummary, detail_path: &str) -> Markup {
    let order = &summary.order;
    let (status_label, status_token) = wo_status_meta(&order.status);
    html! {
        div class="detail-header bg-bg border border-border-soft rounded-md p-6 mb-4 shadow-[var(--shadow-card)]" {
            // title-row
            div class="flex items-center justify-between mb-4 gap-4" {
                div class="flex items-center gap-3 min-w-0" {
                    h1 class="text-xl font-bold font-mono tabular-nums whitespace-nowrap" {
                        (order.doc_number)
                    }
                    (status_pill(status_label, status_token))
                }
                div class="flex items-center gap-2 shrink-0" {
                    (status_actions(order, detail_path))
                }
            }
            // sub-row
            div class="flex items-center flex-wrap gap-2 mb-5 text-sm text-fg-2" {
                strong class="text-fg" { (summary.product_name) }
                span class="inline-flex items-center px-[7px] py-0.5 rounded-full text-[11px] font-medium bg-surface text-fg-2 border border-border" {
                    "自制"
                }
                span class="text-border" { "·" }
                span class="font-mono font-semibold text-fg tabular-nums" {
                    (crate::utils::fmt_qty(order.planned_qty)) " 件"
                }
                @if let Some(wc) = summary.work_center_name.as_ref() {
                    span class="text-border" { "·" }
                    span { (wc) }
                }
                span class="text-border" { "·" }
                span class="font-mono tabular-nums" {
                    (order.scheduled_start.format("%m-%d")) " → " (order.scheduled_end.format("%m-%d"))
                }
            }
            // 状态步骤条
            (status_step_bar::status_step_bar(&summary.status_steps))
            // 物料徽章 + 来源链
            div class="flex items-center gap-3 mb-5 flex-wrap mt-5" {
                (material_badge::material_badge(&summary.material_availability, "d-mat"))
                (source_trace(summary))
            }
            // 进度条
            div class="wo-progress" {
                div class="flex items-center justify-between mb-2 text-sm" {
                    span class="text-muted font-medium" { "完工入库进度" }
                    span class="font-mono font-semibold text-fg tabular-nums" {
                        (crate::utils::fmt_qty(summary.received_qty))
                        " / "
                        (crate::utils::fmt_qty(order.planned_qty))
                        " · "
                        (summary.completion_pct.round_dp(0))
                        "%"
                    }
                }
                div class="w-full h-2 bg-border-soft rounded overflow-hidden" {
                    div class="h-full rounded bg-accent transition-all duration-500"
                        style=(format!("width: {}%", summary.completion_pct.round_dp(0))) {}
                }
            }
        }
    }
}

/// 状态驱动按钮（按 WorkOrderStatus 显隐）
fn status_actions(order: &WorkOrder, detail_path: &str) -> Markup {
    use WorkOrderStatus::*;
    let close_path = OrderClosePath { order_id: order.id }.to_string();
    let release_path = OrderReleasePath { order_id: order.id }.to_string();
    let unrelease_path = OrderUnreleasePath { order_id: order.id }.to_string();
    let cancel_path = OrderCancelPath { order_id: order.id }.to_string();
    html! {
        // 下达（Draft/Planned）
        @if matches!(order.status, Draft | Planned) {
            button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150"
                hx-post=(&release_path)
                hx-confirm="确认下达此工单？下达后将开始生产。"
                hx-disabled-elt="this"
            { (icon::rocket_icon("w-4 h-4")) "下达工单" }
        }
        // 拆批 + 反下达 + 关闭（Released/InProduction）
        @if matches!(order.status, Released | InProduction) {
            button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150"
                type="button"
                _="on click add .open to #split-drawer"
            { (icon::plus_icon("w-4 h-4")) "拆批" }
            @if matches!(order.status, Released) {
                button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150"
                    type="button"
                    _="on click add .is-open to #unrelease-dialog"
                { "反下达" }
            }
            button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-white border-none hover:opacity-90 text-sm font-medium cursor-pointer transition-all duration-150"
                hx-post=(&close_path)
                hx-confirm="确认关闭此工单？"
                hx-disabled-elt="this"
            { (icon::check_circle_icon("w-4 h-4")) "关闭" }
        }
        // 取消（非终态且有未入库）
        @if matches!(order.status, Draft | Planned | Released | InProduction) {
            button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-danger border border-danger/30 hover:bg-danger-bg text-sm font-medium cursor-pointer transition-all duration-150"
                hx-post=(&cancel_path)
                hx-confirm="确认取消此工单？取消后不可恢复。"
                hx-disabled-elt="this"
            { (icon::x_icon("w-4 h-4")) "取消" }
        }
        // 反下达确认弹窗（仅 Released）
        @if matches!(order.status, Released) {
            (unrelease_dialog(&unrelease_path, detail_path))
        }
    }
}

/// 反下达确认弹窗
fn unrelease_dialog(unrelease_path: &str, _detail_path: &str) -> Markup {
    html! {
        div id="unrelease-dialog"
            class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
        {
            div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
                div class="px-6 py-5 border-b border-border-soft" { h2 class="m-0" { "确认反下达？" } }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    p class="text-sm text-fg-2 leading-relaxed m-0" {
                        "反下达将回退工单到 "
                        strong { "草稿" }
                        " 状态，同时取消领料单、释放库存预留、软删除生产批次（若有报工记录则无法反下达）。此操作不可撤销。"
                    }
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
                    button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150"
                        type="button"
                        _="on click remove .is-open from #unrelease-dialog"
                    { "取消" }
                    button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-white border-none hover:opacity-90 text-sm font-medium cursor-pointer"
                        hx-post=(unrelease_path)
                        hx-confirm="确认执行反下达？"
                        hx-disabled-elt="this"
                    { "确认反下达" }
                }
            }
        }
    }
}

/// 来源链（SO → 计划 → 工单 → 批次/入库）
fn source_trace(summary: &WorkOrderHubSummary) -> Markup {
    let sc = &summary.source_chain;
    html! {
        div class="source-trace flex items-center gap-2 text-xs text-muted bg-surface border border-border-soft rounded-md px-3 py-2 flex-1 flex-wrap min-w-0" {
            span class="font-medium" { "来源链" }
            @if let Some(so) = sc.sales_order_doc.as_ref() {
                span class="text-accent font-mono font-medium" { (so) }
                @if let Some(c) = sc.customer_name.as_ref() {
                    span class="text-muted" { "(" (c) ")" }
                }
                span class="text-border" { "→" }
            }
            @if let Some(pdoc) = sc.plan_doc.as_ref() {
                span class="text-accent font-mono font-medium" { (pdoc) }
                span class="text-border" { "→" }
            }
            span class="text-fg font-mono font-semibold" { (summary.order.doc_number) }
            span class="text-border" { "→" }
            span { (sc.batch_count) " 批次 · " (crate::utils::fmt_qty(sc.received_qty)) " 入库" }
        }
    }
}

/// 摘要带（4 格，可点击 drill-down）
fn stat_strip(summary: &WorkOrderHubSummary, _detail_path: &str) -> Markup {
    html! {
        // 整体监听 batchChanged/reportChanged/receiptChanged 局部刷新
        div id="stat-strip"
            class="stat-strip flex bg-bg border border-border-soft rounded-md mb-4 shadow-[var(--shadow-xs)] overflow-hidden"
            hx-get=(OrderDetailPath { id: summary.order.id }.to_string())
            hx-trigger="batchChanged from:body, reportChanged from:body, receiptChanged from:body"
            hx-select="#stat-strip"
            hx-swap="outerHTML"
            hx-disinherit="hx-select"
        {
            // 完工入库（点击展开 d-info）
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 border-r border-border-soft cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                _="on click add .open to #d-info then call #d-info.scrollIntoView() with {behavior:'smooth',block:'center'}"
            {
                span class="font-mono text-xl font-bold text-success tabular-nums leading-tight" {
                    (crate::utils::fmt_qty(summary.received_qty))
                }
                span class="text-xs text-muted font-medium" { "完工入库" }
            }
            // 在制
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 border-r border-border-soft" {
                span class="font-mono text-xl font-bold text-fg tabular-nums leading-tight" {
                    (crate::utils::fmt_qty(summary.in_progress_qty))
                }
                span class="text-xs text-muted font-medium" { "在制" }
            }
            // 批次（点击展开 d-matrix）
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 border-r border-border-soft cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                _="on click add .open to #d-matrix then call #d-matrix.scrollIntoView() with {behavior:'smooth',block:'center'}"
            {
                span class="font-mono text-xl font-bold text-fg tabular-nums leading-tight" {
                    (summary.source_chain.batch_count)
                }
                span class="text-xs text-muted font-medium" { "批次" }
            }
            // FQC（点击展开 d-rcpt）
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                _="on click add .open to #d-rcpt then call #d-rcpt.scrollIntoView() with {behavior:'smooth',block:'center'}"
            {
                span class=({
                    format!(
                        "font-mono text-xl font-bold tabular-nums leading-tight {}",
                        if summary.receipts.fqc_passed { "text-success" } else { "text-muted" }
                    )
                }) {
                    @if summary.receipts.fqc_passed { "通过" } @else { "—" }
                }
                span class="text-xs text-muted font-medium" { "FQC" }
            }
        }
    }
}

// ── Disclosure bodies ──

/// ① 工单信息：4 列 info-grid
fn body_info(summary: &WorkOrderHubSummary) -> Markup {
    let order = &summary.order;
    html! {
        div class="grid grid-cols-4 gap-x-5 gap-y-4 mt-4 max-[900px]:grid-cols-2" {
            (info_item("产品", &summary.product_name))
            (info_item_mono("计划数量", &format!("{} 件", crate::utils::fmt_qty(order.planned_qty))))
            (info_item_mono("版本号", &format!("v{}", order.version)))
            (info_item("工作中心", summary.work_center_name.as_deref().unwrap_or("—")))
            (info_item_mono("计划起止", &format!("{} → {}", order.scheduled_start, order.scheduled_end)))
            (info_item("班组", summary.info.team_label.as_deref().unwrap_or("—")))
            // BOM 快照
            div class="flex flex-col gap-0.5" {
                span class="text-[11px] text-muted font-medium" { "BOM 快照" }
                span class="text-sm text-fg font-medium" {
                    @match summary.info.bom_snapshot_doc.as_deref() {
                        Some(d) => { (d) " 已冻结" }
                        None => { "—" }
                    }
                }
            }
            // 工艺路线
            div class="flex flex-col gap-0.5" {
                span class="text-[11px] text-muted font-medium" { "工艺路线" }
                span class="text-sm text-fg font-medium" {
                    (summary.info.routing_doc.as_deref().unwrap_or("—"))
                }
            }
            (info_item("物料消耗模式", &summary.info.consumption_mode_label))
            (info_item_mono("工序数", &summary.info.routing_step_count.to_string()))
            (info_item_mono("完成", &format!("{} 件", crate::utils::fmt_qty(order.completed_qty))))
            @if order.scrap_qty > rust_decimal::Decimal::ZERO {
                div class="flex flex-col gap-0.5" {
                    span class="text-[11px] text-muted font-medium" { "报废" }
                    span class="text-sm text-danger font-medium font-mono tabular-nums" {
                        (crate::utils::fmt_qty(order.scrap_qty))
                    }
                }
            }
        }
    }
}

fn info_item(label: &str, value: &str) -> Markup {
    html! {
        div class="flex flex-col gap-0.5" {
            span class="text-[11px] text-muted font-medium" { (label) }
            span class="text-sm text-fg font-medium" { (value) }
        }
    }
}

fn info_item_mono(label: &str, value: &str) -> Markup {
    html! {
        div class="flex flex-col gap-0.5" {
            span class="text-[11px] text-muted font-medium" { (label) }
            span class="text-sm text-fg font-medium font-mono tabular-nums" { (value) }
        }
    }
}

/// ② 物料 & 领料：领料单列表 + 4 级可用性行
fn body_material(material: &HubMaterial) -> Markup {
    html! {
        div class="mt-4" {
            // 领料单（可折叠）
            @if material.requisitions.is_empty() {
                p class="text-sm text-muted text-center py-4" { "暂无领料单" }
            } @else {
                div class="border border-border-soft rounded-md overflow-hidden mb-4" {
                    @for req in &material.requisitions {
                        div class="req-item flex items-center gap-3 px-4 py-3 border-b last:border-b-0 border-border-soft cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                            _="on click toggle .open on next <div.req-detail/> then toggle .open on me"
                        {
                            span class="text-muted" { (icon::chevron_right_icon("w-3.5 h-3.5")) }
                            span class="font-mono text-accent font-semibold text-sm" { (req.doc_number) }
                            span class="text-xs text-muted flex-1" {
                                (req.item_count) " 项 · " (crate::utils::fmt_qty(req.total_qty)) " 件"
                            }
                            (status_pill(&req.status_label, "success"))
                        }
                        div class="req-detail hidden bg-surface-raised border-b last:border-b-0 border-border-soft px-4 py-3 [.open&]:block" {
                            table class="w-full border-collapse" {
                                thead {
                                    tr {
                                        th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "物料" }
                                        th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "需求" }
                                        th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "已领" }
                                        th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "库存可用" }
                                    }
                                }
                                tbody {
                                    @for it in &req.items {
                                        tr {
                                            td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" {
                                                (it.product_name) " "
                                                span class="text-muted text-[11px]" { (it.product_code) }
                                            }
                                            td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                                                (crate::utils::fmt_qty(it.required_qty))
                                            }
                                            td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                                                (crate::utils::fmt_qty(it.issued_qty))
                                            }
                                            td class=({
                                                format!(
                                                    "py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft {}",
                                                    if it.available_qty < it.required_qty { "text-danger" } else { "text-success" }
                                                )
                                            }) {
                                                (crate::utils::fmt_qty(it.available_qty))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 物料可用性明细（4 级）
            @if !material.availability.lines.is_empty() {
                div class="text-[11px] text-muted font-semibold uppercase tracking-wide mb-2" { "物料可用性明细" }
                table class="w-full border-collapse" {
                    thead {
                        tr {
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "物料" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "需求" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "已领" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "ATP" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "预计" }
                        }
                    }
                    tbody {
                        @for line in &material.availability.lines {
                            @let (token, _) = level_meta(line.level);
                            tr {
                                td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" {
                                    (line.product_name) " "
                                    span class="text-muted text-[11px]" { (line.product_code) }
                                }
                                td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                                    (crate::utils::fmt_qty(line.required_qty))
                                }
                                td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                                    (crate::utils::fmt_qty(line.issued_qty))
                                }
                                td class=({
                                    format!(
                                        "py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft text-{}",
                                        token
                                    )
                                }) {
                                    (crate::utils::fmt_qty(line.atp))
                                }
                                td class="py-3 px-3 text-sm font-mono tabular-nums text-muted border-b last:border-b-0 border-border-soft" {
                                    (crate::utils::fmt_qty(line.projected))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// ③ 批次 × 工序矩阵
fn body_matrix(matrix: &HubRoutingMatrix, _order_id: i64) -> Markup {
    if matrix.routings.is_empty() || matrix.rows.is_empty() {
        return html! { p class="text-sm text-muted text-center py-4 mt-4" { "暂无批次进度" } };
    }
    html! {
        div class="overflow-x-auto mt-4" {
            table class="w-full border-separate border-spacing-[3px] min-w-[680px]" {
                thead {
                    tr {
                        th class="text-xs font-semibold text-muted p-2 text-left align-bottom w-[180px]" { "批次 \\ 工序" }
                        @for r in &matrix.routings {
                            th class="text-xs font-semibold text-muted p-2 text-center align-bottom" {
                                span class="text-fg font-semibold block" {
                                    (r.step_no) " " (r.process_name.as_str())
                                }
                                @if r.is_inspection_point {
                                    span class="text-[10px] text-warn" { "报检" }
                                }
                            }
                        }
                    }
                }
                tbody {
                    @for row in &matrix.rows {
                        tr {
                            // 批次单元
                            td class="batch-cell align-middle bg-surface-raised rounded-sm p-2 px-3" {
                                a class="font-mono text-xs font-semibold text-accent cursor-pointer"
                                    href=(format!("/admin/mes/batches/{}", row.batch.id))
                                { (row.batch.batch_no.as_str()) }
                                div class="text-[11px] text-muted mt-0.5" {
                                    (crate::utils::fmt_qty(row.batch.batch_qty)) " 件"
                                }
                            }
                            // 工序单元格：done→success / active→accent / pending→surface-muted
                            @for cell in &row.cells {
                                @let (_cls, prefix) = cell_meta(cell.status);
                                @let token = match cell.status {
                                    RoutingCellStatus::Done => "success",
                                    RoutingCellStatus::Active => "accent",
                                    RoutingCellStatus::Pending => "muted",
                                };
                                @let bg_cls = match cell.status {
                                    RoutingCellStatus::Done | RoutingCellStatus::Active => format!("bg-{}-bg", token),
                                    RoutingCellStatus::Pending => "bg-surface".to_string(),
                                };
                                td class=({
                                    format!(
                                        "cell text-center rounded-sm px-2 py-3 font-mono tabular-nums text-sm font-semibold cursor-pointer hover:scale-105 transition-transform duration-150 {} text-{}",
                                        bg_cls, token
                                    )
                                }) {
                                    @if matches!(cell.status, RoutingCellStatus::Pending) {
                                        "—"
                                    } @else {
                                        (prefix) " "
                                        (crate::utils::fmt_qty(cell.completed_qty))
                                        @if cell.defect_qty > rust_decimal::Decimal::ZERO {
                                            span class="block text-[10px] font-medium opacity-75 mt-0.5" {
                                                "报废" (crate::utils::fmt_qty(cell.defect_qty))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 图例
            div class="flex items-center gap-4 mt-3 text-xs text-muted flex-wrap" {
                span class="flex items-center gap-[5px]" { span class="w-3 h-3 rounded-sm bg-success-bg border border-current/30" {} "已完成" }
                span class="flex items-center gap-[5px]" { span class="w-3 h-3 rounded-sm bg-accent-bg border border-current/30" {} "进行中" }
                span class="flex items-center gap-[5px]" { span class="w-3 h-3 rounded-sm bg-surface border border-border-soft" {} "待生产" }
            }
        }
    }
}

/// ④ 报工记录（mini-table）
fn body_reports(reports: &HubReports) -> Markup {
    html! {
        table class="w-full border-collapse mt-4" {
            thead {
                tr {
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "时间" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "批次" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-soft" { "工序" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "完成" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "报废" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "报工人" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "班组" }
                }
            }
            tbody {
                @for r in &reports.items {
                    tr {
                        td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                            @if let Some(dt) = r.reported_at { (fmt_dt(dt)) } @else { "—" }
                        }
                        td class="py-3 px-3 text-sm text-accent font-mono border-b last:border-b-0 border-border-soft" {
                            (r.batch_no.as_str())
                        }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" { (r.op_name.as_str()) }
                        td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                            (crate::utils::fmt_qty(r.completed_qty))
                        }
                        td class=({
                            format!(
                                "py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft {}",
                                if r.defect_qty > rust_decimal::Decimal::ZERO { "text-danger" } else { "" }
                            )
                        }) {
                            (crate::utils::fmt_qty(r.defect_qty))
                        }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" { (r.worker_name.as_str()) }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" {
                            (r.team_label.as_deref().unwrap_or("—"))
                        }
                    }
                }
                @if reports.items.is_empty() {
                    tr { td colspan="7" class="text-center text-muted text-sm py-4" { "暂无报工记录" } }
                }
            }
        }
    }
}

/// ⑤ 入库 & 质检（mini-table）
fn body_receipts(receipts: &HubReceipts) -> Markup {
    html! {
        table class="w-full border-collapse mt-4" {
            thead {
                tr {
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "入库单" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "批次" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "数量" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "仓库" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "FQC" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "倒冲" }
                }
            }
            tbody {
                @for r in &receipts.items {
                    tr {
                        td class="py-3 px-3 text-sm text-accent font-mono border-b last:border-b-0 border-border-soft" { (r.doc_number.as_str()) }
                        td class="py-3 px-3 text-sm text-accent font-mono border-b last:border-b-0 border-border-soft" { (r.batch_no.as_str()) }
                        td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                            (crate::utils::fmt_qty(r.received_qty))
                        }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" { (r.warehouse_name.as_str()) }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" {
                            span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-success-bg text-success" {
                                (r.fqc_label.as_str())
                            }
                        }
                        td class="py-3 px-3 text-sm border-b last:border-b-0 border-border-soft" {
                            span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-success-bg text-success" {
                                (r.backflush_label.as_str())
                            }
                        }
                    }
                }
                @if receipts.items.is_empty() {
                    tr { td colspan="6" class="text-center text-muted text-sm py-4" { "暂无入库记录" } }
                }
            }
        }
    }
}

/// ⑥ 操作日志（时间线）
fn body_audit(logs: &[HubAuditLog]) -> Markup {
    html! {
        div class="relative pl-[22px] mt-4" {
            div class="absolute left-[6px] top-1.5 bottom-1.5 w-0.5 bg-border-soft" {}
            @for log in logs {
                div class="relative pb-4 last:pb-0" {
                    div class=({
                        format!(
                            "absolute -left-[19px] top-[3px] w-2.5 h-2.5 rounded-full border-2 {}",
                            if log.is_current {
                                "bg-accent border-accent ring-4 ring-accent-bg"
                            } else {
                                "bg-success border-success"
                            }
                        )
                    }) {}
                    div class="text-sm font-semibold text-fg" { (log.title.as_str()) }
                    div class="text-[11px] text-muted mt-0.5" { (log.meta.as_str()) }
                }
            }
            @if logs.is_empty() {
                p class="text-center text-muted text-sm py-4" { "暂无操作日志" }
            }
        }
    }
}

// ── Disclosure summaries ──

fn material_summary(material: &HubMaterial) -> String {
    let (token, label) = level_meta(material.availability.level);
    let count = material.requisitions.len();
    let headline = material
        .availability
        .headline
        .as_deref()
        .unwrap_or("");
    let color = match token {
        "danger" | "warn" => "<span class=\"text-danger\">",
        _ => "",
    };
    let close = if color.is_empty() { "" } else { "</span>" };
    format!(
        "{} 领料单 · {} {}{}{}",
        count,
        label,
        color,
        headline,
        close
    )
}

fn batch_summary(matrix: &HubRoutingMatrix) -> String {
    let n = matrix.rows.len();
    if n == 0 {
        return "暂无批次".to_string();
    }
    let in_progress: Vec<&str> = matrix
        .rows
        .iter()
        .filter_map(|r| {
            r.cells
                .iter()
                .any(|c| matches!(c.status, RoutingCellStatus::Active))
                .then_some(r.batch.batch_no.as_str())
        })
        .take(2)
        .collect();
    if in_progress.is_empty() {
        format!("{} 批次", n)
    } else {
        format!("{} 批次 · {} 生产中", n, in_progress.join("、"))
    }
}

fn report_summary(reports: &HubReports) -> String {
    if reports.total_defect > rust_decimal::Decimal::ZERO {
        format!(
            "{} 次 · 累计 {} · <span class=\"text-danger\">报废 {}</span>",
            reports.total_count,
            crate::utils::fmt_qty(reports.total_completed),
            crate::utils::fmt_qty(reports.total_defect)
        )
    } else {
        format!(
            "{} 次 · 累计 {}",
            reports.total_count,
            crate::utils::fmt_qty(reports.total_completed)
        )
    }
}

fn receipt_summary(receipts: &HubReceipts) -> String {
    if receipts.items.is_empty() {
        return "暂无入库".to_string();
    }
    let fqc = if receipts.fqc_passed {
        "<span class=\"text-success\">通过</span>"
    } else {
        "—"
    };
    let backflush = if receipts.backflush_done {
        "倒冲完成"
    } else {
        "倒冲待执行"
    };
    format!(
        "{} 入库 · FQC {} · {}",
        crate::utils::fmt_qty(receipts.total_received),
        fqc,
        backflush
    )
}

// ── Disclosure actions ──

fn can_split_action(order: &WorkOrder) -> Option<Markup> {
    if matches!(
        order.status,
        WorkOrderStatus::Released | WorkOrderStatus::InProduction
    ) {
        Some(disclosure::di_action("新建批次", "add .open to #split-drawer"))
    } else {
        None
    }
}

fn report_action(order: &WorkOrder) -> Option<Markup> {
    if matches!(
        order.status,
        WorkOrderStatus::Released | WorkOrderStatus::InProduction
    ) {
        Some(disclosure::di_action("报工", "add .open to #report-drawer"))
    } else {
        None
    }
}

// ── Drawers ──

/// level → 语义色 token（供 material body 复用）
fn level_meta(l: MaterialAvailabilityLevel) -> (&'static str, &'static str) {
    use MaterialAvailabilityLevel::*;
    match l {
        Available => ("success", "齐套"),
        Expected => ("accent", "待齐套"),
        Late => ("warn", "迟料"),
        Unavailable => ("danger", "缺料"),
    }
}

/// 拆批 drawer（新建批次）
fn split_drawer(order: &WorkOrder, _detail_path: &str) -> Markup {
    let split_path = OrderSplitPath { order_id: order.id }.to_string();
    let body = html! {
        form id="split-form" hx-post=(&split_path) hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#split-drawer').classList.remove('open')"
        {
            // 工单剩余可拆量（只读展示）
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "工单计划量" }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted font-mono tabular-nums"
                    readonly value=(format!("{} 件", crate::utils::fmt_qty(order.planned_qty)));
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "本批数量" }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="number" step="any" name="split_qty" min="1" required;
            }
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "班组（选填）" }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    type="number" name="team_id" placeholder="班组 ID";
            }
            div class="flex gap-2 p-3 bg-accent-bg rounded-md text-xs text-fg-2 items-start" {
                (icon::info_icon("w-[15px] h-[15px] shrink-0 mt-0.5"))
                span { "拆批后将生成新流转卡（card_sn），并初始化全部工序进度。" }
            }
        }
    };
    drawer::drawer("split-drawer", "新建批次", "确认拆批", "split-form", body)
}

/// 报工 drawer：批次/工序/完成量/报废量/报废原因/班次/报工人/日期 + 计件工资实时算
fn report_drawer(matrix: &HubRoutingMatrix, order: &WorkOrder, _detail_path: &str) -> Markup {
    let report_path = OrderReportPath { order_id: order.id }.to_string();
    // 取工单单价（首道工序 unit_price 作为计件单价示例）
    let unit_price = matrix
        .routings
        .first()
        .and_then(|r| r.unit_price)
        .unwrap_or(rust_decimal::Decimal::ZERO);

    let body = html! {
        form id="report-form" hx-post=(&report_path) hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#report-drawer').classList.remove('open')"
        {
            // 批次 + 工序
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "生产批次" }
                    select name="batch_id" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    {
                        option value="" disabled selected { "选择批次" }
                        @for row in &matrix.rows {
                            option value=(row.batch.id) {
                                (row.batch.batch_no.as_str()) " (" (crate::utils::fmt_qty(row.batch.batch_qty)) " 件)"
                            }
                        }
                    }
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "报工工序" }
                    select name="step_no" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    {
                        option value="" disabled selected { "选择工序" }
                        @for r in &matrix.routings {
                            option value=(r.step_no) {
                                (r.step_no) ". " (r.process_name.as_str())
                                @if r.is_inspection_point { "（报检点）" }
                            }
                        }
                    }
                }
            }
            // 报工人 + 班组
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "报工人 ID" }
                    input name="worker_id" type="number" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "班次" }
                    select name="shift" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                        option value="1" { "白班" }
                        option value="2" { "夜班" }
                    }
                }
            }
            // 日期 + 工时
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "报工日期" }
                    input name="report_date" type="date" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                        value=(chrono::Local::now().format("%Y-%m-%d").to_string());
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "工时（小时）" }
                    input name="work_hours" type="number" step="0.5" min="0"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
                }
            }
            // 完成量 + 报废量（实时算工资）
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "完成量" }
                    input id="rpt-done" name="completed_qty" type="number" min="0" step="any" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                        _="on input calcWage()";
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "报废量" }
                    input id="rpt-scrap" name="defect_qty" type="number" min="0" step="any"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                        _="on input calcWage()";
                }
            }
            // 报废原因
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "报废原因" }
                select name="defect_reason" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                    option value="" { "— 无 —" }
                    option value="1" { "来料不良" }
                    option value="2" { "设备异常" }
                    option value="3" { "操作失误" }
                    option value="4" { "工艺问题" }
                }
            }
            // 备注
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                textarea name="remark" rows="2" placeholder="选填"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent resize-y min-h-[72px]";
            }
            // 计件工资实时计算（Hyperscript 调用 calcWage）
            div class="flex items-center justify-between p-3 px-4 bg-accent-bg rounded-md mt-2" {
                span class="text-xs text-muted" {
                    "计件工资（单价 ¥"
                    span id="rpt-price" { (crate::utils::fmt_qty(unit_price)) }
                    " × 完成量）"
                }
                span class="font-mono text-lg font-bold text-accent" id="wage-val" { "¥0.00" }
            }
            // calcWage：纯前端，按完成量 × 单价实时算（无 fetch）。单价从 #rpt-price 文本读取。
            (maud::PreEscaped(
                r#"<script>
(function(){
  var price = parseFloat(document.getElementById('rpt-price').textContent)||0;
  window.calcWage = function(){
    var done = parseFloat(document.getElementById('rpt-done').value)||0;
    document.getElementById('wage-val').textContent = '¥' + (done*price).toFixed(2);
  };
})();
</script>"#,
            ))
        }
    };
    drawer::drawer("report-drawer", "工序报工", "确认报工", "report-form", body)
}
