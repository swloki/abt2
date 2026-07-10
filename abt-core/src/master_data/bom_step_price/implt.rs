use rust_decimal::Decimal;
use sqlx::PgPool;

use super::model::*;
use super::repo::BomStepPriceRepo;
use super::service::BomStepPriceService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PgExecutor, Result, ServiceContext};

pub struct BomStepPriceServiceImpl {
    repo: BomStepPriceRepo,
    pool: PgPool,
}

impl BomStepPriceServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: BomStepPriceRepo,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl BomStepPriceService for BomStepPriceServiceImpl {
    async fn find_prices_by_product(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomStepPrice>> {
        self.repo.find_by_product(db, &product_code).await
    }

    async fn find_price(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<Option<Decimal>> {
        self.repo.find_price(db, &product_code, step_order).await
    }

    async fn upsert_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
        unit_price: Decimal,
        source_type: String,
        source_wo_id: Option<i64>,
    ) -> Result<()> {
        // 校验有工序（拒孤儿单价行）
        let exists = self
            .repo
            .operation_exists(db, &product_code, step_order)
            .await?;
        if !exists {
            return Err(DomainError::business_rule(
                "该工序不存在，无法定价；请先在 BOM 编辑页配置工序（或从工艺路线拷贝）",
            ));
        }

        // upsert + 取旧价
        let old_price = self
            .repo
            .upsert(
                db,
                &product_code,
                step_order,
                unit_price,
                ctx.operator_id,
            )
            .await?;

        // 追加 history（R-15）
        self.repo
            .insert_history(
                db,
                &product_code,
                step_order,
                old_price,
                unit_price,
                &source_type,
                source_wo_id,
                ctx.operator_id,
            )
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomStepPrice",
                    entity_id: 0,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "product_code": product_code,
                        "step_order": step_order,
                        "old_price": old_price,
                        "new_price": unit_price,
                        "source_type": source_type,
                        "source_wo_id": source_wo_id,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }
}
