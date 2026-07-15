//! 委外收货 → 工序进度回写 EventHandler
//!
//! 消费 `OutsourcingReceived` 事件（om OutsourcingOrderService::receive 发布）。
//! 对 Process 类型（OutsourcingType::Process=2）委外单，回写 batch_routing_progress
//! （该道工序完成）+ 推进 batch.current_step，使委外收货后工序自动流转到下一道。
//!
//! 镜像 confirm_routing_step 的尾部（f2 累加 / g1 InProgress / g3 Completed / i 推进），
//! 跳过：d 工资、e3 人工+制费成本（已在 om receive 立加工费 AP + 成本分录）、
//! e4 WIP 产出（已由 om receive 入 WIP-SHOP）、h IPQC。
//!
//! 幂等：brp 已 Completed 则跳过（防 EventProcessor 重试重复推进 current_step）。
//! 事务：handler 用独立 conn（非发布事务），所有写幂等，失败重试安全。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::mes::enums::{BatchStatus, RoutingStatus};
use crate::mes::production_batch::repo::{
    BatchRoutingProgressRepo, ProductionBatchRepo, WorkOrderRoutingRepo,
};
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result};

pub struct OutsourcingReceivedHandler {
    pool: PgPool,
}

impl OutsourcingReceivedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for OutsourcingReceivedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        // 只处理 Process 类型委外（Full/Material/Rework 不回写工序）
        let otype = event
            .payload
            .get("outsourcing_type")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if otype != 2 {
            return Ok(());
        }

        let routing_id = event
            .payload
            .get("routing_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| DomainError::business_rule("OutsourcingReceived 缺 routing_id".to_string()))?;
        let wo_id = event
            .payload
            .get("work_order_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| DomainError::business_rule("OutsourcingReceived 缺 work_order_id".to_string()))?;
        let batch_id = event.payload.get("batch_id").and_then(|v| v.as_i64());
        let iqc_qty: Decimal = event
            .payload
            .get("iqc_passed_qty")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let routing = WorkOrderRoutingRepo::get_by_id(&mut conn, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;

        // 定位批次：优先 batch_id（drawer 创建委外单时携带）；否则取工单下
        // current_step == step_no-1 的活跃批次（Pending/InProgress）兜底
        let target_batches = match batch_id {
            Some(bid) => {
                let b = ProductionBatchRepo::get_by_id(&mut conn, bid)
                    .await?
                    .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;
                vec![b]
            }
            None => ProductionBatchRepo::list_by_work_order(&mut conn, wo_id)
                .await?
                .into_iter()
                .filter(|b| {
                    matches!(b.status, BatchStatus::Pending | BatchStatus::InProgress)
                        && b.current_step == routing.step_no - 1
                })
                .collect(),
        };

        for batch in &target_batches {
            let brp_id =
                BatchRoutingProgressRepo::upsert_and_get_id(&mut conn, batch.id, routing_id).await?;
            let existing =
                BatchRoutingProgressRepo::get_by_batch_and_routing(&mut conn, batch.id, routing_id)
                    .await?;

            // 幂等：已 Completed 跳过（防重试重复推进）
            if existing.as_ref().map(|b| b.status) == Some(RoutingStatus::Completed) {
                continue;
            }

            // f2: 累加完成量（委外合格量 = 本道产出量）
            BatchRoutingProgressRepo::atomic_increment_qty(
                &mut conn,
                brp_id,
                iqc_qty,
                Decimal::ZERO,
            )
            .await?;

            // g1: Pending → InProgress
            let was_pending = existing.as_ref().map(|b| b.status) == Some(RoutingStatus::Pending)
                || existing.is_none();
            if was_pending {
                BatchRoutingProgressRepo::update_status(
                    &mut conn,
                    brp_id,
                    RoutingStatus::InProgress,
                )
                .await?;
            }

            // g3 + i: 工序完成（累计 >= 批次量）→ Completed + 推进 current_step
            let total = existing.as_ref().map(|b| b.completed_qty).unwrap_or(Decimal::ZERO) + iqc_qty;
            if total >= batch.batch_qty {
                BatchRoutingProgressRepo::update_status(
                    &mut conn,
                    brp_id,
                    RoutingStatus::Completed,
                )
                .await?;
                ProductionBatchRepo::update_current_step(&mut conn, batch.id, routing.step_no)
                    .await?;
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "outsourcing_received_routing_writeback"
    }
}
