use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{PurchaseApprovalRule, RuleUpsertRequest};
use super::repo::PurchaseApprovalRuleRepo;
use super::service::PurchaseApprovalService;
use crate::shared::types::{DomainError, PgExecutor};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

pub struct PurchaseApprovalServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl PurchaseApprovalServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseApprovalService for PurchaseApprovalServiceImpl {
    async fn find_rule_by_amount(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        amount: Decimal,
    ) -> Result<Option<PurchaseApprovalRule>> {
        PurchaseApprovalRuleRepo::find_by_amount(&mut *db, amount).await
    }

    async fn list_rules(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<PurchaseApprovalRule>> {
        PurchaseApprovalRuleRepo::list_all(&mut *db).await
    }

    async fn get_rule(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<PurchaseApprovalRule> {
        PurchaseApprovalRuleRepo::find_by_id(&mut *db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("审批规则"))
    }

    async fn create_rule(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: RuleUpsertRequest,
    ) -> Result<i64> {
        PurchaseApprovalRuleRepo::insert(&mut *db, &req).await
    }

    async fn update_rule(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: RuleUpsertRequest,
    ) -> Result<()> {
        PurchaseApprovalRuleRepo::update_by_id(&mut *db, id, &req).await
    }

    async fn delete_rule(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        rule_id: i64,
    ) -> Result<()> {
        PurchaseApprovalRuleRepo::delete_by_id(&mut *db, rule_id).await
    }
}
