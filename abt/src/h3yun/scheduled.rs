//! H3Yun 同步定时任务 — 扫描未同步实体并发送事件

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::info;

use super::models::{EntityType, Priority, SyncEvent};
use super::sync_state::SyncStateRepo;
use crate::service::{ScheduledTask, TaskRunResult};

pub struct H3YunSyncTask {
    pool: PgPool,
}

impl H3YunSyncTask {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScheduledTask for H3YunSyncTask {
    fn name(&self) -> &str {
        "h3yun_sync"
    }

    fn interval_secs(&self) -> u64 {
        300 // 5 分钟
    }

    fn timeout_secs(&self) -> u64 {
        600 // 覆盖默认 60s
    }

    async fn run_once(&self) -> anyhow::Result<TaskRunResult> {
        // 查询未同步的产品实体 ID
        let unsynced_products =
            SyncStateRepo::find_entity_ids_never_synced(&self.pool, EntityType::Product, 500)
                .await?;

        if unsynced_products.is_empty() {
            return Ok(TaskRunResult {
                processed: 0,
                succeeded: 0,
                message: "No unsynced entities".to_string(),
            });
        }

        let sender = crate::h3yun::get_sync_event_sender().clone();
        let mut queued = 0;

        for product_id in &unsynced_products {
            let event = SyncEvent {
                entity_type: EntityType::Product,
                entity_id: *product_id,
                priority: Priority::Low,
            };

            if sender.send(event).await.is_ok() {
                queued += 1;
            }
        }

        info!(queued, "H3Yun sync task queued unsynced products");

        Ok(TaskRunResult {
            processed: unsynced_products.len(),
            succeeded: queued,
            message: format!("Queued {queued} unsynced products"),
        })
    }
}
