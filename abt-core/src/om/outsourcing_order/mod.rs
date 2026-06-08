pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

use sqlx::PgPool;

pub use service::OutsourcingOrderService;
pub use model::{OutsourcingOrder, OutsourcingOrderQuery, OutsourcingMaterial, CreateOutsourcingOrderReq, OutsourcingMaterialItem, UpdateOutsourcingOrderReq, SendOutsourcingReq, ReceiveOutsourcingReq, ConvertToInternalReq, CancelOutsourcingReq};

pub fn new_outsourcing_order_service(pool: PgPool) -> impl OutsourcingOrderService {
    implt::OutsourcingOrderServiceImpl::new(pool)
}
