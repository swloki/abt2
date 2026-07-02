//! 采购需求池 — PurchaseDemandService 实现

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::Local;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::purchase::order::{new_purchase_order_service, PurchaseOrderService};
use crate::purchase::order::model::{CreateOrderItemRequest, CreatePurchaseOrderRequest};
use crate::sales::sales_order::{new_demand_service, DemandService};
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, model::EventPublishRequest};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;
use super::repo::PurchaseDemandRepo;
use super::service::PurchaseDemandService;

pub struct PurchaseDemandServiceImpl {
    pool: PgPool,
}

impl PurchaseDemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseDemandService for PurchaseDemandServiceImpl {
    async fn list_pending_demands(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        PurchaseDemandRepo::find_demands(db, &query, &page).await
    }

    async fn list_material_aggregated(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        PurchaseDemandRepo::find_material_aggregated(db, &query, &page).await
    }

    async fn get_demands_by_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<DemandSummary>> {
        PurchaseDemandRepo::find_by_ids(db, ids).await
    }

    async fn create_order_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOrderFromDemandsReq,
    ) -> Result<CreateDownstreamResult> {
        if req.demand_ids.is_empty() {
            return Err(DomainError::validation("demand_ids 不能为空"));
        }

        // 1. 乐观锁：原子 UPDATE，只处理成功锁定的需求
        let locked = PurchaseDemandRepo::lock_demands_for_purchase(db, &req.demand_ids).await?;

        // 计算被跳过的需求
        let locked_ids: HashSet<i64> = locked.iter().map(|d| d.id).collect();
        let skipped_demands: Vec<SkippedDemand> = req.demand_ids.iter()
            .filter(|id| !locked_ids.contains(id))
            .map(|id| SkippedDemand {
                demand_id: *id,
                reason: "已被他人处理或状态已变更".to_string(),
            })
            .collect();

        if locked.is_empty() {
            return Err(DomainError::business_rule("所有需求已被他人处理或状态已变更"));
        }

        // 2. 按 product_id 聚合
        let mut aggregated: HashMap<i64, Decimal> = HashMap::new();
        for d in &locked {
            *aggregated.entry(d.product_id).or_insert(Decimal::ZERO) += d.required_qty;
        }

        // 3. 创建采购订单草稿
        let today = Local::now().date_naive();
        let mut items: Vec<CreateOrderItemRequest> = Vec::new();
        for (idx, (product_id, qty)) in aggregated.iter().enumerate() {
            items.push(CreateOrderItemRequest {
                product_id: *product_id,
                line_no: (idx as i32) + 1,
                description: String::new(),
                quantity: *qty,
                unit_price: Decimal::ZERO, // 待采购员补充
                quotation_item_id: None,
                expected_delivery_date: req.expected_delivery_date,
                discount_pct: Decimal::ZERO,
                tax_rate_id: None,
            });
        }

        let po_req = CreatePurchaseOrderRequest {
            supplier_id: req.supplier_id,
            order_date: today,
            expected_delivery_date: req.expected_delivery_date,
            payment_terms: None,
            delivery_address: None,
            remark: req.remark.clone(),
            currency_code: String::from("CNY"),
            currency_rate: Decimal::ONE,
            discount_amount: Decimal::ZERO,
            items,
        };

        let po_id = new_purchase_order_service(self.pool.clone())
            .create(ctx, db, po_req, None)
            .await?;

        // 4. 关联需求：更新 target_doc + 发布 DemandConfirmed 事件
        let demand_svc = new_demand_service(self.pool.clone());
        let event_bus = new_domain_event_bus(self.pool.clone());
        for d in &locked {
            demand_svc.update_target_doc(db, d.id, DocumentType::PurchaseOrder as i16, po_id).await?;

            event_bus.publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: d.id,
                payload: serde_json::json!({
                    "order_id": d.source_id,
                    "order_line_id": d.source_line_id,
                    "product_id": d.product_id,
                    "acquire_channel": d.acquire_channel,
                    "target_doc_type": DocumentType::PurchaseOrder as i16,
                    "target_doc_id": po_id,
                }),
                idempotency_key: None,
            }).await?;
        }

        Ok(CreateDownstreamResult {
            doc_id: po_id,
            processed_demand_count: locked.len(),
            skipped_demands,
            demand_status: "Confirmed".to_string(),
        })
    }
}
