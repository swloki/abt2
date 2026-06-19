use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};

use super::model::{LowStockAlert, LowStockAlertFilter};

#[async_trait]
pub trait LowStockAlertService: Send + Sync {
    /// 检查并记录低库存预警：若 product×warehouse 的库存低于安全库存且无未确认预警，
    /// 则创建一条 Active 预警并发布 LowStockAlert 事件。返回新建预警 id（未触发则 None）。
    async fn check_and_record(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<Option<i64>>;

    /// 分页查询预警
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: LowStockAlertFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<LowStockAlert>>;

    /// 确认预警（Active → Acknowledged）
    async fn ack(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
}
