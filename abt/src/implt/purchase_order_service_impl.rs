use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::{
    PurchaseOrderDetail, PurchaseOrderItem, PurchaseOrderItemInput, PurchaseOrderQuery,
    PurchaseOrderWithItems,
};
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    PurchaseOrderItemRepo, PurchaseOrderRepo,
};
use crate::service::PurchaseOrderService;

#[derive(Debug, sqlx::FromRow)]
struct ProductInfo {
    product_id: i64,
    product_code: Option<String>,
    pdt_name: Option<String>,
    unit: Option<String>,
}

pub struct PurchaseOrderServiceImpl {
    pool: Arc<PgPool>,
}

impl PurchaseOrderServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    async fn fetch_product_info(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<HashMap<i64, ProductInfo>> {
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query_as::<_, ProductInfo>(
            "SELECT product_id, product_code, pdt_name, unit \
             FROM products WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        let map: HashMap<i64, ProductInfo> = rows
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect();
        Ok(map)
    }

    fn build_po_items(
        items: &[PurchaseOrderItemInput],
        product_map: &HashMap<i64, ProductInfo>,
        po_id: i64,
    ) -> Result<(Vec<PurchaseOrderItem>, Decimal)> {
        let mut total_amount = Decimal::ZERO;
        let mut po_items = Vec::with_capacity(items.len());

        for input in items {
            let product = product_map.get(&input.product_id).ok_or_else(|| {
                ServiceError::NotFound {
                    resource: "Product".to_string(),
                    id: input.product_id.to_string(),
                }
            })?;

            let subtotal = input.unit_price * input.quantity;
            total_amount += subtotal;

            po_items.push(PurchaseOrderItem {
                item_id: 0,
                po_id,
                product_id: input.product_id,
                product_code: product.product_code.clone(),
                product_name: product.pdt_name.clone(),
                unit: product.unit.clone(),
                unit_price: input.unit_price,
                quantity: input.quantity,
                received_qty: Decimal::ZERO,
                subtotal,
                remark: input.remark.clone(),
                created_at: chrono::Utc::now(),
            });
        }

        Ok((po_items, total_amount))
    }
}

#[async_trait]
impl PurchaseOrderService for PurchaseOrderServiceImpl {
    async fn create(
        &self,
        supplier_id: i64,
        order_type: i16,
        remark: Option<String>,
        operator_id: Option<i64>,
        items: Vec<PurchaseOrderItemInput>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let po_no = DocumentSequenceRepo::next_number(&mut *executor, "PO").await?;

        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let product_map = Self::fetch_product_info(&self.pool, &product_ids).await?;
        let (po_items, total_amount) = Self::build_po_items(&items, &product_map, 0)?;

        let po_id = PurchaseOrderRepo::insert(
            executor,
            &po_no,
            supplier_id,
            order_type,
            total_amount,
            remark.as_deref(),
            operator_id,
        )
        .await?;

        let mut po_items = po_items;
        for item in &mut po_items {
            item.po_id = po_id;
        }
        PurchaseOrderItemRepo::insert_batch(executor, &po_items).await?;

        Ok(po_id)
    }

    async fn update(
        &self,
        po_id: i64,
        supplier_id: i64,
        remark: Option<String>,
        items: Vec<PurchaseOrderItemInput>,
        executor: Executor<'_>,
    ) -> Result<()> {
        let current_status = PurchaseOrderRepo::find_status(&self.pool, po_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchaseOrder".to_string(),
                id: po_id.to_string(),
            })?;

        if current_status != 1 {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!("采购订单状态为{}，不允许修改（仅草稿状态可修改）", status_label(current_status)),
            }));
        }

        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let product_map = Self::fetch_product_info(&self.pool, &product_ids).await?;
        let (po_items, total_amount) = Self::build_po_items(&items, &product_map, po_id)?;

        PurchaseOrderRepo::update(
            executor,
            po_id,
            supplier_id,
            remark.as_deref(),
            total_amount,
        )
        .await?;

        PurchaseOrderItemRepo::delete_by_po(&mut *executor, po_id).await?;
        PurchaseOrderItemRepo::insert_batch(executor, &po_items).await?;

        Ok(())
    }

    async fn delete(&self, po_id: i64, executor: Executor<'_>) -> Result<()> {
        let current_status = PurchaseOrderRepo::find_status(&self.pool, po_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchaseOrder".to_string(),
                id: po_id.to_string(),
            })?;

        if current_status != 1 {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!("采购订单状态为{}，不允许删除（仅草稿状态可删除）", status_label(current_status)),
            }));
        }

        PurchaseOrderRepo::soft_delete(executor, po_id).await?;
        Ok(())
    }

    async fn get_by_id(&self, po_id: i64) -> Result<Option<PurchaseOrderWithItems>> {
        let order = match PurchaseOrderRepo::find_by_id(&self.pool, po_id).await? {
            Some(o) => o,
            None => return Ok(None),
        };

        let items = PurchaseOrderItemRepo::find_by_po(&self.pool, po_id).await?;

        Ok(Some(PurchaseOrderWithItems { order, items }))
    }

    async fn list(&self, query: PurchaseOrderQuery) -> Result<PaginatedResult<PurchaseOrderDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = PurchaseOrderRepo::query(&self.pool, &query).await?;
        let total = PurchaseOrderRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    async fn update_status(
        &self,
        po_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()> {
        let current_status = PurchaseOrderRepo::find_status(&self.pool, po_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchaseOrder".to_string(),
                id: po_id.to_string(),
            })?;

        if !is_valid_transition(current_status, status) {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!(
                    "不允许从状态【{}】变更为【{}】",
                    status_label(current_status),
                    status_label(status)
                ),
            }));
        }

        PurchaseOrderRepo::update_status(executor, po_id, status).await?;
        Ok(())
    }
}

fn is_valid_transition(from: i16, to: i16) -> bool {
    matches!(
        (from, to),
        (1, 2)  // 草稿 → 已提交
        | (2, 3)  // 已提交 → 已审核
        | (3, 4)  // 已审核 → 部分收货
        | (3, 5)  // 已审核 → 全部收货
        | (4, 5)  // 部分收货 → 全部收货
        | (5, 6)  // 全部收货 → 已对账
        | (6, 7)  // 已对账 → 已关闭
    )
}

fn status_label(status: i16) -> &'static str {
    match status {
        1 => "草稿",
        2 => "已提交",
        3 => "已审核",
        4 => "部分收货",
        5 => "全部收货",
        6 => "已对账",
        7 => "已关闭",
        _ => "未知",
    }
}
