use sqlx::FromRow;
use crate::shared::types::Result;

use super::model::*;
use super::super::enums::*;

pub struct ProductionInspectionRepo;

impl ProductionInspectionRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateInspectionReq,
        doc_number: &str,
        operator_id: i64,
    ) -> Result<ProductionInspection> {
        let row = sqlx::query(
            r#"
            INSERT INTO production_inspections
                (doc_number, work_order_id, routing_id, product_id,
                 inspection_type, sample_qty, inspection_date, disposition,
                 result, inspector_id, qualified_qty, unqualified_qty,
                 remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id, doc_number, work_order_id, routing_id, product_id,
                      inspection_type, sample_qty, qualified_qty, unqualified_qty,
                      result, inspector_id, inspection_date, disposition,
                      remark, operator_id, created_at, updated_at
            "#,
        )
        .bind(doc_number)
        .bind(req.work_order_id)
        .bind(req.routing_id)
        .bind(req.product_id)
        .bind(req.inspection_type)
        .bind(req.sample_qty)
        .bind(req.inspection_date)
        .bind(&req.disposition)
        // defaults: result = Pass (1), inspector_id = 0, qualified_qty = 0, unqualified_qty = 0
        .bind(InspectionResultType::Pass)
        .bind(0i64)
        .bind(rust_decimal::Decimal::ZERO)
        .bind(rust_decimal::Decimal::ZERO)
        .bind(&req.remark.clone().unwrap_or_default())
        .bind(operator_id)
        .fetch_one(&mut *executor)
        .await?;

        Ok(ProductionInspection::from_row(&row)?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<ProductionInspection>> {
        let row = sqlx::query(
            r#"
            SELECT id, doc_number, work_order_id, routing_id, product_id,
                   inspection_type, sample_qty, qualified_qty, unqualified_qty,
                   result, inspector_id, inspection_date, disposition,
                   remark, operator_id, created_at, updated_at
            FROM production_inspections
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *executor)
        .await?;

        row.map(|r| ProductionInspection::from_row(&r).map_err(Into::into)).transpose()

    }

    pub async fn update_result(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        result: InspectionResultType,
        qualified_qty: rust_decimal::Decimal,
        unqualified_qty: rust_decimal::Decimal,
    ) -> Result<bool> {
        let rows = sqlx::query(
            r#"
            UPDATE production_inspections
            SET result = $2, qualified_qty = $3, unqualified_qty = $4, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(result)
        .bind(qualified_qty)
        .bind(unqualified_qty)
        .execute(&mut *executor)
        .await?;

        Ok(rows.rows_affected() > 0)
    }
}
