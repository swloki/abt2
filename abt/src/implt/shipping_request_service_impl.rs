//! 发货申请服务实现
//!
//! 提供发货申请管理的业务逻辑具体实现，包括创建、更新、删除、状态流转等。

use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use common::error::ServiceError;
use crate::models::{OperationType, ShippingRequest, ShippingRequestQuery, StockChangeRequest};
use crate::repositories::{DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, SalesOrderRepo, ShippingRequestRepo};
use crate::service::{InventoryService, ShippingRequestService};
use crate::implt::InventoryServiceImpl;

pub struct ShippingRequestServiceImpl {
    pool: Arc<PgPool>,
}

impl ShippingRequestServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

// 发货申请状态常量
const STATUS_PENDING: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_SHIPPED: i16 = 3;
const STATUS_CANCELLED: i16 = 4;

// 销售订单状态常量（与 sales_order_service_impl 保持一致）
const SO_STATUS_CONFIRMED: i16 = 2;
const SO_STATUS_IN_PROGRESS: i16 = 3;

#[async_trait]
impl ShippingRequestService for ShippingRequestServiceImpl {
    async fn create(&self, operator_id: Option<i64>, mut request: ShippingRequest, executor: Executor<'_>) -> Result<i64> {
        // 1. 查找关联的销售订单并验证状态
        let order = SalesOrderRepo::find_by_id(&self.pool, request.order_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesOrder".to_string(),
                id: request.order_id.to_string(),
            })?;

        if order.status != SO_STATUS_CONFIRMED && order.status != SO_STATUS_IN_PROGRESS {
            return Err(ServiceError::BusinessValidation {
                message: "订单状态不允许创建发货申请".to_string(),
            }.into());
        }

        // 2. 获取订单行项目
        let order_items = SalesOrderRepo::find_by_order_id(&self.pool, request.order_id).await?;

        // 3. 验证并填充行项目信息
        for item in &mut request.items {
            let order_item = order_items.iter().find(|oi| oi.item_id == item.order_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "OrderItem".to_string(),
                    id: item.order_item_id.to_string(),
                })?;

            let remaining = order_item.quantity - order_item.shipped_qty;
            if item.quantity > remaining {
                return Err(ServiceError::BusinessValidation {
                    message: format!("行项目 {} 发货数量超过剩余可发量", item.order_item_id),
                }.into());
            }

