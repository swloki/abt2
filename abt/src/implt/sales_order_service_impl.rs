use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use common::error::ServiceError;
use crate::models::SalesOrderQuery;
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    SalesOrderInsertParams, SalesOrderItemRow, SalesOrderRepo, SalesOrderUpdateHeaderParams,
};
use crate::service::{CreateSalesOrderItemParams, CreateSalesOrderParams, SalesOrderService, UpdateSalesOrderHeaderParams};

pub struct SalesOrderServiceImpl {
    pool: Arc<PgPool>,
}

impl SalesOrderServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_DRAFT: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_IN_PROGRESS: i16 = 3;
const STATUS_COMPLETED: i16 = 4;
const STATUS_CANCELLED: i16 = 5;

fn calc_subtotal(unit_price: Decimal, quantity: Decimal, discount: Decimal) -> Decimal {
    unit_price * quantity * (Decimal::ONE - discount)
}

fn build_item_rows(items: &[CreateSalesOrderItemParams]) -> Vec<SalesOrderItemRow<'_>> {
    items
        .iter()
        .map(|i| {
            let subtotal = calc_subtotal(i.unit_price, i.quantity, i.discount);
            SalesOrderItemRow {
                product_id: i.product_id,
                product_code: None,
                product_name: None,
                unit: None,
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
impl SalesOrderService for SalesOrderServiceImpl {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateSalesOrderParams<'_>,
        items: Vec<CreateSalesOrderItemParams>,
    ) -> Result<i64> {
        let order_no = DocumentSequenceRepo::next_number(&mut *executor, "SO").await?;

        let total_amount = items.iter().fold(Decimal::ZERO, |acc, item| {
            acc + calc_subtotal(item.unit_price, item.quantity, item.discount)
        });

        let order_id = SalesOrderRepo::insert(
            &mut *executor,
            &SalesOrderInsertParams {
                order_no: &order_no,
                quotation_id: params.quotation_id,
                customer_name: params.customer_name,
                contact_person: params.contact_person,
                contact_phone: params.contact_phone,
                total_amount,
                remark: params.remark,
                delivery_date: params.delivery_date,
                operator_id: params.operator_id,
            },
        )
        .await?;

        let item_rows = build_item_rows(&items);
        SalesOrderRepo::insert_items(&mut *executor, order_id, &item_rows).await?;

        Ok(order_id)
    }

    async fn update_header(
        &self,
        executor: Executor<'_>,
        order_id: i64,
        params: &UpdateSalesOrderHeaderParams<'_>,
    ) -> Result<()> {
        let current_status = SalesOrderRepo::find_status(&self.pool, order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: order_id.to_string(),
            })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "仅草稿状态可编辑".into(),
            }
            .into());
        }

        let total_amount = Decimal::ZERO; // header update does not recalculate

        SalesOrderRepo::update_header(
            &mut *executor,
            order_id,
            &SalesOrderUpdateHeaderParams {
                customer_name: params.customer_name,
                contact_person: params.contact_person,
                contact_phone: params.contact_phone,
                total_amount,
                remark: params.remark,
                delivery_date: params.delivery_date,
            },
        )
        .await?;

        Ok(())
    }

    async fn delete(&self, executor: Executor<'_>, order_id: i64) -> Result<()> {
        let current_status = SalesOrderRepo::find_status(&self.pool, order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: order_id.to_string(),
            })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "仅草稿状态可删除".into(),
            }
            .into());
        }

        SalesOrderRepo::soft_delete(&mut *executor, order_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, order_id: i64) -> Result<Option<crate::models::SalesOrder>> {
        SalesOrderRepo::find_by_id(&self.pool, order_id).await
    }

    async fn list(&self, query: &SalesOrderQuery) -> Result<PaginatedResult<crate::models::SalesOrder>> {
        let (items, total) = SalesOrderRepo::query(&self.pool, query).await?;
        let pagination = PaginationParams::new(
            query.page.unwrap_or(1) as u32,
            query.page_size.unwrap_or(20) as u32,
        );
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        executor: Executor<'_>,
        order_id: i64,
        new_status: i16,
    ) -> Result<()> {
        let current_status = SalesOrderRepo::find_status(&self.pool, order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: order_id.to_string(),
            })?;

        let valid = matches!(
            (current_status, new_status),
            (STATUS_DRAFT, STATUS_CONFIRMED)
                | (STATUS_CONFIRMED, STATUS_IN_PROGRESS)
                | (STATUS_IN_PROGRESS, STATUS_COMPLETED)
                | (STATUS_DRAFT, STATUS_CANCELLED)
                | (STATUS_CONFIRMED, STATUS_CANCELLED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("非法状态转换: {} → {}", current_status, new_status),
            }
            .into());
        }

        SalesOrderRepo::update_status(&mut *executor, order_id, new_status).await?;
        Ok(())
    }
}
