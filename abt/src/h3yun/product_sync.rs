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

    super::sync_entity(
        pool,
        client,
        schema::PRODUCT,
        EntityType::Product,
        product.product_id,
        &biz_json,
        "Procode",
        &product.product_code,
        "product",
    )
    .await
}

pub async fn fetch_category_path(
    pool: &PgPool,
    product_id: i64,
) -> Option<(String, String, String)> {
    let rows = sqlx::query_as::<_, (i64,)>(
        r#"
        SELECT t.term_id FROM term_relation tr
        JOIN terms t ON tr.term_id = t.term_id
        WHERE tr.product_id = $1 AND t.taxonomy = 'category'
        LIMIT 1
        "#,
    )
    .bind(product_id)
    .fetch_all(pool)
    .await
    .ok()?;

    let term_id = rows.first()?.0;

    let mut path = Vec::new();
    let mut current_id = term_id;

    for _ in 0..3 {
        let term = sqlx::query_as::<_, (String, i64)>(
            "SELECT term_name, term_parent FROM terms WHERE term_id = $1",
        )
        .bind(current_id)
        .fetch_optional(pool)
        .await
        .ok()??;

        path.push(term.0);
        if term.1 == 0 {
            break;
        }
        current_id = term.1;
    }

    path.reverse();

    Some((
        path.first().cloned().unwrap_or_default(),
        path.get(1).cloned().unwrap_or_default(),
        path.get(2).cloned().unwrap_or_default(),
    ))
}

pub async fn delete_product_sync(pool: &PgPool, client: &H3YunClient, product_id: i64) {
    let existing = match SyncStateRepo::find(pool, EntityType::Product, product_id).await {
        Ok(Some(s)) if s.h3yun_object_id.is_some() => s,
        _ => return,
    };

    let Some(object_id) = existing.h3yun_object_id.as_ref() else {
        return;
    };

    // Only delete local mapping after successful H3Yun delete
    match client.delete(schema::PRODUCT, object_id).await {
        Ok(()) => {
            info!(product_id, object_id, "Product deleted from H3Yun");
            if let Err(e) = SyncStateRepo::delete(pool, EntityType::Product, product_id).await {
                warn!(product_id, error = %e, "Failed to delete sync state mapping after H3Yun delete");
            }
        }
        Err(e) => {
            warn!(product_id, object_id, error = %e, "Failed to delete product from H3Yun, keeping local mapping");
        }
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
        "Fa5124b65846244078bedf7d739842cf4": "系统倒冲"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Product, ProductMeta};

    fn test_product() -> Product {
        Product {
            product_id: 1,
            product_code: "P001".to_string(),
            pdt_name: "Test Product".to_string(),
            unit: "个".to_string(),
            meta: ProductMeta {
                specification: "100x200".to_string(),
                acquire_channel: "采购".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn payload_without_category() {
        let product = test_product();
        let payload = build_product_payload(&product, None);
        assert_eq!(payload["Procode"], "P001");
        assert_eq!(payload["Proname"], "Test Product");
        assert_eq!(payload["Pgroup"], "");
        assert_eq!(payload["PgroupM"], "");
        assert_eq!(payload["PgroupS"], "");
    }

    #[test]
    fn payload_with_category() {
        let product = test_product();
        let cat = ("电子".to_string(), "电阻".to_string(), "贴片电阻".to_string());
        let payload = build_product_payload(&product, Some(&cat));
        assert_eq!(payload["Pgroup"], "电子");
        assert_eq!(payload["PgroupM"], "电阻");
        assert_eq!(payload["PgroupS"], "贴片电阻");
    }
}
