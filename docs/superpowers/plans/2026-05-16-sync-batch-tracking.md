# H3Yun 同步批次追踪 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为全量同步（SyncAllProducts / SyncAllInventory）添加批次进度追踪，前端可轮询查询同步状态。

**Architecture:** 新增 `h3yun_sync_batch` 表记录每次全量同步的批次状态。SyncAllProducts/SyncAllInventory 入队时创建批次记录并返回 batch_id。SyncWorker 处理每个事件时更新批次进度。新增 gRPC 方法 GetLatestSyncStatus 供前端轮询。

**Tech Stack:** Rust, sqlx, tonic/prost, PostgreSQL, tokio::mpsc

---

### Task 1: 数据库迁移 — 创建 h3yun_sync_batch 表

**Files:**
- Create: `abt/migrations/043_create_h3yun_sync_batch.sql`

- [ ] **Step 1: 创建迁移文件**

```sql
CREATE TABLE h3yun_sync_batch (
    id SERIAL PRIMARY KEY,
    batch_type VARCHAR(16) NOT NULL,        -- 'product' | 'inventory'
    status VARCHAR(16) NOT NULL DEFAULT 'pending',  -- 'pending' | 'running' | 'completed' | 'failed'
    total INT NOT NULL DEFAULT 0,
    processed INT NOT NULL DEFAULT 0,
    succeeded INT NOT NULL DEFAULT 0,
    failed INT NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

- [ ] **Step 2: 运行迁移验证**

Run: `cargo clippy`
Expected: 编译通过（迁移文件不影响编译，但需确认 sqlx 离线检查兼容）

- [ ] **Step 3: Commit**

```bash
git add abt/migrations/043_create_h3yun_sync_batch.sql
git commit -m "feat(sync): add h3yun_sync_batch migration"
```

---

### Task 2: 新增批次 Model 和 Repository

**Files:**
- Create: `abt/src/models/sync_batch.rs`
- Modify: `abt/src/models/mod.rs`

- [ ] **Step 1: 创建 sync_batch model**

```rust
// abt/src/models/sync_batch.rs
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct SyncBatch {
    pub id: i32,
    pub batch_type: String,
    pub status: String,
    pub total: i32,
    pub processed: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 2: 在 mod.rs 中注册模块**

在 `abt/src/models/mod.rs` 中添加 `pub mod sync_batch;`

- [ ] **Step 3: Commit**

```bash
git add abt/src/models/sync_batch.rs abt/src/models/mod.rs
git commit -m "feat(sync): add SyncBatch model"
```

---

### Task 3: 新增 SyncBatchRepo

**Files:**
- Create: `abt/src/repositories/sync_batch_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

- [ ] **Step 1: 创建 repository**

```rust
// abt/src/repositories/sync_batch_repo.rs
use anyhow::Result;
use sqlx::PgPool;

use crate::models::sync_batch::SyncBatch;

pub struct SyncBatchRepo;

impl SyncBatchRepo {
    pub async fn create(
        pool: &PgPool,
        batch_type: &str,
        total: i32,
    ) -> Result<i32> {
        let id: i32 = sqlx::query_scalar!(
            r#"
            INSERT INTO h3yun_sync_batch (batch_type, status, total, started_at)
            VALUES ($1, 'running', $2, NOW())
            RETURNING id
            "#,
            batch_type,
            total
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn increment_processed(
        pool: &PgPool,
        batch_id: i32,
        succeeded: bool,
    ) -> Result<()> {
        if succeeded {
            sqlx::query!(
                r#"
                UPDATE h3yun_sync_batch
                SET processed = processed + 1, succeeded = succeeded + 1
                WHERE id = $1
                "#,
                batch_id
            )
            .execute(pool)
            .await?;
        } else {
            sqlx::query!(
                r#"
                UPDATE h3yun_sync_batch
                SET processed = processed + 1, failed = failed + 1
                WHERE id = $1
                "#,
                batch_id
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    pub async fn complete(pool: &PgPool, batch_id: i32) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE h3yun_sync_batch
            SET status = 'completed', completed_at = NOW()
            WHERE id = $1
            "#,
            batch_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn fail(pool: &PgPool, batch_id: i32, error_message: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE h3yun_sync_batch
            SET status = 'failed', error_message = $2, completed_at = NOW()
            WHERE id = $1
            "#,
            batch_id,
            error_message
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_latest_by_type(
        pool: &PgPool,
        batch_type: &str,
    ) -> Result<Option<SyncBatch>> {
        let row = sqlx::query_as::<_, SyncBatch>(
            r#"
            SELECT id, batch_type, status, total, processed, succeeded, failed,
                   error_message, started_at, completed_at, created_at
            FROM h3yun_sync_batch
            WHERE batch_type = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(batch_type)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
```

- [ ] **Step 2: 在 mod.rs 中注册模块**

在 `abt/src/repositories/mod.rs` 中添加 `pub mod sync_batch_repo;`

- [ ] **Step 3: 验证编译**

Run: `cargo clippy`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add abt/src/repositories/sync_batch_repo.rs abt/src/repositories/mod.rs
git commit -m "feat(sync): add SyncBatchRepo for batch progress tracking"
```

---

### Task 4: 扩展 SyncEvent 携带 batch_id

**Files:**
- Modify: `abt/src/h3yun/models.rs` — SyncEvent 增加 batch_id 字段
- Modify: `abt/src/h3yun/sync_worker.rs` — handle_event 更新批次进度

- [ ] **Step 1: 修改 SyncEvent 结构体**

在 `abt/src/h3yun/models.rs` 中，给 `SyncEvent` 加 `batch_id` 字段：

```rust
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub batch_id: Option<i32>,
}
```

- [ ] **Step 2: 更新 SyncWorker::handle_event — 在事件处理完成后更新批次进度**

在 `abt/src/h3yun/sync_worker.rs` 的 `handle_event` 方法末尾，处理完成后更新批次：

```rust
async fn handle_event(&self, event: SyncEvent) {
    let succeeded = match event.entity_type {
        EntityType::Product => {
            let product = match self.fetch_product(event.entity_id).await {
                Some(p) => p,
                None => {
                    self.update_batch_progress(&event, false).await;
                    return;
                }
            };
            let category_path = product_sync::fetch_category_path(&self.pool, event.entity_id).await;
            with_retry("product", event.entity_id, || {
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
                    self.update_batch_progress(&event, false).await;
                    return;
                }
            };
            with_retry("inventory", event.entity_id, || {
                let pool = self.pool.clone();
                let client = self.client.clone();
                let data = data.clone();
                Box::pin(async move { inventory_sync::sync_inventory(&pool, &client, &data).await })
            })
            .await
        }
    };

    if let Some(batch_id) = event.batch_id {
        self.update_batch_progress(&event, succeeded.is_ok()).await;
    }
}
```

然后添加辅助方法：

```rust
async fn update_batch_progress(&self, event: &SyncEvent, succeeded: bool) {
    if let Some(batch_id) = event.batch_id {
        use crate::repositories::SyncBatchRepo;
        if let Err(e) = SyncBatchRepo::increment_processed(&self.pool, batch_id, succeeded).await {
            tracing::warn!(batch_id, error = %e, "Failed to update batch progress");
        }

        // 检查是否全部处理完毕
        if let Ok(Some(batch)) = SyncBatchRepo::find_by_id(&self.pool, batch_id).await {
            if batch.processed >= batch.total {
                if let Err(e) = SyncBatchRepo::complete(&self.pool, batch_id).await {
                    tracing::warn!(batch_id, error = %e, "Failed to mark batch complete");
                }
            }
        }
    }
}
```

注意：`SyncBatchRepo::find_by_id` 需要在 Task 3 的 repo 中补充：

```rust
pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<SyncBatch>> {
    let row = sqlx::query_as::<_, SyncBatch>(
        r#"
        SELECT id, batch_type, status, total, processed, succeeded, failed,
               error_message, started_at, completed_at, created_at
        FROM h3yun_sync_batch
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 3: 更新所有创建 SyncEvent 的地方**

`SyncEvent` 新增了 `batch_id: Option<i32>` 字段，所有构建 `SyncEvent` 的地方需要加上该字段（默认 `None`）：

1. `abt/src/h3yun/scheduled.rs` — 定时任务中的同步事件，batch_id = None
2. `abt/src/implt/excel/product_inventory_import.rs` — Excel 导入中的同步事件，batch_id = None
3. `abt-grpc/src/handlers/sync_handler.rs` — `sync_all_products` 和 `sync_inventory` 中的同步事件，batch_id = None（后续 Task 6 会修改）

在每处构建 `SyncEvent` 的地方添加 `batch_id: None,`。

- [ ] **Step 4: 验证编译**

Run: `cargo clippy`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add abt/src/h3yun/models.rs abt/src/h3yun/sync_worker.rs abt/src/repositories/sync_batch_repo.rs abt/src/h3yun/scheduled.rs abt/src/implt/excel/product_inventory_import.rs abt-grpc/src/handlers/sync_handler.rs
git commit -m "feat(sync): extend SyncEvent with batch_id and update worker progress"
```

---

### Task 5: Proto 定义 — 添加 GetLatestSyncStatus 和 SyncResponse.batch_id

**Files:**
- Modify: `proto/abt/v1/sync.proto`

- [ ] **Step 1: 更新 proto 文件**

```protobuf
syntax = "proto3";
package abt.v1;

option go_package = "abt/v1";

// 注意: 需要引入 google/protobuf/timestamp.proto
import "google/protobuf/timestamp.proto";

service AbtSyncService {
  rpc SyncProduct(SyncProductRequest) returns (SyncResponse);
  rpc SyncAllProducts(SyncAllRequest) returns (SyncResponse);
  rpc SyncInventory(SyncInventoryRequest) returns (SyncResponse);
  rpc SyncAllInventory(SyncAllRequest) returns (SyncResponse);
  // 查询最近同步批次状态
  rpc GetLatestSyncStatus(GetLatestSyncStatusRequest) returns (SyncBatchStatus);
  rpc Reconcile(ReconcileRequest) returns (ReconcileResponse);
}

message SyncProductRequest {
  int64 product_id = 1;
}

message SyncAllRequest {}

message SyncInventoryRequest {
  int64 product_id = 1;
}

message SyncResponse {
  int32 processed = 1;
  int32 succeeded = 2;
  string message = 3;
  int64 batch_id = 4;  // 全量同步时返回批次 ID
}

message GetLatestSyncStatusRequest {
  string batch_type = 1;
}

message SyncBatchStatus {
  int64 batch_id = 1;
  string batch_type = 2;
  string status = 3;
  int32 total = 4;
  int32 processed = 5;
  int32 succeeded = 6;
  int32 failed = 7;
  string error_message = 8;
  google.protobuf.Timestamp started_at = 9;
  google.protobuf.Timestamp completed_at = 10;
}

message ReconcileRequest {
  string entity_type = 1;
}

message ReconcileResponse {
  repeated DriftItem drifts = 1;
}

message DriftItem {
  string entity_type = 1;
  int64 entity_id = 2;
  string drift_type = 3;
  string detail = 4;
}
```

- [ ] **Step 2: 编译生成 proto 代码**

Run: `cargo build -p abt-grpc`
Expected: 编译失败（因为 SyncResponse 新增了 batch_id 字段，handler 需要更新）

- [ ] **Step 3: Commit**

```bash
git add proto/abt/v1/sync.proto
git commit -m "feat(sync): add GetLatestSyncStatus RPC and batch_id to SyncResponse"
```

---

### Task 6: 更新 gRPC Handler — 创建批次 + 查询状态

**Files:**
- Modify: `abt-grpc/src/handlers/sync_handler.rs`

- [ ] **Step 1: 更新 sync_all_products — 创建批次并传递 batch_id**

在 handler 中，先创建批次记录，入队时带上 batch_id：

```rust
#[require_permission(Resource::Sync, Action::Write)]
async fn sync_all_products(
    &self,
    _request: Request<SyncAllRequest>,
) -> GrpcResult<SyncResponse> {
    let state = AppState::get().await;
    let pool = state.pool();

    let product_ids: Vec<i64> = {
        use abt::service::ProductService;
        let query = abt::models::ProductQuery {
            page: Some(1),
            page_size: Some(99999),
            ..Default::default()
        };
        state
            .product_service()
            .query(query)
            .await
            .map_err(error::err_to_status)?
            .0
            .into_iter()
            .map(|p| p.product_id)
            .collect()
    };

    let total = product_ids.len() as i32;
    let batch_id = abt::repositories::SyncBatchRepo::create(&pool, "product", total)
        .await
        .map_err(error::err_to_status)?;

    let sender = abt::h3yun::get_sync_event_sender();
    let mut queued = 0i32;

    for product_id in &product_ids {
        if sender
            .send(abt::h3yun::models::SyncEvent {
                entity_type: abt::h3yun::models::EntityType::Product,
                entity_id: *product_id,
                batch_id: Some(batch_id),
            })
            .await
            .is_ok()
        {
            queued += 1;
        }
    }

    Ok(Response::new(SyncResponse {
        processed: queued,
        succeeded: 0,
        message: format!("{queued} sync events queued"),
        batch_id: batch_id as i64,
    }))
}
```

- [ ] **Step 2: 更新 sync_all_inventory — 同样创建批次**

```rust
#[require_permission(Resource::Sync, Action::Write)]
async fn sync_all_inventory(
    &self,
    _request: Request<SyncAllRequest>,
) -> GrpcResult<SyncResponse> {
    let state = AppState::get().await;
    let pool = state.pool();

    let inventory_ids: Vec<i64> = sqlx::query_scalar!(
        r#"SELECT inventory_id FROM inventory WHERE quantity > 0 AND location_id IS NOT NULL"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| error::err_to_status(anyhow::anyhow!(e)))?;

    let total = inventory_ids.len() as i32;
    let batch_id = abt::repositories::SyncBatchRepo::create(&pool, "inventory", total)
        .await
        .map_err(error::err_to_status)?;

    let sender = abt::h3yun::get_sync_event_sender();
    let mut queued = 0i32;

    for inventory_id in &inventory_ids {
        if sender
            .send(abt::h3yun::models::SyncEvent {
                entity_type: abt::h3yun::models::EntityType::Inventory,
                entity_id: *inventory_id,
                batch_id: Some(batch_id),
            })
            .await
            .is_ok()
        {
            queued += 1;
        }
    }

    Ok(Response::new(SyncResponse {
        processed: queued,
        succeeded: 0,
        message: format!("{queued} inventory sync events queued"),
        batch_id: batch_id as i64,
    }))
}
```

- [ ] **Step 3: 更新现有 sync_product / sync_inventory 的 SyncResponse — batch_id = 0**

在 `sync_product` 和 `sync_inventory` 的 `SyncResponse` 构建处加上 `batch_id: 0`。

- [ ] **Step 4: 实现 GetLatestSyncStatus handler**

```rust
#[require_permission(Resource::Sync, Action::Read)]
async fn get_latest_sync_status(
    &self,
    request: Request<GetLatestSyncStatusRequest>,
) -> GrpcResult<SyncBatchStatus> {
    let req = request.into_inner();
    let batch_type = req.batch_type;
    if batch_type != "product" && batch_type != "inventory" {
        return Err(error::validation("batch_type", "必须是 product 或 inventory"));
    }

    let state = AppState::get().await;
    let pool = state.pool();

    let batch = abt::repositories::SyncBatchRepo::find_latest_by_type(&pool, &batch_type)
        .await
        .map_err(error::err_to_status)?
        .ok_or_else(|| error::validation("batch_type", "未找到同步批次记录"))?;

    Ok(Response::new(SyncBatchStatus {
        batch_id: batch.id as i64,
        batch_type: batch.batch_type,
        status: batch.status,
        total: batch.total,
        processed: batch.processed,
        succeeded: batch.succeeded,
        failed: batch.failed,
        error_message: batch.error_message.unwrap_or_default(),
        started_at: batch.started_at.map(|t| {
            prost_types::Timestamp {
                seconds: t.timestamp(),
                nanos: t.timestamp_subsec_nanos() as i32,
            }
        }),
        completed_at: batch.completed_at.map(|t| {
            prost_types::Timestamp {
                seconds: t.timestamp(),
                nanos: t.timestamp_subsec_nanos() as i32,
            }
        }),
    }))
}
```

- [ ] **Step 5: 验证编译**

Run: `cargo clippy`
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add abt-grpc/src/handlers/sync_handler.rs abt-grpc/src/generated/
git commit -m "feat(sync): integrate batch tracking into sync handlers and add GetLatestSyncStatus"
```

---

### Task 7: 最终验证

- [ ] **Step 1: 运行 cargo clippy 全量检查**

Run: `cargo clippy`
Expected: 无错误

- [ ] **Step 2: 运行测试**

Run: `cargo test`
Expected: 所有测试通过

- [ ] **Step 3: 最终 Commit（如有格式调整）**
