use std::sync::Arc;

use super::model::*;
use super::repo::ProductRepo;
use super::service::ProductService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct ProductServiceImpl {
    repo: ProductRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    #[allow(dead_code)]
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}

impl ProductServiceImpl {
    pub fn new(
        repo: ProductRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        state_machine: Arc<dyn StateMachineService>,
    ) -> Self {
        Self { repo, doc_seq, audit, event_bus, state_machine }
    }
}

#[async_trait::async_trait]
impl ProductService for ProductServiceImpl {
    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateProductReq) -> Result<i64, DomainError> {
        let code = self.doc_seq.next_number(ctx.reborrow(), DocumentType::Product).await?;

        if !self.repo.check_code_unique(ctx.executor, &code)
            .await.map_err(DomainError::Internal)?
        {
            return Err(DomainError::duplicate(format!("Product code: {code}")));
        }

        let id = self.repo.create(ctx.executor, &code, &req)
            .await.map_err(DomainError::Internal)?;

        self.state_machine
            .transition(ctx.reborrow(), "ProductStatus", id, "Active", None)
            .await
            .ok();

        self.audit.record(ctx.reborrow(), "Product", id, AuditAction::Create, None, None).await?;

        Ok(id)
    }

    async fn update(&self, mut ctx: ServiceContext<'_>, id: i64, req: UpdateProductReq) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.update(ctx.executor, id, &req)
            .await.map_err(DomainError::Internal)?;

        self.audit.record(ctx.reborrow(), "Product", id, AuditAction::Update, None, None).await?;

        Ok(())
    }

    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.delete(ctx.executor, id)
            .await.map_err(DomainError::Internal)?;

        self.audit.record(ctx, "Product", id, AuditAction::Delete, None, None).await?;
        Ok(())
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Product, DomainError> {
        self.repo.find_by_id(ctx.executor, id)
            .await.map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Product"))
    }

    async fn get_by_ids(&self, ctx: ServiceContext<'_>, ids: Vec<i64>) -> Result<Vec<Product>, DomainError> {
        self.repo.find_by_ids(ctx.executor, ids)
            .await.map_err(DomainError::Internal)
    }

    async fn list(&self, ctx: ServiceContext<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>, DomainError> {
        self.repo.query(ctx.executor, &filter, &page)
            .await.map_err(DomainError::Internal)
    }

    async fn check_product_usage(&self, _ctx: ServiceContext<'_>, _id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>, DomainError> {
        let page = PageParams::new(query.page, query.page_size);
        Ok(PaginatedResult::empty(page.page, page.page_size))
    }
}
