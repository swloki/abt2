pub mod implt;
pub mod model;
pub mod service;

pub use model::*;
pub use service::FmsWorkCenterService;

use sqlx::PgPool;

/// 按需工厂：财务作业中心聚合服务（只持 PgPool，内部按需获取 fms 各域 service）。
pub fn new_fms_work_center_service(pool: PgPool) -> impl FmsWorkCenterService {
    implt::FmsWorkCenterServiceImpl::new(pool)
}
