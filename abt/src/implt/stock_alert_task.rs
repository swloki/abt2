//! 库存告警定时任务

use std::collections::{HashMap, HashSet};
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

        // 1. 重置已回升产品的告警状态
        let low_stock_ids: Vec<i64> = low_stock_products.iter().map(|p| p.product_id).collect();
        let recovered = ProductWatcherRepo::batch_clear_recovered(&self.pool, &low_stock_ids).await?;

        if scanned == 0 {
            return Ok(TaskRunResult {
                processed: 0,
                succeeded: 0,
                message: if recovered > 0 {
                    format!("无低库存产品，重置 {} 条回升告警", recovered)
                } else {
                    "无低库存产品".to_string()
                },
            });
        }

        // 2. 批量查询关注者的告警状态
        let alert_statuses = ProductWatcherRepo::batch_get_alert_status(&self.pool, &low_stock_ids).await?;

        let active_alerts: HashSet<(i64, i64)> = alert_statuses
            .iter()
            .filter(|(_, _, active)| *active)
            .map(|(uid, pid, _)| (*uid, *pid))
            .collect();

        let mut watchers_by_product: HashMap<i64, Vec<i64>> = HashMap::new();
        for (uid, pid, _) in &alert_statuses {
            watchers_by_product.entry(*pid).or_default().push(*uid);
        }

        // 3. 收集需要发送的通知（alert_active=false 的 watcher）
        let mut notifications = Vec::new();
        let mut to_activate: Vec<(i64, i64)> = Vec::new();

        for product in &low_stock_products {
            let watchers = match watchers_by_product.get(&product.product_id) {
                Some(w) => w,
                None => continue,
            };

            for &user_id in watchers {
                if active_alerts.contains(&(user_id, product.product_id)) {
                    continue;
                }

                let metadata = serde_json::json!({
                    "current_quantity": product.current_quantity.to_string(),
                    "safety_stock": product.effective_safety_stock.to_string(),
                    "product_name": product.product_name,
                });

                notifications.push(CreateNotificationRequest {
                    user_id,
                    notification_type: NOTIFICATION_TYPE_STOCK_ALERT.to_string(),
                    title: format!("库存告警: {} 库存不足", product.product_name),
                    content: Some(format!(
                        "产品「{}」当前库存 {}，低于安全库存 {}",
                        product.product_name, product.current_quantity, product.effective_safety_stock
                    )),
                    related_type: Some(RELATED_TYPE_PRODUCT.to_string()),
                    related_id: Some(product.product_id),
                    metadata: Some(metadata),
                });

                to_activate.push((user_id, product.product_id));
            }
        }

        // 4. 批量插入通知
        let alerts_sent = if notifications.is_empty() {
            0
        } else {
            NotificationRepo::batch_insert(&self.pool, &notifications).await?
        };

        // 5. 批量激活告警状态
        if !to_activate.is_empty() {
            ProductWatcherRepo::batch_activate_alerts(&self.pool, &to_activate).await?;
        }

        Ok(TaskRunResult {
            processed: scanned,
            succeeded: alerts_sent,
            message: format!(
                "扫描 {} 个低库存，发送 {} 条告警，重置 {} 条回升",
                scanned, alerts_sent, recovered
            ),
        })
    }
}