            // 4. 从订单行项目填充产品信息
            item.product_id = order_item.product_id;
            item.product_code = order_item.product_code.clone();
            item.product_name = order_item.product_name.clone();
            item.unit = order_item.unit.clone();
        }

        // 5. 生成发货申请编号
        let request_no = DocumentSequenceRepo::next_number(&mut *executor, "SR").await?;
        request.request_no = request_no;
        request.customer_name = order.customer_name.clone();
        request.operator_id = operator_id;
        request.status = STATUS_PENDING;

        // 6. 插入主表
        let request_id = ShippingRequestRepo::insert(&mut *executor, &request).await?;

        // 7. 设置 request_id 并插入行项目
        for item in &mut request.items {
            item.request_id = request_id;
        }
        ShippingRequestRepo::insert_items(&mut *executor, &request.items).await?;

        Ok(request_id)
    }

    async fn update(&self, _operator_id: Option<i64>, request: ShippingRequest, executor: Executor<'_>) -> Result<()> {
        // 1. 查找现有发货申请，验证状态
        let existing = ShippingRequestRepo::find_by_id(&self.pool, request.request_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ShippingRequest".to_string(),
                id: request.request_id.to_string(),
            })?;

        if existing.status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "只有待确认状态的发货申请可以编辑".to_string(),
            }.into());
        }

        // 2. 重新验证数量并填充行项目信息
        let order_items = SalesOrderRepo::find_by_order_id(&self.pool, existing.order_id).await?;
        let mut filled_items = Vec::new();
        for mut item in request.items {
            let order_item = order_items.iter().find(|oi| oi.item_id == item.order_item_id)
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "OrderItem".to_string(),
                    id: item.order_item_id.to_string(),
                })?;

            let remaining = order_item.quantity - order_item.shipped_qty;
            if item.quantity > remaining {
                return Err(ServiceError::BusinessValidation {
                    message: format!("行项目 {} 发货数量超过剩余可发量", item.order_item_id),
                }.into());
            }

            // 填充产品信息
            item.product_id = order_item.product_id;
            item.product_code = order_item.product_code.clone();
            item.product_name = order_item.product_name.clone();
            item.unit = order_item.unit.clone();
            item.request_id = existing.request_id;
            filled_items.push(item);
        }

        // 3. 更新主表、删除旧行项目、插入新行项目
        let update_req = ShippingRequest {
            items: vec![],
            ..request
        };
        ShippingRequestRepo::update(&mut *executor, &update_req).await?;
        ShippingRequestRepo::delete_by_request(&mut *executor, existing.request_id).await?;
        ShippingRequestRepo::insert_items(&mut *executor, &filled_items).await?;

        Ok(())
    }

    async fn delete(&self, request_id: i64, executor: Executor<'_>) -> Result<()> {
        let existing = ShippingRequestRepo::find_by_id(&self.pool, request_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ShippingRequest".to_string(),
                id: request_id.to_string(),
            })?;

        if existing.status != STATUS_PENDING {
            return Err(ServiceError::BusinessValidation {
                message: "只有待确认状态的发货申请可以删除".to_string(),
            }.into());
        }

        ShippingRequestRepo::soft_delete(executor, request_id).await
    }

    async fn get_by_id(&self, request_id: i64) -> Result<Option<ShippingRequest>> {
        let mut request = ShippingRequestRepo::find_by_id(&self.pool, request_id).await?;
        if let Some(ref mut r) = request {
            r.items = ShippingRequestRepo::find_by_request_id(&self.pool, request_id).await?;
        }
        Ok(request)
    }

    async fn list(&self, query: ShippingRequestQuery) -> Result<PaginatedResult<ShippingRequest>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100) as u32;
        let pagination = PaginationParams::new(page, page_size);

        let items = ShippingRequestRepo::query(&self.pool, &query).await?;
        let total = ShippingRequestRepo::query_count(&self.pool, &query).await?;

        // 填充每个发货申请的行项目
        let mut filled_items = Vec::new();
        for mut r in items {
            r.items = ShippingRequestRepo::find_by_request_id(&self.pool, r.request_id).await?;
            filled_items.push(r);
        }

        Ok(PaginatedResult::new(filled_items, total as u64, &pagination))
    }

    async fn update_status(&self, request_id: i64, status: i16, executor: Executor<'_>) -> Result<()> {
        let existing = ShippingRequestRepo::find_by_id(&self.pool, request_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ShippingRequest".to_string(),
                id: request_id.to_string(),
            })?;

        // 验证状态转换是否合法
        let valid = matches!(
            (existing.status, status),
            (STATUS_PENDING, STATUS_CONFIRMED)
                | (STATUS_PENDING, STATUS_CANCELLED)
                | (STATUS_CONFIRMED, STATUS_SHIPPED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("不允许从状态 {} 转换到 {}", existing.status, status),
            }.into());
        }

        // 执行状态更新
        ShippingRequestRepo::update_status(&mut *executor, request_id, status).await?;

        // 根据目标状态执行额外操作
        match (existing.status, status) {
            (STATUS_PENDING, STATUS_CONFIRMED) => {
                ShippingRequestRepo::update_confirmed_at(&mut *executor, request_id).await?;
            }
            (STATUS_CONFIRMED, STATUS_SHIPPED) => {
                ShippingRequestRepo::update_shipped_at(&mut *executor, request_id).await?;

                // 出库并更新已发货数量
                let items = ShippingRequestRepo::find_by_request_id(&self.pool, request_id).await?;
                let inv_srv = InventoryServiceImpl::new(Arc::clone(&self.pool));

                for item in &items {
                    // 调用库存出库
                    let req = StockChangeRequest {
                        product_id: item.product_id,
                        location_id: 0, // 占位，后续集成具体库位
                        quantity: item.quantity,
                        operation_type: OperationType::Out,
                        ref_order_type: Some("shipping_request".to_string()),
                        ref_order_id: Some(existing.request_no.clone()),
                        operator: None,
                        remark: item.remark.clone(),
                    };
                    inv_srv.stock_out(req, &mut *executor).await?;

                    // 累加订单行项目已发货数量
                    SalesOrderRepo::update_shipped_qty(&mut *executor, item.order_item_id, item.quantity).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}
