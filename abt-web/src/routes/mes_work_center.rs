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
#[typed_path("/admin/mes/work-center/orders/{order_id}/report-drawer")]
pub struct WcReportDrawerPath {
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

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/report")]
pub struct WcReportPath {
    pub order_id: i64,
}

/// 工序编辑（工作中心下达 drawer 内行内编辑产出品/单价/工作中心/工时/委外）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/work-center/orders/{order_id}/routings/{routing_id}/edit")]
pub struct WcRoutingEditPath {
    pub order_id: i64,
    pub routing_id: i64,
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

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(WcPath::PATH, get(mes_work_center::get_work_center))
        .route(WcDemandPath::PATH, get(mes_work_center::get_demand_card))
        .route(
            WcReleaseDrawerPath::PATH,
            get(mes_work_center::get_release_drawer),
        )
        .route(
            WcReportDrawerPath::PATH,
            get(mes_work_center::get_report_drawer),
        )
        .route(WcReleasePath::PATH, post(mes_work_center::release_order))
        .route(WcSplitMultiPath::PATH, post(mes_work_center::split_multi))
        .route(WcReportPath::PATH, post(mes_work_center::report_step))
        .route(
            WcRoutingEditPath::PATH,
            get(mes_work_center::get_wc_routing_edit).post(mes_work_center::post_wc_routing_edit),
        )
        .route(
            WcCreatePlanDrawerPath::PATH,
            get(mes_work_center::get_create_plan_drawer),
        )
        .route(WcCreatePlanPath::PATH, post(mes_work_center::create_plan))
        .route(
            WcOrderDetailModalPath::PATH,
            get(mes_work_center::get_order_detail_modal),
        )
}
