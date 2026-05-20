use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use common::error::ServiceError;
use crate::models::{OperationType, SalesReturn, SalesReturnQuery, StockChangeRequest};
use crate::repositories::{DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, SalesOrderRepo, SalesReturnRepo, ShippingRequestRepo};
use crate::service::{InventoryService, SalesReturnService};
use crate::implt::InventoryServiceImpl;

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

const SR_STATUS_SHIPPED: i16 = 3;

#[async_trait]
impl SalesReturnService for SalesReturnServiceImpl {
    async fn create(&self, operator_id: Option<i64>, mut ret: SalesReturn, executor: Executor<'_>) -> Result<i64> {
        // 1. 验证发货单存在且已发货
        let shipping = ShippingRequestRepo::find_by_id(&self.pool, ret.request_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ShippingRequest".to_string(),
                id: ret.request_id.to_string(),
            })?;

        if shipping.status != SR_STATUS_SHIPPED {
            return Err(ServiceError::BusinessValidation {
                message: "发货单未完成发货，无法创建退货".to_string(),
            }.into());
        }

        // 2. 获取发货单行项目和订单行项目
        let shipping_items = ShippingRequestRepo::find_by_request_id(&self.pool, ret.request_id).await?;
        let order_items = SalesOrderRepo::find_by_order_id(&self.pool, shipping.order_id).await?;

        // 3. 验证并填充行项目
        let mut total_amount = Decimal::ZERO;
        for item in &mut ret.items {
            let ship_item = shipping_items.iter().find(|si| si.item_id == item.request_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "ShippingRequestItem".to_string(),
                    id: item.request_item_id.to_string(),
                })?;

