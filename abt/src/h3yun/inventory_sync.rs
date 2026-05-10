//! 库存同步逻辑

use rust_decimal::Decimal;
use sqlx::PgPool;

use super::client::H3YunClient;
use super::models::{schema, EntityType, SyncError};

#[derive(Clone)]
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

pub async fn sync_inventory(
    pool: &PgPool,
    client: &H3YunClient,
    data: &InventorySyncData,
) -> Result<(), SyncError> {
    let payload = serde_json::json!({
        "KW20201118": data.location_code,
        "WH20201118": data.warehouse_name,
        "Pcode20201118": data.product_code,
        "Name": data.product_code,
        "pname": data.product_name,
        "Size": "期初导入",
        "stockqty": data.quantity.to_string(),
        "unit": data.unit,
    });

    let biz_json = serde_json::to_string(&payload).map_err(|e| SyncError::ValidationError {
        record_id: data.product_code.clone(),
        fields: vec![format!("JSON serialize failed: {e}")],
    })?;

    super::sync_entity(pool, client, schema::WAREHOUSE, EntityType::Inventory, data.inventory_id, &biz_json, "inventory").await
}
