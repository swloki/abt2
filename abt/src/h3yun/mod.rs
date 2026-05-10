//! H3Yun ERP 同步模块
//!
//! 单向同步 ABT 产品和库存数据到 H3Yun ERP。

pub mod client;
pub mod inventory_sync;
pub mod models;
pub mod product_sync;
pub mod scheduled;
pub mod sync_state;
pub mod sync_worker;

use client::H3YunClient;
use models::SyncEvent;
use std::sync::OnceLock;
use tokio::sync::mpsc::Sender;

static SYNC_SENDER: OnceLock<Sender<SyncEvent>> = OnceLock::new();
static H3YUN_CLIENT: OnceLock<H3YunClient> = OnceLock::new();

pub fn get_sync_event_sender() -> &'static Sender<SyncEvent> {
    SYNC_SENDER
        .get()
        .expect("Sync event sender not initialized. Call start_sync_channel() first.")
}

pub(crate) fn set_sync_event_sender(sender: Sender<SyncEvent>) {
    SYNC_SENDER
        .set(sender)
        .expect("Sync event sender already initialized");
}

pub fn get_h3yun_client() -> &'static H3YunClient {
    H3YUN_CLIENT
        .get()
        .expect("H3Yun client not initialized. Call init_h3yun_client() first.")
}

pub fn init_h3yun_client(client: H3YunClient) {
    H3YUN_CLIENT
        .set(client)
        .expect("H3Yun client already initialized");
}

pub fn is_initialized() -> bool {
    SYNC_SENDER.get().is_some()
}

/// Shared create-or-update logic for syncing an entity to H3Yun
pub(crate) async fn sync_entity(
    pool: &sqlx::PgPool,
    client: &H3YunClient,
    schema_code: &str,
    entity_type: models::EntityType,
    entity_id: i64,
    biz_json: &str,
    label: &str,
) -> Result<(), models::SyncError> {
    use tracing::info;
    use sync_state::SyncStateRepo;

    let existing = SyncStateRepo::find(pool, entity_type, entity_id)
        .await
        .map_err(|e| models::SyncError::FatalError {
            reason: format!("DB query failed: {e}"),
        })?;

    match existing {
        Some(state) if state.h3yun_object_id.is_some() => {
            let object_id = state.h3yun_object_id.as_ref().unwrap();
            client.update(schema_code, biz_json).await?;

            SyncStateRepo::update_synced(pool, state.id, object_id)
                .await
                .map_err(|e| models::SyncError::FatalError {
                    reason: format!("DB update failed: {e}"),
                })?;

            info!(label, entity_id, object_id, "Synced (update)");
        }
        _ => {
            let object_id = client.create(schema_code, biz_json).await?;

            SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
                .await
                .map_err(|e| models::SyncError::FatalError {
                    reason: format!("DB upsert failed: {e}"),
                })?;

            info!(label, entity_id, object_id, "Synced (create)");
        }
    }

    Ok(())
}
