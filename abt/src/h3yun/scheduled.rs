//! H3Yun 同步定时任务

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::info;

use super::models::{EntityType, SyncEvent};
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

    fn timeout_secs(&self) -> u64 {
        600
    }

    async fn run_once(&self) -> anyhow::Result<TaskRunResult> {
        let sender = crate::h3yun::get_sync_event_sender();
        let channel_avail = sender.capacity();

        // 按 channel 剩余容量动态调整批量大小，避免 try_send 失败
        let limit = channel_avail.min(500) as i64;

        if limit == 0 {
            return Ok(TaskRunResult {
                processed: 0,
                succeeded: 0,
                message: "Channel full, skipping".to_string(),
            });
        }

        let unsynced_products =
            SyncStateRepo::find_entity_ids_never_synced(&self.pool, EntityType::Product, limit)
                .await?;

        if unsynced_products.is_empty() {
            return Ok(TaskRunResult {
                processed: 0,
                succeeded: 0,
                message: "No unsynced entities".to_string(),
            });
        }

        let mut queued = 0;
        let mut channel_full = 0;

        for product_id in &unsynced_products {
            let event = SyncEvent {
                entity_type: EntityType::Product,
                entity_id: *product_id,
            };

            match sender.try_send(event) {
                Ok(()) => queued += 1,
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => channel_full += 1,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    return Err(anyhow::anyhow!("sync channel closed"));
                }
            }
        }

        info!(queued, channel_full, "H3Yun sync task queued unsynced products");

        Ok(TaskRunResult {
            processed: unsynced_products.len(),
            succeeded: queued,
            message: format!("Queued {queued}, channel full skipped {channel_full}"),
        })
    }
}
