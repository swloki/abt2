pub mod implt;
pub mod model;
pub mod service;

pub use model::*;
pub use service::SalesWorkCenterService;

use sqlx::PgPool;

/// 按需工厂：销售作业中心聚合服务（只持 PgPool，内部按需获取销售各域 + fms/master_data service）。
pub fn new_sales_work_center_service(pool: PgPool) -> impl SalesWorkCenterService {
    implt::SalesWorkCenterServiceImpl::new(pool)
}
