use sqlx::Row;

use super::model::*;
use crate::qms::enums::*;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const COLUMNS: &str = "id, doc_number, customer_id, sales_order_id, shipping_request_id, product_id, linked_inspection_result_id, defect_description, severity, root_cause, corrective_action, status, remark, operator_id, created_at, updated_at, deleted_at";
const INSERT_COLUMNS: &str = "doc_number, customer_id, sales_order_id, shipping_request_id, product_id, linked_inspection_result_id, defect_description, severity, root_cause, corrective_action, status, remark, operator_id, created_at, updated_at, deleted_at";

fn row_to_model(row: sqlx::postgres::PgRow) -> Rma {
    Rma {
        id: row.get("id"),
        doc_number: row.get("doc_number"),
        customer_id: row.get("customer_id"),
        sales_order_id: row.get("sales_order_id"),
        shipping_request_id: row.get("shipping_request_id"),
        product_id: row.get("product_id"),
        linked_inspection_result_id: row.get("linked_inspection_result_id"),
        defect_description: row.get("defect_description"),
        severity: Severity::from_i16(row.get::<i16, _>("severity"))
            .unwrap_or(Severity::Minor),
        root_cause: row.get("root_cause"),
        corrective_action: row.get("corrective_action"),
        status: RMAStatus::from_i16(row.get::<i16, _>("status"))
            .unwrap_or(RMAStatus::Reported),
        remark: row.get("remark"),
        operator_id: row.get("operator_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    }
}

pub async fn insert(
    db: &mut sqlx::postgres::PgConnection,
    m: &Rma,
) -> anyhow::Result<i64> {
    let row = sqlx::query(
        &format!(
            "INSERT INTO rmas ({INSERT_COLUMNS}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING id"
        )
    )
    .bind(&m.doc_number)
    .bind(m.customer_id)
    .bind(m.sales_order_id)
    .bind(m.shipping_request_id)
    .bind(m.product_id)
    .bind(m.linked_inspection_result_id)
    .bind(&m.defect_description)
    .bind(m.severity.as_i16())
    .bind(&m.root_cause)
    .bind(&m.corrective_action)
    .bind(m.status.as_i16())
    .bind(&m.remark)
    .bind(m.operator_id)
    .bind(m.created_at)
    .bind(m.updated_at)
    .bind(m.deleted_at)
    .fetch_one(db)
    .await?;

    Ok(row.get("id"))
}

pub async fn find_by_id(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
) -> anyhow::Result<Option<Rma>> {
    let row = sqlx::query(
        &format!("SELECT {COLUMNS} FROM rmas WHERE id = $1 AND deleted_at IS NULL")
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

/// record_root_cause — 写入根因和纠正措施
pub async fn update_root_cause(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    root_cause: &str,
    corrective_action: &str,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE rmas
        SET root_cause = $2, corrective_action = $3, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(root_cause)
    .bind(corrective_action)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_status(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    status: i16,
    expected_status: i16,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE rmas SET status = $2, updated_at = NOW() WHERE id = $1 AND status = $3 AND deleted_at IS NULL"
    )
    .bind(id)
    .bind(status)
    .bind(expected_status)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub async fn list(
    db: &mut sqlx::postgres::PgConnection,
    filter: &RmaFilter,
    page: &PageParams,
) -> anyhow::Result<PaginatedResult<Rma>> {
    let limit = page.page_size as i64;
    let offset = page.offset() as i64;

    let where_clause = format!(
        "WHERE deleted_at IS NULL
          AND ($1::bigint IS NULL OR customer_id = $1)
          AND ($2::bigint IS NULL OR product_id = $2)
          AND ($3::smallint IS NULL OR severity = $3)
          AND ($4::smallint IS NULL OR status = $4)
          AND ($5::date IS NULL OR created_at::date >= $5)
          AND ($6::date IS NULL OR created_at::date <= $6)"
    );

    let count_sql = format!("SELECT COUNT(*) AS cnt FROM rmas {where_clause}");
    let count_row = sqlx::query(&count_sql)
        .bind(filter.customer_id)
        .bind(filter.product_id)
        .bind(filter.severity.map(|s: Severity| s.as_i16()))
        .bind(filter.status.map(|s: RMAStatus| s.as_i16()))
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&mut *db)
        .await?;
    let total: i64 = count_row.get("cnt");

    let data_sql = format!(
        "SELECT {COLUMNS} FROM rmas {where_clause}
         ORDER BY created_at DESC
         LIMIT $7 OFFSET $8"
    );
    let rows = sqlx::query(&data_sql)
        .bind(filter.customer_id)
        .bind(filter.product_id)
        .bind(filter.severity.map(|s: Severity| s.as_i16()))
        .bind(filter.status.map(|s: RMAStatus| s.as_i16()))
        .bind(filter.date_from)
        .bind(filter.date_to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *db)
        .await?;

    let items: Vec<Rma> = rows.into_iter().map(row_to_model).collect();
    Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
}
