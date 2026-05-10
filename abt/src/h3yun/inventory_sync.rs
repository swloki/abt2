//! 库存同步逻辑 — 字段映射 + create/update

use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::info;

use super::client::H3YunClient;
use super::models::{schema, EntityType, SyncError};
use super::sync_state::SyncStateRepo;

/// 库存同步所需的关联数据
pub struct InventorySyncData {
    pub inventory_id: i64,
    pub product_id: i64,
    pub location_code: String,
    pub warehouse_name: String,
    pub product_code: String,
    pub product_name: String,
    pub quantity: Decimal,
    pub unit: String,
}

/// 同步单条库存到 H3Yun
pub async fn sync_inventory(
    pool: &PgPool,
    client: &H3YunClient,
    data: &InventorySyncData,
) -> Result<(), SyncError> {
    let biz_object = build_inventory_payload(data);
    let biz_json = serde_json::to_string(&biz_object).map_err(|e| SyncError::ValidationError {
        record_id: data.product_code.clone(),
        fields: vec![format!("JSON serialize failed: {e}")],
    })?;

    let existing = SyncStateRepo::find(pool, EntityType::Inventory, data.inventory_id)
        .await
        .map_err(|e| SyncError::FatalError {
            reason: format!("DB query failed: {e}"),
        })?;

    match existing {
        Some(state) if state.h3yun_object_id.is_some() => {
            let object_id = state.h3yun_object_id.as_ref().unwrap();
            client
                .update(schema::WAREHOUSE, object_id, &biz_json)
                .await?;

            SyncStateRepo::update_synced(pool, state.id, object_id, None)
                .await
                .map_err(|e| SyncError::FatalError {
                    reason: format!("DB update failed: {e}"),
                })?;

            info!(
                inventory_id = data.inventory_id,
                object_id,
                "Inventory synced (update)"
            );
        }
        _ => {
            let object_id = client.create(schema::WAREHOUSE, &biz_json).await?;

            SyncStateRepo::upsert(
                pool,
                EntityType::Inventory,
                data.inventory_id,
                &object_id,
                None,
            )
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;

            info!(
                inventory_id = data.inventory_id,
                object_id,
                "Inventory synced (create)"
            );
        }
    }

    Ok(())
}

/// 构造库存 H3Yun payload
fn build_inventory_payload(data: &InventorySyncData) -> serde_json::Value {
    serde_json::json!({
        "KW20201118": data.location_code,
        "WH20201118": data.warehouse_name,
        "Pcode20201118": data.product_code,
        "Name": data.product_code,
        "pname": data.product_name,
        "Size": "期初导入",
        "stockqty": data.quantity.to_string(),
        "unit": data.unit,
    })
}
