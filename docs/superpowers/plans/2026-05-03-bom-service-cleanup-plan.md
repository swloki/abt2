# BOM Service Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate N+1 queries in `substitute_product`, remove redundant `BomDetail.created_by`, and clean up unused `executor` parameters.

**Architecture:** Three independent refactorings applied bottom-up: repo layer first, then service layer, then handler/proto layer.

**Tech Stack:** Rust, sqlx, tonic/prost (gRPC), PostgreSQL

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `abt/src/repositories/bom_repo.rs` | Add `find_accessible_boms_by_product` |
| Modify | `abt/src/repositories/bom_node_repo.rs` | Add `find_by_bom_ids_and_product` |
| Modify | `abt/src/models/bom.rs` | Remove `BomDetail.created_by`, `deserialize_created_by` |
| Modify | `abt/src/service/bom_service.rs` | Remove `executor` from `get_leaf_nodes`, `get_product_code` |
| Modify | `abt/src/implt/bom_service_impl.rs` | Rewrite `substitute_product`, simplify `build_bom_detail`, remove `_executor` params |
| Modify | `proto/abt/v1/bom.proto` | Remove `created_by` from `BomDetailProto` |
| Regenerate | `abt-grpc/src/generated/abt.v1.rs` | Auto via `cargo build` |
| Modify | `abt-grpc/src/handlers/convert.rs` | Update `From<BomDetail> for BomDetailProto` |
| Modify | `abt-grpc/src/handlers/bom.rs` | Remove executor from `get_leaf_nodes` and `get_product_code` calls |

---

### Task 1: Add batch repo methods for substitute_product

**Files:**
- Modify: `abt/src/repositories/bom_repo.rs` (after line 137, before `find_by_id_pool`)
- Modify: `abt/src/repositories/bom_node_repo.rs` (after `find_by_bom_ids`, ~line 70)

- [ ] **Step 1: Add `find_accessible_boms_by_product` to BomRepo**

In `abt/src/repositories/bom_repo.rs`, add after the `find_by_id_for_update` method (after line 124):

```rust
    /// 批量查询包含指定产品且用户有权访问的 BOM（带行锁）
    pub async fn find_accessible_boms_by_product(
        executor: Executor<'_>,
        product_id: i64,
        caller_id: i64,
    ) -> Result<Vec<crate::models::Bom>> {
        let rows = sqlx::query_as::<_, crate::models::Bom>(
            "SELECT bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at, published_by
             FROM bom
             WHERE EXISTS (SELECT 1 FROM bom_nodes WHERE bom_nodes.bom_id = bom.bom_id AND product_id = $1)
               AND (status = 'published' OR created_by = $2)
             FOR UPDATE",
        )
        .bind(product_id)
        .bind(caller_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
```

- [ ] **Step 2: Add `find_by_bom_ids_and_product` to BomNodeRepo**

In `abt/src/repositories/bom_node_repo.rs`, add after the `find_by_bom_ids` method (after the method that ends around line 69):

```rust
    /// 批量查询指定 BOM 列表中匹配指定产品的节点（带行锁）
    pub async fn find_by_bom_ids_and_product(
        executor: Executor<'_>,
        bom_ids: &[i64],
        product_id: i64,
    ) -> Result<Vec<BomNodeRow>> {
        if bom_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, BomNodeRow>(
            r#"SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties
               FROM bom_nodes
               WHERE bom_id = ANY($1) AND product_id = $2
               ORDER BY bom_id, "order"
               FOR UPDATE"#,
        )
        .bind(bom_ids)
        .bind(product_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p abt 2>&1 | grep -v "department"`
Expected: No errors related to these new methods (only pre-existing department errors)

- [ ] **Step 4: Commit**

```bash
git add abt/src/repositories/bom_repo.rs abt/src/repositories/bom_node_repo.rs
git commit -m "feat: add batch repo methods for substitute_product optimization"
```

---

### Task 2: Remove BomDetail.created_by

**Files:**
- Modify: `abt/src/models/bom.rs` (lines 108-135)
- Modify: `abt/src/implt/bom_service_impl.rs` (line 50-53, and all callers)

- [ ] **Step 1: Simplify BomDetail struct in bom.rs**

