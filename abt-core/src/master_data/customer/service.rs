use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait CustomerService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateCustomerReq,
    ) -> Result<CreateCustomerResult, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Customer, DomainError>;

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateCustomerReq,
    ) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CustomerQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Customer>, DomainError>;

    async fn add_contact(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        req: CreateContactReq,
    ) -> Result<i64, DomainError>;

    async fn update_contact(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<(), DomainError>;

    async fn delete_contact(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<(), DomainError>;

    async fn list_contacts(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerContact>, DomainError>;

    async fn add_address(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        req: CreateAddressReq,
    ) -> Result<i64, DomainError>;

    async fn update_address(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        address_id: i64,
        req: UpdateAddressReq,
    ) -> Result<(), DomainError>;

    async fn delete_address(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        address_id: i64,
    ) -> Result<(), DomainError>;

    async fn list_addresses(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerAddress>, DomainError>;

    async fn validate_contact_ownership(
        &self,
        ctx: ServiceContext<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<bool, DomainError>;

    async fn claim(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn transfer(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        new_owner_id: i64,
        new_department_id: Option<i64>,
    ) -> Result<(), DomainError>;
}
