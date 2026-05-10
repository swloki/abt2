//! Sync Worker — Channel 消费者，逐记录错误隔离

use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::client::H3YunClient;
use super::inventory_sync::{self, InventorySyncData};
use super::models::{EntityType, SyncError, SyncEvent};
use super::product_sync;

/// 同步结果统计
#[derive(Debug, Default)]
pub struct SyncResult {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub messages: Vec<String>,
}

/// 启动 sync channel，返回 sender
///
/// 创建 bounded channel，spawn worker task，返回 sender 给各触发源使用
pub fn start_sync_channel(pool: PgPool, client: H3YunClient) -> mpsc::Sender<SyncEvent> {
    let (tx, rx) = mpsc::channel::<SyncEvent>(1000);
    let worker = SyncWorker {
        receiver: rx,
        pool,
        client,
    };

    tokio::spawn(async move {
        worker.run().await;
    });

    // 注册全局 sender
    super::set_sync_event_sender(tx.clone());

    info!("H3Yun sync worker started");
    tx
}

struct SyncWorker {
    receiver: mpsc::Receiver<SyncEvent>,
    pool: PgPool,
    client: H3YunClient,
}

impl SyncWorker {
    async fn run(mut self) {
        info!("Sync worker running, waiting for events...");

        while let Some(event) = self.receiver.recv().await {
            self.handle_event(event).await;
        }

        info!("Sync worker stopped (channel closed)");
    }

    async fn handle_event(&self, event: SyncEvent) {
        match event.entity_type {
            EntityType::Product => {
                self.sync_product_with_retry(event.entity_id).await;
            }
            EntityType::Inventory => {
                self.sync_inventory_with_retry(event.entity_id).await;
            }
        }
    }

    async fn sync_product_with_retry(&self, product_id: i64) {
        // 查询产品
        let product = match self.fetch_product(product_id).await {
            Some(p) => p,
            None => return,
        };

        // 查询分类路径
        let category_path = self.fetch_category_path(product_id).await;

        let mut attempts = 0u32;
        let max_retries = 3;

        loop {
            match product_sync::sync_product(&self.pool, &self.client, &product, category_path.as_ref())
                .await
            {
                Ok(()) => return,
                Err(SyncError::Transient { backoff_hint }) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        warn!(
                            product_id,
                            attempts,
                            "Product sync failed after max retries"
                        );
                        return;
                    }
                    warn!(
                        product_id,
                        attempt = attempts,
                        "Product sync transient error, retrying..."
                    );
                    tokio::time::sleep(backoff_hint).await;
                }
                Err(SyncError::ValidationError { record_id, fields }) => {
                    warn!(
                        product_id,
                        record_id,
                        fields = fields.join(","),
                        "Product sync validation error, skipping"
                    );
                    return;
                }
                Err(SyncError::FatalError { reason }) => {
                    error!(product_id, reason, "Product sync fatal error");
                    return;
                }
            }
        }
    }

    async fn sync_inventory_with_retry(&self, inventory_id: i64) {
        let data = match self.fetch_inventory_data(inventory_id).await {
            Some(d) => d,
            None => return,
        };

        let mut attempts = 0u32;
        let max_retries = 3;

        loop {
            match inventory_sync::sync_inventory(&self.pool, &self.client, &data).await {
                Ok(()) => return,
                Err(SyncError::Transient { backoff_hint }) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        warn!(
                            inventory_id,
                            attempts,
                            "Inventory sync failed after max retries"
                        );
                        return;
                    }
                    warn!(
                        inventory_id,
                        attempt = attempts,
                        "Inventory sync transient error, retrying..."
                    );
                    tokio::time::sleep(backoff_hint).await;
                }
                Err(SyncError::ValidationError { record_id, fields }) => {
                    warn!(
                        inventory_id,
                        record_id,
                        fields = fields.join(","),
                        "Inventory sync validation error, skipping"
                    );
                    return;
                }
                Err(SyncError::FatalError { reason }) => {
                    error!(inventory_id, reason, "Inventory sync fatal error");
                    return;
                }
            }
        }
    }

    async fn fetch_product(&self, product_id: i64) -> Option<crate::models::Product> {
        use crate::repositories::ProductRepo;
        ProductRepo::find_by_id(&self.pool, product_id)
            .await
            .ok()
            .flatten()
    }

    async fn fetch_category_path(
        &self,
        product_id: i64,
    ) -> Option<(String, String, String)> {
        // 通过 term_relation 查询产品关联的分类，构建三级路径
        let rows = sqlx::query_as::<_, (i64,)>(
            r#"
            SELECT t.term_id FROM term_relation tr
            JOIN terms t ON tr.term_id = t.term_id
            WHERE tr.product_id = $1 AND t.taxonomy = 'category'
            LIMIT 1
            "#,
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await
        .ok()?;

        let term_id = rows.first()?.0;

        // 构建分类路径：从当前 term 向上追溯到根
        let mut path = Vec::new();
        let mut current_id = term_id;

        for _ in 0..3 {
            let term = sqlx::query_as::<_, (String, i64)>(
                "SELECT term_name, term_parent FROM terms WHERE term_id = $1",
            )
            .bind(current_id)
            .fetch_optional(&self.pool)
            .await
            .ok()??;

            path.push(term.0);
            if term.1 == 0 {
                break;
            }
            current_id = term.1;
        }

        // path 是从子到父，需要反转（从大到小：大分类、中分类、小分类）
        path.reverse();

        let large = path.first().cloned().unwrap_or_default();
        let medium = path.get(1).cloned().unwrap_or_default();
        let small = path.get(2).cloned().unwrap_or_default();

        Some((large, medium, small))
    }

    async fn fetch_inventory_data(&self, inventory_id: i64) -> Option<InventorySyncData> {
        let row = sqlx::query_as::<_, (i64, i64, String, String, String, String, rust_decimal::Decimal, String)>(
            r#"
            SELECT
                i.inventory_id,
                i.product_id,
                l.location_code,
                w.warehouse_name,
                p.product_code,
                p.pdt_name,
                i.quantity,
                p.unit
            FROM inventory i
            JOIN locations l ON i.location_id = l.location_id
            JOIN warehouses w ON l.warehouse_id = w.warehouse_id
            JOIN products p ON i.product_id = p.product_id
            WHERE i.inventory_id = $1
            "#,
        )
        .bind(inventory_id)
        .fetch_optional(&self.pool)
        .await
        .ok()??;

        Some(InventorySyncData {
            inventory_id: row.0,
            product_id: row.1,
            location_code: row.2,
            warehouse_name: row.3,
            product_code: row.4,
            product_name: row.5,
            quantity: row.6,
            unit: row.7,
        })
    }
}
