use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::wms_ledger;
use crate::state::AppState;

/// 单据台账：跨类型统一查询入口（收货 / 出库 / 调拨 / 领料 / 盘点）。
/// 收货/出库/调拨/领料同源 stock_pickings（picking_type 区分），盘点独立 cycle_counts。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/ledger")]
pub struct LedgerPath;

pub fn router() -> Router<AppState> {
    Router::new().route(LedgerPath::PATH, get(wms_ledger::get_ledger_list))
}
