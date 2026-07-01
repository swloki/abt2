use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::mes_order_create;
use crate::state::AppState;

// ── Typed Paths ──
//
// 工单详情页与列表页已下线（MES 作业收口到 work-center）；
// 本模块仅保留「手动创建工单」入口（无上游单据）+ 源单据搜索 API。
// 工单级操作（下达/排程/反下达/关闭/取消/报工/领料/入库）均在 work-center 就地完成。

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes/orders/create")]
pub struct OrderCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/mes/source-orders/search")]
pub struct SourceOrderSearchPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            OrderCreatePath::PATH,
            get(mes_order_create::get_order_create).post(mes_order_create::create_order),
        )
        .route(
            SourceOrderSearchPath::PATH,
            get(mes_order_create::search_source_orders),
        )
}
