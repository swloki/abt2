use std::sync::Arc;

use crate::master_data::customer::model::*;
use crate::master_data::customer::repo::{CustomerAddressRepo, CustomerContactRepo, CustomerRepo};
use crate::master_data::customer::service::CustomerService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct CustomerServiceImpl {
    repo: CustomerRepo,
    contact_repo: CustomerContactRepo,
    address_repo: CustomerAddressRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}

impl CustomerServiceImpl {
    pub fn new(
        repo: CustomerRepo,
        contact_repo: CustomerContactRepo,
        address_repo: CustomerAddressRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        state_machine: Arc<dyn StateMachineService>,
    ) -> Self {
        Self {
            repo,
            contact_repo,
            address_repo,
            doc_seq,
            audit,
            event_bus,
            state_machine,
        }
    }
}

#[async_trait::async_trait]
impl CustomerService for CustomerServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCustomerReq,
    ) -> Result<i64> {
        let code = self
            .doc_seq
            .next_number(ctx, db, DocumentType::Customer)
            .await?;

        let id = self
            .repo
            .create(db, &code, &req, ctx.operator_id)
            .await
            ?;

        // Initialize state machine to Prospective
        self.state_machine
            .transition(
                ctx, db,
                "CustomerStatus",
                id,
                "Prospective",
                None,
            )
            .await
            .ok();

        // Check tax_number — warning only, not a failure
        let mut warnings = Vec::new();
        if let Some(ref tax) = req.tax_number {
            let exists = self
                .repo
                .check_tax_number_exists(db, tax)
                .await
                ?;
            if exists {
                warnings.push(format!(
                    "Tax number '{tax}' already exists in customers or suppliers"
                ));
            }
        }

        self.audit
            .record(ctx, db, "Customer", id, AuditAction::Create, None, None)
            .await?;

        self.event_bus
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::CustomerCreated,
                    aggregate_type: "Customer".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "customer_id": id,
                        "customer_code": code,
                        "customer_name": req.customer_name,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Customer> {
        self.repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))
    }

    #[allow(clippy::collapsible_if)]
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateCustomerReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))?;

        // Handle status transition via state machine
        if let Some(new_status) = req.status {
            if new_status != existing.status {
                let to_state = match new_status {
                    CustomerStatus::Prospective => "Prospective",
                    CustomerStatus::Active => "Active",
                    CustomerStatus::Inactive => "Inactive",
                    CustomerStatus::Blacklisted => "Blacklisted",
                };

                self.state_machine
                    .transition(ctx, db, "CustomerStatus", id, to_state, None)
                    .await?;

                // Publish blacklist event if transitioning to Blacklisted
                if new_status == CustomerStatus::Blacklisted {
                    self.event_bus
                        .publish(
                            ctx, db,
                            EventPublishRequest {
                                event_type: DomainEventType::CustomerBlacklisted,
                                aggregate_type: "Customer".to_string(),
                                aggregate_id: id,
                                payload: serde_json::json!({
                                    "customer_id": id,
                                    "old_status": existing.status.as_i16(),
                                    "new_status": new_status.as_i16(),
                                }),
                                idempotency_key: None,
                            },
                        )
                        .await?;
                }
            }
        }

        self.repo
            .update(db, id, &req)
            .await
            ?;

        self.audit
            .record(ctx, db, "Customer", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn delete(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        self.repo.delete(db, id).await
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CustomerQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Customer>> {
        self.repo
            .query(
                db,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await
            
    }

    async fn add_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        req: CreateContactReq,
    ) -> Result<i64> {
        // Verify customer exists
        self.repo
            .find_by_id(db, cid)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))?;

        let contact_id = self
            .contact_repo
            .create(db, cid, &req)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "CustomerContact",
                contact_id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(contact_id)
    }

    async fn update_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<()> {
        // Validate ownership
        self.validate_contact_ownership(ctx, db, cid, contact_id).await?;

        self.contact_repo
            .update(db, contact_id, &req)
            .await
            ?;

        self.audit
            .record(ctx, db, "CustomerContact", contact_id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn delete_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<()> {
        // Validate ownership
        self.validate_contact_ownership(ctx, db, cid, contact_id).await?;

        self.contact_repo
            .delete(db, contact_id)
            .await
            ?;

        self.audit
            .record(ctx, db, "CustomerContact", contact_id, AuditAction::Delete, None, None)
            .await?;

        Ok(())
    }

    async fn list_contacts(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerContact>> {
        self.contact_repo
            .find_by_customer_id(db, cid)
            .await
            
    }

    async fn add_address(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        req: CreateAddressReq,
    ) -> Result<i64> {
        // Verify customer exists
        self.repo
            .find_by_id(db, cid)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))?;

        let address_id = self
            .address_repo
            .create(db, cid, &req)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "CustomerAddress",
                address_id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(address_id)
    }

    async fn update_address(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        address_id: i64,
        req: UpdateAddressReq,
    ) -> Result<()> {
        // Validate address belongs to customer
        let address = self
            .address_repo
            .find_by_id(db, address_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CustomerAddress"))?;

        if address.customer_id != cid {
            return Err(DomainError::business_rule(
                "Address does not belong to the specified customer",
            ));
        }

        self.address_repo
            .update(db, address_id, &req)
            .await
            ?;

        self.audit
            .record(ctx, db, "CustomerAddress", address_id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn delete_address(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        address_id: i64,
    ) -> Result<()> {
        // Validate address belongs to customer
        let address = self
            .address_repo
            .find_by_id(db, address_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CustomerAddress"))?;

        if address.customer_id != cid {
            return Err(DomainError::business_rule(
                "Address does not belong to the specified customer",
            ));
        }

        self.address_repo
            .delete(db, address_id)
            .await
            ?;

        self.audit
            .record(ctx, db, "CustomerAddress", address_id, AuditAction::Delete, None, None)
            .await?;

        Ok(())
    }

    async fn list_addresses(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
    ) -> Result<Vec<CustomerAddress>> {
        self.address_repo
            .find_by_customer_id(db, cid)
            .await
            
    }

    async fn validate_contact_ownership(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        cid: i64,
        contact_id: i64,
    ) -> Result<bool> {
        let contact = self
            .contact_repo
            .find_by_id(db, contact_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CustomerContact"))?;

        if contact.customer_id != cid {
            return Err(DomainError::business_rule(
                "Contact does not belong to the specified customer",
            ));
        }

        Ok(true)
    }

    async fn claim(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let customer = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))?;

        if customer.owner_id.is_some() {
            return Err(DomainError::business_rule(
                "Customer is already claimed by another user",
            ));
        }

        if customer.status != CustomerStatus::Active {
            return Err(DomainError::business_rule(
                "Only active customers can be claimed",
            ));
        }

        self.repo
            .set_owner(
                db,
                id,
                Some(ctx.operator_id),
                ctx.department_id,
            )
            .await
            ?;

        self.audit
            .record(ctx, db, "Customer", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn transfer(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        new_owner_id: i64,
        new_department_id: Option<i64>,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Customer"))?;

        self.repo
            .set_owner(db, id, Some(new_owner_id), new_department_id)
            .await
            ?;

        self.audit
            .record(
                ctx, db,
                "Customer",
                id,
                AuditAction::Update,
                Some(serde_json::json!({
                    "action": "transfer",
                    "old_owner_id": existing.owner_id,
                    "new_owner_id": new_owner_id,
                    "old_department_id": existing.department_id,
                    "new_department_id": new_department_id,
                })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::CustomerTransferred,
                    aggregate_type: "Customer".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "customer_id": id,
                        "old_owner_id": existing.owner_id,
                        "new_owner_id": new_owner_id,
                        "old_department_id": existing.department_id,
                        "new_department_id": new_department_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }
}
