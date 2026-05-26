//! 产品同步逻辑

use sqlx::PgPool;
use tracing::{info, warn};

use crate::h3yun::client::H3YunClient;
use crate::h3yun::models::{schema, EntityType, SyncError};
use crate::h3yun::sync_state::SyncStateRepo;
use crate::master_data::product::model::Product;

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

    sync_entity(
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

/// 基于分类路径（categories.materialized_path）查询产品的三级分类名称
///
/// abt_v2 使用 `categories.path`（物化路径，如 "/1/5/12/"）替代旧版的
/// `terms` + `term_relation` 递归查询。先通过 `product_categories` 找到产品所属分类，
/// 再解析物化路径获取所有祖先 ID，最后批量查询名称构造三级元组。
pub async fn fetch_category_path(
    pool: &PgPool,
    product_id: i64,
) -> Option<(String, String, String)> {
    // 1. 查询产品所属分类（取第一个）
    let row = sqlx::query_as::<_, (i64, String, String)>(
        r#"
        SELECT c.category_id, c.category_name, c.path
        FROM product_categories pc
        JOIN categories c ON pc.category_id = c.category_id
        WHERE pc.product_id = $1
        LIMIT 1
        "#,
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .ok()??;

    let (_category_id, _category_name, path) = row;

    // 2. 解析物化路径，提取所有祖先 ID
    let ancestor_ids: Vec<i64> = path
        .split('/')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();

    if ancestor_ids.is_empty() {
        return None;
    }

    // 3. 批量查询所有祖先分类名称，按 path 排序保证从根到叶的顺序
    let rows = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT category_id, category_name
        FROM categories
        WHERE category_id = ANY($1)
        ORDER BY path
        "#,
    )
    .bind(&ancestor_ids)
    .fetch_all(pool)
    .await
    .ok()?;

    if rows.is_empty() {
        return None;
    }

    // 4. 构造三级分类：取最深的三层（根/中/叶）
    let names: Vec<String> = rows.into_iter().map(|(_, name)| name).collect();
    let depth = names.len();

    // 取最后三层（最深的三级），如果不足三层则留空
    let pgroup = if depth >= 3 {
        names[depth - 3].clone()
    } else if depth >= 1 {
        names[0].clone()
    } else {
        String::new()
    };
    let pgroup_m = if depth >= 3 {
        names[depth - 2].clone()
    } else if depth >= 2 {
        names[1].clone()
    } else {
        String::new()
    };
    let pgroup_s = if depth >= 3 {
        names[depth - 1].clone()
    } else if depth >= 2 {
        // depth == 2: names[1] already used for pgroup_m, leaf is empty
        String::new()
    } else {
        String::new()
    };

    Some((pgroup, pgroup_m, pgroup_s))
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

/// Shared create-or-update logic for syncing an entity to H3Yun.
/// 每次都先查 H3Yun，存在就 update，不存在就 create。
pub(crate) async fn sync_entity(
    pool: &sqlx::PgPool,
    client: &H3YunClient,
    schema_code: &str,
    entity_type: EntityType,
    entity_id: i64,
    biz_json: &str,
    search_field: &str,
    search_value: &str,
    label: &str,
) -> Result<(), SyncError> {
    // 每次都先查 H3Yun（不依赖本地 mapping 判断存在性）
    let remote_id = client
        .find_by_field(schema_code, search_field, search_value)
        .await
        .ok()
        .flatten();

    if let Some(object_id) = remote_id {
        // H3Yun 已存在 -> update
        client.update(schema_code, &object_id, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (update)");
    } else {
        // H3Yun 不存在 -> create
        let object_id = client.create(schema_code, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (create)");
    }

    Ok(())
}

/// 多字段 AND 匹配版本的 sync_entity，用于库存同步（product_code + location_code + warehouse_name）
pub(crate) async fn sync_entity_by_fields(
    pool: &sqlx::PgPool,
    client: &H3YunClient,
    schema_code: &str,
    entity_type: EntityType,
    entity_id: i64,
    biz_json: &str,
    fields: &[(&str, &str)],
    label: &str,
) -> Result<(), SyncError> {
    let remote_id = client
        .find_by_fields(schema_code, fields)
        .await
        .ok()
        .flatten();

    if let Some(object_id) = remote_id {
        client.update(schema_code, &object_id, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (update)");
    } else {
        let object_id = client.create(schema_code, biz_json).await?;
        SyncStateRepo::upsert(pool, entity_type, entity_id, &object_id)
            .await
            .map_err(|e| SyncError::FatalError {
                reason: format!("DB upsert failed: {e}"),
            })?;
        info!(label, entity_id, object_id, "Synced (create)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_data::product::model::{Product, ProductMeta, ProductStatus};

    fn test_product() -> Product {
        Product {
            product_id: 1,
            product_code: "P001".to_string(),
            pdt_name: "Test Product".to_string(),
            unit: "个".to_string(),
            status: ProductStatus::Active,
            external_code: None,
            owner_department_id: None,
            meta: ProductMeta {
                specification: "100x200".to_string(),
                acquire_channel: "采购".to_string(),
                old_code: None,
            },
            created_at: None,
            updated_at: None,
            deleted_at: None,
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
