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
/// 和旧代码一致：每次都先查 H3Yun，存在就 update，不存在就 create
#[allow(clippy::too_many_arguments)]
pub(crate) async fn sync_entity(
    pool: &sqlx::PgPool,
    client: &H3YunClient,
    schema_code: &str,
    entity_type: models::EntityType,
    entity_id: i64,
    biz_json: &str,
    search_field: &str,
    search_value: &str,
    label: &str,
) -> Result<(), models::SyncError> {
    use tracing::info;
    use sync_state::SyncStateRepo;

    // 每次都先查 H3Yun（和旧代码一致，不依赖本地 mapping 判断存在性）
    let remote_id = client
        .find_by_field(schema_code, search_field, search_value)
        .await
        .ok()
        .flatten();

    if let Some(object_id) = remote_id {
        // H3Yun 已存在 → update
        client.update(schema_code, &object_id, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| models::SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (update)");
    } else {
        // H3Yun 不存在 → create
        let object_id = client.create(schema_code, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| models::SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (create)");
    }

    Ok(())
}

/// 多字段 AND 匹配版本的 sync_entity，用于库存同步（product_code + location_code + warehouse_name）
#[allow(clippy::too_many_arguments)]
pub(crate) async fn sync_entity_by_fields(
    pool: &sqlx::PgPool,
    client: &H3YunClient,
    schema_code: &str,
    entity_type: models::EntityType,
    entity_id: i64,
    biz_json: &str,
    fields: &[(&str, &str)],
    label: &str,
) -> Result<(), models::SyncError> {
    use tracing::info;
    use sync_state::SyncStateRepo;

    let remote_id = client
        .find_by_fields(schema_code, fields)
        .await
        .ok()
        .flatten();

    if let Some(object_id) = remote_id {
        client.update(schema_code, &object_id, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| models::SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (update)");
    } else {
        let object_id = client.create(schema_code, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| models::SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (create)");
    }

    Ok(())
}
