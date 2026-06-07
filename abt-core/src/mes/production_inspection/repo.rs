use sqlx::FromRow;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

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
        .bind(req.remark.as_deref().unwrap_or_default())
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

    pub async fn list(
        executor: &mut sqlx::postgres::PgConnection,
        filter: &InspectionListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InspectionListItem>> {
        let offset = (page.saturating_sub(1)) * page_size;
        let mut where_clauses = vec!["1=1".to_string()];
        let mut param_idx = 0u32;
        if filter.keyword.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pi.doc_number ILIKE ${param_idx}"));
        }
        if filter.inspection_type.is_some() {
            param_idx += 1;
            where_clauses.push(format!("pi.inspection_type = ${param_idx}"));
        }
        let where_sql = where_clauses.join(" AND ");
        let limit_idx = param_idx + 1;
        let offset_idx = param_idx + 2;
        let count_sql = format!(
            "SELECT COUNT(*) FROM production_inspections pi WHERE {where_sql}"
        );
        let data_sql = format!(
            "SELECT pi.id, pi.doc_number, pi.work_order_id, \
             wo.doc_number AS work_order_doc, \
             pi.routing_id, pi.product_id, COALESCE(p.pdt_name, '') AS product_name, \
             pi.inspection_type, pi.sample_qty, pi.qualified_qty, pi.unqualified_qty, \
             pi.result, pi.inspector_id, pi.inspection_date, pi.disposition, \
             pi.remark, pi.operator_id, pi.created_at, pi.updated_at \
             FROM production_inspections pi \
             LEFT JOIN products p ON p.product_id = pi.product_id \
             LEFT JOIN work_orders wo ON wo.id = pi.work_order_id \
             WHERE {where_sql} \
             ORDER BY pi.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );
        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql));
        let mut data_q = sqlx::query(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = &filter.keyword {
            let pattern = format!("%{v}%");
            count_q = count_q.bind(pattern.clone());
            data_q = data_q.bind(pattern);
        }
        if let Some(v) = filter.inspection_type {
            count_q = count_q.bind(v);
            data_q = data_q.bind(v);
        }
        data_q = data_q.bind(page_size as i64).bind(offset as i64);
        let total: i64 = count_q.fetch_one(&mut *executor).await?;
        let rows = data_q.fetch_all(&mut *executor).await?;
        let items: Vec<InspectionListItem> = rows
            .iter()
            .filter_map(|r| InspectionListItem::from_row(r).ok())
            .collect();
        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u64).div_ceil(page_size as u64) as u32
        };
        Ok(PaginatedResult {
            items,
            total: total as u64,
            page,
            page_size,
            total_pages,
        })
    }
}