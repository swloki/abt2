//! 供应商数据访问层
//!
//! 提供供应商、联系人、银行账户的数据库 CRUD 操作。

use anyhow::Result;
use sqlx::PgPool;

use crate::models::{Supplier, SupplierBankAccount, SupplierContact, SupplierQuery};
use crate::repositories::{build_fuzzy_pattern, Executor};

// ============================================================================
// SupplierRepo
// ============================================================================

/// 供应商数据仓库
pub struct SupplierRepo;

impl SupplierRepo {
    /// 创建新供应商，返回 supplier_id
    pub async fn insert(
        executor: Executor<'_>,
        supplier_code: &str,
        supplier_name: &str,
        short_name: Option<&str>,
        classification: &str,
        remark: Option<&str>,
        operator_id: Option<i64>,
    ) -> Result<i64> {
        let supplier_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO suppliers (supplier_code, supplier_name, short_name, classification, remark, operator_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING supplier_id
            "#,
        )
        .bind(supplier_code)
        .bind(supplier_name)
        .bind(short_name)
        .bind(classification)
        .bind(remark)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;

        Ok(supplier_id)
    }

    /// 更新供应商基本信息
    pub async fn update(
        executor: Executor<'_>,
        supplier_id: i64,
        supplier_name: &str,
        short_name: Option<&str>,
        classification: &str,
        remark: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE suppliers
            SET supplier_name = $1, short_name = $2, classification = $3, remark = $4, updated_at = NOW()
            WHERE supplier_id = $5
            "#,
        )
        .bind(supplier_name)
        .bind(short_name)
        .bind(classification)
        .bind(remark)
        .bind(supplier_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 软删除供应商
    pub async fn soft_delete(executor: Executor<'_>, supplier_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE suppliers SET deleted_at = NOW() WHERE supplier_id = $1 AND deleted_at IS NULL",
        )
        .bind(supplier_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 根据 ID 查找供应商（排除已删除）
    pub async fn find_by_id(pool: &PgPool, supplier_id: i64) -> Result<Option<Supplier>> {
        let row = sqlx::query_as::<_, Supplier>(
            "SELECT supplier_id, supplier_code, supplier_name, short_name, classification, \
             status, remark, operator_id, created_at, updated_at, deleted_at \
             FROM suppliers WHERE supplier_id = $1 AND deleted_at IS NULL",
        )
        .bind(supplier_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// 分页查询供应商列表
    pub async fn query(pool: &PgPool, query: &SupplierQuery) -> Result<Vec<Supplier>> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT supplier_id, supplier_code, supplier_name, short_name, classification, \
             status, remark, operator_id, created_at, updated_at, deleted_at \
             FROM suppliers WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &query.keyword
            && !keyword.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (supplier_name ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR supplier_code ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(classification) = &query.classification
            && !classification.is_empty()
        {
            qb.push(" AND classification = ");
            qb.push_bind(classification);
        }

        if let Some(status) = query.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        qb.push(" ORDER BY supplier_id DESC");
        qb.push(" LIMIT ");
        qb.push_bind(page_size as i32);
        qb.push(" OFFSET ");
        qb.push_bind(((page - 1) * page_size) as i32);

        let result = qb.build_query_as::<Supplier>().fetch_all(pool).await?;
        Ok(result)
    }

    /// 查询供应商总数
    pub async fn query_count(pool: &PgPool, query: &SupplierQuery) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM suppliers WHERE deleted_at IS NULL",
        );

        if let Some(keyword) = &query.keyword
            && !keyword.is_empty()
            && let Some(pattern) = build_fuzzy_pattern(keyword)
        {
            qb.push(" AND (supplier_name ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR supplier_code ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        if let Some(classification) = &query.classification
            && !classification.is_empty()
        {
            qb.push(" AND classification = ");
            qb.push_bind(classification);
        }

        if let Some(status) = query.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        let count: i64 = qb.build_query_scalar().fetch_one(pool).await?;
        Ok(count)
    }

    /// 更新供应商状态
    pub async fn update_status(
        executor: Executor<'_>,
        supplier_id: i64,
        status: i16,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE suppliers SET status = $1, updated_at = NOW() WHERE supplier_id = $2",
        )
        .bind(status)
        .bind(supplier_id)
        .execute(executor)
        .await?;

        Ok(())
    }
}

// ============================================================================
// SupplierContactRepo
// ============================================================================

/// 供应商联系人数据仓库
pub struct SupplierContactRepo;

impl SupplierContactRepo {
    /// 批量插入联系人
    pub async fn insert_batch(
        executor: Executor<'_>,
        supplier_id: i64,
        contacts: &[(String, Option<String>, Option<String>, Option<String>, bool)],
    ) -> Result<()> {
        for (name, phone, email, position, is_primary) in contacts {
            sqlx::query(
                r#"
                INSERT INTO supplier_contacts (supplier_id, contact_name, phone, email, position, is_primary)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(supplier_id)
            .bind(name)
            .bind(phone)
            .bind(email)
            .bind(position)
            .bind(is_primary)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 删除供应商下的所有联系人
    pub async fn delete_by_supplier(executor: Executor<'_>, supplier_id: i64) -> Result<()> {
        sqlx::query(
            "DELETE FROM supplier_contacts WHERE supplier_id = $1",
        )
        .bind(supplier_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 查询供应商下的所有联系人
    pub async fn find_by_supplier(
        pool: &PgPool,
        supplier_id: i64,
    ) -> Result<Vec<SupplierContact>> {
        let rows = sqlx::query_as::<_, SupplierContact>(
            "SELECT contact_id, supplier_id, contact_name, phone, email, position, is_primary, created_at \
             FROM supplier_contacts WHERE supplier_id = $1 ORDER BY is_primary DESC, contact_id",
        )
        .bind(supplier_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}

// ============================================================================
// SupplierBankAccountRepo
// ============================================================================

/// 供应商银行账户数据仓库
pub struct SupplierBankAccountRepo;

impl SupplierBankAccountRepo {
    /// 批量插入银行账户
    pub async fn insert_batch(
        executor: Executor<'_>,
        supplier_id: i64,
        accounts: &[(String, String, String, bool)],
    ) -> Result<()> {
        for (bank_name, account_name, account_no, is_default) in accounts {
            sqlx::query(
                r#"
                INSERT INTO supplier_bank_accounts (supplier_id, bank_name, account_name, account_no, is_default)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(supplier_id)
            .bind(bank_name)
            .bind(account_name)
            .bind(account_no)
            .bind(is_default)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// 删除供应商下的所有银行账户
    pub async fn delete_by_supplier(executor: Executor<'_>, supplier_id: i64) -> Result<()> {
        sqlx::query(
            "DELETE FROM supplier_bank_accounts WHERE supplier_id = $1",
        )
        .bind(supplier_id)
        .execute(executor)
        .await?;

        Ok(())
    }

    /// 查询供应商下的所有银行账户
    pub async fn find_by_supplier(
        pool: &PgPool,
        supplier_id: i64,
    ) -> Result<Vec<SupplierBankAccount>> {
        let rows = sqlx::query_as::<_, SupplierBankAccount>(
            "SELECT bank_account_id, supplier_id, bank_name, account_name, account_no, is_default, created_at \
             FROM supplier_bank_accounts WHERE supplier_id = $1 ORDER BY is_default DESC, bank_account_id",
        )
        .bind(supplier_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
