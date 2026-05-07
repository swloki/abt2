# Stock Alert Dedup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `alert_active` / `last_notified_at` fields to `product_watchers` and refactor `StockAlertTask` to use a state-machine dedup strategy instead of checking unread notifications.

**Architecture:** Add two columns to `product_watchers`. The task scan checks `alert_active` to decide whether to notify. Active alerts are only cleared when the product recovers above the safety stock threshold. Unread notification status no longer drives dedup.

**Tech Stack:** Rust, sqlx, PostgreSQL, async-trait

---

### Task 1: Database Migration

**Files:**
- Create: `abt/migrations/041_add_alert_active_to_product_watchers.sql`

- [ ] **Step 1: Create migration file**

```sql
ALTER TABLE product_watchers
  ADD COLUMN IF NOT EXISTS alert_active BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS last_notified_at TIMESTAMPTZ;

COMMENT ON COLUMN product_watchers.alert_active IS '当前是否处于活跃告警状态（库存低于阈值且已发送通知）';
COMMENT ON COLUMN product_watchers.last_notified_at IS '上次发送库存告警通知的时间';
```

- [ ] **Step 2: Verify migration runs**

Run: `cargo clippy 2>&1 | tail -5`
Expected: no errors (migration is SQL-only, but verify project still builds)

- [ ] **Step 3: Commit**

```bash
git add abt/migrations/041_add_alert_active_to_product_watchers.sql
git commit -m "feat(migration): add alert_active and last_notified_at to product_watchers"
```

---

### Task 2: Repo — Batch Alert Status Methods

**Files:**
- Modify: `abt/src/repositories/product_watcher_repo.rs:142-159`

- [ ] **Step 1: Add `batch_get_alert_status` method**

Append to `ProductWatcherRepo` impl block in `abt/src/repositories/product_watcher_repo.rs`, after `find_watchers_by_products`:

```rust
    /// 批量查询关注者的告警状态（Worker 用）
    /// 返回 (user_id, product_id, alert_active) 三元组
    pub async fn batch_get_alert_status(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<(i64, i64, bool)>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT user_id, product_id, alert_active FROM product_watchers WHERE product_id = ANY($1)",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// 批量设置告警状态为活跃（Worker 用）
    pub async fn batch_activate_alerts(
        pool: &PgPool,
        pairs: &[(i64, i64)],
    ) -> Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }
        let user_ids: Vec<i64> = pairs.iter().map(|(uid, _)| *uid).collect();
        let product_ids: Vec<i64> = pairs.iter().map(|(_, pid)| *pid).collect();
        sqlx::query(
            r#"UPDATE product_watchers
            SET alert_active = true, last_notified_at = now(), updated_at = now()
            WHERE (user_id, product_id) IN (
                SELECT * FROM UNNEST($1::bigint[], $2::bigint[])
            )"#,
        )
        .bind(&user_ids)
        .bind(&product_ids)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// 批量重置已回升产品的告警状态（Worker 用）
    /// 重置所有 alert_active=true 但产品不在低库存集合中的 watcher
    pub async fn batch_clear_recovered(
        pool: &PgPool,
        low_stock_product_ids: &[i64],
    ) -> Result<u64> {
        if low_stock_product_ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query(
            r#"UPDATE product_watchers
            SET alert_active = false, updated_at = now()
            WHERE alert_active = true AND product_id != ALL($1)"#,
        )
        .bind(low_stock_product_ids)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo clippy 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add abt/src/repositories/product_watcher_repo.rs
git commit -m "feat(repo): add batch alert status methods to ProductWatcherRepo"
```

---

### Task 3: Refactor StockAlertTask

**Files:**
- Modify: `abt/src/implt/stock_alert_task.rs`

- [ ] **Step 1: Rewrite `run_once` with alert_active state machine**

Replace the entire contents of `abt/src/implt/stock_alert_task.rs`:

```rust
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

        // alert_active 的 (user_id, product_id) 集合
        let active_alerts: HashSet<(i64, i64)> = alert_statuses
            .iter()
            .filter(|(_, _, active)| *active)
            .map(|(uid, pid, _)| (*uid, *pid))
            .collect();

        // 所有关注者映射 product_id → Vec<user_id>
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo clippy 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add abt/src/implt/stock_alert_task.rs
git commit -m "refactor(stock_alert): use alert_active state machine for dedup"
```

---

### Task 4: Cleanup — Remove Unused Unread-Check Code

**Files:**
- Modify: `abt/src/repositories/notification_repo.rs:170-194`

- [ ] **Step 1: Remove `batch_has_unread_alert` and `batch_has_unread_alerts_multi`**

In `abt/src/repositories/notification_repo.rs`, delete the `batch_has_unread_alert` method (lines ~170-193) and `batch_has_unread_alerts_multi` method. Keep the `has_unread_alert` single-check method as it may be useful for other purposes.

- [ ] **Step 2: Remove unused `HashSet` import from stock_alert_task.rs if needed**

Check `stock_alert_task.rs` — `HashSet` is still used for `active_alerts`, so no change needed.

- [ ] **Step 3: Verify compilation**

Run: `cargo clippy 2>&1 | tail -5`
Expected: no errors (confirm no callers of the removed methods exist)

- [ ] **Step 4: Commit**

```bash
git add abt/src/repositories/notification_repo.rs
git commit -m "refactor(repo): remove unused batch_has_unread_alert methods"
```
