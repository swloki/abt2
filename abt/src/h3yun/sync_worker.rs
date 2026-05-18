//! Sync Worker — Channel 消费者，逐记录错误隔离

use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::client::H3YunClient;
use super::inventory_sync::{self, InventorySyncData};
use super::models::{EntityType, SyncError, SyncEvent};
use super::product_sync;

pub fn start_sync_channel(pool: PgPool, client: H3YunClient, shutdown: Arc<AtomicBool>) -> mpsc::Sender<SyncEvent> {
    let (tx, rx) = mpsc::channel::<SyncEvent>(1000);
    let worker = SyncWorker {
        receiver: rx,
        pool,
        client,
        shutdown,
    };

    tokio::spawn(async move {
        worker.run().await;
    });

    super::set_sync_event_sender(tx.clone());
    info!("H3Yun sync worker started");
    tx
}

struct SyncWorker {
    receiver: mpsc::Receiver<SyncEvent>,
    pool: PgPool,
    client: H3YunClient,
    shutdown: Arc<AtomicBool>,
}

impl SyncWorker {
    async fn run(mut self) {
        info!("Sync worker running, waiting for events...");

        loop {
            if self.shutdown.load(Ordering::Acquire) {
                info!("Sync worker shutting down...");
                break;
            }

            tokio::select! {
                event = self.receiver.recv() => {
                    match event {
                        Some(event) => self.handle_event(event).await,
                        None => break, // Channel closed
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    continue; // Periodic shutdown check
                }
            }
        }

        // Drain remaining events before exit
        while let Ok(event) = self.receiver.try_recv() {
            if self.shutdown.load(Ordering::Acquire) {
                break;
            }
            self.handle_event(event).await;
        }

        info!("Sync worker stopped");
    }

    async fn handle_event(&self, event: SyncEvent) {
        let ok = match event.entity_type {
            EntityType::Product => {
                let product = match self.fetch_product(event.entity_id).await {
                    Some(p) => p,
                    None => {
                        self.update_batch(&event, false);
                        return;
                    }
                };
                let category_path = product_sync::fetch_category_path(&self.pool, event.entity_id).await;
                with_retry("product", product.product_id, || {
                    let pool = self.pool.clone();
                    let client = self.client.clone();
                    let product = product.clone();
                    let cat = category_path.clone();
                    Box::pin(async move {
                        product_sync::sync_product(&pool, &client, &product, cat.as_ref()).await
                    })
                })
                .await
            }
            EntityType::Inventory => {
                let data = match self.fetch_inventory_data(event.entity_id).await {
                    Some(d) => d,
                    None => {
                        self.update_batch(&event, false);
                        return;
                    }
                };
                with_retry("inventory", data.inventory_id, || {
                    let pool = self.pool.clone();
                    let client = self.client.clone();
                    let data = data.clone();
                    Box::pin(async move { inventory_sync::sync_inventory(&pool, &client, &data).await })
                })
                .await
            }
        };

        self.update_batch(&event, ok);
    }

    fn update_batch(&self, event: &SyncEvent, succeeded: bool) {
        if event.is_batch {
            super::update_batch_progress(event.entity_type.as_str(), succeeded);
        }
    }

    async fn fetch_product(&self, product_id: i64) -> Option<crate::models::Product> {
        use crate::repositories::ProductRepo;
        match ProductRepo::find_by_id(&self.pool, product_id).await {
            Ok(product) => product,
            Err(e) => {
                warn!(product_id, error = %e, "Failed to fetch product from DB");
                None
            }
        }
    }

    async fn fetch_inventory_data(&self, inventory_id: i64) -> Option<InventorySyncData> {
        let result = sqlx::query_as::<_, (i64, i64, String, String, String, String, rust_decimal::Decimal, String)>(
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
            JOIN location l ON i.location_id = l.location_id
            JOIN warehouse w ON l.warehouse_id = w.warehouse_id
            JOIN products p ON i.product_id = p.product_id
            WHERE i.inventory_id = $1
            "#,
        )
        .bind(inventory_id)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => Some(InventorySyncData {
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
                warn!(inventory_id, "Inventory not found in DB");
                None
            }
            Err(e) => {
                warn!(inventory_id, error = %e, "Failed to fetch inventory data");
                None
            }
        }
    }
}

async fn with_retry<F, Fut>(label: &str, id: i64, f: F) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<(), SyncError>>,
{
    let mut attempts = 0u32;
    loop {
        match f().await {
            Ok(()) => return true,
            Err(SyncError::Transient { backoff_hint }) => {
                attempts += 1;
                if attempts >= 3 {
                    warn!(label, id, attempts, "Sync failed after max retries");
                    return false;
                }
                warn!(label, id, attempt = attempts, "Sync transient error, retrying...");
                tokio::time::sleep(backoff_hint).await;
            }
            Err(SyncError::ValidationError { record_id, fields }) => {
                warn!(label, id, record_id, fields = fields.join(","), "Sync validation error, skipping");
                return false;
            }
            Err(SyncError::FatalError { reason }) => {
                error!(label, id, reason, "Sync fatal error");
                return false;
            }
        }
    }
}
