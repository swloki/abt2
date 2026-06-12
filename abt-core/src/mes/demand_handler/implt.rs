//! MES 需求池 — MesDemandService 实现

use std::collections::HashMap;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::mes::enums::PlanType;
use crate::mes::production_plan::{new_production_plan_service, ProductionPlanService};
use crate::mes::production_plan::model::{CreatePlanItemReq, CreatePlanReq};
use crate::sales::sales_order::repo::DemandRepo;
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

    async fn create_plan_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePlanFromDemandsReq,
    ) -> Result<CreateDownstreamResult> {
        if req.demand_ids.is_empty() {
            return Err(DomainError::validation("demand_ids 不能为空"));
        }

        // 1. 乐观锁：原子 UPDATE，只处理成功锁定的需求
        let locked = MesDemandRepo::lock_demands_for_production(db, &req.demand_ids).await?;

        // 计算被跳过的需求
        let locked_ids: Vec<i64> = locked.iter().map(|d| d.id).collect();
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

        let default_start = req.default_scheduled_start.unwrap_or(req.plan_date);
        let default_end = req.default_scheduled_end.unwrap_or_else(|| {
            req.plan_date + chrono::Duration::days(7)
        });

        // 3. 按 product_id 聚合 + 保留第一条需求的 source 信息
        let mut aggregated: HashMap<i64, (Decimal, &LockedDemand)> = HashMap::new();
        for d in &locked {
            aggregated
                .entry(d.product_id)
                .and_modify(|(qty, _first)| *qty += d.required_qty)
                .or_insert((d.required_qty, d));
        }

        // 4. 创建生产计划草稿
        let plan_type = PlanType::from_i16(req.plan_type).unwrap_or(PlanType::Mto);

        let items: Vec<CreatePlanItemReq> = aggregated.values().map(|(qty, d)| {
            let (scheduled_start, scheduled_end, priority) = match item_map.get(&d.id) {
                Some(item) => (item.scheduled_start, item.scheduled_end, item.priority),
                None => (default_start, default_end, d.priority),
            };

            CreatePlanItemReq {
                product_id: d.product_id,
                planned_qty: *qty,
                scheduled_start,
                scheduled_end,
                sales_order_id: Some(d.source_id),
                sales_order_item_id: Some(d.source_line_id),
                bom_snapshot_id: None,
                routing_id: None,
                work_center_id: None,
                priority,
            }
        }).collect();

        let plan_req = CreatePlanReq {
            plan_type,
            plan_date: req.plan_date,
            remark: req.remark.clone(),
            items,
        };

        let plan_id = new_production_plan_service(self.pool.clone())
            .create(ctx, db, plan_req)
            .await?;

        // 5. 关联需求：更新 target_doc + 发布 DemandConfirmed 事件
        let event_bus = new_domain_event_bus(self.pool.clone());
        for d in &locked {
            DemandRepo::update_target_doc(db, d.id, DocumentType::ProductionPlan as i16, plan_id).await?;

            event_bus.publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: d.id,
                payload: serde_json::json!({
                    "order_id": d.source_id,
                    "order_line_id": d.source_line_id,
                    "product_id": d.product_id,
                    "acquire_channel": d.acquire_channel,
                    "target_doc_type": DocumentType::ProductionPlan as i16,
                    "target_doc_id": plan_id,
                }),
                idempotency_key: None,
            }).await?;
        }

        Ok(CreateDownstreamResult {
            doc_id: plan_id,
            processed_demand_count: locked.len(),
            skipped_demands,
            demand_status: "Confirmed".to_string(),
        })
    }
}
