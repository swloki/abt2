pub mod implt;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use service::PurchaseWorkCenterService;

use sqlx::PgPool;

/// 按需工厂：采购作业中心聚合服务（只持 PgPool，内部按需获取采购各域 service）。
pub fn new_purchase_work_center_service(pool: PgPool) -> impl PurchaseWorkCenterService {
    implt::PurchaseWorkCenterServiceImpl::new(pool)
}
