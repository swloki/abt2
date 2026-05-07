//! 库存告警定时任务

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::CreateNotificationRequest;
use crate::repositories::{NotificationRepo, ProductWatcherRepo};
use crate::service::{ScheduledTask, TaskRunResult};

const NOTIFICATION_TYPE_STOCK_ALERT: &str = "stock_alert";
const RELATED_TYPE_PRODUCT: &str = "product";

pub struct StockAlertTask {
    pool: Arc<PgPool>,
    interval_secs: u64,
}

impl StockAlertTask {
    pub fn new(pool: Arc<PgPool>) -> Self {
        let interval_secs = std::env::var("STOCK_ALERT_SCAN_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        Self { pool, interval_secs }
    }
}

#[async_trait]
impl ScheduledTask for StockAlertTask {
    fn name(&self) -> &str {
        "stock_alert"
    }

    fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    async fn run_once(&self) -> anyhow::Result<TaskRunResult> {
        let low_stock_products =
            ProductWatcherRepo::find_watched_low_stock_products(&self.pool).await?;
        let scanned = low_stock_products.len();
        let mut alerts_sent = 0usize;

        for product in &low_stock_products {
            let current = product.current_quantity;
            let threshold = product.effective_safety_stock;
            let pid = product.product_id;

            let watchers = ProductWatcherRepo::find_watchers_by_product(&self.pool, pid).await?;
            if watchers.is_empty() {
                continue;
            }

            let watcher_ids: Vec<i64> = watchers.iter().map(|w| w.user_id).collect();
            let users_with_unread: HashSet<i64> = NotificationRepo::batch_has_unread_alert(
                &self.pool,
                &watcher_ids,
                NOTIFICATION_TYPE_STOCK_ALERT,
                RELATED_TYPE_PRODUCT,
                pid,
            )
            .await?
            .into_iter()
            .collect();

            for watcher in &watchers {
                if users_with_unread.contains(&watcher.user_id) {
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

        Ok(TaskRunResult {
            processed: scanned,
            succeeded: alerts_sent,
            message: format!("扫描 {} 个低库存产品，发送 {} 条告警", scanned, alerts_sent),
        })
    }
}
