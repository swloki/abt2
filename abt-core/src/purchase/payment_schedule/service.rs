use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::{PaymentSchedule, PaymentScheduleInput};
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait PaymentScheduleService: Send + Sync {
    /// 根据 PO 含税总额和分期配置，生成付款计划行
    async fn generate_for_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        total_amount: Decimal,
        schedule_input: Vec<PaymentScheduleInput>,
    ) -> Result<()>;

    /// 查询某 PO 的付款计划
    async fn list_by_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<PaymentSchedule>>;

    /// 更新已付金额（付款确认时调用）
    async fn update_paid_amount(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        schedule_id: i64,
        paid_amount: Decimal,
    ) -> Result<()>;

    /// 按订单分配付款金额（按到期日顺序）
    async fn allocate_payment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        amount: Decimal,
    ) -> Result<()>;

    /// 删除（PO 取消或重新生成时调用）
    async fn delete_by_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<()>;
}
