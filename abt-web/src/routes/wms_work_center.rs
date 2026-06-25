use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_work_center;
use crate::state::AppState;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center")]
pub struct WmsWorkCenterPath;

/// disclosure 懒加载：某环节的待办队列片段
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/fragments/{domain}")]
pub struct WmsWorkCenterFragmentPath {
    pub domain: String,
}

/// 拣货 drawer：GET 返回 drawer body（明细录入表单），POST 提交 record_pick_items
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/pick/{id}")]
pub struct WmsWorkCenterPickPath {
    pub id: i64,
}

/// 发货 drawer：GET 返回 drawer body（按状态分流：Picking→确认发出 / Confirmed→需先拣货），POST 提交 ship
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/ship/{id}")]
pub struct WcShipPath {
    pub id: i64,
}

/// 领料 drawer：GET 返回 drawer body（Confirmed→全量发料 / PartiallyIssued→去详情页），POST 提交 issue
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/requisition/{id}")]
pub struct WcIssuePath {
    pub id: i64,
}

/// 调拨 drawer：GET 返回 drawer body（按状态分流：Draft→调出 / InTransit→到货确认），POST 提交 dispatch/complete
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/transfer/{id}")]
pub struct WcTransferPath {
    pub id: i64,
}

/// 收货 drawer：GET 返回 drawer body（行级收货量 + 批次），POST 提交 receive
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center/receive/{id}")]
pub struct WcReceivePath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            WmsWorkCenterPath::PATH,
            get(wms_work_center::get_wms_work_center),
        )
        .route(
            WmsWorkCenterFragmentPath::PATH,
            get(wms_work_center::get_domain_fragment),
        )
        .route(
            WmsWorkCenterPickPath::PATH,
            get(wms_work_center::get_pick_drawer).post(wms_work_center::post_pick_items),
        )
        .route(
            WcShipPath::PATH,
            get(wms_work_center::get_wc_ship_drawer).post(wms_work_center::post_wc_ship),
        )
        .route(
            WcIssuePath::PATH,
            get(wms_work_center::get_wc_issue_drawer).post(wms_work_center::post_wc_issue),
        )
        .route(
            WcTransferPath::PATH,
            get(wms_work_center::get_wc_transfer_drawer)
                .post(wms_work_center::post_wc_transfer),
        )
        .route(
            WcReceivePath::PATH,
            get(wms_work_center::get_wc_receive_drawer).post(wms_work_center::post_wc_receive),
        )
}
