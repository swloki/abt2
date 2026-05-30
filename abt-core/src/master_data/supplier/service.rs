use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait SupplierService: Send + Sync {
    // -- Supplier CRUD --
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateSupplierReq,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Supplier>;

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSupplierReq,
    ) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: SupplierQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Supplier>>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    // -- Contacts --
    async fn add_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        req: CreateContactReq,
    ) -> Result<i64>;
    async fn update_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<()>;

    async fn delete_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        contact_id: i64,
    ) -> Result<()>;

    async fn list_contacts(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierContact>>;

    // -- Bank Accounts --
    async fn add_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        req: CreateBankAccountReq,
    ) -> Result<i64>;

    async fn update_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        account_id: i64,
        req: UpdateBankAccountReq,
    ) -> Result<()>;

    async fn delete_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        account_id: i64,
    ) -> Result<()>;

    async fn list_bank_accounts(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierBankAccount>>;
}
