//! H3Yun 事件处理器 — 监听领域事件触发 H3Yun 同步

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::h3yun::client::H3YunClient;
use crate::h3yun::inventory_sync;
use crate::h3yun::models::SyncError;
use crate::h3yun::product_sync;
use crate::master_data::product::repo::ProductRepo;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

/// Product create/update → sync to H3Yun
pub struct ProductSyncHandler {
    pool: Arc<PgPool>,
    client: H3YunClient,
}

impl ProductSyncHandler {
    pub fn new(pool: Arc<PgPool>, client: H3YunClient) -> Self {
        Self { pool, client }
    }
}

#[async_trait]
impl EventHandler for ProductSyncHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let product_id = event.aggregate_id;

        // 1. Fetch product from abt_v2
        let mut conn = self.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
        let product = ProductRepo
            .find_by_id(&mut *conn, product_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| {
                warn!(product_id, "Product not found for H3Yun sync, skipping");
                DomainError::not_found("Product")
            })?;
        drop(conn);

        // 2. Fetch category path
        let category_path = product_sync::fetch_category_path(&self.pool, product_id).await;

        // 3. Sync
        match product_sync::sync_product(&self.pool, &self.client, &product, category_path.as_ref()).await {
            Ok(()) => {
                info!(product_id, "H3Yun product sync succeeded");
                Ok(())
            }
            Err(SyncError::Transient { backoff_hint }) => {
                warn!(product_id, ?backoff_hint, "H3Yun sync transient error, will retry");
                Err(DomainError::Internal(anyhow::anyhow!("H3Yun sync transient error")))
            }
            Err(e) => {
                warn!(product_id, error = %e, "H3Yun sync non-retryable error, skipping");
                Ok(())
            }
        }
    }

    fn name(&self) -> &str {
        "h3yun_product_sync"
    }
}

/// Product delete → delete from H3Yun
pub struct ProductDeleteHandler {
    pool: Arc<PgPool>,
    client: H3YunClient,
}

impl ProductDeleteHandler {
    pub fn new(pool: Arc<PgPool>, client: H3YunClient) -> Self {
        Self { pool, client }
    }
}

#[async_trait]
impl EventHandler for ProductDeleteHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let product_id = event.aggregate_id;

        product_sync::delete_product_sync(&self.pool, &self.client, product_id).await;
        info!(product_id, "H3Yun product delete sync completed");
        Ok(())
    }

    fn name(&self) -> &str {
        "h3yun_product_delete"
    }
}

/// Inventory sync → sync to H3Yun
pub struct InventorySyncHandler {
    pool: Arc<PgPool>,
    client: H3YunClient,
}

impl InventorySyncHandler {
    pub fn new(pool: Arc<PgPool>, client: H3YunClient) -> Self {
        Self { pool, client }
    }
}

#[async_trait]
impl EventHandler for InventorySyncHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let stock_ledger_id = event.aggregate_id;

        let data = match fetch_inventory_data(&self.pool, stock_ledger_id).await {
            Some(d) => d,
            None => {
                warn!(stock_ledger_id, "Stock ledger not found, skipping sync");
                return Ok(());
            }
        };

        match inventory_sync::sync_inventory(&self.pool, &self.client, &data).await {
            Ok(()) => {
                info!(stock_ledger_id, "H3Yun inventory sync succeeded");
                Ok(())
            }
            Err(SyncError::Transient { backoff_hint }) => {
                warn!(stock_ledger_id, ?backoff_hint, "H3Yun sync transient error, will retry");
                Err(DomainError::Internal(anyhow::anyhow!("H3Yun sync transient error")))
            }
            Err(e) => {
                warn!(stock_ledger_id, error = %e, "H3Yun sync non-retryable error, skipping");
                Ok(())
            }
        }
    }

    fn name(&self) -> &str {
        "h3yun_inventory_sync"
    }
}

async fn fetch_inventory_data(
    pool: &PgPool,
    stock_ledger_id: i64,
) -> Option<inventory_sync::InventorySyncData> {
    use rust_decimal::Decimal;

    let result = sqlx::query_as::<_, (i64, i64, String, String, String, String, Decimal, String)>(
        r#"
        SELECT
            sl.id,
            sl.product_id,
            b.code AS bin_code,
            w.name AS warehouse_name,
            p.product_code,
            p.pdt_name,
            sl.quantity,
            ''
        FROM stock_ledger sl
        JOIN bins b ON sl.bin_id = b.id
        JOIN zones z ON sl.zone_id = z.id
        JOIN warehouses w ON sl.warehouse_id = w.id
        JOIN products p ON sl.product_id = p.product_id
        WHERE sl.id = $1
        "#,
    )
    .bind(stock_ledger_id)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => Some(inventory_sync::InventorySyncData {
            inventory_id: row.0,
            product_id: row.1,
            location_code: row.2,
            warehouse_name: row.3,
            product_code: row.4,
            product_name: row.5,
            quantity: row.6,
            unit: row.7,
        }),
        Ok(None) => {
            warn!(stock_ledger_id, "Stock ledger not found in DB");
            None
        }
        Err(e) => {
            warn!(stock_ledger_id, error = %e, "Failed to fetch inventory data");
            None
        }
    }
}
