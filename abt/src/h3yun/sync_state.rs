//! H3Yun 同步映射表数据访问

use anyhow::Result;
use sqlx::PgPool;

use super::models::{EntityType, SyncState};

/// 同步映射表 CRUD
pub struct SyncStateRepo;

impl SyncStateRepo {
    /// 查询单条映射
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

    /// 查询未同步实体（last_synced_at IS NULL）
    pub async fn find_unsynced(
        pool: &PgPool,
        entity_type: EntityType,
        limit: i64,
    ) -> Result<Vec<SyncState>> {
        let rows = sqlx::query_as::<_, SyncState>(
            r#"
            SELECT id, entity_type, entity_id, h3yun_object_id,
                   last_synced_at, content_hash, created_at
            FROM h3yun_sync_state
            WHERE entity_type = $1 AND last_synced_at IS NULL
            LIMIT $2
            "#,
        )
        .bind(entity_type.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 插入或更新映射（首次同步写入 ObjectId，后续更新）
    pub async fn upsert(
        pool: &PgPool,
        entity_type: EntityType,
        entity_id: i64,
        h3yun_object_id: &str,
        content_hash: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO h3yun_sync_state (entity_type, entity_id, h3yun_object_id, last_synced_at, content_hash)
            VALUES ($1, $2, $3, NOW(), $4)
            ON CONFLICT (entity_type, entity_id)
            DO UPDATE SET h3yun_object_id = $3, last_synced_at = NOW(), content_hash = $4
            "#,
        )
        .bind(entity_type.as_str())
        .bind(entity_id)
        .bind(h3yun_object_id)
        .bind(content_hash)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 更新同步时间（ObjectId 已存在时）
    pub async fn update_synced(
        pool: &PgPool,
        id: i32,
        h3yun_object_id: &str,
        content_hash: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE h3yun_sync_state
            SET h3yun_object_id = $2, last_synced_at = NOW(), content_hash = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(h3yun_object_id)
        .bind(content_hash)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 删除映射行（删除同步用）
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

    /// 查询所有映射（对账用）
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

    /// 查询需要同步的实体 ID 列表（用于定时任务扫描未同步的产品）
    pub async fn find_entity_ids_never_synced(
        pool: &PgPool,
        entity_type: EntityType,
        limit: i64,
    ) -> Result<Vec<i64>> {
        let rows = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT s.entity_id
            FROM h3yun_sync_state s
            WHERE s.entity_type = $1 AND s.last_synced_at IS NULL
            LIMIT $2
            "#,
        )
        .bind(entity_type.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
