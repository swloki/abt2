use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use common::error::ServiceError;
use crate::models::ReconciliationQuery;
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    ReconciliationInsertParams, ReconciliationItemRow, ReconciliationRepo,
};
use crate::service::{AdjustmentItemParams, CreateReconciliationParams, ReconciliationService};

pub struct ReconciliationServiceImpl {
    pool: Arc<PgPool>,
}

impl ReconciliationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_DRAFT: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_APPROVED: i16 = 3;

#[async_trait]
impl ReconciliationService for ReconciliationServiceImpl {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateReconciliationParams<'_>,
    ) -> Result<i64> {
        let statement_no = DocumentSequenceRepo::next_number(&mut *executor, "RC").await?;

        // Auto-aggregate shipping and return items for this customer + period
        let shipping_items = Self::fetch_shipping_items(
            &self.pool,
            params.customer_name,
            params.period_year,
            params.period_month,
        )
        .await?;

        let return_items = Self::fetch_return_items(
            &self.pool,
            params.customer_name,
            params.period_year,
            params.period_month,
        )
        .await?;

        let shipping_total: Decimal = shipping_items.iter().map(|i| i.amount).sum();
        let return_total: Decimal = return_items.iter().map(|i| i.amount.abs()).sum();
        let net_amount = shipping_total - return_total;

        let statement_id = ReconciliationRepo::insert(
            &mut *executor,
            &ReconciliationInsertParams {
                statement_no: &statement_no,
                customer_name: params.customer_name,
                period_year: params.period_year,
                period_month: params.period_month,
                shipping_total,
                return_total,
                adjustment_total: Decimal::ZERO,
                net_amount,
                remark: params.remark,
                operator_id: params.operator_id,
            },
        )
        .await?;

        // Insert aggregated shipping items
        if !shipping_items.is_empty() {
            ReconciliationRepo::insert_items(&mut *executor, statement_id, &shipping_items).await?;
        }

        // Insert aggregated return items (negative amounts)
        if !return_items.is_empty() {
            ReconciliationRepo::insert_items(&mut *executor, statement_id, &return_items).await?;
        }

        Ok(statement_id)
    }

    async fn add_adjustments(
        &self,
        executor: Executor<'_>,
        statement_id: i64,
        items: Vec<AdjustmentItemParams>,
    ) -> Result<()> {
        let current_status = ReconciliationRepo::find_status(&self.pool, statement_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "reconciliation".into(),
                id: statement_id.to_string(),
            })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "仅草稿状态可添加调整项".into(),
            }
            .into());
        }

        // Delete existing adjustments
        ReconciliationRepo::delete_adjustments_by_statement(&mut *executor, statement_id).await?;

        // Insert new adjustment items
        let adj_rows: Vec<ReconciliationItemRow<'_>> = items
            .iter()
            .map(|i| ReconciliationItemRow {
                source_type: "adjustment",
                source_id: None,
                product_id: i.product_id,
                product_code: None,
                product_name: None,
                unit: None,
                quantity: Decimal::ONE,
                unit_price: i.unit_price,
                amount: i.amount,
                remark: i.remark.as_deref(),
            })
            .collect();

        if !adj_rows.is_empty() {
            ReconciliationRepo::insert_items(&mut *executor, statement_id, &adj_rows).await?;
        }

        // Recalculate totals
        ReconciliationRepo::recalculate_totals(&mut *executor, statement_id).await?;

        Ok(())
    }

    async fn update(&self, executor: Executor<'_>, statement_id: i64, remark: Option<&str>) -> Result<()> {
        let current_status = ReconciliationRepo::find_status(&self.pool, statement_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "reconciliation".into(),
                id: statement_id.to_string(),
            })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "仅草稿状态可修改备注".into(),
            }
            .into());
        }

        ReconciliationRepo::update_remark(executor, statement_id, remark).await?;
        Ok(())
    }

    async fn delete(&self, executor: Executor<'_>, statement_id: i64) -> Result<()> {
        let current_status = ReconciliationRepo::find_status(&self.pool, statement_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "reconciliation".into(),
                id: statement_id.to_string(),
            })?;

        if current_status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "仅草稿状态可删除".into(),
            }
            .into());
        }

        ReconciliationRepo::soft_delete(&mut *executor, statement_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, statement_id: i64) -> Result<Option<crate::models::ReconciliationStatement>> {
        ReconciliationRepo::find_by_id(&self.pool, statement_id).await
    }

    async fn list(&self, query: &ReconciliationQuery) -> Result<PaginatedResult<crate::models::ReconciliationStatement>> {
        let (items, total) = ReconciliationRepo::query(&self.pool, query).await?;
        let pagination = PaginationParams::new(
            query.page.unwrap_or(1) as u32,
            query.page_size.unwrap_or(20) as u32,
        );
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        executor: Executor<'_>,
        statement_id: i64,
        new_status: i16,
    ) -> Result<()> {
        let current_status = ReconciliationRepo::find_status(&self.pool, statement_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "reconciliation".into(),
                id: statement_id.to_string(),
            })?;

        let valid = matches!(
            (current_status, new_status),
            (STATUS_DRAFT, STATUS_CONFIRMED) | (STATUS_CONFIRMED, STATUS_APPROVED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("非法状态转换: {} → {}", current_status, new_status),
            }
            .into());
        }

        ReconciliationRepo::update_status(&mut *executor, statement_id, new_status).await?;
        Ok(())
    }
}