In `abt/src/models/bom.rs`, replace the `BomDetail` struct (lines 109-116) and remove the `deserialize_created_by` function (lines 118-135):

Remove these lines:
```rust
/// BOM 详情（节点从 bom_nodes 表加载）
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BomDetail {
    /// BOM 节点列表
    pub nodes: Vec<BomNode>,
    /// 创建者（用户 ID）
    #[serde(deserialize_with = "deserialize_created_by")]
    pub created_by: Option<i64>,
}

/// 兼容旧数据：created_by 可能是字符串或数字
fn deserialize_created_by<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64()),
        Some(serde_json::Value::String(_)) => Ok(None), // 旧字符串数据忽略
        Some(other) => Err(de::Error::custom(format!(
            "expected number or string for created_by, got {:?}",
            other
        ))),
    }
}
```

Replace with:
```rust
/// BOM 详情（节点从 bom_nodes 表加载）
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BomDetail {
    pub nodes: Vec<BomNode>,
}
```

- [ ] **Step 2: Simplify build_bom_detail in bom_service_impl.rs**

In `abt/src/implt/bom_service_impl.rs`, change `build_bom_detail` (line 50-53):

From:
```rust
    async fn build_bom_detail(&self, bom_id: i64, created_by: Option<i64>) -> Result<BomDetail> {
        let nodes = BomNodeRepo::find_bom_nodes_by_bom_id(&self.pool, bom_id).await?;
        Ok(BomDetail { nodes, created_by })
    }
```

To:
```rust
    async fn build_bom_detail(&self, bom_id: i64) -> Result<BomDetail> {
        let nodes = BomNodeRepo::find_bom_nodes_by_bom_id(&self.pool, bom_id).await?;
        Ok(BomDetail { nodes })
    }
```

- [ ] **Step 3: Remove created_by from all build_bom_detail callers**

In `abt/src/implt/bom_service_impl.rs`, update all 5 call sites. Remove the second argument (`bom.created_by` or `published_bom.created_by` or `draft_bom.created_by`):

