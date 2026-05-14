//! H3Yun 同步映射表数据访问

use anyhow::Result;
use sqlx::PgPool;

use super::models::{EntityType, SyncState};

pub struct SyncStateRepo;

impl SyncStateRepo {
    pub async fn find(
        pool: &PgPool,
        entity_type: EntityType,
        entity_id: i64,
    ) -> Result<Option<SyncState>> {
        let row = sqlx::query_as::<_, SyncState>(
            r#"
            SELECT id, entity_type, entity_id, h3yun_object_id,
                   last_synced_at, content_hash, created_at
            FROM h3yun_sync_state
            WHERE entity_type = $1 AND entity_id = $2
            "#,
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    pub async fn upsert(
        pool: &PgPool,
        entity_type: EntityType,
        entity_id: i64,
        h3yun_object_id: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO h3yun_sync_state (entity_type, entity_id, h3yun_object_id, last_synced_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (entity_type, entity_id)
            DO UPDATE SET h3yun_object_id = $3, last_synced_at = NOW()
            "#,
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .bind(h3yun_object_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_synced(
        pool: &PgPool,
        id: i32,
        h3yun_object_id: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE h3yun_sync_state
            SET h3yun_object_id = $2, last_synced_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(h3yun_object_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete(
        pool: &PgPool,
        entity_type: EntityType,
        entity_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM h3yun_sync_state
            WHERE entity_type = $1 AND entity_id = $2
            "#,
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_all_by_type(
        pool: &PgPool,
        entity_type: EntityType,
    ) -> Result<Vec<SyncState>> {
        let rows = sqlx::query_as::<_, SyncState>(
            r#"
            SELECT id, entity_type, entity_id, h3yun_object_id,
                   last_synced_at, content_hash, created_at
            FROM h3yun_sync_state
            WHERE entity_type = $1
            "#,
        )
        .bind(entity_type.as_str())
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn find_entity_ids_never_synced(
        pool: &PgPool,
        entity_type: EntityType,
        limit: i64,
    ) -> Result<Vec<i64>> {
        // Find products that either have no sync_state row at all, or have one but never completed
        let rows = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT p.product_id
            FROM products p
            LEFT JOIN h3yun_sync_state s
                ON s.entity_id = p.product_id AND s.entity_type = $1
            WHERE (s.id IS NULL OR s.last_synced_at IS NULL)
            LIMIT $2
            "#,
        )
        .bind(entity_type.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 查询未同步的库存记录 (inventory_id 列表)
    pub async fn find_unsynced_inventories(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<i64>> {
        let rows = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT i.inventory_id
            FROM inventory i
            LEFT JOIN h3yun_sync_state s
                ON s.entity_id = i.inventory_id AND s.entity_type = 'inventory'
            WHERE (s.id IS NULL OR s.last_synced_at IS NULL)
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
