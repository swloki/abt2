use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::super::enums::PlanItemStatus;
use super::super::enums::PlanStatus;
use super::model::*;
use super::repo::ProductionPlanRepo;
use super::service::ProductionPlanService;
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::types::PgExecutor;
use crate::shared::enums::DocumentType;
use crate::mes::work_order::{new_work_order_service, model::CreateWorkOrderReq, service::WorkOrderService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

/// 构造批量下达失败项（统一 index 转换，避免多处重复字面量）
fn batch_failure(item_id: i64, err: DomainError) -> BatchFailure {
    BatchFailure { index: item_id as i32, error: err }
}

pub struct ProductionPlanServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProductionPlanServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionPlanService for ProductionPlanServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePlanReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ProductionPlan)
            .await
            .unwrap_or_else(|_| format!("PP{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let plan = ProductionPlanRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !req.items.is_empty() {
            ProductionPlanRepo::insert_items(&mut *db, plan.id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(plan.id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionPlan> {
        ProductionPlanRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<ProductionPlanItem>> {
        ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn confirm(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let plan = ProductionPlanRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionPlan"))?;

        if plan.status != PlanStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: plan.status.to_string(),
                to: PlanStatus::Confirmed.to_string(),
            });
        }

        ProductionPlanRepo::update_status(&mut *db, id, PlanStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 预校验：检查 Routing、BOM、物料可用性
    async fn pre_validate(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<ReleaseValidation>> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let mut validations = Vec::new();

        for item in &items {
            let mut warnings = Vec::new();
            let mut material_shortages = Vec::new();

            // 检查产品
            let product = match new_product_service(self.pool.clone())
                .get(ctx, db, item.product_id).await
            {
                Ok(p) => p,
                Err(_) => {
                    warnings.push(format!("产品 ID {} 不存在", item.product_id));
                    validations.push(ReleaseValidation {
                        plan_item_id: item.id,
                        product_id: item.product_id,
                        has_routing: false,
                        has_published_bom: false,
                        routing_id: None,
                        warnings,
                        material_shortages,
                    });
                    continue;
                }
            };

            // 检查 Routing
            let routing_detail = new_routing_service(self.pool.clone())
                .get_bom_routing(ctx, db, product.product_code.clone())
                .await
                .ok()
                .flatten();

            let has_routing = routing_detail.is_some();
            if !has_routing {
                warnings.push("该产品无关联工艺路线，将使用虚拟默认工序".to_string());
            }

            // 检查已发布 BOM
            let bom_id = new_bom_query_service(self.pool.clone())
                .find_published_bom_by_product_code(ctx, db, &product.product_code)
                .await
                .ok()
                .flatten();

            let has_published_bom = bom_id.is_some();
            if !has_published_bom {
                warnings.push("该产品无已发布 BOM，将跳过快照和物料预检".to_string());
            }

            // 物料可用性预检（仅当有 BOM 快照时）
            if let Some(snapshot_id) = item.bom_snapshot_id {
                let snapshot_opt = new_bom_query_service(self.pool.clone())
                    .get_snapshot_by_id(ctx, db, snapshot_id).await
                    .ok()
                    .flatten();

                if let Some(snapshot) = snapshot_opt {
                    let leaf_nodes = snapshot.bom_detail.leaf_nodes();

                    for node in &leaf_nodes {
                        let required_qty = node.quantity * item.planned_qty;
                        // 查询可用库存：stock_ledger SUM(available_qty)
                        let available_qty: Decimal = sqlx::query_scalar(
                            r#"SELECT COALESCE(SUM(available_qty), 0)
                               FROM stock_ledger
                               WHERE product_id = $1"#,
                        )
                        .bind(node.product_id)
                        .fetch_one(&mut *db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;

                        if available_qty < required_qty {
                            material_shortages.push(MaterialShortage {
                                product_id: node.product_id,
                                required_qty,
                                available_qty,
                                shortage_qty: required_qty - available_qty,
                            });
                        }
                    }
                }
            }

            if !material_shortages.is_empty() {
                warnings.push(format!(
                    "物料不足：{} 种组件短缺",
                    material_shortages.len()
                ));
            }

            validations.push(ReleaseValidation {
                plan_item_id: item.id,
                product_id: item.product_id,
                has_routing,
                has_published_bom,
                routing_id: routing_detail.map(|rd| rd.routing.id),
                warnings,
                material_shortages,
            });
        }

        Ok(validations)
    }

    /// 一键下达：预校验 → 逐个创建+release → 失败隔离
    async fn release_to_work_orders(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<BatchReleaseResult> {
        let items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. 预校验
        let validations = self.pre_validate(ctx, db, plan_id).await?;

        let mut successful = Vec::new();
        let mut failed = Vec::new();

        let work_order_svc = new_work_order_service(self.pool.clone());

        // 2. 逐个创建 + release（单工单失败不影响其余）
        for item in &items {
            let scheduled_start = item.scheduled_start;
            let scheduled_end = item.scheduled_end;

            // 创建工单
            let create_result = work_order_svc.create(
                ctx, db,
                CreateWorkOrderReq {
                    plan_item_id: Some(item.id),
                    product_id: item.product_id,
                    bom_snapshot_id: None, // release() 中动态创建
                    routing_id: item.routing_id,
                    planned_qty: item.planned_qty,
                    scheduled_start,
                    scheduled_end,
                    work_center_id: item.work_center_id,
                    sales_order_id: item.sales_order_id,
                    remark: None,
                },
            ).await;

            let wo_id = match create_result {
                Ok(id) => id,
                Err(e) => {
                    failed.push(batch_failure(item.id, e));
                    continue;
                }
            };

            // 立即 release
            let wo = match work_order_svc.find_by_id(ctx, db, wo_id).await {
                Ok(wo) => wo,
                Err(e) => {
                    failed.push(batch_failure(item.id, e));
                    continue;
                }
            };

            match work_order_svc.release(ctx, db, wo_id, wo.version).await {
                Ok(()) => {
                    // 更新 PlanItem 状态 → Released
                    if let Err(_e) = ProductionPlanRepo::update_item_status(
                        &mut *db, item.id,
                        PlanItemStatus::Released,
                    ).await {
                        // PlanItem 状态更新失败不影响主流程
                    }

                    if let Ok(released_wo) = work_order_svc.find_by_id(ctx, db, wo_id).await {
                        successful.push(released_wo);
                    }
                }
                Err(e) => {
                    failed.push(batch_failure(item.id, e));
                }
            }
        }

        // 3. 更新计划状态
        if !successful.is_empty() {
            ProductionPlanRepo::update_status(&mut *db, plan_id, PlanStatus::InProgress)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        let total = items.len() as i32;
        Ok(BatchReleaseResult {
            plan_id,
            successful_work_orders: successful,
            failed_items: failed,
            validations,
            total,
        })
    }

    async fn generate_work_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
        items: Vec<WorkOrderPlanItem>,
    ) -> Result<Vec<i64>> {
        // 日期校验
        for item in &items {
            if item.scheduled_end < item.scheduled_start {
                return Err(DomainError::Validation(format!(
                    "排程结束日期不能早于开始日期（plan_item_id={}）",
                    item.plan_item_id
                )));
            }
        }

        let work_order_svc = new_work_order_service(self.pool.clone());
        let mut wo_ids = Vec::with_capacity(items.len());

        for item in &items {
            let wo_id = work_order_svc
                .create(
                    ctx,
                    db,
                    CreateWorkOrderReq {
                        plan_item_id: Some(item.plan_item_id),
                        product_id: item.product_id,
                        bom_snapshot_id: None,
                        routing_id: item.routing_id,
                        planned_qty: item.planned_qty,
                        scheduled_start: item.scheduled_start,
                        scheduled_end: item.scheduled_end,
                        work_center_id: item.work_center_id,
                        sales_order_id: None,
                        remark: None,
                    },
                )
                .await?;
            wo_ids.push(wo_id);
        }

        Ok(wo_ids)
    }

    async fn mark_in_progress(
        &self,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<()> {
        ProductionPlanRepo::update_status(db, plan_id, PlanStatus::InProgress)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: PlanFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ProductionPlan>> {
        ProductionPlanRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_plan_stats(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        plan_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, PlanExtraStats>> {
        ProductionPlanRepo::get_plan_stats(&mut *db, plan_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 排程 V1：按交期倒推排程日期，标记紧急项
    /// - 按优先级排序（priority 越小越优先）
    /// - 交期早的排在前面
    /// - scheduled_start < today() → 标记紧急（priority = 0）
    async fn schedule_v1(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<()> {
        let mut items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let today = chrono::Local::now().date_naive();

        // 按优先级排序，交期早的排在前面
        items.sort_by(|a, b| {
            a.priority.cmp(&b.priority)
                .then_with(|| a.scheduled_end.cmp(&b.scheduled_end))
        });

        // 标记紧急项
        for item in &items {
            if item.scheduled_start < today && item.priority > 0 {
                ProductionPlanRepo::update_item_priority(
                    &mut *db, item.id, 0,
                ).await.map_err(|e| DomainError::Internal(e.into()))?;
            }
        }

        Ok(())
    }
}
