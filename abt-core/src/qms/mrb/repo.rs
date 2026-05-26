use sqlx::Row;
use crate::shared::types::RepoResult;

use super::model::*;
use crate::qms::enums::*;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const COLUMNS: &str = "id, doc_number, inspection_result_id, product_id, defect_description, disposition, responsible_party, cost_impact, status, remark, operator_id, created_at, updated_at, deleted_at";
const INSERT_COLUMNS: &str = "doc_number, inspection_result_id, product_id, defect_description, disposition, responsible_party, cost_impact, status, remark, operator_id, created_at, updated_at, deleted_at";

fn row_to_model(row: sqlx::postgres::PgRow) -> Mrb {
    Mrb {
        id: row.get("id"),
        doc_number: row.get("doc_number"),
        inspection_result_id: row.get("inspection_result_id"),
        product_id: row.get("product_id"),
        defect_description: row.get("defect_description"),
        disposition: MRBDisposition::from_i16(row.get::<i16, _>("disposition"))
            .unwrap_or(MRBDisposition::Scrap),
        responsible_party: ResponsibleParty::from_i16(row.get::<i16, _>("responsible_party"))
            .unwrap_or(ResponsibleParty::Internal),
        cost_impact: row.get("cost_impact"),
        status: MRBStatus::from_i16(row.get::<i16, _>("status"))
            .unwrap_or(MRBStatus::Draft),
        remark: row.get("remark"),
        operator_id: row.get("operator_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        deleted_at: row.get("deleted_at"),
    }
}

pub async fn insert(
    db: &mut sqlx::postgres::PgConnection,
    m: &Mrb,
) -> RepoResult<i64> {
    let row = sqlx::query(
        &format!(
            "INSERT INTO mrbs ({INSERT_COLUMNS}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id"
        )
    )
    .bind(&m.doc_number)
    .bind(m.inspection_result_id)
    .bind(m.product_id)
    .bind(&m.defect_description)
    .bind(m.disposition.as_i16())
    .bind(m.responsible_party.as_i16())
    .bind(m.cost_impact)
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
) -> RepoResult<Option<Mrb>> {
    let row = sqlx::query(
        &format!("SELECT {COLUMNS} FROM mrbs WHERE id = $1 AND deleted_at IS NULL")
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;

    Ok(row.map(row_to_model))
}

pub async fn update_status(
    db: &mut sqlx::postgres::PgConnection,
    id: i64,
    status: i16,
    expected_status: i16,
) -> RepoResult<u64> {
    let result = sqlx::query(
        "UPDATE mrbs SET status = $2, updated_at = NOW() WHERE id = $1 AND status = $3 AND deleted_at IS NULL"
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
    filter: &MrbFilter,
    page: &PageParams,
) -> RepoResult<PaginatedResult<Mrb>> {
    let limit = page.page_size as i64;
    let offset = page.offset() as i64;

    let where_clause = format!(
        "WHERE deleted_at IS NULL
          AND ($1::bigint IS NULL OR inspection_result_id = $1)
          AND ($2::bigint IS NULL OR product_id = $2)
          AND ($3::smallint IS NULL OR disposition = $3)
          AND ($4::smallint IS NULL OR status = $4)
          AND ($5::smallint IS NULL OR responsible_party = $5)"
    );

    let count_sql = format!("SELECT COUNT(*) AS cnt FROM mrbs {where_clause}");
    let count_row = sqlx::query(&count_sql)
        .bind(filter.inspection_result_id)
        .bind(filter.product_id)
        .bind(filter.disposition.map(|d: MRBDisposition| d.as_i16()))
        .bind(filter.status.map(|s: MRBStatus| s.as_i16()))
        .bind(filter.responsible_party.map(|r: ResponsibleParty| r.as_i16()))
        .fetch_one(&mut *db)
        .await?;
    let total: i64 = count_row.get("cnt");

    let data_sql = format!(
        "SELECT {COLUMNS} FROM mrbs {where_clause}
         ORDER BY created_at DESC
         LIMIT $6 OFFSET $7"
    );
    let rows = sqlx::query(&data_sql)
        .bind(filter.inspection_result_id)
        .bind(filter.product_id)
        .bind(filter.disposition.map(|d: MRBDisposition| d.as_i16()))
        .bind(filter.status.map(|s: MRBStatus| s.as_i16()))
        .bind(filter.responsible_party.map(|r: ResponsibleParty| r.as_i16()))
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *db)
        .await?;

    let items: Vec<Mrb> = rows.into_iter().map(row_to_model).collect();
    Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
}
