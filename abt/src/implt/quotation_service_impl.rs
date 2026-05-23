use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use common::error::ServiceError;
use crate::models::QuotationQuery;
use crate::repositories::{
    DocumentSequenceRepo, PaginatedResult, PaginationParams,
    QuotationInsertParams, QuotationItemRow, QuotationRepo, QuotationUpdateParams, Executor,
};
use crate::service::{CreateQuotationItemParams, CreateQuotationParams, QuotationService, UpdateQuotationParams};

pub struct QuotationServiceImpl {
    pool: Arc<PgPool>,
}

impl QuotationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_DRAFT: i16 = 1;
const STATUS_SUBMITTED: i16 = 2;
const STATUS_ACCEPTED: i16 = 3;
const STATUS_REJECTED: i16 = 4;
const STATUS_EXPIRED: i16 = 5;

fn calc_subtotal(unit_price: Decimal, quantity: Decimal, discount: Decimal) -> Decimal {
    unit_price * quantity * (Decimal::ONE - discount)
}

fn build_item_rows(items: &[CreateQuotationItemParams]) -> Vec<QuotationItemRow<'_>> {
    items
        .iter()
        .map(|i| {
            let subtotal = calc_subtotal(i.unit_price, i.quantity, i.discount);
            QuotationItemRow {
                product_id: i.product_id,
                product_code: i.product_code.as_deref(),
                product_name: i.product_name.as_deref(),
                unit: i.unit.as_deref(),
                unit_price: i.unit_price,
                quantity: i.quantity,
                discount: i.discount,
                subtotal,
                remark: i.remark.as_deref(),
            }
        })
        .collect()
}

#[async_trait]
impl QuotationService for QuotationServiceImpl {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateQuotationParams<'_>,
        items: Vec<CreateQuotationItemParams>,
    ) -> Result<i64> {
        let quotation_no = DocumentSequenceRepo::next_number(&mut *executor, "QT").await?;

        let total_amount = items.iter().fold(Decimal::ZERO, |acc, item| {
            acc + calc_subtotal(item.unit_price, item.quantity, item.discount)
        });

        let quotation_id = QuotationRepo::insert(
            &mut *executor,
            &QuotationInsertParams {
                quotation_no: &quotation_no,
                customer_name: params.customer_name,
                contact_person: params.contact_person,
                contact_phone: params.contact_phone,
                total_amount,
                remark: params.remark,
                valid_until: params.valid_until,
                operator_id: params.operator_id,
            },
        )
        .await?;

        let item_rows = build_item_rows(&items);
        QuotationRepo::insert_items(&mut *executor, quotation_id, &item_rows).await?;

        Ok(quotation_id)
    }

    async fn update(
        &self,
        executor: Executor<'_>,
        quotation_id: i64,
        params: &UpdateQuotationParams<'_>,
        items: Vec<CreateQuotationItemParams>,
    ) -> Result<()> {
        let current_status = QuotationRepo::find_status(&self.pool, quotation_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound { resource: "quotation".into(), id: quotation_id.to_string() })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation { message: "仅草稿状态可编辑".into() }.into());
        }

        let total_amount = items.iter().fold(Decimal::ZERO, |acc, item| {
            acc + calc_subtotal(item.unit_price, item.quantity, item.discount)
        });

        QuotationRepo::update(
            &mut *executor,
            quotation_id,
            &QuotationUpdateParams {
                customer_name: params.customer_name,
                contact_person: params.contact_person,
                contact_phone: params.contact_phone,
                total_amount,
                remark: params.remark,
                valid_until: params.valid_until,
            },
        )
        .await?;

        QuotationRepo::delete_by_quotation(&mut *executor, quotation_id).await?;

        let item_rows = build_item_rows(&items);
        QuotationRepo::insert_items(&mut *executor, quotation_id, &item_rows).await?;

        Ok(())
    }

    async fn delete(&self, executor: Executor<'_>, quotation_id: i64) -> Result<()> {
        let current_status = QuotationRepo::find_status(&self.pool, quotation_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound { resource: "quotation".into(), id: quotation_id.to_string() })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation { message: "仅草稿状态可删除".into() }.into());
        }

        QuotationRepo::soft_delete(&mut *executor, quotation_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, quotation_id: i64) -> Result<Option<crate::models::Quotation>> {
        QuotationRepo::find_by_id(&self.pool, quotation_id).await
    }

    async fn list(&self, query: &QuotationQuery) -> Result<PaginatedResult<crate::models::Quotation>> {
        let (items, total) = QuotationRepo::query(&self.pool, query).await?;
        let pagination = PaginationParams::new(
            query.page.unwrap_or(1) as u32,
            query.page_size.unwrap_or(20) as u32,
        );
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        executor: Executor<'_>,
        quotation_id: i64,
        new_status: i16,
    ) -> Result<()> {
        let current_status = QuotationRepo::find_status(&self.pool, quotation_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound { resource: "quotation".into(), id: quotation_id.to_string() })?;

        let valid = matches!(
            (current_status, new_status),
            (STATUS_DRAFT, STATUS_SUBMITTED)
                | (STATUS_DRAFT, STATUS_EXPIRED)
                | (STATUS_SUBMITTED, STATUS_ACCEPTED)
                | (STATUS_SUBMITTED, STATUS_REJECTED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("非法状态转换: {} → {}", current_status, new_status),
            }
            .into());
        }

        QuotationRepo::update_status(&mut *executor, quotation_id, new_status).await?;
        Ok(())
    }
}
