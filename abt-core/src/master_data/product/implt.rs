use sqlx::PgPool;

use super::model::*;
use super::repo::ProductRepo;
use super::service::ProductService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::enums::event::DomainEventType;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct ProductServiceImpl {
    repo: ProductRepo,
    pool: PgPool,
}

impl ProductServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: ProductRepo, pool }
    }
}

#[async_trait::async_trait]
impl ProductService for ProductServiceImpl {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateProductReq) -> Result<i64> {
        let code = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::Product).await?;

        if !self.repo.check_code_unique(db, &code)
            .await?
        {
            return Err(DomainError::duplicate(format!("Product code: {code}")));
        }

        let id = self.repo.create(db, &code, &req)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ProductStatus", id, "Active", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Product", entity_id: id, action: AuditAction::Create, changes: None, context: None }).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::ProductCreated,
                aggregate_type: "Product".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "product_id": id, "product_code": code }),
                idempotency_key: None,
            }).await?;

        Ok(id)
    }

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateProductReq) -> Result<()> {
        let _existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.update(db, id, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Product", entity_id: id, action: AuditAction::Update, changes: None, context: None }).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::ProductUpdated,
                aggregate_type: "Product".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "product_id": id }),
                idempotency_key: None,
            }).await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let _existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.delete(db, id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Product", entity_id: id, action: AuditAction::Delete, changes: None, context: None }).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::ProductDeleted,
                aggregate_type: "Product".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "product_id": id }),
                idempotency_key: None,
            }).await?;

        Ok(())
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Product> {
        self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))
    }

    async fn get_by_ids(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, ids: Vec<i64>) -> Result<Vec<Product>> {
        self.repo.find_by_ids(db, ids)
            .await
    }

    async fn list(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>> {
        self.repo.query(db, &filter, &page)
            .await
    }

    async fn check_product_usage(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, product_id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>> {
        let page = PageParams::new(query.page, query.page_size);

        let total = ProductRepo::count_product_usage_in_boms(db, product_id).await?;
        let items = ProductRepo::query_product_usage_in_boms(db, product_id, page.page_size as i64, page.offset() as i64).await?;

        Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
    }
}
