# 级联查询库存 - 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增 gRPC 接口，给定产品 ID/code，查询该产品被哪些 BOM 引用，按 BOM 分组展示直接子节点及其库存总量。

**Architecture:** 两次 SQL 查询——第一次用 CTE+LEFT JOIN 一次获取产品信息及 BOM 子节点结构，第二次批量获取子节点库存汇总。Repository 返回扁平结果，Service 负责按 bom_id 分组和库存合并。rpc 挂在现有 `AbtInventoryService` proto 服务下。

**Tech Stack:** Rust, tonic/prost (gRPC), sqlx (PostgreSQL), rust_decimal

---

### Task 1: Proto 定义

**Files:**
- Create: `proto/abt/v1/inventory_cascade.proto`
- Modify: `proto/abt/v1/inventory.proto`

- [ ] **Step 1: 创建 inventory_cascade.proto**

创建 `proto/abt/v1/inventory_cascade.proto`：

```protobuf
syntax = "proto3";
package abt.v1;

option go_package = "abt/v1";

message CascadeInventoryRequest {
  oneof product_identifier {
    int64 product_id = 1;
    string product_code = 2;
  }
  optional int32 max_results = 3;  // 默认 500，上限 2000
}

message BomCascadeGroup {
  int64 bom_id = 1;
  string bom_name = 2;
  repeated ChildNodeInventory children = 3;
}

message ChildNodeInventory {
  int64 node_id = 1;
  int64 product_id = 2;
  string product_code = 3;
  string product_name = 4;
  string unit = 5;
  double quantity = 6;
  double total_stock = 7;
  double loss_rate = 8;
  int32 order = 9;
  optional int64 parent_node_id = 10;
}

message CascadeInventoryResponse {
  int64 product_id = 1;
  string product_code = 2;
  string product_name = 3;
  repeated BomCascadeGroup bom_groups = 4;
}
```

- [ ] **Step 2: 在 inventory.proto 中添加 rpc 和 import**

在 `proto/abt/v1/inventory.proto` 顶部 import 区域添加：

```protobuf
import "abt/v1/inventory_cascade.proto";
```

在 `AbtInventoryService` service 块的末尾（`rpc GetLogsByWarehouse` 之后）添加：

```protobuf
  // 级联查询库存
  rpc CascadeInventory(CascadeInventoryRequest) returns (CascadeInventoryResponse);
```

- [ ] **Step 3: cargo build 生成 proto 代码**

Run: `cargo build -p abt-grpc`
Expected: 编译成功，`abt-grpc/src/generated/` 中生成包含新 message 和更新后 service trait 的代码

- [ ] **Step 4: Commit**

```bash
git add proto/abt/v1/inventory_cascade.proto proto/abt/v1/inventory.proto
git commit -m "feat(proto): add CascadeInventory rpc and messages"
```

---

### Task 2: Model 结构体

**Files:**
- Create: `abt/src/models/inventory_cascade.rs`
- Modify: `abt/src/models/mod.rs`

- [ ] **Step 1: 创建 model 文件**

创建 `abt/src/models/inventory_cascade.rs`：

```rust
//! 级联查询库存模型

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 级联查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeInventoryResult {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub bom_groups: Vec<BomCascadeGroup>,
}

/// 按 BOM 分组的级联数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomCascadeGroup {
    pub bom_id: i64,
    pub bom_name: String,
    pub children: Vec<ChildNodeInventory>,
}

/// 子节点库存信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildNodeInventory {
    pub node_id: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub unit: Option<String>,
    pub quantity: Decimal,
    pub total_stock: Decimal,
    pub loss_rate: Decimal,
    pub order: i32,
    pub parent_node_id: Option<i64>,
}
```

- [ ] **Step 2: 注册 model module**

在 `abt/src/models/mod.rs` 中，在 `mod warehouse;` 后添加：

```rust
mod inventory_cascade;
```

在 `pub use warehouse::*;` 后添加：

```rust
pub use inventory_cascade::*;
```

- [ ] **Step 3: cargo check 验证**

