use sqlx::Row;
use crate::shared::types::Result;

use super::model::*;
use crate::qms::enums::*;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const COLUMNS: &str = "id, doc_number, spec_id, source_type, source_id, inspection_type, batch_no, sample_qty, qualified_qty, unqualified_qty, result, check_results, inspector_id, inspection_date, status, operator_id, created_at, updated_at, deleted_at";
const INSERT_COLUMNS: &str = "doc_number, spec_id, source_type, source_id, inspection_type, batch_no, sample_qty, qualified_qty, unqualified_qty, result, check_results, inspector_id, inspection_date, status, operator_id, created_at, updated_at, deleted_at";

fn row_to_model(row: sqlx::postgres::PgRow) -> InspectionResult {
    let check_results_val: serde_json::Value = row.get("check_results");
    InspectionResult {
        id: row.get("id"),
        doc_number: row.get("doc_number"),
        spec_id: row.get("spec_id"),
        source_type: InspectionSourceType::from_i16(row.get::<i16, _>("source_type"))
            .unwrap_or(InspectionSourceType::ArrivalNotice),
        source_id: row.get("source_id"),
        inspection_type: InspectionType::from_i16(row.get::<i16, _>("inspection_type"))
            .unwrap_or(InspectionType::Iqc),
        batch_no: row.get("batch_no"),
        sample_qty: row.get("sample_qty"),
        qualified_qty: row.get("qualified_qty"),
        unqualified_qty: row.get("unqualified_qty"),
        result: InspectionResultType::from_i16(row.get::<i16, _>("result"))
            .unwrap_or(InspectionResultType::Pass),
        check_results: serde_json::from_value(check_results_val).unwrap_or_default(),
        inspector_id: row.get("inspector_id"),
        inspection_date: row.get("inspection_date"),
        status: InspectionStatus::from_i16(row.get::<i16, _>("status"))
            .unwrap_or(InspectionStatus::Pending),
        operator_id: row.get("operator_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    }
}

pub async fn insert(
    db: &mut sqlx::postgres::PgConnection,
    m: &InspectionResult,
) -> Result<i64> {
    let check_results_json = serde_json::to_value(&m.check_results)?;

    let row = sqlx::query(
        &format!(
            "INSERT INTO inspection_results ({INSERT_COLUMNS}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) RETURNING id"
        )
    )
    .bind(&m.doc_number)
    .bind(m.spec_id)
    .bind(m.source_type.as_i16())
    .bind(m.source_id)
    .bind(m.inspection_type.as_i16())
    .bind(&m.batch_no)
    .bind(m.sample_qty)
    .bind(m.qualified_qty)
    .bind(m.unqualified_qty)
    .bind(m.result.as_i16())
    .bind(check_results_json)
    .bind(m.inspector_id)
    .bind(m.inspection_date)
    .bind(m.status.as_i16())
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
) -> Result<Option<InspectionResult>> {
    let row = sqlx::query(
        &format!("SELECT {COLUMNS} FROM inspection_results WHERE id = $1 AND deleted_at IS NULL")
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

pub async fn find_by_source(
    db: &mut sqlx::postgres::PgConnection,
    source_type: i16,
    source_id: i64,
    inspection_type: i16,
) -> Result<Option<InspectionResult>> {
    let row = sqlx::query(
        &format!(
            "SELECT {COLUMNS} FROM inspection_results WHERE source_type = $1 AND source_id = $2 AND inspection_type = $3 AND deleted_at IS NULL"
        )
    )
    .bind(source_type)
    .bind(source_id)
    .bind(inspection_type)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

/// 记录检验结果 — 更新 result/qualified/unqualified/check_results/inspector/date 并推进状态
pub async fn record_result(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    result: i16,
    qualified_qty: rust_decimal::Decimal,
    unqualified_qty: rust_decimal::Decimal,
    check_results: Vec<CheckResult>,
    inspector_id: i64,
    inspection_date: chrono::NaiveDate,
) -> Result<u64> {
    let check_results_json = serde_json::to_value(&check_results)?;
    let r = sqlx::query(
        r#"
        UPDATE inspection_results
        SET result = $2, qualified_qty = $3, unqualified_qty = $4,
            check_results = $5, inspector_id = $6, inspection_date = $7,
            status = 2, updated_at = NOW()
        WHERE id = $1 AND status = 1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(result)
    .bind(qualified_qty)
    .bind(unqualified_qty)
    .bind(check_results_json)
    .bind(inspector_id)
    .bind(inspection_date)
    .execute(db)
    .await?;

    Ok(r.rows_affected())
}

pub async fn update_status(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    status: i16,
    expected_status: i16,
) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE inspection_results SET status = $2, updated_at = NOW() WHERE id = $1 AND status = $3 AND deleted_at IS NULL"
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
    filter: &InspectionResultFilter,
    page: &PageParams,
) -> Result<PaginatedResult<InspectionResult>> {
    let limit = page.page_size as i64;
    let offset = page.offset() as i64;

    let where_clause = format!(
        "WHERE deleted_at IS NULL
          AND ($1::smallint IS NULL OR source_type = $1)
          AND ($2::bigint IS NULL OR source_id = $2)
          AND ($3::smallint IS NULL OR inspection_type = $3)
          AND ($4::smallint IS NULL OR result = $4)
          AND ($5::smallint IS NULL OR status = $5)
          AND ($6::date IS NULL OR inspection_date >= $6)
          AND ($7::date IS NULL OR inspection_date <= $7)"
    );

    let count_sql = format!("SELECT COUNT(*) AS cnt FROM inspection_results {where_clause}");
    let count_row = sqlx::query(&count_sql)
        .bind(filter.source_type.map(|t: InspectionSourceType| t.as_i16()))
        .bind(filter.source_id)
        .bind(filter.inspection_type.map(|t: InspectionType| t.as_i16()))
        .bind(filter.result.map(|r: InspectionResultType| r.as_i16()))
        .bind(filter.status.map(|s: InspectionStatus| s.as_i16()))
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&mut *db)
        .await?;
    let total: i64 = count_row.get("cnt");

    let data_sql = format!(
        "SELECT {COLUMNS} FROM inspection_results {where_clause}
         ORDER BY created_at DESC
         LIMIT $8 OFFSET $9"
    );
    let rows = sqlx::query(&data_sql)
        .bind(filter.source_type.map(|t: InspectionSourceType| t.as_i16()))
        .bind(filter.source_id)
        .bind(filter.inspection_type.map(|t: InspectionType| t.as_i16()))
        .bind(filter.result.map(|r: InspectionResultType| r.as_i16()))
        .bind(filter.status.map(|s: InspectionStatus| s.as_i16()))
        .bind(filter.date_from)
        .bind(filter.date_to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *db)
        .await?;

    let items: Vec<InspectionResult> = rows.into_iter().map(row_to_model).collect();
    Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
}
