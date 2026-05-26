//! 库存同步逻辑

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::h3yun::client::H3YunClient;
use crate::h3yun::models::{schema, EntityType, SyncError};
use crate::h3yun::product_sync::sync_entity_by_fields;

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
        "LotsCost20201118": 0,
    });

    let biz_json = serde_json::to_string(&payload).map_err(|e| SyncError::ValidationError {
        record_id: data.product_code.clone(),
        fields: vec![format!("JSON serialize failed: {e}")],
    })?;

    // 和旧代码一致：三字段联合匹配（product_code + location_code + warehouse_name）
    let fields: &[(&str, &str)] = &[
        ("Pcode20201118", &data.product_code),
        ("KW20201118", &data.location_code),
        ("WH20201118", &data.warehouse_name),
    ];

    sync_entity_by_fields(
        pool,
        client,
        schema::WAREHOUSE,
        EntityType::Inventory,
        data.inventory_id,
        &biz_json,
        fields,
        "inventory",
    )
    .await
}
