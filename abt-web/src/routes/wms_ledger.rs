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

/// 行内展开：按需加载某个作业单据的明细行（Issue #225）。
/// 返回单个 `<tr class="row-detail">`，由前端 `hx-swap="afterend"` 注入到该单据行之后。
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/ledger/{id}/items")]
pub struct LedgerItemRowsPath {
    pub id: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(LedgerPath::PATH, get(wms_ledger::get_ledger_list))
        .route(LedgerItemRowsPath::PATH, get(wms_ledger::get_ledger_items))
}
