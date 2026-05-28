use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

#[async_trait]
pub trait CustomerService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateCustomerReq,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Customer>;

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateCustomerReq,
    ) -> Result<()>;

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: CustomerQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Customer>>;

    async fn add_contact(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        req: CreateContactReq,
    ) -> Result<i64>;

    async fn update_contact(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<()>;

    async fn delete_contact(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<()>;

    async fn list_contacts(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerContact>>;

    async fn add_address(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        req: CreateAddressReq,
    ) -> Result<i64>;

    async fn update_address(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        address_id: i64,
        req: UpdateAddressReq,
    ) -> Result<()>;

    async fn delete_address(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        address_id: i64,
    ) -> Result<()>;

    async fn list_addresses(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerAddress>>;

    async fn validate_contact_ownership(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<bool>;

    async fn claim(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    async fn transfer(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        new_owner_id: i64,
        new_department_id: Option<i64>,
    ) -> Result<()>;
}
