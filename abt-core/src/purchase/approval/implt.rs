use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::PurchaseApprovalRule;
use super::repo::PurchaseApprovalRuleRepo;
use super::service::PurchaseApprovalService;
use crate::shared::types::PgExecutor;
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
        PurchaseApprovalRuleRepo::list_active(&mut *db).await
    }

    async fn create_rule(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        name: String,
        min_amount: Decimal,
        max_amount: Option<Decimal>,
        approver_role: String,
        approver_id: Option<i64>,
        sort_order: i32,
    ) -> Result<()> {
        PurchaseApprovalRuleRepo::insert(
            &mut *db, &name, min_amount, max_amount, &approver_role, approver_id, sort_order,
        )
        .await
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
