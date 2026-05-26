use std::sync::Arc;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::model::*;
use super::repo::PriceRepo;
use super::service::ProductPriceService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{PageParams, PaginatedResult, ServiceContext, Result};

pub struct PriceServiceImpl {
    repo: PriceRepo,
    audit: Arc<dyn AuditLogService>,
}

impl PriceServiceImpl {
    pub fn new(repo: PriceRepo, audit: Arc<dyn AuditLogService>) -> Self {
        Self { repo, audit }
    }
}

#[async_trait::async_trait]
impl ProductPriceService for PriceServiceImpl {
    async fn update_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, new_price: Decimal, remark: String) -> Result<()> {
        let old_price = self.repo.find_latest_price(ctx.executor, product_id, price_type)
            .await?
            .map(|e| e.new_price);

        self.repo.create(ctx.executor, product_id, price_type, old_price, new_price, ctx.operator_id, &remark)
            .await?;

        let changes = serde_json::json!({
            "product_id": product_id,
            "price_type": price_type.as_i16(),
            "old_price": old_price,
            "new_price": new_price,
            "remark": remark,
        });
        self.audit.record(ctx, "PriceLog", product_id, AuditAction::Update, Some(changes), None).await?;
        Ok(())
    }

    async fn list_price_history(&self, ctx: ServiceContext<'_>, query: PriceQuery, page: PageParams) -> Result<PaginatedResult<PriceLogEntry>> {
        self.repo.query(ctx.executor, &query, &page)
            .await
    }

    async fn get_current_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType) -> Result<Option<Decimal>> {
        Ok(self.repo.find_latest_price(ctx.executor, product_id, price_type)
            .await?
            .map(|e| e.new_price))
    }

    async fn get_price_at(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, as_of: DateTime<Utc>) -> Result<Option<Decimal>> {
        Ok(self.repo.find_price_at(ctx.executor, product_id, price_type, as_of)
            .await?
            .map(|e| e.new_price))
    }
}
