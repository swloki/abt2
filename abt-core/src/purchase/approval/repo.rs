use rust_decimal::Decimal;

use crate::shared::types::Result;

use super::model::PurchaseApprovalRule;

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

    /// 查询所有启用的规则
    pub async fn list_active(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> Result<Vec<PurchaseApprovalRule>> {
        sqlx::query_as::<_, PurchaseApprovalRule>(
            r#"
            SELECT id, name, min_amount, max_amount, approver_role, approver_id,
                   is_active, sort_order, created_at, updated_at, deleted_at
            FROM purchase_approval_rules
            WHERE is_active = TRUE AND deleted_at IS NULL
            ORDER BY sort_order, min_amount
            "#,
        )
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// 创建审批规则
    pub async fn insert(
        executor: &mut sqlx::postgres::PgConnection,
        name: &str,
        min_amount: Decimal,
        max_amount: Option<Decimal>,
        approver_role: &str,
        approver_id: Option<i64>,
        sort_order: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO purchase_approval_rules
                (name, min_amount, max_amount, approver_role, approver_id, is_active, sort_order)
            VALUES ($1, $2, $3, $4, $5, TRUE, $6)
            "#,
        )
        .bind(name)
        .bind(min_amount)
        .bind(max_amount)
        .bind(approver_role)
        .bind(approver_id)
        .bind(sort_order)
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
