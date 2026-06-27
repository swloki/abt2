use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::{DefectReason, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{
    ProductionBatchService, SplitReq, StepConfirmationReq, WorkOrderRouting,
};
use abt_core::mes::production_receipt::{CreateReceiptReq, ProductionReceiptService};
use abt_core::mes::work_order::{
    HubAuditLog, HubMaterial, HubReceipts, HubReports, HubRoutingMatrix,
    MaterialAvailabilityLevel, RoutingCellStatus, WorkOrder, WorkOrderHubSummary,
    WorkOrderService,
};
use abt_core::wms::material_requisition::{CreateManualReq, MaterialRequisitionService};

use crate::components::{disclosure, drawer, icon, material_badge, product_picker, routing_picker, status_step_bar};
use crate::components::overlay::modal_shell;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::toast::{toast_response, ToastType};
use crate::routes::mes_order::{
    OrderCancelPath, OrderClosePath, OrderDetailPath, OrderListPath, OrderReceiptPath,
    OrderRequisitionPath, OrderReportPath, OrderReleasePath, OrderSchedulePath,
    OrderRoutingApplyFromRoutingPath, OrderRoutingDeletePath, OrderRoutingEditPath,
    OrderRoutingLoadRecentPath, OrderSplitPath, OrderUnreleasePath,
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

/// 紧凑日期（月-日）—— 用于报工记录等高频列表，对齐原型
fn fmt_date_short(d: chrono::NaiveDate) -> String {
    d.format("%m-%d").to_string()
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
    let batch_svc = state.production_batch_service();
    let product_svc = state.product_service();

    // 工作台聚合：一次取全（detail-header + 摘要带 + disclosure 全部数据）
    let summary = wo_svc
        .get_hub_summary(&service_ctx, &mut conn, path.id)
        .await?;

    // 工序编辑 disclosure 额外数据：整单是否已报工（控制编辑/删除/加载可见性）+ 产出品名
    let order_has_report = batch_svc
        .order_has_any_report(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or(false);
    use abt_core::master_data::product::ProductService;
    let routing_product_ids: Vec<i64> = summary
        .matrix
        .routings
        .iter()
        .filter_map(|r| r.product_id)
        .collect();
    let routing_product_names: std::collections::HashMap<i64, String> = if routing_product_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, routing_product_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, p.pdt_name))
            .collect()
    };

    let detail_path = OrderDetailPath { id: path.id }.to_string();
    let content = hub_page(&summary, &detail_path, order_has_report, &routing_product_names);
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

#[derive(Debug, serde::Deserialize)]
pub struct RoutingEditForm {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
    pub unit_price: rust_decimal::Decimal,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub work_center_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub standard_time: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub is_outsourced: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplyFromRoutingForm {
    pub routing_id: i64,
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

#[derive(Debug, serde::Deserialize)]
pub struct RequisitionForm {
    pub warehouse_id: i64,
    pub requisition_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
    /// 行项目物料 ID（按 BOM 可用性行顺序，前端逐行 name="product_id"）
    #[serde(default)]
    pub product_id: Vec<i64>,
    /// 行项目本次申请量（与 product_id 按下标对齐；空行/0 跳过）
    #[serde(default)]
    pub requested_qty: Vec<String>,
}

/// 手动创建领料单（事务包裹 + HX-Trigger 局部刷新）。
///
/// 调 `MaterialRequisitionService::create_manual`：按用户在 drawer 中按 BOM 填写的本次
/// 申请量建单（CreateManualItemReq{product_id, requested_qty}）。成功广播
/// `requisitionChanged`，物料 disclosure + 摘要带各自监听并自刷新（不 redirect）。
#[require_permission("WORK_ORDER", "update")]
pub async fn create_requisition(
    _path: OrderRequisitionPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RequisitionForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 行项目对齐收集（product_id 与 requested_qty 等长；前端按 BOM 行数对齐传参，
    // 缺失项以空串/0 视为不申请并跳过）
    let parse_qty = |s: &str| -> std::result::Result<rust_decimal::Decimal, abt_core::shared::types::DomainError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(rust_decimal::Decimal::ZERO);
        }
        trimmed.parse::<rust_decimal::Decimal>().map_err(|_| {
            abt_core::shared::types::DomainError::validation("数量格式错误")
        })
    };

    let mut items = Vec::new();
    let n = form.product_id.len().max(form.requested_qty.len());
    for i in 0..n {
        let product_id = form.product_id.get(i).copied();
        let raw = form.requested_qty.get(i).map(String::as_str).unwrap_or("");
        let qty = parse_qty(raw)?;
        // 跳过无 product_id 或申请量 ≤ 0 的行
        if let Some(pid) = product_id
            && qty > rust_decimal::Decimal::ZERO
        {
            items.push(abt_core::wms::material_requisition::CreateManualItemReq {
                product_id: pid,
                requested_qty: qty,
            });
        }
    }

    if items.is_empty() {
        return Err(abt_core::shared::types::DomainError::validation(
            "至少填写一项领料申请量",
        )
        .into());
    }

    let req = CreateManualReq {
        warehouse_id: form.warehouse_id,
        requisition_date: form.requisition_date,
        remark: form.remark,
        items,
    };
    let req_svc = state.material_requisition_service();
    req_svc.create_manual(&service_ctx, &mut tx, req).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 广播 requisitionChanged：物料 disclosure + 摘要带各自监听并自刷新
    Ok(([("HX-Trigger", "requisitionChanged")], Html(String::new())))
}

#[derive(Debug, serde::Deserialize)]
pub struct ReceiptForm {
    pub batch_id: i64,
    pub warehouse_id: i64,
    pub received_qty: String,
    pub receipt_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 创建并确认入库单（事务包裹 + HX-Trigger 局部刷新）。
///
/// 调 `ProductionReceiptService::create` 建单，紧接 `confirm(id)`：触发倒冲 + 成本 +
/// FQC 门控。FQC 未通过/倒冲失败时 confirm 返回 `DomainError`，事务回滚，错误经 `?`
/// 上抛回 drawer（drawer 的 hx-on 在请求失败时保留表单不关闭，前端可看到错误提示）。
/// 成功广播 `receiptChanged`，入库 disclosure + 摘要带各自监听并自刷新（不 redirect）。
#[require_permission("WORK_ORDER", "update")]
pub async fn create_receipt(
    path: OrderReceiptPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReceiptForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    let received_qty = form
        .received_qty
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| abt_core::shared::types::DomainError::validation("入库数量格式错误"))?;
    if received_qty <= rust_decimal::Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::validation("入库数量必须大于 0").into());
    }

    // 取工单产出产品 ID（入库产品 = 工单 product_id）
    let wo_svc = state.work_order_service();
    let order = wo_svc
        .find_by_id(&service_ctx, &mut tx, path.order_id)
        .await?;

    let rcpt_svc = state.production_receipt_service();
    let req = CreateReceiptReq {
        work_order_id: path.order_id,
        batch_id: Some(form.batch_id),
        product_id: order.product_id,
        received_qty,
        warehouse_id: form.warehouse_id,
        zone_id: None,
        bin_id: None,
        receipt_date: form.receipt_date,
        remark: form.remark,
    };
    let receipt_id = rcpt_svc.create(&service_ctx, &mut tx, req).await?;
    // confirm：倒冲 + FQC 门控；失败回滚整事务（含 create 的插入）
    rcpt_svc
        .confirm(&service_ctx, &mut tx, receipt_id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 广播 receiptChanged：入库 disclosure + 摘要带各自监听并自刷新
    Ok(([("HX-Trigger", "receiptChanged")], Html(String::new())))
}

