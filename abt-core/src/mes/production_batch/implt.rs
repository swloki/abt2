//! ProductionBatchService 具体实现
//!
//! 核心方法 `confirm_routing_step` 是 MES 执行层的原子事务入口。
//! WorkOrderRouting 属于工单级，批次通过 work_order_id 引用工序。

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::{ProductionBatchRepo, WorkOrderRoutingRepo, WorkReportRepo};
use super::service::ProductionBatchService;
use crate::mes::enums::*;
use crate::mes::work_order::repo::WorkOrderRepo;
use crate::mes::production_inspection::model::CreateInspectionReq;
use crate::mes::production_inspection::repo::ProductionInspectionRepo;
use crate::mes::stubs::{DocumentSequenceStub, InventoryReservationStub};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

pub struct ProductionBatchServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl ProductionBatchServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionBatchService for ProductionBatchServiceImpl {
    /// 创建生产批次（流转卡）
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateBatchReq,
    ) -> Result<i64, DomainError> {
        let batch_no = DocumentSequenceStub::next_number(ctx.reborrow(), "PB-")
            .await
            .unwrap_or_else(|_| format!("PB{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let card_sn = DocumentSequenceStub::next_number(ctx.reborrow(), "CS-")
            .await
            .unwrap_or_else(|_| format!("CS{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f")));

        let batch = ProductionBatchRepo::insert(
            &mut *ctx.executor,
            &req,
            &batch_no,
            &card_sn,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(batch.id)
    }

    /// 按工单拆分多个批次
    async fn split_work_order(
        &self,
        mut ctx: ServiceContext<'_>,
        work_order_id: i64,
        splits: Vec<SplitReq>,
    ) -> Result<Vec<i64>, DomainError> {
        if splits.is_empty() {
            return Err(DomainError::validation("至少需要一个拆分项"));
        }

        let work_order = WorkOrderRepo::get_by_id(&mut *ctx.executor, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        let mut results = Vec::with_capacity(splits.len());

        for split in &splits {
            let req = CreateBatchReq {
                work_order_id,
                product_id: work_order.product_id,
                batch_qty: split.batch_qty,
                team_id: split.team_id,
            };

            let id = self.create(ctx.reborrow(), req).await?;
            results.push(id);
        }

        Ok(results)
    }

    /// 按ID查找批次
    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionBatch, DomainError> {
        ProductionBatchRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))
    }

    /// 按工单ID列出所有批次
    async fn list_by_work_order(
        &self,
        ctx: ServiceContext<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ProductionBatch>, DomainError> {
        ProductionBatchRepo::list_by_work_order(&mut *ctx.executor, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 确认工序报工 — MES 执行层核心原子事务
    ///
    /// WorkOrderRouting 属于工单级，通过 batch.work_order_id 查找工序。
    async fn confirm_routing_step(
        &self,
        mut ctx: ServiceContext<'_>,
        batch_id: i64,
        step_no: i32,
        req: StepConfirmationReq,
    ) -> Result<StepConfirmationResult, DomainError> {
        // --- a. 获取批次并验证状态 ---
        let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        match batch.status {
            BatchStatus::Pending if step_no == 1 => {}
            BatchStatus::InProgress => {}
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
            &mut *ctx.executor,
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
        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "WR-")
            .await
            .unwrap_or_else(|_| format!("WR{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let remark_str = req.remark.as_deref().unwrap_or("");

        let (report, was_inserted) = WorkReportRepo::insert_or_get_existing(
            &mut *ctx.executor,
            &doc_number,
            batch.work_order_id,
            batch_id,
            routing.id,
            req.report_date,
            req.shift,
            req.worker_id,
            req.completed_qty,
            req.defect_qty,
            req.defect_reason,
            req.work_hours,
            remark_str,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let work_report_id = report.id;

        // --- f. 原子增量 completed_qty / defect_qty ---
        if was_inserted {
            WorkOrderRoutingRepo::atomic_increment_qty(
                &mut *ctx.executor,
                routing.id,
                req.completed_qty,
                req.defect_qty,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            // --- g. 更新工序状态为 InProgress ---
            if routing.status == RoutingStatus::Pending {
                WorkOrderRoutingRepo::update_status(
                    &mut *ctx.executor,
                    routing.id,
                    RoutingStatus::InProgress,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
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
            let inspection_doc = DocumentSequenceStub::next_number(ctx.reborrow(), "PI-")
                .await
                .unwrap_or_else(|_| format!("PI{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

            ProductionInspectionRepo::insert(
                &mut *ctx.executor,
                &inspection_req,
                &inspection_doc,
                ctx.operator_id,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

            inspection_triggered = true;

            ProductionBatchRepo::update_status(
                &mut *ctx.executor,
                batch_id,
                BatchStatus::Suspended,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // --- i. 更新 batch.current_step ---
        if was_inserted {
            ProductionBatchRepo::update_current_step(&mut *ctx.executor, batch_id, step_no)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // --- j. 计算下一工序 ---
        let all_routings = WorkOrderRoutingRepo::get_by_work_order_id(
            &mut *ctx.executor,
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
                    &mut *ctx.executor,
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
            let updated_batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;
            batch_status = updated_batch.status;
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

    /// 推进到待入库状态
    async fn advance_to_receipt(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
    ) -> Result<(), DomainError> {
        let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress && batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "PendingReceipt".to_string(),
            });
        }

        let routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *ctx.executor, batch.work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let last_routing = routings.iter().max_by_key(|r| r.step_no);
        if let Some(last) = last_routing
            && last.status != RoutingStatus::Completed
        {
            return Err(DomainError::business_rule(format!(
                "最后工序 {} 尚未完成，无法推进到待入库",
                last.step_no
            )));
        }

        ProductionBatchRepo::update_status(&mut *ctx.executor, batch_id, BatchStatus::PendingReceipt)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 暂停批次
    async fn suspend(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
        _reason: String,
    ) -> Result<(), DomainError> {
        let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "Suspended".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *ctx.executor, batch_id, BatchStatus::Suspended)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 恢复批次
    async fn resume(
        &self,
        ctx: ServiceContext<'_>,
        batch_id: i64,
    ) -> Result<(), DomainError> {
        let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "InProgress".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *ctx.executor, batch_id, BatchStatus::InProgress)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 报废批次：标记为 Cancelled，释放 HARD 预留
    async fn scrap(
        &self,
        mut ctx: ServiceContext<'_>,
        batch_id: i64,
        _reason: String,
    ) -> Result<(), DomainError> {
        let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, batch_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

        if batch.status != BatchStatus::InProgress && batch.status != BatchStatus::Suspended {
            return Err(DomainError::InvalidStateTransition {
                from: batch.status.to_string(),
                to: "Cancelled".to_string(),
            });
        }

        ProductionBatchRepo::update_status(&mut *ctx.executor, batch_id, BatchStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 释放 HARD 预留
        let _ = InventoryReservationStub::release(ctx.reborrow(), "production_batch", batch_id).await;

        Ok(())
    }
}
