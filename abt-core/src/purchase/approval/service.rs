use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::{PurchaseApprovalRule, RuleUpsertRequest};
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

    /// 查询所有规则（含停用，管理页用）
    async fn list_rules(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<PurchaseApprovalRule>>;

    /// 单条规则（编辑回填）
    async fn get_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<PurchaseApprovalRule>;

    /// 创建审批规则，返回新 id
    async fn create_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: RuleUpsertRequest,
    ) -> Result<i64>;

    /// 更新审批规则
    async fn update_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: RuleUpsertRequest,
    ) -> Result<()>;

    /// 删除审批规则
    async fn delete_rule(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        rule_id: i64,
    ) -> Result<()>;
}