impl ReconciliationServiceImpl {
    async fn fetch_shipping_items(
        pool: &PgPool,
        customer_name: &str,
        period_year: i16,
        period_month: i16,
    ) -> Result<Vec<ReconciliationItemRow<'static>>> {
        let start_date = format!("{}-{:02}-01", period_year, period_month);
        let end_month = if period_month == 12 { 1 } else { period_month + 1 };
        let end_year = if period_month == 12 { period_year + 1 } else { period_year };
        let end_date = format!("{}-{:02}-01", end_year, end_month);

        let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<String>, Option<String>, Decimal, Decimal)>(
            r#"
            SELECT si.item_id, si.product_id, p.product_code, p.product_name, p.unit,
                   si.quantity, si.quantity * COALESCE(oi.unit_price, 0) as amount
            FROM shipping_request_items si
            JOIN shipping_requests sr ON sr.request_id = si.request_id
            LEFT JOIN sales_order_items oi ON oi.item_id = si.order_item_id
            LEFT JOIN products p ON p.product_id = si.product_id
            WHERE sr.customer_name = $1
              AND sr.shipped_at >= $2::date
              AND sr.shipped_at < $3::date
              AND sr.status = 3
              AND sr.deleted_at IS NULL
            "#,
        )
        .bind(customer_name)
        .bind(&start_date)
        .bind(&end_date)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(source_id, product_id, product_code, product_name, unit, quantity, amount)| {
                ReconciliationItemRow {
                    source_type: "shipping",
                    source_id: Some(source_id),
                    product_id,
                    product_code: product_code.as_deref().map(|s| s.to_string().leak() as &str),
                    product_name: product_name.as_deref().map(|s| s.to_string().leak() as &str),
                    unit: unit.as_deref().map(|s| s.to_string().leak() as &str),
                    quantity,
                    unit_price: if quantity != Decimal::ZERO { amount / quantity } else { Decimal::ZERO },
                    amount,
                    remark: None,
                }
            })
            .collect())
    }

    async fn fetch_return_items(
        pool: &PgPool,
        customer_name: &str,
        period_year: i16,
        period_month: i16,
    ) -> Result<Vec<ReconciliationItemRow<'static>>> {
        let start_date = format!("{}-{:02}-01", period_year, period_month);
        let end_month = if period_month == 12 { 1 } else { period_month + 1 };
        let end_year = if period_month == 12 { period_year + 1 } else { period_year };
        let end_date = format!("{}-{:02}-01", end_year, end_month);

        let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<String>, Option<String>, Decimal, Decimal)>(
            r#"
            SELECT ri.item_id, ri.product_id, p.product_code, p.product_name, p.unit,
                   ri.quantity, -(ri.quantity * ri.unit_price) as amount
            FROM sales_return_items ri
            JOIN sales_returns r ON r.return_id = ri.return_id
            LEFT JOIN products p ON p.product_id = ri.product_id
            WHERE r.customer_name = $1
              AND r.created_at >= $2::date
              AND r.created_at < $3::date
              AND r.status = 4
              AND r.deleted_at IS NULL
            "#,
        )
        .bind(customer_name)
        .bind(&start_date)
        .bind(&end_date)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(source_id, product_id, product_code, product_name, unit, quantity, amount)| {
                ReconciliationItemRow {
                    source_type: "return",
                    source_id: Some(source_id),
                    product_id,
                    product_code: product_code.as_deref().map(|s| s.to_string().leak() as &str),
                    product_name: product_name.as_deref().map(|s| s.to_string().leak() as &str),
                    unit: unit.as_deref().map(|s| s.to_string().leak() as &str),
                    quantity,
                    unit_price: if quantity != Decimal::ZERO { amount.abs() / quantity } else { Decimal::ZERO },
                    amount,
                    remark: None,
                }
            })
            .collect())
    }
}
