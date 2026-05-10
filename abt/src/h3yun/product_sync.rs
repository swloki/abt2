//! 产品同步逻辑

use sqlx::PgPool;
use tracing::{info, warn};

use super::client::H3YunClient;
use super::models::{schema, EntityType, SyncError};
use super::sync_state::SyncStateRepo;
use crate::models::Product;

pub async fn sync_product(
    pool: &PgPool,
    client: &H3YunClient,
    product: &Product,
    category_path: Option<&(String, String, String)>,
) -> Result<(), SyncError> {
    let payload = build_product_payload(product, category_path);
    let biz_json = serde_json::to_string(&payload).map_err(|e| SyncError::ValidationError {
        record_id: product.product_code.clone(),
        fields: vec![format!("JSON serialize failed: {e}")],
    })?;

    super::sync_entity(pool, client, schema::PRODUCT, EntityType::Product, product.product_id, &biz_json, "product").await
}

pub async fn delete_product_sync(pool: &PgPool, client: &H3YunClient, product_id: i64) {
    let existing = match SyncStateRepo::find(pool, EntityType::Product, product_id).await {
        Ok(Some(s)) if s.h3yun_object_id.is_some() => s,
        _ => return,
    };

    let object_id = existing.h3yun_object_id.as_ref().unwrap();

    match client.delete(schema::PRODUCT, object_id).await {
        Ok(()) => {
            info!(product_id, object_id, "Product deleted from H3Yun");
        }
        Err(e) => {
            warn!(product_id, object_id, error = %e, "Failed to delete product from H3Yun");
        }
    }

    if let Err(e) = SyncStateRepo::delete(pool, EntityType::Product, product_id).await {
        warn!(product_id, error = %e, "Failed to delete sync state mapping");
    }
}

fn build_product_payload(
    product: &Product,
    category_path: Option<&(String, String, String)>,
) -> serde_json::Value {
    let (pgroup, pgroup_m, pgroup_s) = category_path
        .map(|(l, m, s)| (l.as_str(), m.as_str(), s.as_str()))
        .unwrap_or_default();

    serde_json::json!({
        "Procode": product.product_code,
        "Proname": product.pdt_name,
        "Prospec": product.meta.specification,
        "Unit": product.unit,
        "huoqu": product.meta.acquire_channel,
        "Pgroup": pgroup,
        "PgroupM": pgroup_m,
        "PgroupS": pgroup_s,
        "Fa5124b81d6f8b7c245d4bf99b59a04b62a2e519": "系统倒冲"
    })
}
