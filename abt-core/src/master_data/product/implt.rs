use std::sync::Arc;

use super::model::*;
use super::repo::ProductRepo;
use super::service::ProductService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::enums::event::DomainEventType;
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
    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateProductReq) -> Result<i64> {
        let code = self.doc_seq.next_number(ctx.reborrow(), DocumentType::Product).await?;

        if !self.repo.check_code_unique(ctx.executor, &code)
            .await?
        {
            return Err(DomainError::duplicate(format!("Product code: {code}")));
        }

        let id = self.repo.create(ctx.executor, &code, &req)
            .await?;

        self.state_machine
            .transition(ctx.reborrow(), "ProductStatus", id, "Active", None)
            .await
            .ok();

        self.audit.record(ctx.reborrow(), "Product", id, AuditAction::Create, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::ProductCreated,
            aggregate_type: "Product".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "product_id": id, "product_code": code }),
            idempotency_key: None,
        }).await?;

        Ok(id)
    }

    async fn update(&self, mut ctx: ServiceContext<'_>, id: i64, req: UpdateProductReq) -> Result<()> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.update(ctx.executor, id, &req)
            .await?;

        self.audit.record(ctx.reborrow(), "Product", id, AuditAction::Update, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::ProductUpdated,
            aggregate_type: "Product".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "product_id": id }),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn delete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))?;

        self.repo.delete(ctx.executor, id)
            .await?;

        self.audit.record(ctx.reborrow(), "Product", id, AuditAction::Delete, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::ProductDeleted,
            aggregate_type: "Product".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "product_id": id }),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Product> {
        self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product"))
    }

    async fn get_by_ids(&self, ctx: ServiceContext<'_>, ids: Vec<i64>) -> Result<Vec<Product>> {
        self.repo.find_by_ids(ctx.executor, ids)
            .await
    }

    async fn list(&self, ctx: ServiceContext<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>> {
        self.repo.query(ctx.executor, &filter, &page)
            .await
    }

    async fn check_product_usage(&self, ctx: ServiceContext<'_>, product_id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>> {
        let page = PageParams::new(query.page, query.page_size);

        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(DISTINCT b.bom_id) FROM bom_nodes bn JOIN boms b ON bn.bom_id = b.bom_id WHERE bn.product_id = $1 AND b.deleted_at IS NULL"#,
        )
        .bind(product_id)
        .fetch_one(&mut *ctx.executor)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let items = sqlx::query_as::<sqlx::Postgres, UsageEntry>(
            r#"SELECT 'bom' AS source_type, b.bom_id AS source_id, b.bom_name AS source_name
               FROM bom_nodes bn JOIN boms b ON bn.bom_id = b.bom_id
               WHERE bn.product_id = $1 AND b.deleted_at IS NULL
               GROUP BY b.bom_id, b.bom_name
               ORDER BY b.bom_id DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(product_id)
        .bind(page.page_size as i64)
        .bind(page.offset() as i64)
        .fetch_all(ctx.executor)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total as u64, page.page, page.page_size))
    }
}
