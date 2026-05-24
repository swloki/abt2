use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait SupplierService: Send + Sync {
    // -- Supplier CRUD --
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateSupplierReq,
    ) -> Result<CreateSupplierResult, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Supplier, DomainError>;

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateSupplierReq,
    ) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: SupplierQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Supplier>, DomainError>;

    // -- Contacts --
    async fn add_contact(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        req: CreateContactReq,
    ) -> Result<i64, DomainError>;

    async fn update_contact(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<(), DomainError>;

    async fn delete_contact(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        contact_id: i64,
    ) -> Result<(), DomainError>;

    async fn list_contacts(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierContact>, DomainError>;

    // -- Bank Accounts --
    async fn add_bank_account(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        req: CreateBankAccountReq,
    ) -> Result<i64, DomainError>;

    async fn update_bank_account(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        account_id: i64,
        req: UpdateBankAccountReq,
    ) -> Result<(), DomainError>;

    async fn delete_bank_account(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
        account_id: i64,
    ) -> Result<(), DomainError>;

    async fn list_bank_accounts(
        &self,
        ctx: ServiceContext<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierBankAccount>, DomainError>;
}
