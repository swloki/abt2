use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::shared::types::Result;

use super::model::PaymentSchedule;

pub struct PaymentScheduleRepo;

impl PaymentScheduleRepo {
    /// 批量插入付款计划行
    pub async fn insert_batch(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
        schedules: &[(NaiveDate, Decimal, Decimal, String)], // (due_date, pct, amount, desc)
    ) -> Result<()> {
        for (i, (due_date, pct, amount, desc)) in schedules.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO purchase_payment_schedules
                    (order_id, line_no, due_date, payment_pct, payment_amount, paid_amount, description)
                VALUES ($1, $2, $3, $4, $5, 0, $6)
                "#,
            )
            .bind(order_id)
            .bind((i + 1) as i32)
            .bind(due_date)
            .bind(pct)
            .bind(amount)
            .bind(desc)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 按订单查询付款计划
    pub async fn list_by_order(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<Vec<PaymentSchedule>> {
        sqlx::query_as::<_, PaymentSchedule>(
            r#"
            SELECT id, order_id, line_no, due_date, payment_pct, payment_amount,
                   paid_amount, description, created_at, updated_at
            FROM purchase_payment_schedules
            WHERE order_id = $1
            ORDER BY line_no
            "#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 更新已付金额
    pub async fn update_paid_amount(
        executor: &mut sqlx::postgres::PgConnection,
        schedule_id: i64,
        paid_amount: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_payment_schedules SET paid_amount = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(schedule_id)
        .bind(paid_amount)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 按订单删除全部付款计划
    pub async fn delete_by_order(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<()> {
        sqlx::query("DELETE FROM purchase_payment_schedules WHERE order_id = $1")
            .bind(order_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    /// 标记订单已生成付款计划
    pub async fn mark_generated(
        executor: &mut sqlx::postgres::PgConnection,
        order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE purchase_orders SET payment_schedule_generated = TRUE WHERE id = $1",
        )
        .bind(order_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
