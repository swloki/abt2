//! 采购订单直收入库（取消来料通知后的采购入库闭环）。
//!
//! 职责：PO 收货即入库 + 回写 PO received_qty/状态 + 立应付台账 + 成本分录，事务内同步编排。
//! 详见 [`service::PurchaseStockInService`]。

pub mod implt;
pub mod model;
pub mod service;

pub use implt::PurchaseStockInServiceImpl;
pub use model::{PoStockInRow, ReceiveAndStockInReq};
pub use service::PurchaseStockInService;

use sqlx::postgres::PgPool;

/// 按需工厂（CLAUDE.md：struct 持 PgPool，方法内 new_xxx_service）
pub fn new_purchase_stock_in_service(pool: PgPool) -> impl PurchaseStockInService {
    PurchaseStockInServiceImpl::new(pool)
}
