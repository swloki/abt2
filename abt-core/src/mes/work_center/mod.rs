pub mod implt;
pub mod model;
pub mod service;

pub use model::*;
pub use service::MesWorkCenterService;

use sqlx::PgPool;

/// 按需工厂：生产作业中心聚合服务（只持 PgPool，内部按需获取工单等服务）。
pub fn new_mes_work_center_service(pool: PgPool) -> impl MesWorkCenterService {
    implt::MesWorkCenterServiceImpl::new(pool)
}
