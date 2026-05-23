use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;


use common::error::ServiceError;
use crate::models::ShippingRequestQuery;
use sqlx::PgPool;
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    ShippingRequestInsertParams, ShippingRequestItemRow, ShippingRequestRepo,
    ShippingRequestUpdateParams, SalesOrderRepo,
};
use crate::service::{CreateShippingRequestItemParams, CreateShippingRequestParams, ShippingRequestService, UpdateShippingRequestParams};

pub struct ShippingRequestServiceImpl {
    pool: Arc<PgPool>,
}

impl ShippingRequestServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_PENDING: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_SHIPPED: i16 = 3;
const STATUS_CANCELLED: i16 = 4;

#[async_trait]
impl ShippingRequestService for ShippingRequestServiceImpl {
    async fn create(
        &self,
        executor: Executor<'_>,
        params: &CreateShippingRequestParams<'_>,
        items: Vec<CreateShippingRequestItemParams>,
    ) -> Result<i64> {
        // Validate order exists and is in a shippable state
        let order_status = SalesOrderRepo::find_status(&self.pool, params.order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: params.order_id.to_string(),
            })?;

        if !matches!(order_status, 2 | 3) {
            return Err(ServiceError::BusinessValidation {
                message: "销售订单状态不允许创建发货申请".into(),
            }
            .into());
        }

        // Validate quantities against order items
        let order = SalesOrderRepo::find_by_id(&self.pool, params.order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "sales_order".into(),
                id: params.order_id.to_string(),
            })?;

        validate_shipping_quantities(&order.items, &items)?;

        // Get customer_name from order
        let customer_name = &order.customer_name;

        let request_no = DocumentSequenceRepo::next_number(&mut *executor, "SR").await?;

        let request_id = ShippingRequestRepo::insert(
            &mut *executor,
            &ShippingRequestInsertParams {
                request_no: &request_no,
                order_id: params.order_id,
                customer_name,
                remark: params.remark,
                operator_id: params.operator_id,
            },
        )
        .await?;

        let item_rows: Vec<ShippingRequestItemRow> = items
            .iter()
            .map(|i| ShippingRequestItemRow {
                order_item_id: i.order_item_id,
                product_id: 0, // will be filled from order item lookup
                product_code: None,
                product_name: None,
                unit: None,
                quantity: i.quantity,
                remark: i.remark.as_deref(),
            })
            .collect();

        // Fill product info from order items
        let item_rows = enrich_shipping_items(&order.items, &item_rows);

        ShippingRequestRepo::insert_items(&mut *executor, request_id, &item_rows).await?;

        Ok(request_id)
    }

    async fn update(
        &self,
        executor: Executor<'_>,
        request_id: i64,
        params: &UpdateShippingRequestParams<'_>,
        _items: Vec<CreateShippingRequestItemParams>,
    ) -> Result<()> {
        let current_status = ShippingRequestRepo::find_status(&self.pool, request_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "shipping_request".into(),
                id: request_id.to_string(),
            })?;

        if current_status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "仅待处理状态可编辑".into(),
            }
            .into());
        }

        ShippingRequestRepo::update(
            &mut *executor,
            request_id,
            &ShippingRequestUpdateParams {
                remark: params.remark,
            },
        )
        .await?;

        Ok(())
    }

    async fn delete(&self, executor: Executor<'_>, request_id: i64) -> Result<()> {
        let current_status = ShippingRequestRepo::find_status(&self.pool, request_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "shipping_request".into(),
                id: request_id.to_string(),
            })?;

        if current_status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "仅待处理状态可删除".into(),
            }
            .into());
        }

        ShippingRequestRepo::soft_delete(&mut *executor, request_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, request_id: i64) -> Result<Option<crate::models::ShippingRequest>> {
        ShippingRequestRepo::find_by_id(&self.pool, request_id).await
    }

    async fn list(&self, query: &ShippingRequestQuery) -> Result<PaginatedResult<crate::models::ShippingRequest>> {
        let (items, total) = ShippingRequestRepo::query(&self.pool, query).await?;
        let pagination = PaginationParams::new(
            query.page.unwrap_or(1) as u32,
            query.page_size.unwrap_or(20) as u32,
        );
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        executor: Executor<'_>,
        request_id: i64,
        new_status: i16,
    ) -> Result<()> {
        let current_status = ShippingRequestRepo::find_status(&self.pool, request_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "shipping_request".into(),
                id: request_id.to_string(),
            })?;

        let valid = matches!(
            (current_status, new_status),
            (STATUS_PENDING, STATUS_CONFIRMED)
                | (STATUS_CONFIRMED, STATUS_SHIPPED)
                | (STATUS_PENDING, STATUS_CANCELLED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("非法状态转换: {} → {}", current_status, new_status),
            }
            .into());
        }

        ShippingRequestRepo::update_status(&mut *executor, request_id, new_status).await?;

        // On confirm, set confirmed_at
        if new_status == STATUS_CONFIRMED {
            ShippingRequestRepo::update_confirmed_at(&mut *executor, request_id).await?;
        }

        // On ship: set shipped_at, update order item shipped quantities
        if new_status == STATUS_SHIPPED {
            ShippingRequestRepo::update_shipped_at(&mut *executor, request_id).await?;

            let shipping = ShippingRequestRepo::find_by_id(&self.pool, request_id)
                .await?
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "shipping_request".into(),
                    id: request_id.to_string(),
                })?;

            for item in &shipping.items {
                SalesOrderRepo::update_shipped_qty(&mut *executor, item.order_item_id, item.quantity)
                    .await?;
            }

            // Update order status to InProgress if currently Confirmed
            let order_status = SalesOrderRepo::find_status(&self.pool, shipping.order_id).await?;
            if order_status == Some(STATUS_CONFIRMED) {
                SalesOrderRepo::update_status(&mut *executor, shipping.order_id, 3).await?;
            }
        }

        Ok(())
    }
}

fn validate_shipping_quantities(
    order_items: &[crate::models::SalesOrderItem],
    shipping_items: &[CreateShippingRequestItemParams],
) -> Result<()> {
    for si in shipping_items {
        let order_item = order_items
            .iter()
            .find(|oi| oi.item_id == si.order_item_id)
            .ok_or_else(|| ServiceError::BusinessValidation {
                message: format!("订单行项目 {} 不存在", si.order_item_id),
            })?;

        let remaining = order_item.quantity - order_item.shipped_qty;
        if si.quantity > remaining {
            return Err(ServiceError::BusinessValidation {
                message: format!(
                    "发货数量 {} 超出可发数量 {}（订单行 {}）",
                    si.quantity, remaining, si.order_item_id
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn enrich_shipping_items<'a>(
    order_items: &'a [crate::models::SalesOrderItem],
    shipping_items: &[ShippingRequestItemRow<'a>],
) -> Vec<ShippingRequestItemRow<'a>> {
    shipping_items
        .iter()
        .map(|si| {
            let order_item = order_items.iter().find(|oi| oi.item_id == si.order_item_id);
            ShippingRequestItemRow {
                order_item_id: si.order_item_id,
                product_id: order_item.map(|oi| oi.product_id).unwrap_or(0),
                product_code: order_item.and_then(|oi| oi.product_code.as_deref()),
                product_name: order_item.and_then(|oi| oi.product_name.as_deref()),
                unit: order_item.and_then(|oi| oi.unit.as_deref()),
                quantity: si.quantity,
                remark: si.remark,
            }
        })
        .collect()
}
