use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use common::error::ServiceError;
use crate::models::{Quotation, QuotationQuery};
use crate::repositories::{DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, ProductRepo, QuotationRepo};
use crate::service::QuotationService;

pub struct QuotationServiceImpl {
    pool: Arc<PgPool>,
}

impl QuotationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

// Status constants
const STATUS_DRAFT: i16 = 1;
const STATUS_SUBMITTED: i16 = 2;
const STATUS_ACCEPTED: i16 = 3;
const STATUS_REJECTED: i16 = 4;
const STATUS_EXPIRED: i16 = 5;

#[async_trait]
impl QuotationService for QuotationServiceImpl {
    async fn create(&self, operator_id: Option<i64>, mut quotation: Quotation, executor: Executor<'_>) -> Result<i64> {
        // 1. Validate product_ids exist
        let product_ids: Vec<i64> = quotation.items.iter().map(|i| i.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;
        if products.len() != product_ids.len() {
            let found_ids: Vec<i64> = products.iter().map(|p| p.product_id).collect();
            let missing: Vec<i64> = product_ids.into_iter().filter(|id| !found_ids.contains(id)).collect();
            return Err(ServiceError::NotFound {
                resource: "Product".to_string(),
                id: missing.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","),
            }.into());
        }

        // 2. Generate quotation number
        let quotation_no = DocumentSequenceRepo::next_number(&mut *executor, "QT").await?;
        quotation.quotation_no = quotation_no;
        quotation.operator_id = operator_id;

        // 3. Calculate subtotals and total
        let mut total_amount = Decimal::ZERO;
        for item in &mut quotation.items {
            item.subtotal = item.unit_price * item.quantity * item.discount;
            total_amount += item.subtotal;
            // Fill product info from products
            if let Some(product) = products.iter().find(|p| p.product_id == item.product_id) {
                item.product_code = Some(product.product_code.clone());
                item.product_name = Some(product.pdt_name.clone());
                item.unit = Some(product.unit.clone());
            }
        }
        quotation.total_amount = total_amount;
        quotation.status = STATUS_DRAFT;

        // 4. Insert main table
        let quotation_id = QuotationRepo::insert(&mut *executor, &quotation).await?;

        // 5. Set quotation_id on items and insert
        for item in &mut quotation.items {
            item.quotation_id = quotation_id;
        }
        QuotationRepo::insert_items(&mut *executor, &quotation.items).await?;

        Ok(quotation_id)
    }

    async fn update(&self, _operator_id: Option<i64>, mut quotation: Quotation, executor: Executor<'_>) -> Result<()> {
        // 1. Find existing and validate status is Draft
        let existing = QuotationRepo::find_by_id(&self.pool, quotation.quotation_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Quotation".to_string(),
                id: quotation.quotation_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的报价单可以编辑".to_string(),
            }.into());
        }

        // 2. Validate product_ids
        let product_ids: Vec<i64> = quotation.items.iter().map(|i| i.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;
        if products.len() != product_ids.len() {
            let found_ids: Vec<i64> = products.iter().map(|p| p.product_id).collect();
            let missing: Vec<i64> = product_ids.into_iter().filter(|id| !found_ids.contains(id)).collect();
            return Err(ServiceError::NotFound {
                resource: "Product".to_string(),
                id: missing.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","),
            }.into());
        }

        // 3. Recalculate
        let mut total_amount = Decimal::ZERO;
        for item in &mut quotation.items {
            item.quotation_id = existing.quotation_id;
            item.subtotal = item.unit_price * item.quantity * item.discount;
            total_amount += item.subtotal;
            if let Some(product) = products.iter().find(|p| p.product_id == item.product_id) {
                item.product_code = Some(product.product_code.clone());
                item.product_name = Some(product.pdt_name.clone());
                item.unit = Some(product.unit.clone());
            }
        }
        quotation.total_amount = total_amount;

        // 4. Update main table, delete old items, insert new items
        QuotationRepo::update(&mut *executor, &quotation).await?;
        QuotationRepo::delete_by_quotation(&mut *executor, existing.quotation_id).await?;
        QuotationRepo::insert_items(&mut *executor, &quotation.items).await?;

        Ok(())
    }

    async fn delete(&self, quotation_id: i64, executor: Executor<'_>) -> Result<()> {
        let existing = QuotationRepo::find_by_id(&self.pool, quotation_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Quotation".to_string(),
                id: quotation_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的报价单可以删除".to_string(),
            }.into());
        }

        QuotationRepo::soft_delete(executor, quotation_id).await
    }

    async fn get_by_id(&self, quotation_id: i64) -> Result<Option<Quotation>> {
        let mut quotation = QuotationRepo::find_by_id(&self.pool, quotation_id).await?;
        if let Some(ref mut q) = quotation {
            q.items = QuotationRepo::find_by_quotation_id(&self.pool, quotation_id).await?;
        }
        Ok(quotation)
    }

    async fn list(&self, query: QuotationQuery) -> Result<PaginatedResult<Quotation>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100) as u32;
        let pagination = PaginationParams::new(page, page_size);

        let items = QuotationRepo::query(&self.pool, &query).await?;
        let total = QuotationRepo::query_count(&self.pool, &query).await?;

        // Fill items for each quotation
        let mut filled_items = Vec::new();
        for mut q in items {
            q.items = QuotationRepo::find_by_quotation_id(&self.pool, q.quotation_id).await?;
            filled_items.push(q);
        }

        Ok(PaginatedResult::new(filled_items, total as u64, &pagination))
    }

    async fn update_status(&self, quotation_id: i64, status: i16, executor: Executor<'_>) -> Result<()> {
        let existing = QuotationRepo::find_by_id(&self.pool, quotation_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "Quotation".to_string(),
                id: quotation_id.to_string(),
            })?;

        // Validate transition
        let valid = matches!(
            (existing.status, status),
            (STATUS_DRAFT, STATUS_SUBMITTED)
            | (STATUS_DRAFT, STATUS_EXPIRED)
            | (STATUS_SUBMITTED, STATUS_ACCEPTED)
            | (STATUS_SUBMITTED, STATUS_REJECTED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("不允许从状态 {} 转换到 {}", existing.status, status),
            }.into());
        }

        QuotationRepo::update_status(executor, quotation_id, status).await
    }
}
