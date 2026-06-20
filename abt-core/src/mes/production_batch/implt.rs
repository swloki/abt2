//! ProductionBatchService 具体实现
//!
//! 核心方法 `confirm_routing_step` 是 MES 执行层的原子事务入口。
//! WorkOrderRouting 属于工单级，批次通过 work_order_id 引用工序。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::{ProductionBatchRepo, WorkOrderRoutingRepo, WorkReportRepo, BatchRoutingProgressRepo};
use super::service::ProductionBatchService;
use crate::mes::enums::*;
use crate::mes::work_order::repo::WorkOrderRepo;
use crate::mes::production_plan::repo::ProductionPlanRepo;
use crate::mes::production_inspection::model::CreateInspectionReq;
use crate::mes::production_inspection::repo::ProductionInspectionRepo;
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::mes::work_order::{new_work_order_service, service::WorkOrderService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::types::PgExecutor;
use crate::shared::enums::DocumentType;
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::audit_log::{new_audit_log_service, RecordAuditLogReq, service::AuditLogService};
use crate::shared::enums::audit::AuditAction;
use serde_json::json;

pub struct ProductionBatchServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProductionBatchServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionBatchService for ProductionBatchServiceImpl {
    /// 创建生产批次（流转卡）
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateBatchReq,
    ) -> Result<i64> {
        let batch_no = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ProductionBatch)
            .await
            .unwrap_or_else(|_| format!("PB{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let card_sn = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::FlowCard)
            .await
            .unwrap_or_else(|_| format!("FC{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f")));

        let batch = ProductionBatchRepo::insert(
            &mut *db,
            &req,
            &batch_no,
            &card_sn,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 为新批次初始化所有工序的 batch_routing_progress 记录
        BatchRoutingProgressRepo::init_for_batch(&mut *db, batch.id, req.work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(batch.id)
    }

    /// 按工单拆分多个批次
    async fn split_work_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
        splits: Vec<SplitReq>,
    ) -> Result<Vec<i64>> {
        if splits.is_empty() {
            return Err(DomainError::validation("至少需要一个拆分项"));
        }

        // 校验拆分量必须 > 0
        if splits.iter().any(|s| s.batch_qty <= Decimal::ZERO) {
            return Err(DomainError::validation("拆分量必须大于 0"));
        }

        let work_order = WorkOrderRepo::get_by_id(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        // 校验工单状态：仅 Released/InProduction 可拆批
        if work_order.status != WorkOrderStatus::Released
            && work_order.status != WorkOrderStatus::InProduction
        {
            return Err(DomainError::BusinessRule(
                "仅已下达/生产中工单可拆批".to_string(),
            ));
        }

        // 校验总量：已有批次总量 + 本次拆分总量 ≤ planned_qty × (1 + tolerance)
        let existing_batches = ProductionBatchRepo::list_by_work_order(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let existing_qty: Decimal = existing_batches.iter().map(|b| b.batch_qty).sum();
        let split_qty: Decimal = splits.iter().map(|s| s.batch_qty).sum();
        let tolerance = get_over_completion_tolerance(&self.pool, ctx, db, work_order_id).await?;
        let max_allowed = work_order.planned_qty * (Decimal::ONE + tolerance);
        if existing_qty + split_qty > max_allowed {
            return Err(DomainError::BusinessRule(format!(
                "拆分总量 {} + 已有 {} 超过计划量 {} 的容差上限",
                split_qty, existing_qty, max_allowed
            )));
        }

        let mut results = Vec::with_capacity(splits.len());

        for split in &splits {
            let req = CreateBatchReq {
                work_order_id,
                product_id: work_order.product_id,
                batch_qty: split.batch_qty,
                team_id: split.team_id,
            };

            let id = self.create(ctx, db, req).await?;
            results.push(id);
        }

        Ok(results)
    }

    /// 按ID查找批次
    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionBatch> {
        ProductionBatchRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))
    }

    /// 按流转卡序列号查找批次
    async fn find_by_card_sn(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        card_sn: String,
    ) -> Result<Option<ProductionBatch>> {
        ProductionBatchRepo::find_by_card_sn(&mut *db, &card_sn)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 按工单ID列出所有批次
    async fn list_by_work_order(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ProductionBatch>> {
        ProductionBatchRepo::list_by_work_order(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 确认工序报工 — MES 执行层核心原子事务
    ///
    /// WorkOrderRouting 属于工单级，通过 batch.work_order_id 查找工序。
    async fn confirm_routing_step(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        step_no: i32,
        req: StepConfirmationReq,
    ) -> Result<StepConfirmationResult> {
        // --- a. 获取批次并验证状态 ---
        let batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        // 状态校验：Pending / InProgress 均允许报工。工序顺序由下方防跳序 Guard 保证，
        // 此处不再用 step_no==1 限制 Pending —— 否则历史脏数据批次（status=Pending 但
        // current_step>0，由旧版首道报工未同步状态导致）报后续工序会在此被拒，
        // 到不了 k 段的自愈逻辑。
        match batch.status {
            BatchStatus::Pending | BatchStatus::InProgress => {}
            other => {
                return Err(DomainError::InvalidStateTransition {
                    from: other.to_string(),
                    to: "InProgress".to_string(),
                });
            }
        }

        // --- b. 防跳序 Guard ---
        if batch.current_step != step_no - 1 {
            return Err(DomainError::business_rule(format!(
                "工序跳序拦截：当前工序 {}，请求报工工序 {}",
                batch.current_step, step_no
            )));
        }

        // --- c. 获取工序（工单级） ---
        let routing = WorkOrderRoutingRepo::get_by_work_order_and_step(
            &mut *db,
            batch.work_order_id,
            step_no,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .ok_or_else(|| DomainError::not_found(format!(
            "WorkOrderRouting({}, {})", batch.work_order_id, step_no
        )))?;

        // --- d. 计算工资 ---
        let unit_price = routing.unit_price.unwrap_or(Decimal::ZERO);
        let non_operator_defect_qty = match req.defect_reason {
            Some(reason) if reason.affect_wage() => req.defect_qty,
            _ => Decimal::ZERO,
        };
        let wage_amount = (req.completed_qty + non_operator_defect_qty) * unit_price;

        // --- e. 幂等 INSERT work_reports ---
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::WorkReport)
            .await
            .unwrap_or_else(|_| format!("WR{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let remark_str = req.remark.as_deref().unwrap_or("");

        let (report, was_inserted) = WorkReportRepo::insert_or_get_existing(
            &mut *db,
            &InsertWorkReportParams {
                doc_number: &doc_number,
                work_order_id: batch.work_order_id,
                batch_id,
                routing_id: routing.id,
                report_date: req.report_date,
                shift: req.shift,
                worker_id: req.worker_id,
                completed_qty: req.completed_qty,
                defect_qty: req.defect_qty,
                defect_reason: req.defect_reason,
                work_hours: req.work_hours,
                remark: remark_str,
                operator_id: ctx.operator_id,
            },
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let work_report_id = report.id;
        // --- f1. UPSERT batch_routing_progress (batch_id, routing_id) ---
        let brp_id = BatchRoutingProgressRepo::upsert_and_get_id(
            &mut *db, batch_id, routing.id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 查现有 brp（用于超额校验和状态判断）
        let existing_brp = BatchRoutingProgressRepo::get_by_batch_and_routing(
            &mut *db, batch_id, routing.id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let prev_completed = existing_brp.as_ref().map(|b| b.completed_qty).unwrap_or(Decimal::ZERO);
        let prev_defect = existing_brp.as_ref().map(|b| b.defect_qty).unwrap_or(Decimal::ZERO);
        let was_pending = existing_brp.as_ref().map(|b| b.status) == Some(RoutingStatus::Pending)
            || existing_brp.is_none();

        // --- e2. 超额容差校验（最后工序，基于批次自身累计而非工单级共享） ---
        if was_inserted {
            let all_routings_for_check = WorkOrderRoutingRepo::get_by_work_order_id(
                &mut *db, batch.work_order_id,
            ).await.map_err(|e| DomainError::Internal(e.into()))?;

            let max_step: i32 = all_routings_for_check.iter().map(|r| r.step_no).max().unwrap_or(0);
            let is_last_step = step_no == max_step;

            if is_last_step {
                let total_reported = prev_completed + prev_defect
                    + req.completed_qty + req.defect_qty;

                let tolerance = get_over_completion_tolerance(&self.pool, ctx, db, batch.work_order_id).await?;
                let max_allowed = batch.batch_qty * (Decimal::ONE + tolerance);

                if total_reported > max_allowed {
                    return Err(DomainError::BusinessRule(
                        format!(
                            "报工量 {} 超出计划量 {} 的允许偏差范围（容差 {}%）",
                            total_reported,
                            batch.batch_qty,
                            tolerance * Decimal::ONE_HUNDRED
                        ),
                    ));
                }
            }
        }

        // --- f2-f4. 四层同步累加（行锁原子操作，同事务内） ---
        if was_inserted {
            // f2: batch_routing_progress（写真相源）
            BatchRoutingProgressRepo::atomic_increment_qty(
                &mut *db, brp_id, req.completed_qty, req.defect_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // f3: production_batches（冗余字段，供批次列表/详情）
            ProductionBatchRepo::atomic_increment_qty(
                &mut *db, batch_id, req.completed_qty, req.defect_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // f4: work_orders（冗余字段，供工单列表筛选）
            WorkOrderRepo::atomic_increment_completed_qty(
                &mut *db, batch.work_order_id, req.completed_qty, req.defect_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // --- g1. brp 状态: Pending → InProgress ---
            if was_pending {
                BatchRoutingProgressRepo::update_status(
                    &mut *db, brp_id, RoutingStatus::InProgress,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }

            // --- g2. 首次报工设 actual_start（对标 Odoo button_start 记录 date_start） ---
            if was_pending {
                sqlx::query(
                    "UPDATE batch_routing_progress SET started_at = NOW() WHERE id = $1 AND started_at IS NULL",
                )
                .bind(brp_id)
                .execute(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                sqlx::query(
                    "UPDATE production_batches SET actual_start = NOW() WHERE id = $1 AND actual_start IS NULL",
                )
                .bind(batch_id)
                .execute(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }

            // --- g3. 工序完成判定（对标 Odoo button_finish 设 done + date_finished） ---
            let new_completed = prev_completed + req.completed_qty;
            if new_completed >= batch.batch_qty {
                BatchRoutingProgressRepo::update_status(
                    &mut *db, brp_id, RoutingStatus::Completed,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                sqlx::query(
                    "UPDATE batch_routing_progress SET completed_at = NOW() WHERE id = $1",
                )
                .bind(brp_id)
                .execute(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

                // 最后工序完成 → 设 batch.actual_end
                let max_step: i32 = WorkOrderRoutingRepo::get_by_work_order_id(
                    &mut *db, batch.work_order_id,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .iter()
                .map(|r| r.step_no)
                .max()
                .unwrap_or(0);
                if routing.step_no == max_step {
                    sqlx::query(
                        "UPDATE production_batches SET actual_end = NOW() WHERE id = $1",
                    )
                    .bind(batch_id)
                    .execute(&mut *db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }

        // --- h. 检查是否需要报检 ---
        let mut inspection_triggered = false;
        if routing.is_inspection_point && was_inserted {
            let inspection_req = CreateInspectionReq {
                work_order_id: batch.work_order_id,
                product_id: batch.product_id,
                routing_id: Some(routing.id),
                inspection_type: InspectionType::InProcess,
                sample_qty: req.completed_qty,
                inspection_date: req.report_date,
                disposition: None,
                remark: Some(format!("工序 {step_no} 自动触发 IPQC")),
            };
            let inspection_doc = new_document_sequence_service(self.pool.clone())
                .next_number(ctx, db, DocumentType::ProductionInspection)
                .await
                .unwrap_or_else(|_| format!("PI{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

            ProductionInspectionRepo::insert(
                &mut *db,
                &inspection_req,
                &inspection_doc,
                ctx.operator_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            inspection_triggered = true;

            ProductionBatchRepo::update_status(
                &mut *db,
                batch_id,
                BatchStatus::Suspended,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // --- i. 更新 batch.current_step ---
        if was_inserted {
            ProductionBatchRepo::update_current_step(&mut *db, batch_id, step_no)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // --- j. 计算下一工序 ---
        let all_routings = WorkOrderRoutingRepo::get_by_work_order_id(
            &mut *db,
            batch.work_order_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let max_step = all_routings.iter().map(|r| r.step_no).max().unwrap_or(0);
        let next_step_no = if step_no < max_step { Some(step_no + 1) } else { None };

        // --- k. 判断是否最后一道工序 → PendingReceipt ---
        let mut batch_status = batch.status;
        if step_no == max_step && was_inserted {
            if !routing.is_inspection_point {
                ProductionBatchRepo::update_status(
                    &mut *db,
                    batch_id,
                    BatchStatus::PendingReceipt,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                batch_status = BatchStatus::PendingReceipt;
            } else {
                // 最后一道工序有检验点，批次已在步骤 h 设为 Suspended
                batch_status = BatchStatus::Suspended;
            }
        } else if was_inserted {
            // 非最后工序：首次报工（批次原为 Pending）需把批次推进到 InProgress。
            // 否则 current_step 已前进但 status 仍为 Pending，后续工序报工时
            // 步骤 a 的状态校验会拒绝（Pending 仅允许 step_no==1）。
            if batch.status == BatchStatus::Pending {
                ProductionBatchRepo::update_status(
                    &mut *db,
                    batch_id,
                    BatchStatus::InProgress,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                batch_status = BatchStatus::InProgress;
            } else {
                let updated_batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;
                batch_status = updated_batch.status;
            }
        }

        // --- k2. 状态传播：首次报工时推进上游工单和计划行状态 ---
        // batch.status 是步骤 a 读取的原始值（Pending 表示首次报工）
        if was_inserted && batch.status == BatchStatus::Pending {
            // WorkOrder: Released → InProduction
            new_work_order_service(self.pool.clone())
                .mark_in_production(ctx, db, batch.work_order_id)
                .await?;

            // PlanItem: Released → InProduction
            ProductionPlanRepo::update_item_status_by_work_order(
                &mut *db,
                batch.work_order_id,
                PlanItemStatus::InProduction,
            )
            .await?;
        }

        // --- l. 返回结果 ---
        Ok(StepConfirmationResult {
            work_report_id,
            batch_id,
            step_no,
            next_step_no,
            batch_status,
            inspection_triggered,
            wage_amount,
        })
    }

    async fn update_routing_unit_price(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        unit_price: Decimal,
    ) -> Result<WorkOrderRouting> {
        // 守卫 1：单价 > 0
        if unit_price <= Decimal::ZERO {
            return Err(DomainError::validation("计件单价必须大于 0"));
        }
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 守卫 2/3：routing 存在且属于该工单
        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }

        // 守卫 2：工单状态 ∈ {Released, InProduction}
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *tx, work_order_id)
            .await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许修改工序单价"));
        }

        // 守卫 4：该工序未报工（事务内复查防并发）
        if WorkOrderRoutingRepo::has_report(&mut *tx, routing_id).await? {
            return Err(DomainError::business_rule("该工序已报工，单价不可修改"));
        }

        let old_price = routing.unit_price;
        WorkOrderRoutingRepo::update_unit_price(&mut *tx, routing_id, unit_price).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, &mut *tx, RecordAuditLogReq {
                entity_type: "WorkOrderRouting",
                entity_id: routing_id,
                action: AuditAction::Update,
                changes: Some(json!(format!(
                    "unit_price: {:?} → {:?}",
                    old_price, unit_price
                ))),
                context: Some(json!(format!("work_order_id={}", work_order_id))),
            })
            .await?;

        let updated = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;

        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(updated)
    }

    async fn delete_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let routing = WorkOrderRoutingRepo::get_by_id(&mut *tx, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if routing.work_order_id != work_order_id {
            return Err(DomainError::not_found("WorkOrderRouting"));
        }

        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, &mut *tx, work_order_id)
            .await?;
        if !matches!(wo.status, WorkOrderStatus::Released | WorkOrderStatus::InProduction) {
            return Err(DomainError::business_rule("工单当前状态不允许删除工序"));
        }

        // 守卫：整单零报工
        if WorkOrderRoutingRepo::has_any_report(&mut *tx, work_order_id).await? {
            return Err(DomainError::business_rule("工单已有报工记录，不可删除工序"));
        }
        // 守卫：至少保留一道
        let remaining: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint FROM work_order_routings WHERE work_order_id = $1"#,
        )
        .bind(work_order_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if remaining <= 1 {
            return Err(DomainError::business_rule("至少保留一道工序"));
        }

        WorkOrderRoutingRepo::delete(&mut *tx, routing_id).await?;
        WorkOrderRoutingRepo::renumber_steps(&mut *tx, work_order_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, &mut *tx, RecordAuditLogReq {
                entity_type: "WorkOrderRouting",
                entity_id: routing_id,
                action: AuditAction::Delete,
                changes: Some(json!(format!(
                    "删除工序 {} {}",
                    routing.step_no, routing.process_name
                ))),
                context: Some(json!(format!("work_order_id={}", work_order_id))),
            })
            .await?;

        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        Ok(())
    }

    /// 推进到待入库状态
    async fn advance_to_receipt(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<()> {
        let batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress && batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "PendingReceipt".to_string(),
            });
        }

        let routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, batch.work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let last_routing = routings.iter().max_by_key(|r| r.step_no);
        if let Some(last) = last_routing {
            // 检查该批次在最后工序的执行进度是否完成
            let brp = BatchRoutingProgressRepo::get_by_batch_and_routing(
                &mut *db, batch_id, last.id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            let is_completed = brp.as_ref().map(|b| b.status) == Some(RoutingStatus::Completed);
            if !is_completed {
                return Err(DomainError::business_rule(format!(
                    "最后工序 {} 尚未完成，无法推进到待入库",
                    last.step_no
                )));
            }

            // 标记 brp 为 Completed
            if let Some(b) = &brp {
                BatchRoutingProgressRepo::update_status(
                    &mut *db, b.id, RoutingStatus::Completed,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }

        ProductionBatchRepo::update_status(&mut *db, batch_id, BatchStatus::PendingReceipt)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 暂停批次
    async fn suspend(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        reason: String,
    ) -> Result<()> {
        let batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "Suspended".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *db, batch_id, BatchStatus::Suspended)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        tracing::info!(batch_id, reason = %reason, "batch suspended");

        Ok(())
    }

    /// 恢复批次
    async fn resume(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<()> {
        let batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "InProgress".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *db, batch_id, BatchStatus::InProgress)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 报废批次：标记为 Cancelled，释放 HARD 预留
    async fn scrap(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        batch_id: i64,
        reason: String,
    ) -> Result<()> {
        let batch = ProductionBatchRepo::get_by_id(&mut *db, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress && batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "Cancelled".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *db, batch_id, BatchStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 释放 HARD 预留
        new_inventory_reservation_service(self.pool.clone())
            .cancel_by_source(ctx, db, DocumentType::WorkOrder, batch_id).await?;

        tracing::info!(batch_id, reason = %reason, "batch scrapped");

        Ok(())
    }

    async fn list_batches(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: BatchListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<crate::shared::types::PaginatedResult<BatchListItem>> {
        let (items, total) = ProductionBatchRepo::list_batches(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(crate::shared::types::PaginatedResult::new(items, total as u64, page, page_size))
    }

    async fn list_routings(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderRouting>> {
        WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_product_name(
        &self,
        db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT pdt_name FROM products WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(|r| r.0))
    }
}

/// 获取超额完工容差（优先级：产品 → 系统默认 5%）
async fn get_over_completion_tolerance(
    pool: &PgPool,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    work_order_id: i64,
) -> Result<Decimal> {
    let wo = new_work_order_service(pool.clone())
        .find_by_id(ctx, db, work_order_id).await?;

    let product = new_product_service(pool.clone())
        .get(ctx, db, wo.product_id).await?;

    Ok(product
        .meta
        .over_completion_tolerance
        .unwrap_or_else(crate::master_data::product::default_over_completion_tolerance))
}