Run: `cargo check -p abt`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add abt/src/models/inventory_cascade.rs abt/src/models/mod.rs
git commit -m "feat(models): add cascade inventory structs"
```

---

### Task 3: Repository

**Files:**
- Create: `abt/src/repositories/inventory_cascade_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

- [ ] **Step 1: 创建 repository 文件**

创建 `abt/src/repositories/inventory_cascade_repo.rs`：

```rust
//! 级联查询库存数据访问层

use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::PgPool;

/// 第一次查询的扁平行结构（LEFT JOIN 产生，bom/child 列可能为 NULL）
#[derive(Debug, FromRow)]
pub struct CascadeNodeRow {
    pub root_product_id: i64,
    pub root_product_code: String,
    pub root_product_name: String,
    pub bom_id: Option<i64>,
    pub bom_name: Option<String>,
    pub node_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_code: Option<String>,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<Decimal>,
    pub loss_rate: Option<Decimal>,
    pub order: Option<i32>,
    pub parent_node_id: Option<i64>,
}

/// 库存汇总行
#[derive(Debug, FromRow)]
pub struct StockSummaryRow {
    pub product_id: i64,
    pub total_stock: Decimal,
}

pub struct InventoryCascadeRepo;

impl InventoryCascadeRepo {
    /// 查询产品的 BOM 引用及子节点结构
    ///
    /// 使用 CTE + LEFT JOIN：
    /// - 产品不存在 → 空结果
    /// - 产品存在但无 BOM 引用 → 1 行（bom 列为 NULL）
    /// - 有 BOM 引用 → 多行
    pub async fn find_cascade_nodes(
        pool: &PgPool,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<Vec<CascadeNodeRow>> {
        let rows = sqlx::query_as::<_, CascadeNodeRow>(
            r#"
            WITH parent_product AS (
              SELECT product_id, product_code, pdt_name
              FROM products
              WHERE (product_id = $1 OR product_code = $2)
                AND deleted_at IS NULL
              LIMIT 1
            )
            SELECT
              pp.product_id AS root_product_id,
              pp.product_code AS root_product_code,
              pp.pdt_name AS root_product_name,
              b.bom_id,
              b.bom_name,
              child.id AS node_id,
              child.product_id,
              child.product_code,
              p_child.pdt_name AS product_name,
              child.unit,
              child.quantity,
              child.loss_rate,
              child."order",
              bn_parent.id AS parent_node_id
            FROM parent_product pp
            LEFT JOIN bom_nodes bn_parent ON bn_parent.product_id = pp.product_id
            LEFT JOIN bom b ON b.bom_id = bn_parent.bom_id
                      AND b.deleted_at IS NULL
            LEFT JOIN bom_nodes child ON child.parent_id = bn_parent.id
                       AND child.bom_id = bn_parent.bom_id
            LEFT JOIN products p_child ON p_child.product_id = child.product_id
                      AND p_child.deleted_at IS NULL
            ORDER BY b.bom_id, child."order"
            LIMIT $3
            "#,
        )
        .bind(product_id)
        .bind(product_code)
        .bind(max_results)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// 批量查询产品库存汇总
    pub async fn find_stock_summary(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<Vec<StockSummaryRow>> {
        if product_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows = sqlx::query_as::<_, StockSummaryRow>(
            r#"
            SELECT
              i.product_id,
              SUM(i.quantity) AS total_stock
            FROM inventory i
            WHERE i.product_id = ANY($1)
            GROUP BY i.product_id
            "#,
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
```

- [ ] **Step 2: 注册 repository module**

在 `abt/src/repositories/mod.rs` 中，在 `mod warehouse_repo;` 后添加：

```rust
mod inventory_cascade_repo;
```

在 `pub use warehouse_repo::WarehouseRepo;` 后添加：

```rust
pub use inventory_cascade_repo::{CascadeNodeRow, InventoryCascadeRepo, StockSummaryRow};
```

- [ ] **Step 3: cargo check 验证**

Run: `cargo check -p abt`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add abt/src/repositories/inventory_cascade_repo.rs abt/src/repositories/mod.rs
git commit -m "feat(repo): add cascade inventory queries"
```

---

### Task 4: Service Trait

**Files:**
- Create: `abt/src/service/inventory_cascade_service.rs`
- Modify: `abt/src/service/mod.rs`

- [ ] **Step 1: 创建 service trait 文件**

创建 `abt/src/service/inventory_cascade_service.rs`：

```rust
//! 级联查询库存服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::CascadeInventoryResult;

