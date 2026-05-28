use chrono::{DateTime, Utc};
use crate::shared::types::Result;

use super::model::{OutsourcingTracking, OverdueTrackingQuery};
use crate::om::enums::TrackingNodeType;
use crate::shared::types::pagination::PageParams;

// ---------------------------------------------------------------------------
// OutsourcingTrackingRepo
// ---------------------------------------------------------------------------

pub struct OutsourcingTrackingRepo;

impl OutsourcingTrackingRepo {
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        node_type: TrackingNodeType,
        tracked_at: Option<DateTime<Utc>>,
        remark: Option<&str>,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO outsourcing_trackings
                (outsourcing_id, node_type, tracked_at, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(outsourcing_id)
        .bind(node_type)
        .bind(tracked_at)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        use sqlx::Row;
        Ok(row.try_get("id")?)
    }

    pub async fn get_max_node_ordinal(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
    ) -> Result<Option<i16>> {
        let row = sqlx::query(
            "SELECT MAX(node_type) AS max_ordinal FROM outsourcing_trackings WHERE outsourcing_id = $1",
        )
        .bind(outsourcing_id)
        .fetch_one(executor)
        .await?;

        use sqlx::Row;
        let val: Option<i16> = row.try_get("max_ordinal")?;
        Ok(val)
    }

    pub async fn has_node_type(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        node_type: TrackingNodeType,
    ) -> Result<bool> {
        let row = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM outsourcing_trackings WHERE outsourcing_id = $1 AND node_type = $2) AS exists_flag",
        )
        .bind(outsourcing_id)
        .bind(node_type)
        .fetch_one(executor)
        .await?;

        use sqlx::Row;
        Ok(row.try_get("exists_flag")?)
    }

    pub async fn list_by_outsourcing_id(
        executor: &mut sqlx::postgres::PgConnection,
        outsourcing_id: i64,
        page: &PageParams,
    ) -> Result<(Vec<OutsourcingTracking>, u64)> {
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;

        let count_row = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM outsourcing_trackings WHERE outsourcing_id = $1",
        )
        .bind(outsourcing_id)
        .fetch_one(&mut *executor)
        .await?;

        use sqlx::Row;
        let total: i64 = count_row.try_get("cnt")?;

        let rows = sqlx::query_as::<_, OutsourcingTracking>(
            r#"
            SELECT id, outsourcing_id, node_type, tracked_at, planned_at,
                   remark, operator_id, created_at
            FROM outsourcing_trackings
            WHERE outsourcing_id = $1
            ORDER BY node_type, created_at
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(outsourcing_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *executor)
        .await?;

        Ok((rows, total as u64))
    }

    pub async fn query_overdue(
        executor: &mut sqlx::postgres::PgConnection,
        q: &OverdueTrackingQuery,
        page: &PageParams,
    ) -> Result<(Vec<OutsourcingTracking>, u64)> {
        let limit = page.page_size as i64;
        let offset = page.offset() as i64;

        let where_clause = "
            WHERE t.planned_at IS NOT NULL
              AND t.tracked_at IS NULL
              AND ($1::bigint IS NULL OR o.supplier_id = $1)
              AND ($2::smallint IS NULL OR t.node_type = $2)
              AND ($3::timestamptz IS NULL OR t.planned_at < $3)
              AND o.deleted_at IS NULL
        ";

        let count_sql = format!(
            "SELECT COUNT(*) AS cnt
             FROM outsourcing_trackings t
             JOIN outsourcing_orders o ON o.id = t.outsourcing_id
             {where_clause}"
        );
        let count_row = sqlx::query(sqlx::AssertSqlSafe(count_sql))
            .bind(q.supplier_id)
            .bind(q.node_type)
            .bind(q.overdue_before)
            .fetch_one(&mut *executor)
            .await?;

        use sqlx::Row;
        let total: i64 = count_row.try_get("cnt")?;

        let data_sql = format!(
            "SELECT t.id, t.outsourcing_id, t.node_type, t.tracked_at, t.planned_at,
                    t.remark, t.operator_id, t.created_at
             FROM outsourcing_trackings t
             JOIN outsourcing_orders o ON o.id = t.outsourcing_id
             {where_clause}
             ORDER BY t.planned_at
             LIMIT $4 OFFSET $5"
        );
        let rows = sqlx::query_as::<_, OutsourcingTracking>(sqlx::AssertSqlSafe(data_sql))
            .bind(q.supplier_id)
            .bind(q.node_type)
            .bind(q.overdue_before)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *executor)
            .await?;

        Ok((rows, total as u64))
    }
}
