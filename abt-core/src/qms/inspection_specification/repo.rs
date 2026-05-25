use sqlx::Row;

use super::model::*;
use crate::qms::enums::*;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const COLUMNS: &str = "id, doc_number, product_id, inspection_type, check_items, sample_plan, status, version, operator_id, created_at, updated_at, deleted_at";
const INSERT_COLUMNS: &str = "doc_number, product_id, inspection_type, check_items, sample_plan, status, version, operator_id, created_at, updated_at, deleted_at";

fn row_to_model(row: sqlx::postgres::PgRow) -> InspectionSpecification {
    let check_items_val: serde_json::Value = row.get("check_items");
    let sample_plan_val: serde_json::Value = row.get("sample_plan");
    InspectionSpecification {
        id: row.get("id"),
        doc_number: row.get("doc_number"),
        product_id: row.get("product_id"),
        inspection_type: InspectionType::from_i16(row.get::<i16, _>("inspection_type"))
            .unwrap_or(InspectionType::Iqc),
        check_items: serde_json::from_value(check_items_val).unwrap_or_default(),
        sample_plan: serde_json::from_value(sample_plan_val).unwrap_or_default(),
        status: SpecStatus::from_i16(row.get::<i16, _>("status")).unwrap_or(SpecStatus::Draft),
        version: row.get("version"),
        operator_id: row.get("operator_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    }
}

pub async fn insert(
    db: &mut sqlx::postgres::PgConnection,
    m: &InspectionSpecification,
) -> anyhow::Result<i64> {
    let check_items_json = serde_json::to_value(&m.check_items)?;
    let sample_plan_json = serde_json::to_value(&m.sample_plan)?;

    let row = sqlx::query(
        &format!(
            "INSERT INTO inspection_specifications ({INSERT_COLUMNS}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id"
        )
    )
    .bind(&m.doc_number)
    .bind(m.product_id)
    .bind(m.inspection_type.as_i16())
    .bind(check_items_json)
    .bind(sample_plan_json)
    .bind(m.status.as_i16())
    .bind(m.version)
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
) -> anyhow::Result<Option<InspectionSpecification>> {
    let row = sqlx::query(
        &format!("SELECT {COLUMNS} FROM inspection_specifications WHERE id = $1 AND deleted_at IS NULL")
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

pub async fn find_active_by_product_and_type(
    db: &mut sqlx::postgres::PgConnection,
    product_id: i64,
    inspection_type: i16,
) -> anyhow::Result<Option<InspectionSpecification>> {
    let row = sqlx::query(
        &format!(
            "SELECT {COLUMNS} FROM inspection_specifications WHERE product_id = $1 AND inspection_type = $2 AND status = 2 AND deleted_at IS NULL"
        )
    )
    .bind(product_id)
    .bind(inspection_type)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

pub async fn update_fields(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    check_items: Option<Vec<CheckItem>>,
    sample_plan: Option<SamplePlan>,
    status: Option<SpecStatus>,
    expected_version: i32,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE inspection_specifications
        SET check_items = COALESCE($2, check_items),
            sample_plan = COALESCE($3, sample_plan),
            status = COALESCE($4, status),
            version = version + 1,
            updated_at = NOW()
        WHERE id = $1 AND version = $5 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(check_items.map(serde_json::to_value).transpose()?)
    .bind(sample_plan.map(serde_json::to_value).transpose()?)
    .bind(status.map(|s| s.as_i16()))
    .bind(expected_version)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub async fn list(
    db: &mut sqlx::postgres::PgConnection,
    filter: &InspectionSpecFilter,
    page: &PageParams,
) -> anyhow::Result<PaginatedResult<InspectionSpecification>> {
    let limit = page.page_size as i64;
    let offset = page.offset() as i64;

    let keyword_filter = if filter.keyword.is_some() {
        "AND ($4::text IS NULL OR doc_number ILIKE '%' || $4 || '%')"
    } else {
        "AND ($4::text IS NULL OR TRUE)"
    };

    let where_clause = format!(
        "WHERE deleted_at IS NULL
          AND ($1::smallint IS NULL OR inspection_type = $1)
          AND ($2::smallint IS NULL OR status = $2)
          AND ($3::bigint IS NULL OR product_id = $3)
          {keyword_filter}"
    );

    let count_sql = format!("SELECT COUNT(*) AS cnt FROM inspection_specifications {where_clause}");
    let count_row = sqlx::query(&count_sql)
        .bind(filter.inspection_type.map(|t: InspectionType| t.as_i16()))
        .bind(filter.status.map(|s: SpecStatus| s.as_i16()))
        .bind(filter.product_id)
        .bind(&filter.keyword)
        .fetch_one(&mut *db)
        .await?;
    let total: i64 = count_row.get("cnt");

    let data_sql = format!(
        "SELECT {COLUMNS} FROM inspection_specifications {where_clause}
         ORDER BY created_at DESC
         LIMIT $5 OFFSET $6"
    );
    let rows = sqlx::query(&data_sql)
        .bind(filter.inspection_type.map(|t: InspectionType| t.as_i16()))
        .bind(filter.status.map(|s: SpecStatus| s.as_i16()))
        .bind(filter.product_id)
        .bind(&filter.keyword)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *db)
        .await?;

    let items: Vec<InspectionSpecification> = rows.into_iter().map(row_to_model).collect();
    Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
}