/// 级联查询库存服务
#[async_trait]
pub trait InventoryCascadeService: Send + Sync {
    /// 级联查询产品的 BOM 引用和子节点库存
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<CascadeInventoryResult>;
}
```

- [ ] **Step 2: 注册 service trait module**

在 `abt/src/service/mod.rs` 中，在 `mod warehouse_service;` 后添加：

```rust
mod inventory_cascade_service;
```

在 `pub use warehouse_service::WarehouseService;` 后添加：

```rust
pub use inventory_cascade_service::InventoryCascadeService;
```

- [ ] **Step 3: cargo check 验证**

Run: `cargo check -p abt`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add abt/src/service/inventory_cascade_service.rs abt/src/service/mod.rs
git commit -m "feat(service): add InventoryCascadeService trait"
```

---

### Task 5: Service 实现

**Files:**
- Create: `abt/src/implt/inventory_cascade_service_impl.rs`
- Modify: `abt/src/implt/mod.rs`
- Modify: `abt/src/lib.rs`

- [ ] **Step 1: 创建 service impl 文件**

创建 `abt/src/implt/inventory_cascade_service_impl.rs`：

```rust
//! 级联查询库存服务实现

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{BomCascadeGroup, CascadeInventoryResult, ChildNodeInventory};
use crate::repositories::InventoryCascadeRepo;
use crate::service::InventoryCascadeService;

pub struct InventoryCascadeServiceImpl {
    pool: Arc<PgPool>,
}

impl InventoryCascadeServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryCascadeService for InventoryCascadeServiceImpl {
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<CascadeInventoryResult> {
        let max = max_results.clamp(1, 2000);

        let start = std::time::Instant::now();

        // 第一次查询：产品信息 + BOM 子节点结构
        let rows = InventoryCascadeRepo::find_cascade_nodes(
            &self.pool,
            product_id,
            product_code,
            max,
        )
        .await?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            row_count = rows.len(),
            "cascade structure query completed"
        );

        // 空结果 = 产品不存在
        if rows.is_empty() {
            anyhow::bail!(
                "产品不存在: {}",
                product_id
                    .map(|id| id.to_string())
                    .or(product_code.clone())
                    .unwrap_or_default()
            );
        }

        let first = &rows[0];
        let result_product_id = first.root_product_id;
        let result_product_code = first.root_product_code.clone();
        let result_product_name = first.root_product_name.clone();

        // 收集有子节点的行
        let child_rows: Vec<_> = rows
            .into_iter()
            .filter(|r| r.bom_id.is_some() && r.node_id.is_some())
            .collect();

        // 收集子节点 product_id 用于库存查询
        let child_product_ids: Vec<i64> = child_rows
            .iter()
            .filter_map(|r| r.product_id)
            .collect();

        // 第二次查询：批量获取库存汇总
        let stock_start = std::time::Instant::now();
        let stock_map = if child_product_ids.is_empty() {
            HashMap::new()
        } else {
            let stocks = InventoryCascadeRepo::find_stock_summary(
                &self.pool,
                &child_product_ids,
            )
            .await?;

            stocks
                .into_iter()
                .map(|s| (s.product_id, s.total_stock))
                .collect()
        };

        tracing::info!(
            elapsed_ms = stock_start.elapsed().as_millis() as u64,
            product_count = child_product_ids.len(),
            "cascade stock query completed"
        );

        // 按 bom_id 分组构建 BomCascadeGroup
        let mut groups: HashMap<i64, BomCascadeGroup> = HashMap::new();

        for row in child_rows {
            let bom_id = match row.bom_id {
                Some(id) => id,
                None => continue,
            };
            let bom_name = row.bom_name.unwrap_or_default();

            let child = ChildNodeInventory {
                node_id: row.node_id.unwrap_or(0),
                product_id: row.product_id.unwrap_or(0),
                product_code: row.product_code.unwrap_or_default(),
                product_name: row.product_name.unwrap_or_default(),
                unit: row.unit,
                quantity: row.quantity.unwrap_or(Decimal::ZERO),
                total_stock: row
                    .product_id
                    .and_then(|pid| stock_map.get(&pid).copied())
                    .unwrap_or(Decimal::ZERO),
                loss_rate: row.loss_rate.unwrap_or(Decimal::ZERO),
                order: row.order.unwrap_or(0),
                parent_node_id: row.parent_node_id,
            };

            groups
                .entry(bom_id)
                .or_insert_with(|| BomCascadeGroup {
                    bom_id,
                    bom_name,
                    children: Vec::new(),
                })
                .children
                .push(child);
        }

        // 按 bom_id 排序输出
        let mut bom_groups: Vec<BomCascadeGroup> = groups.into_values().collect();
        bom_groups.sort_by_key(|g| g.bom_id);

        Ok(CascadeInventoryResult {
            product_id: result_product_id,
            product_code: result_product_code,
            product_name: result_product_name,
            bom_groups,
        })
    }
}
```

