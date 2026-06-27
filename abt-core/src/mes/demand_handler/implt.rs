//! MES 需求池 — MesDemandService 实现

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::mes::work_order::{new_work_order_service, WorkOrderService};
use crate::mes::work_order::model::CreateWorkOrderReq;
use crate::sales::sales_order::{new_demand_service, DemandService};
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus, model::EventPublishRequest};
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

use super::model::*;
use super::repo::MesDemandRepo;
use super::service::MesDemandService;

pub struct MesDemandServiceImpl {
    pool: PgPool,
}

impl MesDemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MesDemandService for MesDemandServiceImpl {
    async fn list_pending_demands(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandPoolQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<DemandSummary>> {
        MesDemandRepo::find_demands(db, &query, &page).await
    }

    async fn list_material_aggregated(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<MaterialAggSummary>> {
        MesDemandRepo::find_material_aggregated(db, &query, &page).await
    }


    async fn create_work_orders_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateWorkOrdersFromDemandsReq,
    ) -> Result<CreateWorkOrdersResult> {
        if req.demand_ids.is_empty() {
            return Err(DomainError::validation("demand_ids 不能为空"));
        }

        // 1. 乐观锁：原子 UPDATE，只处理成功锁定的需求
        let locked = MesDemandRepo::lock_demands_for_production(db, &req.demand_ids).await?;

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

        // 2. 构建每条需求的排程参数映射
        let item_map: HashMap<i64, &PlanDemandItemReq> = req.items
            .as_ref()
            .map(|items| items.iter().map(|i| (i.demand_id, i)).collect())
            .unwrap_or_default();

        let today = chrono::Local::now().date_naive();
        let default_start = req.default_scheduled_start.unwrap_or(today);
        let default_end = req.default_scheduled_end.unwrap_or_else(|| today + chrono::Duration::days(7));

        // 3. 按 product_id 聚合 — 一个物料一个 Draft 工单（合并该物料所有销售订单需求），
        //    数量分批交给工单下达时拆批（ProductionBatch）。sales_order_id 关联首个 SO。
        let mut aggregated: HashMap<i64, (Decimal, &LockedDemand)> = HashMap::new();
        for d in &locked {
            aggregated
                .entry(d.product_id)
                .and_modify(|(qty, _first)| *qty += d.required_qty)
                .or_insert((d.required_qty, d));
        }

        // 4. 每个聚合组调 WorkOrderService::create 生成 Draft 工单（扁平化：plan_item_id=None）
        let wo_svc = new_work_order_service(self.pool.clone());
        let mut product_wo: HashMap<i64, i64> = HashMap::new();
        let mut wo_ids: Vec<i64> = Vec::new();
        for (product_id, (qty, d)) in &aggregated {
            let (scheduled_start, scheduled_end) = match item_map.get(&d.id) {
                Some(item) => (item.scheduled_start, item.scheduled_end),
                None => (default_start, default_end),
            };
            let wo_id = wo_svc.create(ctx, db, CreateWorkOrderReq {
                plan_item_id: None,
                product_id: *product_id,
                bom_snapshot_id: None,
                routing_id: None,
                planned_qty: *qty,
                scheduled_start,
                scheduled_end,
                work_center_id: None,
                sales_order_id: Some(d.source_id),
                remark: req.remark.clone(),
            }).await?;
            product_wo.insert(*product_id, wo_id);
            wo_ids.push(wo_id);
        }

        // 5. 关联需求：更新 target_doc=(WorkOrder, wo_id) + 发布 DemandConfirmed 事件
        let demand_svc = new_demand_service(self.pool.clone());
        let event_bus = new_domain_event_bus(self.pool.clone());
        for d in &locked {
            let wo_id = *product_wo.get(&d.product_id)
                .ok_or_else(|| DomainError::business_rule("聚合工单映射缺失"))?;
            demand_svc.update_target_doc(db, d.id, DocumentType::WorkOrder as i16, wo_id).await?;

            event_bus.publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: d.id,
                payload: serde_json::json!({
                    "order_id": d.source_id,
                    "order_line_id": d.source_line_id,
                    "product_id": d.product_id,
                    "acquire_channel": d.acquire_channel,
                    "target_doc_type": DocumentType::WorkOrder as i16,
                    "target_doc_id": wo_id,
                }),
                idempotency_key: None,
            }).await?;
        }

        Ok(CreateWorkOrdersResult {
            wo_ids,
            processed_demand_count: locked.len(),
            skipped_demands,
            demand_status: "Confirmed".to_string(),
        })
    }
}
