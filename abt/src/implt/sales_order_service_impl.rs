use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use sqlx::PgPool;
use common::error::ServiceError;
use crate::models::{SalesOrder, SalesOrderItem, SalesOrderQuery};
use crate::repositories::{DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, ProductRepo, QuotationRepo, SalesOrderRepo};
use crate::service::SalesOrderService;

pub struct SalesOrderServiceImpl {
    pool: Arc<PgPool>,
}

impl SalesOrderServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

// 状态常量
const STATUS_DRAFT: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_IN_PROGRESS: i16 = 3;
const STATUS_COMPLETED: i16 = 4;
const STATUS_CANCELLED: i16 = 5;

#[async_trait]
impl SalesOrderService for SalesOrderServiceImpl {
    async fn create(&self, operator_id: Option<i64>, mut order: SalesOrder, executor: Executor<'_>) -> Result<i64> {
        // 1. 如果关联了报价单，从报价单复制行项目
        if let Some(quotation_id) = order.quotation_id {
            let quotation = QuotationRepo::find_by_id(&self.pool, quotation_id).await?
                .ok_or_else(|| ServiceError::NotFound {
                    resource: "Quotation".to_string(),
                    id: quotation_id.to_string(),
                })?;

            if quotation.status != 3 {
                return Err(ServiceError::BusinessValidation {
                    message: "只能基于已接受的报价单创建销售订单".to_string(),
                }.into());
            }

            // 从报价单复制行项目
            order.items = quotation.items.iter().map(|qi| {
                SalesOrderItem {
                    item_id: 0,
                    order_id: 0,
                    product_id: qi.product_id,
                    product_code: qi.product_code.clone(),
                    product_name: qi.product_name.clone(),
                    unit: qi.unit.clone(),
                    unit_price: qi.unit_price,
                    quantity: qi.quantity,
                    discount: qi.discount,
                    subtotal: qi.subtotal,
                    shipped_qty: Decimal::ZERO,
                    returned_qty: Decimal::ZERO,
                    remark: qi.remark.clone(),
                    created_at: NaiveDateTime::default(),
                }
            }).collect();
        }

        // 2. 验证产品是否存在
        let product_ids: Vec<i64> = order.items.iter().map(|i| i.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;
        if products.len() != product_ids.len() {
            let found_ids: Vec<i64> = products.iter().map(|p| p.product_id).collect();
            let missing: Vec<i64> = product_ids.into_iter().filter(|id| !found_ids.contains(id)).collect();
            return Err(ServiceError::NotFound {
                resource: "Product".to_string(),
                id: missing.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","),
            }.into());
        }

        // 3. 生成订单编号
        let order_no = DocumentSequenceRepo::next_number(&mut *executor, "SO").await?;
        order.order_no = order_no;
        order.operator_id = operator_id;

        // 4. 计算小计和总计
        let mut total_amount = Decimal::ZERO;
        for item in &mut order.items {
            item.subtotal = item.unit_price * item.quantity * item.discount;
            total_amount += item.subtotal;
            // 填充产品信息
            if let Some(product) = products.iter().find(|p| p.product_id == item.product_id) {
                item.product_code = Some(product.product_code.clone());
                item.product_name = Some(product.pdt_name.clone());
                item.unit = Some(product.unit.clone());
            }
        }
        order.total_amount = total_amount;
        order.status = STATUS_DRAFT;

        // 5. 插入主表
        let order_id = SalesOrderRepo::insert(&mut *executor, &order).await?;

        // 6. 设置 order_id 并插入行项目
        for item in &mut order.items {
            item.order_id = order_id;
        }
        SalesOrderRepo::insert_items(&mut *executor, &order.items).await?;

        Ok(order_id)
    }

    async fn update_header(
        &self,
        order_id: i64,
        customer_name: String,
        contact_person: Option<String>,
        contact_phone: Option<String>,
        remark: Option<String>,
        delivery_date: Option<NaiveDateTime>,
    ) -> Result<()> {
        // 1. 查找现有订单
        let _existing = SalesOrderRepo::find_by_id(&self.pool, order_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesOrder".to_string(),
                id: order_id.to_string(),
            })?;

        // 2. 更新头部信息
        SalesOrderRepo::update_header(
            &self.pool,
            order_id,
            customer_name,
            contact_person,
            contact_phone,
            remark,
            delivery_date,
        )
        .await?;

        Ok(())
    }

    async fn delete(&self, order_id: i64, executor: Executor<'_>) -> Result<()> {
        let existing = SalesOrderRepo::find_by_id(&self.pool, order_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesOrder".to_string(),
                id: order_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的订单可以删除".to_string(),
            }.into());
        }

        SalesOrderRepo::soft_delete(executor, order_id).await
    }

    async fn get_by_id(&self, order_id: i64) -> Result<Option<SalesOrder>> {
        let mut order = SalesOrderRepo::find_by_id(&self.pool, order_id).await?;
        if let Some(ref mut o) = order {
            o.items = SalesOrderRepo::find_by_order_id(&self.pool, order_id).await?;
        }
        Ok(order)
    }

    async fn list(&self, query: SalesOrderQuery) -> Result<PaginatedResult<SalesOrder>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100) as u32;
        let pagination = PaginationParams::new(page, page_size);

        let items = SalesOrderRepo::query(&self.pool, &query).await?;
        let total = SalesOrderRepo::query_count(&self.pool, &query).await?;

        // 填充每个订单的行项目
        let mut filled_items = Vec::new();
        for mut o in items {
            o.items = SalesOrderRepo::find_by_order_id(&self.pool, o.order_id).await?;
            filled_items.push(o);
        }

        Ok(PaginatedResult::new(filled_items, total as u64, &pagination))
    }

    async fn update_status(&self, order_id: i64, status: i16, executor: Executor<'_>) -> Result<()> {
        let existing = SalesOrderRepo::find_by_id(&self.pool, order_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "SalesOrder".to_string(),
                id: order_id.to_string(),
            })?;

        // 验证状态转换是否合法
        let valid = matches!(
            (existing.status, status),
            (STATUS_DRAFT, STATUS_CONFIRMED)
                | (STATUS_DRAFT, STATUS_CANCELLED)
                | (STATUS_CONFIRMED, STATUS_IN_PROGRESS)
                | (STATUS_IN_PROGRESS, STATUS_COMPLETED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("不允许从状态 {} 转换到 {}", existing.status, status),
            }.into());
        }

        SalesOrderRepo::update_status(executor, order_id, status).await
    }
}