- [ ] **Step 2: 注册 impl module**

在 `abt/src/implt/mod.rs` 中，在 `mod warehouse_service_impl;` 后添加：

```rust
mod inventory_cascade_service_impl;
```

在 `pub use warehouse_service_impl::WarehouseServiceImpl;` 后添加：

```rust
pub use inventory_cascade_service_impl::InventoryCascadeServiceImpl;
```

- [ ] **Step 3: 添加工厂函数到 lib.rs**

在 `abt/src/lib.rs` 中，在 `get_department_service` 函数后添加：

```rust
/// 获取级联查询库存服务
pub fn get_inventory_cascade_service(ctx: &AppContext) -> impl crate::service::InventoryCascadeService {
    crate::implt::InventoryCascadeServiceImpl::new(Arc::new(ctx.pool().clone()))
}
```

- [ ] **Step 4: cargo check 验证**

Run: `cargo check -p abt`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add abt/src/implt/inventory_cascade_service_impl.rs abt/src/implt/mod.rs abt/src/lib.rs
git commit -m "feat(service): implement cascade inventory with grouping logic"
```

---

### Task 6: Handler & Server 注册

**Files:**
- Modify: `abt-grpc/src/handlers/inventory.rs`
- Modify: `abt-grpc/src/server.rs`

- [ ] **Step 1: 在 handler 中添加 cascade_inventory 方法**

在 `abt-grpc/src/handlers/inventory.rs` 中：

1) 在文件顶部 imports 中添加：

```rust
use abt::InventoryCascadeService;
```

2) 在 `impl GrpcInventoryService for InventoryHandler` 块末尾（`get_logs_by_warehouse` 方法之后）添加：

```rust
    #[require_permission(Resource::Inventory, Action::Read)]
    async fn cascade_inventory(
        &self,
        request: Request<CascadeInventoryRequest>,
    ) -> GrpcResult<CascadeInventoryResponse> {
        let req = request.into_inner();

        // 输入校验
        let product_id = req.product_identifier.and_then(|id| match id {
            cascade_inventory_request::ProductIdentifier::ProductId(id) => Some(id),
            cascade_inventory_request::ProductCode(code) => None,
        });
        let product_code = req.product_identifier.and_then(|id| match id {
            cascade_inventory_request::ProductIdentifier::ProductId(_) => None,
            cascade_inventory_request::ProductIdentifier::ProductCode(code) => Some(code),
        });

        if product_id.is_none() && product_code.is_none() {
            return Err(common::error::validation(
                "product_identifier",
                "必须提供 product_id 或 product_code",
            ));
        }

        let max_results = req.max_results.unwrap_or(500);

        let state = AppState::get().await;
        let srv = state.inventory_cascade_service();

        let result = srv
            .cascade_inventory(product_id, product_code, max_results)
            .await
            .map_err(common::error::err_to_status)?;

        Ok(Response::new(CascadeInventoryResponse {
            product_id: result.product_id,
            product_code: result.product_code,
            product_name: result.product_name,
            bom_groups: result
                .bom_groups
                .into_iter()
                .map(|g| BomCascadeGroup {
                    bom_id: g.bom_id,
                    bom_name: g.bom_name,
                    children: g
                        .children
                        .into_iter()
                        .map(|c| ChildNodeInventory {
                            node_id: c.node_id,
                            product_id: c.product_id,
                            product_code: c.product_code,
                            product_name: c.product_name,
                            unit: c.unit.unwrap_or_default(),
                            quantity: c.quantity.to_string().parse().unwrap_or(0.0),
                            total_stock: c.total_stock.to_string().parse().unwrap_or(0.0),
                            loss_rate: c.loss_rate.to_string().parse().unwrap_or(0.0),
                            order: c.order,
                            parent_node_id: c.parent_node_id,
                        })
                        .collect(),
                })
                .collect(),
        }))
    }
