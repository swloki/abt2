//! 产品同步逻辑 — 字段映射 + create/update/delete

use sqlx::PgPool;
use tracing::{info, warn};

use super::client::H3YunClient;
use super::models::{schema, EntityType, SyncError};
use super::sync_state::SyncStateRepo;
use crate::models::Product;

/// 同步单个产品到 H3Yun
pub async fn sync_product(
    pool: &PgPool,
    client: &H3YunClient,
    product: &Product,
    category_path: Option<&(String, String, String)>,
) -> Result<(), SyncError> {
    let biz_object = build_product_payload(product, category_path);
    let biz_json = serde_json::to_string(&biz_object).map_err(|e| SyncError::ValidationError {
        record_id: product.product_code.clone(),
        fields: vec![format!("JSON serialize failed: {e}")],
    })?;

    // 查映射表
    let existing = SyncStateRepo::find(pool, EntityType::Product, product.product_id)
        .await
        .map_err(|e| SyncError::FatalError {
            reason: format!("DB query failed: {e}"),
        })?;

    match existing {
        Some(state) if state.h3yun_object_id.is_some() => {
            // 已有 ObjectId → UpdateBizObject
            let object_id = state.h3yun_object_id.as_ref().unwrap();
            client
                .update(schema::PRODUCT, object_id, &biz_json)
                .await?;

            SyncStateRepo::update_synced(pool, state.id, object_id, None)
                .await
                .map_err(|e| SyncError::FatalError {
                    reason: format!("DB update failed: {e}"),
                })?;

            info!(
                product_id = product.product_id,
                object_id,
                "Product synced (update)"
            );
        }
        _ => {
            // 无映射 → CreateBizObject
            let object_id = client.create(schema::PRODUCT, &biz_json).await?;

            SyncStateRepo::upsert(
                pool,
                EntityType::Product,
                product.product_id,
                &object_id,
                None,
            )
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;

            info!(
                product_id = product.product_id,
                object_id,
                "Product synced (create)"
            );
        }
    }

    Ok(())
}

/// 删除产品同步 — 产品在 ABT 被删除时调用
pub async fn delete_product_sync(pool: &PgPool, client: &H3YunClient, product_id: i64) {
    let existing = match SyncStateRepo::find(pool, EntityType::Product, product_id).await {
        Ok(Some(s)) if s.h3yun_object_id.is_some() => s,
        _ => return, // 未同步过，跳过
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

    // 无论 H3Yun 删除是否成功，都清理映射行
    if let Err(e) = SyncStateRepo::delete(pool, EntityType::Product, product_id).await {
        warn!(product_id, error = %e, "Failed to delete sync state mapping");
    }
}

/// 构造产品 H3Yun payload
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
