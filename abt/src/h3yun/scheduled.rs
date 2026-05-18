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

        let mut total_processed = 0;
        let mut total_queued = 0;
        let mut total_channel_full = 0;

        // 1. 同步未同步的产品
        let unsynced_products =
            SyncStateRepo::find_entity_ids_never_synced(&self.pool, EntityType::Product, limit)
                .await?;

        total_processed += unsynced_products.len();

        for product_id in &unsynced_products {
            let event = SyncEvent {
                entity_type: EntityType::Product,
                entity_id: *product_id,
                is_batch: false,
            };

            match sender.try_send(event) {
                Ok(()) => total_queued += 1,
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => total_channel_full += 1,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    return Err(anyhow::anyhow!("sync channel closed"));
                }
            }
        }

        // 2. 同步未同步的库存
        let remaining_capacity = sender.capacity().min(500) as i64;
        if remaining_capacity > 0 {
            let unsynced_inventories =
                SyncStateRepo::find_unsynced_inventories(&self.pool, remaining_capacity).await?;

            total_processed += unsynced_inventories.len();

            for inventory_id in &unsynced_inventories {
                let event = SyncEvent {
                    entity_type: EntityType::Inventory,
                    entity_id: *inventory_id,
                    is_batch: false,
                };

                match sender.try_send(event) {
                    Ok(()) => total_queued += 1,
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => total_channel_full += 1,
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        return Err(anyhow::anyhow!("sync channel closed"));
                    }
                }
            }
        }

        info!(
            total_queued,
            total_channel_full,
            products = unsynced_products.len(),
            "H3Yun sync task queued unsynced entities"
        );

        Ok(TaskRunResult {
            processed: total_processed,
            succeeded: total_queued,
            message: format!("Queued {total_queued}, channel full skipped {total_channel_full}"),
        })
    }
}