Line 122: `self.build_bom_detail(bom_id, bom.created_by).await?` → `self.build_bom_detail(bom_id).await?`
Line 211: `self.build_bom_detail(bom_id, bom.created_by).await?` → `self.build_bom_detail(bom_id).await?`
Line 216: `self.build_bom_detail(bom_id, published_bom.created_by).await?` → `self.build_bom_detail(bom_id).await?`
Line 232: `self.build_bom_detail(bom_id, bom.created_by).await?` → `self.build_bom_detail(bom_id).await?`
Line 237: `self.build_bom_detail(bom_id, draft_bom.created_by).await?` → `self.build_bom_detail(bom_id).await?`

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p abt 2>&1 | grep -v "department"`
Expected: No errors related to BomDetail (only pre-existing department errors)

- [ ] **Step 5: Commit**

```bash
git add abt/src/models/bom.rs abt/src/implt/bom_service_impl.rs
git commit -m "refactor: remove redundant BomDetail.created_by field"
```

---

### Task 3: Remove created_by from proto and convert layer

**Files:**
- Modify: `proto/abt/v1/bom.proto` (lines 57-60)
- Modify: `abt-grpc/src/handlers/convert.rs` (lines 121-128)
- Regenerate: `abt-grpc/src/generated/abt.v1.rs`

- [ ] **Step 1: Update bom.proto**

In `proto/abt/v1/bom.proto`, change `BomDetailProto` message (lines 57-60):

From:
```protobuf
message BomDetailProto {
  repeated BomNodeProto nodes = 1;
  int64 created_by = 2;
}
```

To:
```protobuf
message BomDetailProto {
  repeated BomNodeProto nodes = 1;
  reserved 2;
}
```

Note: Using `reserved 2` prevents future field numbers from colliding with the old `created_by` field.

- [ ] **Step 2: Regenerate proto code**

Run: `cargo build -p abt-grpc 2>&1 | tail -5`

This triggers `build.rs` which regenerates `abt-grpc/src/generated/abt.v1.rs`. The build will fail because `convert.rs` still references the old field — that's expected, fix in next step.

- [ ] **Step 3: Update convert.rs**

In `abt-grpc/src/handlers/convert.rs`, change `From<abt::BomDetail> for BomDetailProto` (lines 121-128):

From:
```rust
impl From<abt::BomDetail> for BomDetailProto {
    fn from(detail: abt::BomDetail) -> Self {
        BomDetailProto {
            nodes: detail.nodes.into_iter().map(|n| n.into()).collect(),
            created_by: detail.created_by.unwrap_or(0),
        }
    }
}
```

To:
```rust
impl From<abt::BomDetail> for BomDetailProto {
    fn from(detail: abt::BomDetail) -> Self {
        BomDetailProto {
            nodes: detail.nodes.into_iter().map(|n| n.into()).collect(),
        }
    }
}
```

- [ ] **Step 4: Verify full compilation**

Run: `cargo check -p abt-grpc 2>&1 | grep -v "department"`
Expected: No errors related to BomDetailProto (only pre-existing department errors)

- [ ] **Step 5: Commit**

```bash
git add proto/abt/v1/bom.proto abt-grpc/src/generated/abt.v1.rs abt-grpc/src/handlers/convert.rs
git commit -m "refactor: remove created_by from BomDetailProto (proto breaking change)"
```

---

### Task 4: Rewrite substitute_product to use batch queries

**Files:**
- Modify: `abt/src/implt/bom_service_impl.rs` (lines 298-384)

- [ ] **Step 1: Rewrite the `substitute_product` method**

In `abt/src/implt/bom_service_impl.rs`, replace the entire `substitute_product` method body (lines 298-384):

```rust
    async fn substitute_product(
        &self,
        old_product_id: i64,
        new_product_id: i64,
        bom_id: Option<i64>,
        overrides: AttributeOverrides,
        caller_id: i64,
        executor: Executor<'_>,
    ) -> Result<(i64, i64)> {
        if old_product_id == new_product_id {
            return Ok((0, 0));
        }

        let products = ProductRepo::find_by_ids(&self.pool, &[new_product_id]).await?;
        let new_product = products
            .first()
            .ok_or_else(|| anyhow::anyhow!("替换物料不存在: {}", new_product_id))?;
        let new_product_code = new_product.meta.product_code.clone();

        let affected_boms: Vec<crate::models::Bom> = match bom_id {
            Some(id) => {
                let bom = BomRepo::find_by_id_for_update(&mut *executor, id).await?
                    .ok_or_else(|| anyhow::anyhow!("BOM not found"))?;
                bom.require_creator_or_published(caller_id, true)?;
                vec![bom]
            }
            None => {
                BomRepo::find_accessible_boms_by_product(&mut *executor, old_product_id, caller_id).await?
            }
        };

        let bom_ids: Vec<i64> = affected_boms.iter().map(|b| b.bom_id).collect();
        let nodes = BomNodeRepo::find_by_bom_ids_and_product(&mut *executor, &bom_ids, old_product_id).await?;

        let mut affected_bom_count: i64 = 0;
        let mut replaced_node_count: i64 = 0;
        let mut changed_bom_ids: HashSet<i64> = HashSet::new();

        for node in &nodes {
            let quantity = overrides.quantity
                .map(f64_to_decimal)
                .unwrap_or(node.quantity);
            let loss_rate = overrides.loss_rate
                .map(f64_to_decimal)
                .unwrap_or(node.loss_rate);
            let unit = overrides.unit.as_deref().or(node.unit.as_deref());
            let remark = overrides.remark.as_deref().or(node.remark.as_deref());
            let position = overrides.position.as_deref().or(node.position.as_deref());
            let work_center = overrides.work_center.as_deref().or(node.work_center.as_deref());
            let properties = overrides.properties.as_deref().or(node.properties.as_deref());

            BomNodeRepo::substitute_node_product(
                &mut *executor,
                node.id,
                new_product_id,
                Some(&new_product_code),
                quantity,
                loss_rate,
                unit,
                remark,
                position,
                work_center,
                properties,
            ).await?;

            replaced_node_count += 1;
            changed_bom_ids.insert(node.bom_id);
        }

        affected_bom_count = changed_bom_ids.len() as i64;

        Ok((affected_bom_count, replaced_node_count))
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p abt-grpc 2>&1 | grep -v "department"`
Expected: No new errors

- [ ] **Step 3: Commit**

```bash
git add abt/src/implt/bom_service_impl.rs
git commit -m "perf: batch queries in substitute_product (3N+M → 2+M)"
```

---

### Task 5: Remove unused executor parameters from trait

**Files:**
- Modify: `abt/src/service/bom_service.rs` (lines 55, 67)
- Modify: `abt/src/implt/bom_service_impl.rs` (lines 241, 278)
- Modify: `abt-grpc/src/handlers/bom.rs` (lines 244-258, 296-310)

- [ ] **Step 1: Update BomService trait signatures**

In `abt/src/service/bom_service.rs`, change two method signatures:

Line 55:
```rust
async fn get_leaf_nodes(&self, bom_id: i64, executor: Executor<'_>) -> Result<Vec<BomNode>>;
```
→
```rust
async fn get_leaf_nodes(&self, bom_id: i64) -> Result<Vec<BomNode>>;
```

Line 67:
```rust
async fn get_product_code(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<String>>;
```
→
```rust
async fn get_product_code(&self, bom_id: i64) -> Result<Option<String>>;
```

- [ ] **Step 2: Update BomServiceImpl signatures**

In `abt/src/implt/bom_service_impl.rs`:

Line 241:
```rust
async fn get_leaf_nodes(&self, bom_id: i64, _executor: Executor<'_>) -> Result<Vec<BomNode>> {
```
→
```rust
async fn get_leaf_nodes(&self, bom_id: i64) -> Result<Vec<BomNode>> {
```

Line 278:
```rust
async fn get_product_code(&self, bom_id: i64, _executor: Executor<'_>) -> Result<Option<String>> {
```
→
```rust
async fn get_product_code(&self, bom_id: i64) -> Result<Option<String>> {
```

- [ ] **Step 3: Update handler — get_product_code**

In `abt-grpc/src/handlers/bom.rs`, replace `get_product_code` handler (lines 234-261):

From:
```rust
    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_product_code(
        &self,
        request: Request<GetProductCodeRequest>,
    ) -> GrpcResult<StringResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        Self::find_and_authorize(&srv, req.bom_id, auth.user_id, false, &mut tx).await?;

        let code = srv
            .get_product_code(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(StringResponse { value: code }))
    }
```

To:
```rust
    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_product_code(
        &self,
        request: Request<GetProductCodeRequest>,
    ) -> GrpcResult<StringResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        Self::find_and_authorize_pool(&state.pool(), req.bom_id, auth.user_id).await?;

        let code = srv
            .get_product_code(req.bom_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(StringResponse { value: code }))
    }
```

- [ ] **Step 4: Update handler — get_leaf_nodes**

In `abt-grpc/src/handlers/bom.rs`, replace `get_leaf_nodes` handler (lines 286-311):

From:
```rust
    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_leaf_nodes(
        &self,
        request: Request<GetLeafNodesRequest>,
    ) -> GrpcResult<BomNodesResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        Self::find_and_authorize(&srv, req.bom_id, auth.user_id, false, &mut tx).await?;

        let nodes = srv
            .get_leaf_nodes(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomNodesResponse {
            items: nodes.into_iter().map(|n| n.into()).collect(),
        }))
    }
```

To:
```rust
    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_leaf_nodes(
        &self,
        request: Request<GetLeafNodesRequest>,
    ) -> GrpcResult<BomNodesResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        Self::find_and_authorize_pool(&state.pool(), req.bom_id, auth.user_id).await?;

        let nodes = srv
            .get_leaf_nodes(req.bom_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomNodesResponse {
            items: nodes.into_iter().map(|n| n.into()).collect(),
        }))
    }
```

- [ ] **Step 5: Verify full compilation**

Run: `cargo check -p abt-grpc 2>&1 | grep -v "department"`
Expected: No new errors

- [ ] **Step 6: Commit**

```bash
git add abt/src/service/bom_service.rs abt/src/implt/bom_service_impl.rs abt-grpc/src/handlers/bom.rs
git commit -m "refactor: remove unused executor from get_leaf_nodes and get_product_code"
```
