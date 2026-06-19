use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{LowStockAlert, LowStockAlertFilter};
use super::repo::LowStockAlertRepo;
use super::service::LowStockAlertService;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, EventPublishRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};

pub struct LowStockAlertServiceImpl {
    pool: PgPool,
}

impl LowStockAlertServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LowStockAlertService for LowStockAlertServiceImpl {
    async fn check_and_record(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<Option<i64>> {
        let (current_qty, safety_stock) =
            LowStockAlertRepo::stock_summary(&mut *db, product_id, warehouse_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        // 未设安全库存或仍高于安全线 → 不预警
        if safety_stock <= Decimal::ZERO || current_qty >= safety_stock {
            return Ok(None);
        }

        // 已有未确认预警 → 不重复创建
        if LowStockAlertRepo::has_active(&mut *db, product_id, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
        {
            return Ok(None);
        }

        let alert = LowStockAlertRepo::insert(
            &mut *db,
            product_id,
            warehouse_id,
            current_qty,
            safety_stock,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 发布 LowStockAlert 事件（outbox 异步消费 → 待办/通知）
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::LowStockAlert,
                    aggregate_type: "LowStockAlert".to_string(),
                    aggregate_id: alert.id,
                    payload: serde_json::json!({
                        "product_id": product_id,
                        "warehouse_id": warehouse_id,
                        "current_qty": current_qty.to_string(),
                        "safety_stock": safety_stock.to_string(),
                    }),
                    idempotency_key: Some(format!("LowStockAlert:{product_id}:{warehouse_id}")),
                },
            )
            .await?;

        Ok(Some(alert.id))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: LowStockAlertFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<LowStockAlert>> {
        LowStockAlertRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn ack(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let affected = LowStockAlertRepo::ack(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if affected == 0 {
            return Err(DomainError::business_rule("预警不存在或已确认"));
        }
        Ok(())
    }
}