/// 工序级排程（事务包裹 + Toast 反馈）。
///
/// 调 `WorkOrderService::schedule`：按 BOM 工艺路径逐工序计算时长
/// （setup + cleanup + cycle × std_time × 100 / 效率），在工作中心日历上找可用时段并建
/// booking。schedule 非幂等（重复调用追加 booking），故前端 `hx-confirm` 提示。结果
/// （bookings_created / warnings）按分支拼 Toast：全成功→Success；部分警告→Warning 汇总；
/// 未创建→Warning 原因。
#[require_permission("WORK_ORDER", "update")]
pub async fn schedule_order(
    path: OrderSchedulePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, claims, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    let result = state
        .work_order_service()
        .schedule(&service_ctx, &mut tx, path.order_id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // Toast 文案为纯文本（render_single_toast 经 maud 转义 HTML）
    let (msg, ttype) = if result.bookings_created > 0 && result.warnings.is_empty() {
        (
            format!(
                "排程完成：已创建 {} 条工序预约，可在「排程看板」查看",
                result.bookings_created
            ),
            ToastType::Success,
        )
    } else if result.bookings_created > 0 {
        (
            format!(
                "排程完成：创建 {} 条预约；{} 项警告：{}",
                result.bookings_created,
                result.warnings.len(),
                result.warnings.join("；")
            ),
            ToastType::Warning,
        )
    } else {
        (
            format!("未创建工序预约：{}", result.warnings.join("；")),
            ToastType::Warning,
        )
    };
    Ok(toast_response(claims.sub, msg, ttype))
}

// ============================================================================
// 工作台渲染（hub_page + 各 disclosure body + drawer）
// ============================================================================

/// 工作台整页：detail-header（状态步骤条 + 物料徽章 + 来源链 + 进度条）+ 摘要带 +
/// 6 个 disclosure + 拆批/报工 drawer。
///
/// `detail_path` — `OrderDetailPath{id}` 字符串，供 disclosure 局部刷新 hx-get 复用。
fn hub_page(
    summary: &WorkOrderHubSummary,
    detail_path: &str,
    order_has_report: bool,
    routing_product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let order = &summary.order;
    // 工序就绪门控：每道工序都配齐产出品 + 单价(>0) 才解锁批次/报工/入库
    let routing_ready = !summary.matrix.routings.is_empty()
        && summary.matrix.routings.iter().all(|r| {
            r.product_id.is_some()
                && r.unit_price.map_or(false, |p| p > rust_decimal::Decimal::ZERO)
        });
    let routing_missing = summary
        .matrix
        .routings
        .iter()
        .filter(|r| {
            r.product_id.is_none()
                || r.unit_price.map_or(true, |p| p <= rust_decimal::Decimal::ZERO)
        })
        .count();
    let matrix_sum = if routing_ready { Some(batch_summary(&summary.matrix)) } else { None };
    let report_sum = if routing_ready { Some(report_summary(&summary.reports)) } else { None };
    let receipt_sum = if routing_ready { Some(receipt_summary(&summary.receipts)) } else { None };
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
            // 工序定义（产出品/单价编辑 — Draft/Released/InProduction 且整单未报工可改）
            (disclosure::disclosure(
                "d-routing",
                "工序定义",
                icon::tool_icon("w-4 h-4"),
                Some(&format!("{} 工序", summary.matrix.routings.len())),
                false,
                None,
                body_routing(&summary.matrix, summary.order.id, order_has_report, routing_product_names),
                detail_path,
                Some("routingChanged"),
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
                requisition_action(order),
                body_material(&summary.material),
                detail_path,
                Some("requisitionChanged"),
            ))
            // ③ 批次 × 工序矩阵
            (disclosure::disclosure(
                "d-matrix",
                "批次进度",
                icon::grid_4_icon("w-4 h-4"),
                matrix_sum.as_deref(),
                false,
                if routing_ready { can_split_action(order) } else { None },
                if routing_ready { body_matrix(&summary.matrix, order.id) } else { routing_locked_hint(routing_missing) },
                detail_path,
                if routing_ready { Some("batchChanged") } else { None },
            ))
            // ④ 报工记录（报废异常染色）
            (disclosure::disclosure(
                "d-report",
                "报工记录",
                icon::clipboard_list_icon("w-4 h-4"),
                report_sum.as_deref(),
                if routing_ready { summary.reports.total_defect > rust_decimal::Decimal::ZERO } else { false },
                if routing_ready { report_action(order) } else { None },
                if routing_ready { body_reports(&summary.reports) } else { routing_locked_hint(routing_missing) },
                detail_path,
                if routing_ready { Some("reportChanged") } else { None },
            ))
            // ⑤ 入库 & 质检
            (disclosure::disclosure(
                "d-rcpt",
                "入库 & 质检",
                icon::package_icon("w-4 h-4"),
                receipt_sum.as_deref(),
                false,
                if routing_ready { receipt_action(order) } else { None },
                if routing_ready { body_receipts(&summary.receipts) } else { routing_locked_hint(routing_missing) },
                detail_path,
                if routing_ready { Some("receiptChanged") } else { None },
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
            // 领料 drawer
            (requisition_drawer(&summary.material, order))
            // 入库 drawer
            (receipt_drawer(&summary.matrix, order))
            // 工序编辑 drawer + 产出品/工艺路径 picker modal
            (routing_edit_drawer(order))
            (product_picker::product_picker_modal(
                "routing-product-modal",
                "routing-product-id",
                "routing-product-display",
            ))
            (routing_picker::routing_picker_modal(
                "routing-picker-modal",
                "routing-id-hidden",
                "routing-name-display",
            ))
            form id="routing-apply-form"
                hx-post=({ OrderRoutingApplyFromRoutingPath { order_id: order.id }.to_string() })
                hx-trigger="routingSelected from:body"
                hx-swap="none"
            {
                input type="hidden" name="routing_id" id="routing-id-hidden" {};
                // routing_picker_modal 选中后会 put 工艺名到此元素，需存在（隐藏即可）
                span id="routing-name-display" class="hidden" {};
            }
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
        div class="detail-header bg-bg border border-border-soft rounded-lg p-6 mb-4 shadow-[var(--shadow-card)]" {
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
            div class="flex items-center gap-3 mb-5 flex-wrap" {
                (material_badge::material_badge(&summary.material_availability, "d-mat"))
                (source_trace(summary))
            }
            // 进度条
            div {
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
    let schedule_path = OrderSchedulePath { order_id: order.id }.to_string();
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
        // 排程 + 拆批 + 反下达 + 关闭（Released/InProduction）
        @if matches!(order.status, Released | InProduction) {
            button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150"
                hx-post=(&schedule_path)
                hx-confirm="对此工单进行工序级排程（按工作中心日历/产能/时段冲突计算工序时长），将在排程看板创建工作中心预约。重复排程会追加新预约，请确认。"
                hx-disabled-elt="this"
            { (icon::calendar_icon("w-4 h-4")) "排程" }
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
    modal_shell("unrelease-dialog", "z-[1000]", html! {
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
        })
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
            class="stat-strip flex bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-xs)] overflow-hidden"
            hx-get=(OrderDetailPath { id: summary.order.id }.to_string())
            hx-trigger="batchChanged from:body, reportChanged from:body, receiptChanged from:body"
            hx-select="#stat-strip"
            hx-swap="outerHTML"
            hx-disinherit="hx-select"
        {
            // 完工入库（点击展开 d-info）
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 border-r border-border-soft cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                _="on click call openAndScroll('d-info')"
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
                _="on click call openAndScroll('d-matrix')"
            {
                span class="font-mono text-xl font-bold text-fg tabular-nums leading-tight" {
                    (summary.source_chain.batch_count)
                }
                span class="text-xs text-muted font-medium" { "批次" }
            }
            // FQC（点击展开 d-rcpt）
            div class="ss-item flex-1 px-5 py-4 flex flex-col gap-0.5 cursor-pointer hover:bg-surface-raised transition-colors duration-150"
                _="on click call openAndScroll('d-rcpt')"
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
                            // 子文本规则（对齐原型）：
                            //   - done + 报废>0：显示「报废N」
                            //   - active + 非末道工序：显示「completed/planned」进度比
                            //   - active + 末道工序：显示「已入库」状态描述
                            @for (idx, cell) in row.cells.iter().enumerate() {
                                @let is_last_step = idx + 1 == row.cells.len();
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
                                        @if matches!(cell.status, RoutingCellStatus::Done) && cell.defect_qty > rust_decimal::Decimal::ZERO {
                                            span class="block text-[10px] font-medium opacity-75 mt-0.5" {
                                                "报废" (crate::utils::fmt_qty(cell.defect_qty))
                                            }
                                        } @else if matches!(cell.status, RoutingCellStatus::Active) {
                                            span class="block text-[10px] font-medium opacity-75 mt-0.5" {
                                                @if is_last_step {
                                                    "已入库"
                                                } @else {
                                                    (crate::utils::fmt_qty(cell.completed_qty)) "/" (crate::utils::fmt_qty(cell.planned_qty))
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
        // 筛选栏（纯前端）：搜索报工人/批次号 + 工序 + 班组。
        // 选项由 initReportFilter 从 tbody 行的 data-* 动态去重填充；filterReports 按三条件筛选。
        div class="flex gap-2 mt-4 mb-2 flex-wrap"
            _="on load call initReportFilter() then on input or change call filterReports()"
        {
            input id="rpt-kw" class="w-[200px] px-2.5 py-1.5 border border-border rounded-sm text-sm bg-bg outline-none focus:border-accent" placeholder="搜索 报工人 / 批次号";
            select id="rpt-op" class="px-2.5 py-1.5 border border-border rounded-sm text-sm bg-bg outline-none focus:border-accent" {}
            select id="rpt-team" class="px-2.5 py-1.5 border border-border rounded-sm text-sm bg-bg outline-none focus:border-accent" {}
        }
        table class="w-full border-collapse mt-4" {
            thead {
                tr {
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "时间" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "批次" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "工序" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "完成" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "报废" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "报工人" }
                    th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "班组" }
                }
            }
            tbody {
                @for r in &reports.items {
                    tr data-worker=(r.worker_name.as_str())
                       data-batch=(r.batch_no.as_str())
                       data-op=(r.op_name.as_str())
                       data-team=(r.team_label.as_deref().unwrap_or(""))
                    {
                        td class="py-3 px-3 text-sm font-mono tabular-nums border-b last:border-b-0 border-border-soft" {
                            (fmt_date_short(r.report_date))
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

fn requisition_action(order: &WorkOrder) -> Option<Markup> {
    if matches!(
        order.status,
        WorkOrderStatus::Released | WorkOrderStatus::InProduction
    ) {
        Some(disclosure::di_action("申请领料", "add .open to #requisition-drawer"))
    } else {
        None
    }
}

fn receipt_action(order: &WorkOrder) -> Option<Markup> {
    if matches!(
        order.status,
        WorkOrderStatus::Released | WorkOrderStatus::InProduction
    ) {
        Some(disclosure::di_action("入库", "add .open to #receipt-drawer"))
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
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent resize-y min-h-[72px]" {}
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

/// 领料 drawer：目标仓库/期望日期/备注 + 按 BOM 物料明细（每项本次申请量 input）。
///
/// 行项来源：`material.availability.lines`（product_id / product_code / product_name /
/// required_qty）。前端按行顺序生成 name="product_id" 与 name="requested_qty"，handler
/// 按下标对齐收集（空行/0 跳过）。
fn requisition_drawer(material: &HubMaterial, order: &WorkOrder) -> Markup {
    let req_path = OrderRequisitionPath { order_id: order.id }.to_string();
    let body = html! {
        form id="requisition-form" hx-post=(&req_path) hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#requisition-drawer').classList.remove('open')"
        {
            // 目标仓库
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "目标仓库 ID" }
                input name="warehouse_id" type="number" required min="1"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono";
            }
            // 期望日期
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "期望领料日期" }
                input name="requisition_date" type="date" required
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    value=(chrono::Local::now().format("%Y-%m-%d").to_string());
            }
            // 按 BOM 物料明细
            div class="mb-4" {
                div class="text-xs text-muted font-medium mb-2" { "按 BOM 物料明细（填写本次申请量，留空跳过）" }
                @if material.availability.lines.is_empty() {
                    p class="text-sm text-muted text-center py-4" { "暂无 BOM 物料明细" }
                } @else {
                    div class="flex flex-col gap-2" {
                        @for line in &material.availability.lines {
                            div class="flex items-center gap-2 px-3 py-2 bg-surface border border-border-soft rounded-sm" {
                                input type="hidden" name="product_id" value=(line.product_id);
                                div class="flex-1 min-w-0" {
                                    div class="text-sm text-fg truncate" { (line.product_name) }
                                    div class="text-[11px] text-muted font-mono" {
                                        (line.product_code) " · 需求 " (crate::utils::fmt_qty(line.required_qty))
                                        " · ATP " (crate::utils::fmt_qty(line.atp))
                                    }
                                }
                                input name="requested_qty" type="number" min="0" step="any"
                                    placeholder="0"
                                    class="w-[96px] px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono tabular-nums text-right";
                            }
                        }
                    }
                }
            }
            // 备注
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                textarea name="remark" rows="2" placeholder="选填"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent resize-y min-h-[72px]" {}
            }
            div class="flex gap-2 p-3 bg-accent-bg rounded-md text-xs text-fg-2 items-start" {
                (icon::info_icon("w-[15px] h-[15px] shrink-0 mt-0.5"))
                span { "提交后创建领料单（草稿态），需在 WMS 领料单列表中确认并发料后扣减库存。" }
            }
        }
    };
    drawer::drawer("requisition-drawer", "申请领料", "确认申请", "requisition-form", body)
}

/// 入库 drawer：批次/入库数量/目标仓库/日期/备注。提交即 create + confirm
/// （倒冲 + FQC 门控）。FQC 未过或倒冲失败由 handler 返回错误，drawer 保留表单。
fn receipt_drawer(matrix: &HubRoutingMatrix, order: &WorkOrder) -> Markup {
    let rcpt_path = OrderReceiptPath { order_id: order.id }.to_string();
    let remaining = (order.planned_qty - order.completed_qty).max(rust_decimal::Decimal::ZERO);
    let body = html! {
        form id="receipt-form" hx-post=(&rcpt_path) hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#receipt-drawer').classList.remove('open')"
        {
            // 生产批次
            div class="mb-4" {
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
            // 入库数量 + 目标仓库
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "入库数量" }
                    input name="received_qty" type="number" min="0" step="any" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono";
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "目标仓库 ID" }
                    input name="warehouse_id" type="number" min="1" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono";
                }
            }
            // 工单剩余可入库量（只读展示）
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "工单剩余可入库量" }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted font-mono tabular-nums"
                    readonly value=(format!("{} 件", crate::utils::fmt_qty(remaining)));
            }
            // 入库日期
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "入库日期" }
                input name="receipt_date" type="date" required
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                    value=(chrono::Local::now().format("%Y-%m-%d").to_string());
            }
            // 备注
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                textarea name="remark" rows="2" placeholder="选填"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent resize-y min-h-[72px]" {}
            }
            div class="flex gap-2 p-3 bg-accent-bg rounded-md text-xs text-fg-2 items-start" {
                (icon::info_icon("w-[15px] h-[15px] shrink-0 mt-0.5"))
                span { "确认后立即建单并执行入库：触发倒冲扣料 + 成本结转 + FQC 门控。若工单含报检工序且 FQC 未通过，入库将被拒绝。" }
            }
        }
    };
    drawer::drawer("receipt-drawer", "完工入库", "确认入库", "receipt-form", body)
}

// ============================================================================
// 工序定义（产出品/单价编辑）— disclosure body + 行 + 编辑 drawer + handlers
// ============================================================================

/// 工序未就绪时，批次/报工/入库 disclosure 的锁定提示（替代其 body）
fn routing_locked_hint(missing: usize) -> Markup {
    html! {
        div class="mt-4 p-4 rounded-md bg-warn-bg border border-warn/30" {
            div class="flex items-center gap-2 text-warn font-medium text-sm mb-1.5" {
                (icon::alert_triangle_icon("w-4 h-4"))
                "工序定义未完成"
            }
            p class="text-sm text-fg-2 m-0 leading-relaxed" {
                "有 " (missing) " 道工序尚未配置产出品或计件单价。"
                "请在上方「工序定义」完成配置后，批次流转、报工、入库才会可用。"
            }
        }
    }
}

/// 工序定义 disclosure body：工序表（产出品/单价/操作）+ 加载按钮（整单未报工时）
fn body_routing(
    matrix: &HubRoutingMatrix,
    order_id: i64,
    order_has_report: bool,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        div class="mt-4" {
            @if !order_has_report {
                div class="flex justify-end gap-2 mb-3" {
                    button
                        class="inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-xs font-medium cursor-pointer transition-all duration-150"
                        type="button"
                        title="选择工艺路径，用其工序步骤替换当前工序"
                        _="on click add .is-open to #routing-picker-modal"
                    { (icon::download_icon("w-3.5 h-3.5")) "从工艺路径加载" }
                    button
                        class="inline-flex items-center gap-1.5 py-1.5 px-3 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-xs font-medium cursor-pointer transition-all duration-150"
                        hx-post=({ OrderRoutingLoadRecentPath { order_id }.to_string() })
                        hx-swap="none"
                        hx-confirm="从最近同路径工单批量加载产出品？"
                    { (icon::copy_icon("w-3.5 h-3.5")) "从最近工单加载" }
                }
            }
            div class="overflow-x-auto" {
                table class="w-full border-collapse" {
                    thead {
                        tr {
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft w-12" { "序号" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "工序" }
                            th class="text-left text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "产出品" }
                            th class="text-right text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "计件单价" }
                            th class="text-right text-[11px] font-semibold text-muted py-2 px-3 border-b border-border-soft" { "操作" }
                        }
                    }
                    tbody {
                        @for r in &matrix.routings {
                            (routing_row_fragment(r, order_id, order_has_report, product_names))
                        }
                        @if matrix.routings.is_empty() {
                            tr { td colspan="5" class="text-center text-muted text-sm py-8" { "暂无工序" } }
                        }
                    }
                }
            }
            @if order_has_report {
                p class="text-[11px] text-muted mt-2" { "工单已有报工记录，工序已锁定不可修改。" }
            }
        }
    }
}

/// 工序单行：序号/工序/产出品/单价/操作（编辑+删除，整单未报工时可改）
fn routing_row_fragment(
    r: &WorkOrderRouting,
    order_id: i64,
    order_has_report: bool,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let pname = r.product_id.and_then(|pid| product_names.get(&pid).cloned());
    html! {
        tr id=(format!("routing-row-{}", r.id)) {
            td class="py-2.5 px-3 text-sm font-mono tabular-nums text-muted border-b border-border-soft" { (r.step_no) }
            td class="py-2.5 px-3 text-sm font-medium text-fg border-b border-border-soft" { (r.process_name.as_str()) }
            td class="py-2.5 px-3 text-sm border-b border-border-soft" {
                @if let Some(pn) = pname.as_deref() {
                    span class="inline-block max-w-[200px] truncate align-bottom" title=(pn) { (pn) }
                } @else if let Some(pid) = r.product_id {
                    span class="text-muted font-mono" { "#" (pid) }
                } @else {
                    span class="text-muted" { "—" }
                }
            }
            td class="py-2.5 px-3 text-sm font-mono tabular-nums text-right border-b border-border-soft" {
                @if let Some(p) = r.unit_price {
                    "¥" (crate::utils::fmt_qty(p))
                } @else {
                    span class="text-muted" { "—" }
                }
            }
            td class="py-2.5 px-3 text-right whitespace-nowrap border-b border-border-soft" {
                @if !order_has_report {
                    button
                        class="text-muted hover:text-accent cursor-pointer border-none bg-transparent p-1 align-middle"
                        title="编辑产出品/单价"
                        hx-get=({ OrderRoutingEditPath { order_id, routing_id: r.id }.to_string() })
                        hx-target="#routing-edit-drawer-body"
                        hx-swap="innerHTML"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #routing-edit-drawer"
                    { (icon::edit_icon("w-4 h-4")) }
                    button
                        class="text-muted hover:text-danger cursor-pointer border-none bg-transparent p-1 ml-1 align-middle"
                        title="删除该工序"
                        hx-post=({ OrderRoutingDeletePath { order_id, routing_id: r.id }.to_string() })
                        hx-confirm="删除该工序并重排后续工序号？"
                        hx-swap="none"
                        hx-disabled-elt="this"
                    { (icon::trash_icon("w-4 h-4")) }
                } @else {
                    span class="text-muted text-xs" { "已锁定" }
                }
            }
        }
    }
}

/// 工序编辑 drawer（body 由 get_routing_edit 异步填入 #routing-edit-drawer-body）
fn routing_edit_drawer(_order: &WorkOrder) -> Markup {
    let body = html! {
        div id="routing-edit-drawer-body" class="text-sm text-muted text-center py-10" {
            "点击工序的编辑按钮修改产出品与计件单价"
        }
    };
    drawer::drawer("routing-edit-drawer", "编辑工序", "保存", "routing-edit-form", body)
}

/// 工序编辑表单：产出品 picker + 工作中心 + 计件单价 + 标准工时 + 委外
fn routing_edit_form(
    work_order_id: i64,
    routing_id: i64,
    r: &WorkOrderRouting,
    product_name: &str,
    work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
) -> Markup {
    html! {
        form id="routing-edit-form"
            hx-post=({ OrderRoutingEditPath { order_id: work_order_id, routing_id }.to_string() })
            hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#routing-edit-drawer').classList.remove('open')"
        {
            input type="hidden" name="product_id" id="routing-product-id"
                value=(r.product_id.map(|p| p.to_string()).unwrap_or_default()) {};
            div class="mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1.5" { "产出品" }
                div class="flex gap-2" {
                    span id="routing-product-display"
                        class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-surface inline-flex items-center min-h-[38px]"
                    {
                        @if product_name.is_empty() {
                            span class="text-muted" { "点击右侧选择产出品…" }
                        } @else {
                            (product_name)
                        }
                    }
                    button type="button"
                        class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg-2 cursor-pointer hover:bg-surface"
                        _="on click add .is-open to #routing-product-modal"
                    { "选择" }
                }
            }
            div class="mb-4" {
                label class="block text-xs font-medium text-fg-2 mb-1.5" { "工作中心" }
                select name="work_center_id"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                {
                    option value="" { "—" }
                    @for wc in work_centers {
                        option value=(wc.id) selected[r.work_center_id == Some(wc.id)] { (wc.name) }
                    }
                }
            }
            div class="grid grid-cols-2 gap-3 mb-4" {
                div {
                    label class="block text-xs font-medium text-fg-2 mb-1.5" { "计件单价（元/件）" }
                    input name="unit_price" type="number" step="any" required
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                        value=(r.unit_price.map(|p| p.to_string()).unwrap_or_default());
                }
                div {
                    label class="block text-xs font-medium text-fg-2 mb-1.5" { "标准工时（小时）" }
                    input name="standard_time" type="number" step="any"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                        value=(r.standard_time.map(|t| t.to_string()).unwrap_or_default());
                }
            }
            div class="mb-2" {
                label class="flex items-center gap-2 cursor-pointer" {
                    input type="checkbox" name="is_outsourced" value="true"
                        class="w-4 h-4 accent-accent" checked[r.is_outsourced] {};
                    span class="text-sm text-fg-2 select-none" { "委外工序" }
                }
            }
        }
    }
}

/// 解析单个产出品名（无则 #id 或空串）
async fn resolve_product_name(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    pid: Option<i64>,
) -> String {
    use abt_core::master_data::product::ProductService;
    match pid {
        Some(id) => state
            .product_service()
            .get_by_ids(ctx, db, vec![id])
            .await
            .ok()
            .and_then(|v| v.into_iter().next())
            .map(|p| p.pdt_name)
            .unwrap_or_else(|| format!("#{}", id)),
        None => String::new(),
    }
}

/// GET：返回编辑表单到 #routing-edit-drawer-body
#[require_permission("WORK_ORDER", "update")]
pub async fn get_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    use abt_core::master_data::work_center::WorkCenterService;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    let routing = routings
        .iter()
        .find(|r| r.id == path.routing_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
    let pname = resolve_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
    let work_centers = state
        .work_center_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    Ok(Html(
        html! {
            (routing_edit_form(path.order_id, path.routing_id, routing, &pname, &work_centers))
        }
        .into_string(),
    ))
}

/// POST：保存产出品/单价 → 广播 routingChanged（d-routing 自刷新），失败 OOB 回填表单+错误
#[require_permission("WORK_ORDER", "update")]
pub async fn post_routing_edit(
    path: OrderRoutingEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingEditForm>,
) -> Result<impl IntoResponse> {
    use abt_core::master_data::work_center::WorkCenterService;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    match svc
        .update_routing(
            &service_ctx,
            &mut conn,
            path.order_id,
            path.routing_id,
            form.product_id,
            form.unit_price,
            form.work_center_id,
            form.standard_time,
            form.is_outsourced,
        )
        .await
    {
        Ok(_updated) => Ok(([("HX-Trigger", "routingChanged")], Html(String::new()))),
        Err(e) => {
            // 失败：OOB 回填表单到 drawer body（保留抽屉）+ 顶部展示错误，不静默丢弃
            let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
            let routing = routings
                .iter()
                .find(|r| r.id == path.routing_id)
                .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
            let pname = resolve_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
            let work_centers = state
                .work_center_service()
                .list_active(&service_ctx, &mut conn)
                .await
                .unwrap_or_default();
            Ok((
                [("HX-Trigger", "")],
                Html(html! {
                    div hx-swap-oob="innerHTML:#routing-edit-drawer-body" {
                        div class="mb-3 p-2.5 rounded-sm bg-danger-bg text-danger text-xs" {
                            (format!("保存失败：{e}"))
                        }
                        (routing_edit_form(path.order_id, path.routing_id, routing, &pname, &work_centers))
                    }
                }.into_string()),
            ))
        }
    }
}

/// 删除工序 → 广播 routingChanged
#[require_permission("WORK_ORDER", "update")]
pub async fn delete_routing(
    path: OrderRoutingDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state
        .production_batch_service()
        .delete_routing(&service_ctx, &mut conn, path.order_id, path.routing_id)
        .await?;
    Ok(([("HX-Trigger", "routingChanged")], Html(String::new())))
}

/// 从工艺路径加载工序模板 → 广播 routingChanged（d-routing 自刷新）
#[require_permission("WORK_ORDER", "update")]
pub async fn post_apply_from_routing(
    path: OrderRoutingApplyFromRoutingPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ApplyFromRoutingForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state
        .production_batch_service()
        .load_routings_from_template(&service_ctx, &mut conn, path.order_id, form.routing_id)
        .await?;
    Ok(([("HX-Trigger", "routingChanged")], Html(String::new())))
}

/// 从最近同路径工单加载产出品 → 广播 routingChanged
#[require_permission("WORK_ORDER", "update")]
pub async fn load_routings_from_recent(
    path: OrderRoutingLoadRecentPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state
        .production_batch_service()
        .load_routings_from_recent(&service_ctx, &mut conn, path.order_id)
        .await?;
    Ok(([("HX-Trigger", "routingChanged")], Html(String::new())))
}
