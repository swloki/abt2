use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_work_center;
use crate::state::AppState;

/// 作业中心**唯一端点**：GET（整页 / 懒加载某卡片 / 加载 drawer body），POST（执行就地操作）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center")]
pub struct WmsWorkCenterPath;

/// 盘点创建 drawer body（CycleCount tab「新建盘点单」按钮 hx-get 加载）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/cycle-counts/create-drawer")]
pub struct WcCycleCountCreateDrawerPath;

/// 盘点创建提交（drawer 内 form hx-post，返空 body + HX-Trigger: wcChanged 保 tab）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/cycle-counts/create")]
pub struct WcCycleCountCreatePath;

/// 领料创建 drawer body（Requisition tab「新建领料单」按钮 hx-get 加载）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/requisitions/create-drawer")]
pub struct WcRequisitionCreateDrawerPath;

/// 领料创建提交（drawer 内 form hx-post）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/requisitions/create")]
pub struct WcRequisitionCreatePath;

/// 调拨创建 drawer body（Transfer tab「新建调拨单」按钮 hx-get 加载）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/transfers/create-drawer")]
pub struct WcTransferCreateDrawerPath;

/// 调拨创建提交（drawer 内 form hx-post）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/transfers/create")]
pub struct WcTransferCreatePath;

/// 发货创建 drawer body（Outbound tab「新建发货单」按钮 hx-get 加载）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/shippings/create-drawer")]
pub struct WcShippingCreateDrawerPath;

/// 发货创建提交（drawer 内 form hx-post）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/shippings/create")]
pub struct WcShippingCreatePath;

/// 入库创建 drawer body（Arrival tab「新建入库单」按钮 hx-get 加载）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/stock-ins/create-drawer")]
pub struct WcStockInCreateDrawerPath;

/// 入库创建提交（drawer 内 form hx-post）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/stock-ins/create")]
pub struct WcStockInCreatePath;

/// 发货 drawer 选仓库后查询各产品可用库存（JSON）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/ship-stock-avail")]
pub struct WcShipStockAvailPath;

/// 调拨 drawer 选源仓后查询各产品可用库存（JSON，INVENTORY 权限）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/transfer-stock-avail")]
pub struct WcTransferStockAvailPath;

/// 盘点 drawer 选库位后查询该物料系统账面数量（JSON）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/cycle-counts/system-qty")]
pub struct WcCycleCountSystemQtyPath;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WmsWorkCenterPath::PATH,
            get(wms_work_center::get_wms_work_center).post(wms_work_center::post_work_center_action),
        )
        .route(
            WcCycleCountCreateDrawerPath::PATH,
            get(wms_work_center::get_cycle_count_create_drawer),
        )
        .route(
            WcCycleCountCreatePath::PATH,
            post(wms_work_center::post_cycle_count_create),
        )
        .route(
            WcRequisitionCreateDrawerPath::PATH,
            get(wms_work_center::get_requisition_create_drawer),
        )
        .route(
            WcRequisitionCreatePath::PATH,
            post(wms_work_center::post_requisition_create),
        )
        .route(
            WcTransferCreateDrawerPath::PATH,
            get(wms_work_center::get_transfer_create_drawer),
        )
        .route(
            WcTransferCreatePath::PATH,
            post(wms_work_center::post_transfer_create),
        )
        .route(
            WcShippingCreateDrawerPath::PATH,
            get(wms_work_center::get_shipping_create_drawer),
        )
        .route(
            WcShippingCreatePath::PATH,
            post(wms_work_center::post_shipping_create),
        )
        .route(
            WcStockInCreateDrawerPath::PATH,
            get(wms_work_center::get_stock_in_create_drawer),
        )
        .route(
            WcStockInCreatePath::PATH,
            post(wms_work_center::post_stock_in_create),
        )
        .route(
            WcShipStockAvailPath::PATH,
            get(wms_work_center::get_ship_stock_avail),
        )
        .route(
            WcTransferStockAvailPath::PATH,
            get(wms_work_center::get_transfer_stock_avail),
        )
        .route(
            WcCycleCountSystemQtyPath::PATH,
            get(wms_work_center::get_cycle_count_system_qty),
        )
}
