use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{PaymentSchedule, PaymentScheduleInput};
use super::repo::PaymentScheduleRepo;
use super::service::PaymentScheduleService;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct PaymentScheduleServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl PaymentScheduleServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PaymentScheduleService for PaymentScheduleServiceImpl {
    async fn generate_for_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        total_amount: Decimal,
        schedule_input: Vec<PaymentScheduleInput>,
    ) -> Result<()> {
        // 校验百分比之和 = 100
        let sum: Decimal = schedule_input.iter().map(|s| s.payment_pct).sum();
        if sum != Decimal::from(100) {
            return Err(DomainError::validation(format!(
                "付款计划百分比之和必须为 100，当前为 {}",
                sum
            )));
        }

        let rows: Vec<_> = schedule_input
            .iter()
            .map(|s| {
                let amount = total_amount * s.payment_pct / Decimal::from(100);
                (s.due_date, s.payment_pct, amount, s.description.clone())
            })
            .collect();

        PaymentScheduleRepo::insert_batch(&mut *db, order_id, &rows).await?;
        PaymentScheduleRepo::mark_generated(&mut *db, order_id).await?;
        Ok(())
    }

    async fn list_by_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<PaymentSchedule>> {
        PaymentScheduleRepo::list_by_order(&mut *db, order_id).await
    }

    async fn update_paid_amount(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        schedule_id: i64,
        paid_amount: Decimal,
    ) -> Result<()> {
        PaymentScheduleRepo::update_paid_amount(&mut *db, schedule_id, paid_amount).await
    }

    async fn allocate_payment(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        amount: Decimal,
    ) -> Result<()> {
        let schedules = PaymentScheduleRepo::list_by_order(&mut *db, order_id).await?;
        let mut remaining = amount;

        for sched in schedules
            .iter()
            .filter(|s| s.paid_amount < s.payment_amount)
        {
            if remaining <= Decimal::ZERO {
                break;
            }
            let to_pay = remaining.min(sched.payment_amount - sched.paid_amount);
            PaymentScheduleRepo::update_paid_amount(
                &mut *db,
                sched.id,
                sched.paid_amount + to_pay,
            )
            .await?;
            remaining -= to_pay;
        }

        Ok(())
    }

    async fn delete_by_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<()> {
        PaymentScheduleRepo::delete_by_order(&mut *db, order_id).await
    }
}
