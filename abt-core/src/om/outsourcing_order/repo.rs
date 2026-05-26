use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::Row;
use crate::shared::types::Result;

use super::model::{
    CreateOutsourcingOrderReq, OutsourcingMaterialItem, OutsourcingOrder, OutsourcingOrderQuery,
};
use crate::om::enums::OutsourcingStatus;
use crate::shared::types::pagination::{DataScope, PageParams};

// ---------------------------------------------------------------------------
// OutsourcingOrderRepo
// ---------------------------------------------------------------------------

pub struct OutsourcingOrderRepo;

impl OutsourcingOrderRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &CreateOutsourcingOrderReq,
        doc_number: &str,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO outsourcing_orders
                (doc_number, work_order_id, routing_id, supplier_id, product_id,
                 outsourcing_type, planned_qty, completed_qty, unit_price,
                 scheduled_date, status, virtual_warehouse_id, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(doc_number)
        .bind(req.work_order_id)
        .bind(req.routing_id)
        .bind(req.supplier_id)
        .bind(req.product_id)
        .bind(req.outsourcing_type)
        .bind(req.planned_qty)
        .bind(Decimal::ZERO)         // completed_qty
        .bind(req.unit_price)
        .bind(req.scheduled_date)
        .bind(OutsourcingStatus::Draft)
        .bind(req.virtual_warehouse_id)
        .bind(req.remark.as_deref().unwrap_or(""))
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(row.try_get("id")?)
    }

    pub async fn get_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<OutsourcingOrder>> {
        sqlx::query_as::<_, OutsourcingOrder>(
            r#"
            SELECT id, doc_number, work_order_id, routing_id, supplier_id, product_id,
                   outsourcing_type, planned_qty, completed_qty, unit_price,
                   scheduled_date, status, virtual_warehouse_id, version,
                   remark, operator_id, created_at, updated_at, deleted_at
            FROM outsourcing_orders
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await.map_err(Into::into)
    }

    pub async fn update(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        expected_version: i32,
        supplier_id: Option<i64>,
        planned_qty: Option<Decimal>,
        unit_price: Option<Decimal>,
        scheduled_date: Option<NaiveDate>,
        remark: Option<&str>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE outsourcing_orders
            SET supplier_id  = COALESCE($3, supplier_id),
                planned_qty  = COALESCE($4, planned_qty),
                unit_price   = COALESCE($5, unit_price),
                scheduled_date = COALESCE($6, scheduled_date),
                remark       = COALESCE($7, remark),
                updated_at   = NOW(),
                version      = version + 1
            WHERE id = $1 AND version = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(expected_version)
        .bind(supplier_id)
        .bind(planned_qty)
        .bind(unit_price)
        .bind(scheduled_date)
        .bind(remark)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn update_status_and_version(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        expected_version: i32,
        status: OutsourcingStatus,
        extra_set: &str,
        extra_params: &[(&str, Decimal)],
    ) -> Result<u64> {
        let sql = format!(
            r#"
            UPDATE outsourcing_orders
            SET status = $3, updated_at = NOW(), version = version + 1{extra_set}
            WHERE id = $1 AND version = $2 AND deleted_at IS NULL
            "#
        );
        let mut query = sqlx::query(&sql).bind(id).bind(expected_version).bind(status);
        for (_col, val) in extra_params {
            query = query.bind(*val);
        }
        let result = query.execute(executor).await?;
        Ok(result.rows_affected())
    }

    pub async fn update_completed_qty(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        expected_version: i32,
        status: OutsourcingStatus,
        add_qty: Decimal,
    ) -> Result<u64> {
        Self::update_status_and_version(
            executor,
            id,
            expected_version,
            status,
            ", completed_qty = completed_qty + $4",
            &[("completed_qty", add_qty)],
        )
        .await
    }

    pub async fn query(
        executor: &mut sqlx::postgres::PgConnection,
        q: &OutsourcingOrderQuery,
        page: &PageParams,
        scope: (DataScope, i64, Option<i64>),
    ) -> Result<(Vec<OutsourcingOrder>, u64)> {
        let (data_scope, operator_id, _department_id) = scope;
        let scoped = !matches!(data_scope, DataScope::All);
        let scope_clause = if scoped { "AND operator_id = $8" } else { "" };
        let where_clause = format!(
            "WHERE deleted_at IS NULL
              AND ($1::smallint IS NULL OR status = $1)
              AND ($2::bigint IS NULL OR supplier_id = $2)
              AND ($3::smallint IS NULL OR outsourcing_type = $3)
              AND ($4::bigint IS NULL OR work_order_id = $4)
              AND ($5::date IS NULL OR scheduled_date >= $5)
              AND ($6::date IS NULL OR scheduled_date <= $6)
              AND ($7::text IS NULL OR doc_number ILIKE '%' || $7 || '%')
              {scope_clause}"
        );

        let (date_start, date_end) = match q.date_range {
            Some((s, e)) => (Some(s), Some(e)),
            None => (None, None),
        };

        let limit = page.page_size as i64;
        let offset = page.offset() as i64;
        let limit_idx = if scoped { 9 } else { 8 };
        let offset_idx = limit_idx + 1;

        // Count
        let count_sql = format!("SELECT COUNT(*) AS cnt FROM outsourcing_orders {where_clause}");
        let mut count_query = sqlx::query(&count_sql)
            .bind(q.status)
            .bind(q.supplier_id)
            .bind(q.outsourcing_type)
            .bind(q.work_order_id)
            .bind(date_start)
            .bind(date_end)
            .bind(&q.keyword);
        if scoped {
            count_query = count_query.bind(operator_id);
        }
        let count_row = count_query.fetch_one(&mut *executor).await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let data_sql = format!(
            "SELECT id, doc_number, work_order_id, routing_id, supplier_id, product_id,
                    outsourcing_type, planned_qty, completed_qty, unit_price,
                    scheduled_date, status, virtual_warehouse_id, version,
                    remark, operator_id, created_at, updated_at, deleted_at
             FROM outsourcing_orders {where_clause}
             ORDER BY created_at DESC
             LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );
        let mut data_query = sqlx::query_as::<_, OutsourcingOrder>(&data_sql)
            .bind(q.status)
            .bind(q.supplier_id)
            .bind(q.outsourcing_type)
            .bind(q.work_order_id)
            .bind(date_start)
            .bind(date_end)
            .bind(&q.keyword);
        if scoped {
            data_query = data_query.bind(operator_id);
        }
        data_query = data_query.bind(limit).bind(offset);
        let rows = data_query.fetch_all(&mut *executor).await?;

        Ok((rows, total as u64))
    }
}

// ---------------------------------------------------------------------------
// OutsourcingMaterialRepo
// ---------------------------------------------------------------------------

pub struct OutsourcingMaterialRepo;

impl OutsourcingMaterialRepo {
    pub async fn insert_batch(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        items: &[OutsourcingMaterialItem],
    ) -> Result<()> {
        for item in items {
            sqlx::query(
                r#"
                INSERT INTO outsourcing_materials
                    (outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(outsourcing_id)
            .bind(item.product_id)
            .bind(item.planned_qty)
            .bind(Decimal::ZERO)
            .bind(Decimal::ZERO)
            .bind(item.unit_cost.unwrap_or(Decimal::ZERO))
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn list_by_outsourcing_id(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
    ) -> Result<Vec<super::model::OutsourcingMaterial>> {
        sqlx::query_as::<_, super::model::OutsourcingMaterial>(
            r#"
            SELECT id, outsourcing_id, product_id, planned_qty, sent_qty, returned_qty, unit_cost
            FROM outsourcing_materials
            WHERE outsourcing_id = $1
            "#,
        )
        .bind(outsourcing_id)
        .fetch_all(executor)
        .await.map_err(Into::into)
    }

    pub async fn replace_batch(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        items: &[OutsourcingMaterialItem],
    ) -> Result<()> {
        sqlx::query("DELETE FROM outsourcing_materials WHERE outsourcing_id = $1")
            .bind(outsourcing_id)
            .execute(&mut *executor)
            .await?;
        Self::insert_batch(executor, outsourcing_id, items).await
    }

    pub async fn update_sent_qty(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        product_id: i64,
        add_qty: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE outsourcing_materials SET sent_qty = sent_qty + $3 WHERE outsourcing_id = $1 AND product_id = $2",
        )
        .bind(outsourcing_id)
        .bind(product_id)
        .bind(add_qty)
        .execute(executor)
        .await?;
        Ok(())
    }
}
