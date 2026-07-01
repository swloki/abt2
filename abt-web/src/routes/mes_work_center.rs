use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::mes_work_center;
use crate::state::AppState;

// ── Typed Paths ──
//
// 工作中心采用单端点模式：首页内联渲染 2 个 card（生产需求池 / 工单），
// 需求池 card 内含 3 个 tab（物料汇总 / 订单行明细 / 订单排期），
// tab/筛选/分页走各自 GET 端点 + hx-select="#wc-xxx-card" 局部刷新；
// 写操作 POST 广播 HX-Trigger: woChanged，相关 card 监听自刷新。
// 工序加载/编辑/删除复用既有 mes_order 端点（广播 routingChanged）。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center")]
pub struct WcPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/demand")]
pub struct WcDemandPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/release-drawer")]
pub struct WcReleaseDrawerPath {
    pub order_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/release")]
pub struct WcReleasePath {
    pub order_id: i64,
}

/// 分批：一次事务创建多批（既有 split_order 只建 1 批，故工作中心新建多批端点）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/split-multi")]
pub struct WcSplitMultiPath {
    pub order_id: i64,
}

/// 取消工单（→ Cancelled）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/cancel")]
pub struct WcCancelPath {
    pub order_id: i64,
}

/// 创建工单 drawer body（物料汇总行「创建工单」就地打开）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/create-plan-drawer/{product_id}")]
pub struct WcCreatePlanDrawerPath {
    pub product_id: i64,
}

/// 创建工单提交（调 MesDemandService.create_work_orders_from_demands）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/create-plan/{product_id}")]
pub struct WcCreatePlanPath {
    pub product_id: i64,
}

/// 销售订单详情 modal（drawer 内订单号点击就地查看，不跳转）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/order-detail-modal/{order_id}")]
pub struct WcOrderDetailModalPath {
    pub order_id: i64,
}

// ── 批次 tab：batch 维度 drawer + 操作（报工/暂停/恢复/报废/推进入库）──

/// 批次处理 drawer body（批次 tab 行尾「处理」按钮就地打开，不跳转）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/drawer")]
pub struct WcBatchDrawerPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/report")]
pub struct WcBatchReportPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/suspend")]
pub struct WcBatchSuspendPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/resume")]
pub struct WcBatchResumePath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/scrap")]
pub struct WcBatchScrapPath {
    pub batch_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/advance")]
pub struct WcBatchAdvancePath {
    pub batch_id: i64,
}

/// 批次开工（Pending → InProgress）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/start")]
pub struct WcBatchStartPath {
    pub batch_id: i64,
}

/// 批次领料提交（create_for_routing_step）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/requisition")]
pub struct WcBatchReqPath {
    pub batch_id: i64,
}

/// 批次入库 modal 表单（GET：加载入库弹窗内容）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/receipt-modal")]
pub struct WcBatchReceiptModalPath {
    pub batch_id: i64,
}

/// 批次入库提交（ProductionReceipt.create+confirm）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/receipt")]
pub struct WcBatchReceiptPath {
    pub batch_id: i64,
}

/// 批次工序缺料明细（齐套徽章展开）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/routings/{routing_id}/shortage")]
pub struct WcBatchShortagePath {
    pub batch_id: i64,
    pub routing_id: i64,
}

/// 批次收料（开工）：Pending → InProgress，复用 start_batch。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/receive")]
pub struct WcBatchReceivePath {
    pub batch_id: i64,
}

/// 批次报废 modal 表单（GET：加载报废弹窗内容）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/scrap-modal")]
pub struct WcBatchScrapModalPath {
    pub batch_id: i64,
}

/// 批次报废提交（POST：部分报废，不取消批次，仅递增 scrap_qty）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/scrap-submit")]
pub struct WcBatchScrapSubmitPath {
    pub batch_id: i64,
}

/// 批次工序报工 modal 表单（GET：加载报工弹窗内容，预填 step_no）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/batches/{batch_id}/routings/{step_no}/report-modal")]
pub struct WcBatchReportModalPath {
    pub batch_id: i64,
    pub step_no: i32,
}

/// 报工工人行（GET ?worker_id=X → 渲染一行进报工表格 tbody，worker_picker add-row 模式）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/worker-row")]
pub struct WcWorkerRowPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(WcPath::PATH, get(mes_work_center::get_work_center))
        .route(WcDemandPath::PATH, get(mes_work_center::get_demand_card))
        .route(
            WcReleaseDrawerPath::PATH,
            get(mes_work_center::get_release_drawer),
        )
        .route(WcReleasePath::PATH, post(mes_work_center::release_order))
        .route(WcSplitMultiPath::PATH, post(mes_work_center::split_multi))
        .route(WcCancelPath::PATH, post(mes_work_center::cancel_order))
        .route(
            WcCreatePlanDrawerPath::PATH,
            get(mes_work_center::get_create_plan_drawer),
        )
        .route(WcCreatePlanPath::PATH, post(mes_work_center::create_plan))
        .route(
            WcOrderDetailModalPath::PATH,
            get(mes_work_center::get_order_detail_modal),
        )
        .route(WcBatchDrawerPath::PATH, get(mes_work_center::get_batch_drawer))
        .route(WcBatchReportPath::PATH, post(mes_work_center::batch_report))
        .route(WcBatchSuspendPath::PATH, post(mes_work_center::batch_suspend))
        .route(WcBatchResumePath::PATH, post(mes_work_center::batch_resume))
        .route(WcBatchScrapPath::PATH, post(mes_work_center::batch_scrap))
        .route(WcBatchAdvancePath::PATH, post(mes_work_center::batch_advance))
        .route(WcBatchStartPath::PATH, post(mes_work_center::batch_start))
        .route(WcBatchReqPath::PATH, post(mes_work_center::batch_requisition))
        .route(
            WcBatchReceiptModalPath::PATH,
            get(mes_work_center::get_batch_receipt_modal),
        )
        .route(WcBatchReceiptPath::PATH, post(mes_work_center::batch_receipt))
        .route(
            WcBatchShortagePath::PATH,
            get(mes_work_center::get_batch_shortage),
        )
        .route(
            WcBatchReceivePath::PATH,
            post(mes_work_center::batch_receive),
        )
        .route(
            WcBatchScrapModalPath::PATH,
            get(mes_work_center::get_batch_scrap_modal),
        )
        .route(
            WcBatchScrapSubmitPath::PATH,
            post(mes_work_center::batch_scrap_submit),
        )
        .route(
            WcBatchReportModalPath::PATH,
            get(mes_work_center::get_batch_report_modal),
        )
        .route(WcWorkerRowPath::PATH, get(mes_work_center::get_worker_row))
}
