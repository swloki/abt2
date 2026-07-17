//! 委外产出品入库 → 工序进度回写（同步直调）
//!
//! 历史上是消费 `OutsourcingReceived` 事件的 EventHandler；Issue #277 起 om.receive 不再发布
//! 该事件（产出品入库与工序闭环解耦），writeback 改由调用方同步直调：
//! - MES 委外收货（mes_work_center::osa_receive）：工序委外（Process）收货确认时推进工序
//! - OM 详情页 receive_order（Process 类型）：管理员一步产出品入库后推进工序
//!
//! 对 Process 类型（OutsourcingType::Process=2）委外单，回写 batch_routing_progress
//! （该道工序完成）+ 推进 batch.current_step，使委外收货后工序自动流转到下一道。
//!
//! 镜像 confirm_routing_step 的尾部（f2 累加 / g1 InProgress / g3 Completed / i 推进），
//! 跳过：d 工资、e3 人工+制费成本（已在 om.receive 立加工费 AP + 成本分录）、
//! e4 WIP 产出（已由 om.receive 入 WIP-SHOP）、h IPQC。
//!
//! 幂等：brp 已 Completed 则跳过（防重复调用重复推进 current_step）。
//! 事务：用独立 conn（非发布事务），所有写幂等，失败重试安全。

use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::mes::enums::{BatchStatus, RoutingStatus};
use crate::mes::production_batch::repo::{
    BatchRoutingProgressRepo, ProductionBatchRepo, WorkOrderRoutingRepo,
};
use crate::shared::types::{DomainError, Result};

pub struct OutsourcingReceivedHandler {
    pool: PgPool,
}

impl OutsourcingReceivedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 委外收货回写工序进度（MES osa_receive / OM 详情页 Process 收货同步直调，统一逻辑）。
    /// 镜像 confirm_routing_step 尾部：累加完成量 / Pending→InProgress / Completed+推进 current_step。
    /// Issue #277：om.receive 产出品入库不再发事件，writeback 完全由调用方同步触发。
    pub async fn writeback(
        &self,
        routing_id: i64,
        wo_id: i64,
        batch_id: Option<i64>,
        iqc_qty: Decimal,
    ) -> Result<()> {
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
}