```

注意：`use crate::generated::abt::v1::cascade_inventory_request::ProductIdentifier;` 已在 proto 生成的模块中。如果编译器找不到 `ProductIdentifier`，需要在 imports 中显式添加：

```rust
use crate::generated::abt::v1::cascade_inventory_request::ProductIdentifier;
```

或使用完整路径 `cascade_inventory_request::ProductIdentifier::...`（如上代码所示，通过 `use crate::generated::abt::v1::*` 已导入）。

- [ ] **Step 2: 在 server.rs 中添加 inventory_cascade_service 方法**

在 `abt-grpc/src/server.rs` 的 `AppState` impl 块中，在 `bom_category_service` 方法后添加：

```rust
    pub fn inventory_cascade_service(&self) -> impl abt::InventoryCascadeService {
        abt::get_inventory_cascade_service(self.abt_context)
    }
```

注意：rpc 注册不需要修改。因为 `CascadeInventory` 是添加到现有的 `AbtInventoryService` proto service 中，它已经被注册为 `AbtInventoryServiceServer::with_interceptor(InventoryHandler::new(), auth_interceptor)`。新增的 rpc 方法会在 `InventoryHandler` impl 中自动匹配。

- [ ] **Step 3: cargo build 验证**

Run: `cargo build -p abt-grpc`
Expected: 编译通过。如果 handler trait 缺少方法，会报编译错误——确认方法签名与 proto 生成的 trait 完全一致。

- [ ] **Step 4: Commit**

```bash
git add abt-grpc/src/handlers/inventory.rs abt-grpc/src/server.rs
git commit -m "feat(handler): add CascadeInventory rpc to InventoryHandler"
```

---

### Task 7: 索引迁移

**Files:**
- Create: `abt/migrations/XXX_add_cascade_indexes.sql`

- [ ] **Step 1: 查找下一个迁移编号**

Run: `ls abt/migrations/ | sort | tail -5`
查看最新编号，确定下一个编号。

- [ ] **Step 2: 创建迁移文件**

假设下一个编号为 `038`，创建 `abt/migrations/038_add_cascade_indexes.sql`：

```sql
-- 级联查询库存所需索引
CREATE INDEX IF NOT EXISTS idx_bom_nodes_product_bom_parent
  ON bom_nodes(product_id, bom_id, parent_id);

CREATE INDEX IF NOT EXISTS idx_bom_nodes_parent_bom_order
  ON bom_nodes(parent_id, bom_id, "order");

CREATE INDEX IF NOT EXISTS idx_inventory_product
  ON inventory(product_id);
```

注意：如果某些索引已存在，`IF NOT EXISTS` 会安全跳过。

- [ ] **Step 3: Commit**

```bash
git add abt/migrations/038_add_cascade_indexes.sql
git commit -m "feat(migration): add indexes for cascade inventory queries"
```

---

### Task 8: 全量构建与测试

**Files:** 无新增/修改

- [ ] **Step 1: 全量 cargo build**

Run: `cargo build`
Expected: 编译通过，无 error

- [ ] **Step 2: cargo test**

Run: `cargo test`
Expected: 所有测试通过

- [ ] **Step 3: 最终 Commit（如有修复）**

如果构建或测试过程中修复了任何问题：

```bash
git add -A
git commit -m "fix: resolve build/test issues from cascade inventory implementation"
```
