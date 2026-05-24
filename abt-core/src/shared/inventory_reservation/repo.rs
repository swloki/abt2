use rust_decimal::Decimal;
use sqlx::{FromRow, Row};

use super::model::{InventoryReservation, ReserveRequest};
use crate::shared::enums::{DocumentType, ReservationStatus};

pub struct InventoryReservationRepo;

impl InventoryReservationRepo {
    /// 获取事务级 advisory lock，序列化同一 product+warehouse 的并发预留
    pub async fn lock_for_reserve(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
            .bind(product_id)
            .bind(warehouse_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// INSERT 单条预留记录，返回生成的实体
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &ReserveRequest,
    ) -> Result<InventoryReservation, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_reservations
                (product_id, warehouse_id, reserved_qty, reservation_type,
                 source_type, source_id, source_line_id, status, priority, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, product_id, warehouse_id, reserved_qty, reservation_type,
                      source_type, source_id, source_line_id, status, priority, expires_at, created_at
            "#,
        )
        .bind(req.product_id)
        .bind(req.warehouse_id)
        .bind(req.reserved_qty)
        .bind(req.reservation_type)
        .bind(req.source_type)
        .bind(req.source_id)
        .bind(req.source_line_id)
        .bind(ReservationStatus::Active)
        .bind(req.priority)
        .bind(req.expires_at)
        .fetch_one(&mut *executor)
        .await?;

        InventoryReservation::from_row(&row)
    }

    /// 履行预留：UPDATE status = Fulfilled WHERE id = $1 AND status = Active
    /// 返回受影响行数
    pub async fn fulfill(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $2
            WHERE id = $1 AND status = $3
            "#,
        )
        .bind(id)
        .bind(ReservationStatus::Fulfilled)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 取消预留：UPDATE status = Cancelled WHERE id = $1 AND status = Active
    /// 返回受影响行数
    pub async fn cancel(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $2
            WHERE id = $1 AND status = $3
            "#,
        )
        .bind(id)
        .bind(ReservationStatus::Cancelled)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 查询 Active 状态的预留总量
    /// warehouse_id 为 None 时汇总所有仓库
    pub async fn total_reserved(
        executor: &mut sqlx::postgres::PgConnection,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(reserved_qty), 0) AS total
            FROM inventory_reservations
            WHERE product_id = $1
              AND ($2::bigint IS NULL OR warehouse_id = $2)
              AND status = $3
            "#,
        )
        .bind(product_id)
        .bind(warehouse_id)
        .bind(ReservationStatus::Active)
        .fetch_one(&mut *executor)
        .await?;

        let total: Decimal = row.try_get("total")?;
        Ok(total)
    }

    /// 按来源取消全部 Active 预留
    pub async fn cancel_by_source(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $1
            WHERE source_type = $2 AND source_id = $3 AND status = $4
            "#,
        )
        .bind(ReservationStatus::Cancelled)
        .bind(source_type)
        .bind(source_id)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// 按来源行履行 Active 预留
    pub async fn fulfill_by_source_line(
        executor: &mut sqlx::postgres::PgConnection,
        source_type: DocumentType,
        source_line_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE inventory_reservations
            SET status = $1
            WHERE source_type = $2 AND source_line_id = $3 AND status = $4
            "#,
        )
        .bind(ReservationStatus::Fulfilled)
        .bind(source_type)
        .bind(source_line_id)
        .bind(ReservationStatus::Active)
        .execute(&mut *executor)
        .await?;

        Ok(result.rows_affected())
    }
}
