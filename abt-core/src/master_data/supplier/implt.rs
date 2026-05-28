use std::sync::Arc;

use super::model::*;
use super::repo::{SupplierBankAccountRepo, SupplierContactRepo, SupplierRepo};
use super::service::SupplierService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct SupplierServiceImpl {
    repo: SupplierRepo,
    contact_repo: SupplierContactRepo,
    bank_account_repo: SupplierBankAccountRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}

impl SupplierServiceImpl {
    pub fn new(
        repo: SupplierRepo,
        contact_repo: SupplierContactRepo,
        bank_account_repo: SupplierBankAccountRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        state_machine: Arc<dyn StateMachineService>,
    ) -> Self {
        Self {
            repo,
            contact_repo,
            bank_account_repo,
            doc_seq,
            audit,
            event_bus,
            state_machine,
        }
    }
}

#[async_trait::async_trait]
impl SupplierService for SupplierServiceImpl {
    // -- Supplier CRUD ---------------------------------------------------------

    #[allow(clippy::collapsible_if)]
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateSupplierReq,
    ) -> Result<i64> {
        let code = self
            .doc_seq
            .next_number(ctx, db, DocumentType::Supplier)
            .await?;

        // Check tax_number — warn but don't fail
        let mut warnings = Vec::new();
        if let Some(ref tax) = req.tax_number {
            if !tax.is_empty() {
                let exists = self
                    .repo
                    .check_tax_number_exists(db, tax)
                    .await
                    ?;
                if exists {
                    warnings.push(format!("tax_number '{tax}' already exists in suppliers or customers"));
                }
            }
        }

        let id = self
            .repo
            .create(db, &code, &req, ctx.operator_id)
            .await
            ?;

        // Init state machine — Prospective
        self.state_machine
            .transition(
                ctx, db,
                "SupplierStatus",
                id,
                SupplierStatus::Prospective.as_str(),
                None,
            )
            .await
            .ok();

        // Audit
        self.audit
            .record(
                ctx, db,
                "Supplier",
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        // Event: SupplierCreated
        let payload = serde_json::json!({
            "supplier_id": id,
            "supplier_code": code,
            "supplier_name": req.supplier_name,
        });
        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SupplierCreated,
                    aggregate_type: "Supplier".to_string(),
                    aggregate_id: id,
                    payload,
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Supplier> {
        self.repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Supplier"))
    }

    #[allow(clippy::collapsible_if)]
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSupplierReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Supplier"))?;

        // Handle status transition
        if let Some(new_status) = req.status {
            if new_status != existing.status {
                self.state_machine
                    .transition(
                        ctx, db,
                        "SupplierStatus",
                        id,
                        new_status.as_str(),
                        None,
                    )
                    .await?;

                // If blacklisted, publish SupplierBlacklisted event
                if new_status == SupplierStatus::Blacklisted {
                    let payload = serde_json::json!({
                        "supplier_id": id,
                        "old_status": existing.status.as_i16(),
                        "new_status": new_status.as_i16(),
                    });
                    self.event_bus
                        .publish(
                            ctx, db,
                            EventPublishRequest {
                                event_type: DomainEventType::SupplierBlacklisted,
                                aggregate_type: "Supplier".to_string(),
                                aggregate_id: id,
                                payload,
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
            .record(
                ctx,
                db,
                "Supplier",
                id,
                AuditAction::Update,
                None,
                None,
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: SupplierQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Supplier>> {
        self.repo
            .query(db, &filter, &page)
            .await
            
    }

    // -- Contacts --------------------------------------------------------------

    async fn add_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        req: CreateContactReq,
    ) -> Result<i64> {
        // Verify supplier exists
        self.repo
            .find_by_id(db, sid)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Supplier"))?;

        let contact_id = self
            .contact_repo
            .create(db, sid, &req)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "SupplierContact",
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
        sid: i64,
        contact_id: i64,
        req: UpdateContactReq,
    ) -> Result<()> {
        // Verify contact belongs to supplier
        let existing = self
            .contact_repo
            .find_by_id(db, contact_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("SupplierContact"))?;

        if existing.supplier_id != sid {
            return Err(DomainError::not_found("SupplierContact"));
        }

        self.contact_repo
            .update(db, contact_id, sid, &req)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "SupplierContact",
                contact_id,
                AuditAction::Update,
                None,
                None,
            )
            .await?;

        Ok(())
    }

    async fn delete_contact(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        contact_id: i64,
    ) -> Result<()> {
        // Verify contact belongs to supplier
        let existing = self
            .contact_repo
            .find_by_id(db, contact_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("SupplierContact"))?;

        if existing.supplier_id != sid {
            return Err(DomainError::not_found("SupplierContact"));
        }

        self.contact_repo
            .delete(db, contact_id, sid)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "SupplierContact",
                contact_id,
                AuditAction::Delete,
                None,
                None,
            )
            .await?;

        Ok(())
    }

    async fn list_contacts(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierContact>> {
        self.contact_repo
            .find_by_supplier_id(db, sid)
            .await
            
    }

    // -- Bank Accounts (P0 high-risk) ------------------------------------------

    async fn add_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        req: CreateBankAccountReq,
    ) -> Result<i64> {
        // Verify supplier exists
        self.repo
            .find_by_id(db, sid)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Supplier"))?;

        let account_id = self
            .bank_account_repo
            .create(db, sid, &req)
            .await
            ?;

        // P0: mandatory audit with field-level detail
        let changes = serde_json::json!({
            "action": "add_bank_account",
            "supplier_id": sid,
            "account_id": account_id,
            "bank_name": req.bank_name,
            "account_name": req.account_name,
            "account_number": req.account_number,
        });
        self.audit
            .record(
                ctx, db,
                "SupplierBankAccount",
                account_id,
                AuditAction::Create,
                Some(changes),
                None,
            )
            .await?;

        // P0: event SupplierBankAccountChanged
        let payload = serde_json::json!({
            "supplier_id": sid,
            "account_id": account_id,
            "action": "created",
        });
        self.event_bus
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::SupplierBankAccountChanged,
                    aggregate_type: "Supplier".to_string(),
                    aggregate_id: sid,
                    payload,
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(account_id)
    }

    #[allow(clippy::collapsible_if)]
    async fn update_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        account_id: i64,
        req: UpdateBankAccountReq,
    ) -> Result<()> {
        // Update returns the before-state for diff generation
        let before = self
            .bank_account_repo
            .update(db, account_id, sid, &req)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("SupplierBankAccount"))?;

        // P0: mandatory audit with field-level diff
        let mut field_diffs = serde_json::Map::new();
        if let Some(ref new_val) = req.bank_name {
            if before.bank_name != *new_val {
                field_diffs.insert("bank_name".into(), serde_json::json!({
                    "old": before.bank_name,
                    "new": new_val,
                }));
            }
        }
        if let Some(ref new_val) = req.account_name {
            if before.account_name != *new_val {
                field_diffs.insert("account_name".into(), serde_json::json!({
                    "old": before.account_name,
                    "new": new_val,
                }));
            }
        }
        if let Some(ref new_val) = req.account_number {
            if before.account_number != *new_val {
                field_diffs.insert("account_number".into(), serde_json::json!({
                    "old": before.account_number,
                    "new": new_val,
                    "sensitive": true,
                }));
            }
        }
        if let Some(new_val) = req.is_default {
            if before.is_default != new_val {
                field_diffs.insert("is_default".into(), serde_json::json!({
                    "old": before.is_default,
                    "new": new_val,
                }));
            }
        }

        let changes = serde_json::json!({
            "action": "update_bank_account",
            "supplier_id": sid,
            "account_id": account_id,
            "diffs": field_diffs,
        });
        self.audit
            .record(
                ctx, db,
                "SupplierBankAccount",
                account_id,
                AuditAction::Update,
                Some(changes),
                None,
            )
            .await?;

        // P0: event SupplierBankAccountChanged
        let payload = serde_json::json!({
            "supplier_id": sid,
            "account_id": account_id,
            "action": "updated",
            "changed_fields": field_diffs.keys().collect::<Vec<_>>(),
        });
        self.event_bus
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::SupplierBankAccountChanged,
                    aggregate_type: "Supplier".to_string(),
                    aggregate_id: sid,
                    payload,
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete_bank_account(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
        account_id: i64,
    ) -> Result<()> {
        // Verify account belongs to supplier
        let existing = self
            .bank_account_repo
            .find_by_id(db, account_id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("SupplierBankAccount"))?;

        if existing.supplier_id != sid {
            return Err(DomainError::not_found("SupplierBankAccount"));
        }

        self.bank_account_repo
            .delete(db, account_id, sid)
            .await
            ?;

        // P0: mandatory audit
        let changes = serde_json::json!({
            "action": "delete_bank_account",
            "supplier_id": sid,
            "account_id": account_id,
        });
        self.audit
            .record(
                ctx, db,
                "SupplierBankAccount",
                account_id,
                AuditAction::Delete,
                Some(changes),
                None,
            )
            .await?;

        // P0: event SupplierBankAccountChanged
        let payload = serde_json::json!({
            "supplier_id": sid,
            "account_id": account_id,
            "action": "deleted",
        });
        self.event_bus
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::SupplierBankAccountChanged,
                    aggregate_type: "Supplier".to_string(),
                    aggregate_id: sid,
                    payload,
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn list_bank_accounts(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        sid: i64,
    ) -> Result<Vec<SupplierBankAccount>> {
        self.bank_account_repo
            .find_by_supplier_id(db, sid)
            .await
            
    }
}
