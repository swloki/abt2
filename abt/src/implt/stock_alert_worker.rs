//! 库存告警 Worker
//!
//! 每 5 分钟扫描关注列表，检测低库存产品，通过"回升再跌"策略推送告警通知。

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sqlx::PgPool;

use crate::models::CreateNotificationRequest;
use crate::repositories::{NotificationRepo, ProductWatcherRepo};

const NOTIFICATION_TYPE_STOCK_ALERT: &str = "stock_alert";
const RELATED_TYPE_PRODUCT: &str = "product";

pub struct StockAlertWorker {
    pool: Arc<PgPool>,
    shutdown: Arc<AtomicBool>,
}

impl StockAlertWorker {
    pub fn new(pool: Arc<PgPool>, shutdown: Arc<AtomicBool>) -> Self {
        Self { pool, shutdown }
    }

    pub async fn run(&self) {
        let interval_secs: u64 = std::env::var("STOCK_ALERT_SCAN_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        // 回升标记：product_id → 是否曾回升到安全线以上
        let mut recovered: HashSet<i64> = HashSet::new();

        tracing::info!(
            interval_secs,
            "StockAlertWorker started"
        );

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("StockAlertWorker shutting down");
                return;
            }

            let start = std::time::Instant::now();
            let result = self.scan_once(&mut recovered).await;
            let elapsed = start.elapsed();

            match result {
                Ok((scanned, alerted)) => {
                    tracing::info!(
                        elapsed_ms = elapsed.as_millis() as u64,
                        low_stock_count = scanned,
                        alerts_sent = alerted,
                        "StockAlertWorker scan completed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        elapsed_ms = elapsed.as_millis() as u64,
                        "StockAlertWorker scan failed"
                    );
                }
            }

            // 等待 interval，期间检查 shutdown
            for _ in 0..interval_secs {
                if self.shutdown.load(Ordering::Relaxed) {
                    tracing::info!("StockAlertWorker shutting down during sleep");
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }

    async fn scan_once(&self, recovered: &mut HashSet<i64>) -> anyhow::Result<(usize, usize)> {
        let low_stock_products = ProductWatcherRepo::find_watched_low_stock_products(&self.pool).await?;
        let scanned = low_stock_products.len();
        let mut alerts_sent = 0usize;

        for product in &low_stock_products {
            let current = product.current_quantity;
            let threshold = product.effective_safety_stock;
            let pid = product.product_id;

            if current >= threshold {
                recovered.insert(pid);
                continue;
            }

            let should_alert = recovered.remove(&pid);

            // 查询关注者（一次）
            let watchers = ProductWatcherRepo::find_watchers_by_product(&self.pool, pid).await?;
            if watchers.is_empty() {
                continue;
            }

            if !should_alert {
                let mut found = false;
                for watcher in &watchers {
                    let has_unread = NotificationRepo::has_unread_alert(
                        &self.pool,
                        watcher.user_id,
                        NOTIFICATION_TYPE_STOCK_ALERT,
                        RELATED_TYPE_PRODUCT,
                        pid,
                    )
                    .await?;
                    if !has_unread {
                        found = true;
                        break;
                    }
                }
                if !found {
                    continue;
                }
            }

            // 发送告警
            for watcher in &watchers {
                let has_unread = NotificationRepo::has_unread_alert(
                    &self.pool,
                    watcher.user_id,
                    NOTIFICATION_TYPE_STOCK_ALERT,
                    RELATED_TYPE_PRODUCT,
                    pid,
                )
                .await?;

                if has_unread {
                    continue;
                }

                let metadata = serde_json::json!({
                    "current_quantity": current.to_string(),
                    "safety_stock": threshold.to_string(),
                    "product_name": product.product_name,
                });

                let req = CreateNotificationRequest {
                    user_id: watcher.user_id,
                    notification_type: NOTIFICATION_TYPE_STOCK_ALERT.to_string(),
                    title: format!("库存告警: {} 库存不足", product.product_name),
                    content: Some(format!(
                        "产品「{}」当前库存 {}，低于安全库存 {}",
                        product.product_name, current, threshold
                    )),
                    related_type: Some(RELATED_TYPE_PRODUCT.to_string()),
                    related_id: Some(pid),
                    metadata: Some(metadata),
                };

                match NotificationRepo::insert(&self.pool, &req).await {
                    Ok(_) => alerts_sent += 1,
                    Err(e) => {
                        tracing::error!(
                            product_id = pid,
                            user_id = watcher.user_id,
                            error = %e,
                            "Failed to create stock alert notification"
                        );
                    }
                }
            }
        }

        // 清理不再低库存的产品的回升标记
        if !recovered.is_empty() {
            let low_stock_ids: HashSet<i64> = low_stock_products.iter().map(|p| p.product_id).collect();
            recovered.retain(|pid| low_stock_ids.contains(pid));
        }

        Ok((scanned, alerts_sent))
    }
}
