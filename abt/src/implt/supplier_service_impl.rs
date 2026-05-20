//! 供应商服务实现
//!
//! 实现供应商管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::{Supplier, SupplierDetail, SupplierQuery};
use crate::repositories::{
    Executor, PaginatedResult, PaginationParams,
    SupplierBankAccountRepo, SupplierContactRepo, SupplierRepo,
};
use crate::service::{SupplierBankAccountInput, SupplierContactInput, SupplierService};

/// 供应商服务实现
pub struct SupplierServiceImpl {
    pool: Arc<PgPool>,
}

impl SupplierServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SupplierService for SupplierServiceImpl {
    async fn create(
        &self,
        supplier_code: String,
        supplier_name: String,
        short_name: Option<String>,
        classification: String,
        remark: Option<String>,
        operator_id: Option<i64>,
        contacts: Vec<SupplierContactInput>,
        bank_accounts: Vec<SupplierBankAccountInput>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let supplier_id = SupplierRepo::insert(
            executor,
            &supplier_code,
            &supplier_name,
            short_name.as_deref(),
            &classification,
            remark.as_deref(),
            operator_id,
        )
        .await
        .map_err(|e| map_duplicate_error(e, &supplier_code))?;

        // 批量插入联系人
        if !contacts.is_empty() {
            let contact_tuples: Vec<_> = contacts
                .into_iter()
                .map(|c| (c.contact_name, c.phone, c.email, c.position, c.is_primary))
                .collect();
            SupplierContactRepo::insert_batch(executor, supplier_id, &contact_tuples).await?;
        }

        // 批量插入银行账户
        if !bank_accounts.is_empty() {
            let account_tuples: Vec<_> = bank_accounts
                .into_iter()
                .map(|a| (a.bank_name, a.account_name, a.account_no, a.is_default))
                .collect();
            SupplierBankAccountRepo::insert_batch(executor, supplier_id, &account_tuples).await?;
        }

        Ok(supplier_id)
    }

    async fn update(
        &self,
        supplier_id: i64,
        supplier_name: String,
        short_name: Option<String>,
        classification: String,
        remark: Option<String>,
        contacts: Vec<SupplierContactInput>,
        bank_accounts: Vec<SupplierBankAccountInput>,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 验证供应商存在
        SupplierRepo::find_by_id(&self.pool, supplier_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Supplier".to_string(),
                id: supplier_id.to_string(),
            })?;

        // 更新基本信息
        SupplierRepo::update(
            executor,
            supplier_id,
            &supplier_name,
            short_name.as_deref(),
            &classification,
            remark.as_deref(),
        )
        .await?;

        // 替换联系人：先删后插
        SupplierContactRepo::delete_by_supplier(&mut *executor, supplier_id).await?;
        if !contacts.is_empty() {
            let contact_tuples: Vec<_> = contacts
                .into_iter()
                .map(|c| (c.contact_name, c.phone, c.email, c.position, c.is_primary))
                .collect();
            SupplierContactRepo::insert_batch(executor, supplier_id, &contact_tuples).await?;
        }

        // 替换银行账户：先删后插
        SupplierBankAccountRepo::delete_by_supplier(&mut *executor, supplier_id).await?;
        if !bank_accounts.is_empty() {
            let account_tuples: Vec<_> = bank_accounts
                .into_iter()
                .map(|a| (a.bank_name, a.account_name, a.account_no, a.is_default))
                .collect();
            SupplierBankAccountRepo::insert_batch(executor, supplier_id, &account_tuples).await?;
        }

        Ok(())
    }

    async fn delete(&self, supplier_id: i64, executor: Executor<'_>) -> Result<()> {
        // 验证供应商存在
        SupplierRepo::find_by_id(&self.pool, supplier_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Supplier".to_string(),
                id: supplier_id.to_string(),
            })?;

        // TODO: 检查是否有采购订单引用此供应商（采购订单模块实现后添加）

        SupplierRepo::soft_delete(executor, supplier_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, supplier_id: i64) -> Result<Option<SupplierDetail>> {
        let supplier = match SupplierRepo::find_by_id(&self.pool, supplier_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let contacts = SupplierContactRepo::find_by_supplier(&self.pool, supplier_id).await?;
        let bank_accounts =
            SupplierBankAccountRepo::find_by_supplier(&self.pool, supplier_id).await?;

        Ok(Some(SupplierDetail {
            supplier,
            contacts,
            bank_accounts,
        }))
    }

    async fn list(&self, query: SupplierQuery) -> Result<PaginatedResult<Supplier>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = SupplierRepo::query(&self.pool, &query).await?;
        let total = SupplierRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        supplier_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 验证供应商存在
        SupplierRepo::find_by_id(&self.pool, supplier_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Supplier".to_string(),
                id: supplier_id.to_string(),
            })?;

        SupplierRepo::update_status(executor, supplier_id, status).await?;
        Ok(())
    }
}

/// 将数据库 UNIQUE 约束冲突转换为 ServiceError::Conflict
fn map_duplicate_error(e: anyhow::Error, supplier_code: &str) -> anyhow::Error {
    if let Some(sqlx::Error::Database(db_err)) = e.downcast_ref::<sqlx::Error>()
        && db_err.code().as_deref() == Some("23505")
    {
        return anyhow::Error::from(ServiceError::Conflict {
            resource: "Supplier".to_string(),
            message: format!("供应商编码 '{}' 已存在", supplier_code),
        });
    }
    e
}
