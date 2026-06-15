use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::PurchaseApprovalRule;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

#[async_trait]
pub trait PurchaseApprovalService: Send + Sync {
    /// 按金额查找匹配的审批规则
    async fn find_rule_by_amount(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        amount: Decimal,
    ) -> Result<Option<PurchaseApprovalRule>>;

    /// 查询所有启用的规则
    async fn list_rules(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<PurchaseApprovalRule>>;

    /// 创建审批规则
    async fn create_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        name: String,
        min_amount: Decimal,
        max_amount: Option<Decimal>,
        approver_role: String,
        approver_id: Option<i64>,
        sort_order: i32,
    ) -> Result<()>;

    /// 删除审批规则
    async fn delete_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        rule_id: i64,
    ) -> Result<()>;
}