            let order_item = order_items.iter().find(|oi| oi.item_id == ship_item.order_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "OrderItem".to_string(),
                    id: ship_item.order_item_id.to_string(),
                })?;

            // 计算该 order_item 已退货总量（含 Pending/Approved/Received/Completed 状态的退货单）
            let already_returned = SalesReturnRepo::sum_returned_qty(&self.pool, order_item.item_id).await?;
            let remaining = ship_item.quantity - already_returned;

            if item.quantity > remaining {
                return Err(ServiceError::BusinessValidation {
                    message: format!("行项目 {} 退货数量超过可退量", item.request_item_id),
                }.into());
            }

            item.product_id = ship_item.product_id;
            item.product_code = ship_item.product_code.clone();
            item.product_name = ship_item.product_name.clone();
            item.unit = ship_item.unit.clone();
            item.order_item_id = order_item.item_id;
            item.unit_price = order_item.unit_price;
            item.subtotal = item.unit_price * item.quantity;
            total_amount += item.subtotal;
        }

        // 4. 生成编号
        let return_no = DocumentSequenceRepo::next_number(&mut *executor, "RT").await?;
        ret.return_no = return_no;
        ret.order_id = shipping.order_id;
        ret.customer_name = shipping.customer_name.clone();
        ret.operator_id = operator_id;
        ret.status = STATUS_PENDING;
        ret.total_amount = total_amount;

        // 5. 插入主表
        let return_id = SalesReturnRepo::insert(&mut *executor, &ret).await?;

        // 6. 插入行项目
        for item in &mut ret.items {
            item.return_id = return_id;
        }
        SalesReturnRepo::insert_items(&mut *executor, &ret.items).await?;

        Ok(return_id)
    }

    async fn update(&self, _operator_id: Option<i64>, ret: SalesReturn, executor: Executor<'_>) -> Result<()> {
        let existing = SalesReturnRepo::find_by_id(&self.pool, ret.return_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesReturn".to_string(),
                id: ret.return_id.to_string(),
            })?;

        if existing.status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "只有待处理状态的退货单可以编辑".to_string(),
            }.into());
        }

        // 重新验证数量
        let shipping_items = ShippingRequestRepo::find_by_request_id(&self.pool, existing.request_id).await?;
        let order_items = SalesOrderRepo::find_by_order_id(&self.pool, existing.order_id).await?;

        let mut total_amount = Decimal::ZERO;
        let mut filled_items = Vec::new();
        for mut item in ret.items {
            let ship_item = shipping_items.iter().find(|si| si.item_id == item.request_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "ShippingRequestItem".to_string(),
                    id: item.request_item_id.to_string(),
                })?;

            let order_item = order_items.iter().find(|oi| oi.item_id == ship_item.order_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "OrderItem".to_string(),
                    id: ship_item.order_item_id.to_string(),
                })?;

            let already_returned = SalesReturnRepo::sum_returned_qty(&self.pool, order_item.item_id).await?;
            let remaining = ship_item.quantity - already_returned;

            if item.quantity > remaining {
                return Err(ServiceError::BusinessValidation {
                    message: format!("行项目 {} 退货数量超过可退量", item.request_item_id),
                }.into());
            }

            item.product_id = ship_item.product_id;
            item.product_code = ship_item.product_code.clone();
            item.product_name = ship_item.product_name.clone();
            item.unit = ship_item.unit.clone();
            item.order_item_id = order_item.item_id;
            item.return_id = existing.return_id;
            item.unit_price = order_item.unit_price;
            item.subtotal = item.unit_price * item.quantity;
            total_amount += item.subtotal;
            filled_items.push(item);
        }

        let update_ret = SalesReturn {
            total_amount,
            items: vec![],
            ..ret
        };
        SalesReturnRepo::update(&mut *executor, &update_ret).await?;
        SalesReturnRepo::delete_by_return(&mut *executor, existing.return_id).await?;
        SalesReturnRepo::insert_items(&mut *executor, &filled_items).await?;

        Ok(())
    }

    async fn delete(&self, return_id: i64, executor: Executor<'_>) -> Result<()> {
        let existing = SalesReturnRepo::find_by_id(&self.pool, return_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesReturn".to_string(),
                id: return_id.to_string(),
            })?;

        if existing.status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "只有待处理状态的退货单可以删除".to_string(),
            }.into());
        }

        SalesReturnRepo::soft_delete(executor, return_id).await
    }

    async fn get_by_id(&self, return_id: i64) -> Result<Option<SalesReturn>> {
        let mut ret = SalesReturnRepo::find_by_id(&self.pool, return_id).await?;
        if let Some(ref mut r) = ret {
            r.items = SalesReturnRepo::find_by_return_id(&self.pool, return_id).await?;
        }
        Ok(ret)
    }

    async fn list(&self, query: SalesReturnQuery) -> Result<PaginatedResult<SalesReturn>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100) as u32;
        let pagination = PaginationParams::new(page, page_size);

        let items = SalesReturnRepo::query(&self.pool, &query).await?;
        let total = SalesReturnRepo::query_count(&self.pool, &query).await?;

        let mut filled_items = Vec::new();
        for mut r in items {
            r.items = SalesReturnRepo::find_by_return_id(&self.pool, r.return_id).await?;
            filled_items.push(r);
        }

        Ok(PaginatedResult::new(filled_items, total as u64, &pagination))
    }

    async fn update_status(&self, return_id: i64, status: i16, executor: Executor<'_>) -> Result<()> {
        let existing = SalesReturnRepo::find_by_id(&self.pool, return_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesReturn".to_string(),
                id: return_id.to_string(),
            })?;

        let valid = matches!(
            (existing.status, status),
            (STATUS_PENDING, STATUS_APPROVED)
                | (STATUS_APPROVED, STATUS_RECEIVED)
                | (STATUS_RECEIVED, STATUS_COMPLETED)
                | (STATUS_PENDING, STATUS_REJECTED)
                | (STATUS_APPROVED, STATUS_REJECTED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("不允许从状态 {} 转换到 {}", existing.status, status),
            }.into());
        }

        SalesReturnRepo::update_status(&mut *executor, return_id, status).await?;

        if status == STATUS_COMPLETED {
            let items = SalesReturnRepo::find_by_return_id(&self.pool, return_id).await?;
            let inv_srv = InventoryServiceImpl::new(Arc::clone(&self.pool));

            for item in &items {
                let req = StockChangeRequest {
                    product_id: item.product_id,
                    location_id: 0,
                    quantity: item.quantity,
                    operation_type: OperationType::In,
                    ref_order_type: Some("sales_return".to_string()),
                    ref_order_id: Some(existing.return_no.clone()),
                    operator: None,
                    remark: item.remark.clone(),
                };
                inv_srv.stock_in(req, &mut *executor).await?;

                SalesOrderRepo::update_returned_qty(&mut *executor, item.order_item_id, item.quantity).await?;
            }
        }

        Ok(())
    }
}
