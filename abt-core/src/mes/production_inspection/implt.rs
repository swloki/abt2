use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::ProductionInspectionRepo;
use super::service::ProductionInspectionService;
use super::super::enums::InspectionResultType;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::types::PgExecutor;
use crate::shared::enums::DocumentType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct ProductionInspectionServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
    doc_seq: Arc<dyn DocumentSequenceService>,
}

impl ProductionInspectionServiceImpl {
    pub fn new(pool: PgPool, doc_seq: Arc<dyn DocumentSequenceService>) -> Self {
        Self { pool, doc_seq }
    }
}

#[async_trait]
impl ProductionInspectionService for ProductionInspectionServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionReq,
    ) -> Result<i64> {
        let doc_number = self.doc_seq.next_number(ctx, db, DocumentType::ProductionInspection)
            .await
            .unwrap_or_else(|_| format!("PI{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let inspection = ProductionInspectionRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(inspection.id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionInspection> {
        ProductionInspectionRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionInspection"))
    }

    async fn record_result(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        result: InspectionResultType,
    ) -> Result<()> {
        let inspection = ProductionInspectionRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionInspection"))?;

        // result is always set (defaults to Pass on insert), but still guard against re-recording
        if inspection.result != InspectionResultType::Pass || inspection.inspector_id != 0 {
            return Err(DomainError::BusinessRule(
                "Inspection result already recorded".to_string(),
            ));
        }

        // 根据检验结果计算合格/不合格数量
        let (qualified_qty, unqualified_qty) = match result {
            InspectionResultType::Pass => (inspection.sample_qty, rust_decimal::Decimal::ZERO),
            InspectionResultType::Fail => (rust_decimal::Decimal::ZERO, inspection.sample_qty),
            InspectionResultType::Conditional => (rust_decimal::Decimal::ZERO, inspection.sample_qty),
        };

        let updated = ProductionInspectionRepo::update_result(
            &mut *db,
            id,
            result,
            qualified_qty,
            unqualified_qty,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::not_found("ProductionInspection"));
        }

        Ok(())
    }
}
