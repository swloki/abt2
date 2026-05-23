use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use common::error::ServiceError;
use crate::models::{SalesReturnQuery, ShippingRequestItem};
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    SalesReturnInsertParams, SalesReturnItemRow, SalesReturnRepo,
    SalesReturnUpdateParams, ShippingRequestRepo, SalesOrderRepo,
};
use crate::service::{CreateSalesReturnItemParams, CreateSalesReturnParams, SalesReturnService, UpdateSalesReturnParams};

pub struct SalesReturnServiceImpl {
    pool: Arc<PgPool>,
}

impl SalesReturnServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_PENDING: i16 = 1;
const STATUS_APPROVED: i16 = 2;
const STATUS_RECEIVED: i16 = 3;
const STATUS_COMPLETED: i16 = 4;
const STATUS_REJECTED: i16 = 5;

#[async_trait]
impl SalesReturnService for SalesReturnServiceImpl {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateSalesReturnParams<'_>,
        items: Vec<CreateSalesReturnItemParams>,
    ) -> Result<i64> {
        // Validate shipping request exists and is Shipped
        let shipping = ShippingRequestRepo::find_by_id(&self.pool, params.request_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "shipping_request".into(),
                id: params.request_id.to_string(),
            })?;

        if shipping.status != 3 {
            return Err(ServiceError::BusinessValidation {
                message: "发货申请未完成发货，无法退货".into(),
            }
            .into());
        }

        // Validate return quantities
        validate_return_quantities(&shipping.items, &items)?;

        let order_id = shipping.order_id;
        let customer_name = &shipping.customer_name;

        let return_no = DocumentSequenceRepo::next_number(&mut *executor, "RT").await?;

        // Calculate total amount from order items
        let order = SalesOrderRepo::find_by_id(&self.pool, order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: order_id.to_string(),
            })?;

        let total_amount = items.iter().fold(Decimal::ZERO, |acc, ri| {
            let shipping_item = shipping.items.iter().find(|si| si.item_id == ri.request_item_id);
            let unit_price = shipping_item
                .and_then(|si| order.items.iter().find(|oi| oi.item_id == si.order_item_id))
                .map(|oi| oi.unit_price)
                .unwrap_or(Decimal::ZERO);
            acc + unit_price * ri.quantity
        });

        let return_id = SalesReturnRepo::insert(
            &mut *executor,
            &SalesReturnInsertParams {
                return_no: &return_no,
                request_id: params.request_id,
                order_id,
                customer_name,
                total_amount,
                remark: params.remark,
                reason: params.reason,
                operator_id: params.operator_id,
            },
        )
        .await?;

        let item_rows = build_return_item_rows(&shipping.items, &order.items, &items);

        SalesReturnRepo::insert_items(&mut *executor, return_id, &item_rows).await?;

        Ok(return_id)
    }

    async fn update(
        &self,
        executor: Executor<'_>,
        return_id: i64,
        params: &UpdateSalesReturnParams<'_>,
        _items: Vec<CreateSalesReturnItemParams>,
    ) -> Result<()> {
        let current_status = SalesReturnRepo::find_status(&self.pool, return_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_return".into(),
                id: return_id.to_string(),
            })?;

        if current_status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "仅待处理状态可编辑".into(),
            }
            .into());
        }

        SalesReturnRepo::update(
            &mut *executor,
            return_id,
            &SalesReturnUpdateParams {
                remark: params.remark,
                reason: params.reason,
            },
        )
        .await?;

        Ok(())
    }

    async fn delete(&self, executor: Executor<'_>, return_id: i64) -> Result<()> {
        let current_status = SalesReturnRepo::find_status(&self.pool, return_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_return".into(),
                id: return_id.to_string(),
            })?;

        if current_status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "仅待处理状态可删除".into(),
            }
            .into());
        }

        SalesReturnRepo::soft_delete(&mut *executor, return_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, return_id: i64) -> Result<Option<crate::models::SalesReturn>> {
        SalesReturnRepo::find_by_id(&self.pool, return_id).await
    }

    async fn list(&self, query: &SalesReturnQuery) -> Result<PaginatedResult<crate::models::SalesReturn>> {
        let (items, total) = SalesReturnRepo::query(&self.pool, query).await?;
        let pagination = PaginationParams::new(
            query.page.unwrap_or(1) as u32,
            query.page_size.unwrap_or(20) as u32,
        );
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        executor: Executor<'_>,
        return_id: i64,
        new_status: i16,
    ) -> Result<()> {
        let current_status = SalesReturnRepo::find_status(&self.pool, return_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_return".into(),
                id: return_id.to_string(),
            })?;

        let valid = matches!(
            (current_status, new_status),
            (STATUS_PENDING, STATUS_APPROVED)
                | (STATUS_APPROVED, STATUS_RECEIVED)
                | (STATUS_RECEIVED, STATUS_COMPLETED)
                | (STATUS_PENDING, STATUS_REJECTED)
                | (STATUS_APPROVED, STATUS_REJECTED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("非法状态转换: {} → {}", current_status, new_status),
            }
            .into());
        }

        SalesReturnRepo::update_status(&mut *executor, return_id, new_status).await?;

        // On completed: update order item returned quantities
        if new_status == STATUS_COMPLETED {
            let ret = SalesReturnRepo::find_by_id(&self.pool, return_id)
                .await?
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "sales_return".into(),
                    id: return_id.to_string(),
                })?;

            for item in &ret.items {
                SalesOrderRepo::update_returned_qty(&mut *executor, item.order_item_id, item.quantity)
                    .await?;
            }
        }

        Ok(())
    }
}

fn validate_return_quantities(
    shipping_items: &[ShippingRequestItem],
    return_items: &[CreateSalesReturnItemParams],
) -> Result<()> {
    for ri in return_items {
        let shipping_item = shipping_items
            .iter()
            .find(|si| si.item_id == ri.request_item_id)
            .ok_or_else(|| ServiceError::BusinessValidation {
                message: format!("发货行项目 {} 不存在", ri.request_item_id),
            })?;

        if ri.quantity > shipping_item.quantity {
            return Err(ServiceError::BusinessValidation {
                message: format!(
                    "退货数量 {} 超出发货数量 {}（发货行 {}）",
                    ri.quantity, shipping_item.quantity, ri.request_item_id
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn build_return_item_rows<'a>(
    shipping_items: &'a [ShippingRequestItem],
    order_items: &'a [crate::models::SalesOrderItem],
    return_items: &'a [CreateSalesReturnItemParams],
) -> Vec<SalesReturnItemRow<'a>> {
    return_items
        .iter()
        .map(|ri| {
            let shipping_item = shipping_items.iter().find(|si| si.item_id == ri.request_item_id);
            let order_item_id = shipping_item.map(|si| si.order_item_id).unwrap_or(0);
            let order_item = order_items.iter().find(|oi| oi.item_id == order_item_id);
            let unit_price = order_item.map(|oi| oi.unit_price).unwrap_or(Decimal::ZERO);

            SalesReturnItemRow {
                request_item_id: ri.request_item_id,
                order_item_id,
                product_id: shipping_item.map(|si| si.product_id).unwrap_or(0),
                product_code: shipping_item.and_then(|si| si.product_code.as_deref()),
                product_name: shipping_item.and_then(|si| si.product_name.as_deref()),
                unit: shipping_item.and_then(|si| si.unit.as_deref()),
                unit_price,
                quantity: ri.quantity,
                subtotal: unit_price * ri.quantity,
                remark: ri.remark.as_deref(),
            }
        })
        .collect()
}
