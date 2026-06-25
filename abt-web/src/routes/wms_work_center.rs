use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_work_center;
use crate::state::AppState;

/// 作业中心**唯一端点**：GET（整页 / 懒加载某卡片 / 加载 drawer body），POST（执行就地操作）。
/// 交互收敛到一个地址：卡片自洽、用 hx-select / hx-select-oob 协调多区更新（见 wms-work-center-hub.md）。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/work-center")]
pub struct WmsWorkCenterPath;

pub fn router() -> Router<AppState> {
    Router::new().route(
        WmsWorkCenterPath::PATH,
        get(wms_work_center::get_wms_work_center).post(wms_work_center::post_work_center_action),
    )
}
