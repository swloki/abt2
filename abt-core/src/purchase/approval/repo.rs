use rust_decimal::Decimal;

use crate::shared::types::Result;

use super::model::{PurchaseApprovalRule, RuleUpsertRequest};

pub struct PurchaseApprovalRuleRepo;

impl PurchaseApprovalRuleRepo {
    /// 按金额查找匹配的审批规则
    pub async fn find_by_amount(
        executor: &mut sqlx::postgres::PgConnection,
        amount: Decimal,
    ) -> Result<Option<PurchaseApprovalRule>> {
        sqlx::query_as::<_, PurchaseApprovalRule>(
            r#"
            SELECT id, name, min_amount, max_amount, approver_role, approver_id,
                   is_active, sort_order, created_at, updated_at, deleted_at
            FROM purchase_approval_rules
            WHERE is_active = TRUE
              AND deleted_at IS NULL
              AND min_amount <= $1
              AND (max_amount IS NULL OR max_amount >= $1)
            ORDER BY sort_order
            LIMIT 1
            "#,
        )
        .bind(amount)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }

    /// 查询所有规则（含停用，管理页用）
    pub async fn list_all(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<PurchaseApprovalRule>> {
        sqlx::query_as::<_, PurchaseApprovalRule>(
            r#"
            SELECT id, name, min_amount, max_amount, approver_role, approver_id,
                   is_active, sort_order, created_at, updated_at, deleted_at
            FROM purchase_approval_rules
            WHERE deleted_at IS NULL
            ORDER BY sort_order, min_amount
            "#,
        )
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 按主键查询单条（编辑回填）
    pub async fn find_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<Option<PurchaseApprovalRule>> {
        sqlx::query_as::<_, PurchaseApprovalRule>(
            r#"
            SELECT id, name, min_amount, max_amount, approver_role, approver_id,
                   is_active, sort_order, created_at, updated_at, deleted_at
            FROM purchase_approval_rules
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(Into::into)
    }

    /// 创建审批规则
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        req: &RuleUpsertRequest,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO purchase_approval_rules
                (name, min_amount, max_amount, approver_role, approver_id, is_active, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(&req.name)
        .bind(req.min_amount)
        .bind(req.max_amount)
        .bind(&req.approver_role)
        .bind(req.approver_id)
        .bind(req.is_active)
        .bind(req.sort_order)
        .fetch_one(&mut *executor)
        .await?;
        Ok(id)
    }

    /// 更新审批规则
    pub async fn update_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
        req: &RuleUpsertRequest,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE purchase_approval_rules SET
                name = $1, min_amount = $2, max_amount = $3, approver_role = $4,
                approver_id = $5, is_active = $6, sort_order = $7, updated_at = NOW()
            WHERE id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(&req.name)
        .bind(req.min_amount)
        .bind(req.max_amount)
        .bind(&req.approver_role)
        .bind(req.approver_id)
        .bind(req.is_active)
        .bind(req.sort_order)
        .bind(id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// 删除审批规则（软删除）
    pub async fn delete_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        id: i64,
    ) -> Result<()> {
        sqlx::query("UPDATE purchase_approval_rules SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }
}
