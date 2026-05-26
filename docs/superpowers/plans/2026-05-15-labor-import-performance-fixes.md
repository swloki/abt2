# 工序导入性能修复实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除工序导入的 N+1 查询问题，添加连接池超时保护。

**Architecture:** 在 routing_repo 新增 3 个批量查询方法，导入函数一次性获取所有路线数据后循环内只做 HashMap 查找。连接池添加 acquire_timeout / idle_timeout / max_lifetime。

**Tech Stack:** Rust, sqlx, PostgreSQL

---

### Task 1: 添加批量查询方法到 routing_repo

**Files:**
- Modify: `abt/src/repositories/routing_repo.rs` (在 `bom_routing` 操作区块前新增方法)

在 `impl RoutingRepo` 中，`find_matching_routing_tx` 方法之后、`bom_routing` 区块之前，新增以下三个方法：

- [ ] **Step 1: 添加 `find_bom_routing_batch`**

在 `routing_repo.rs` 的 `// bom_routing 表操作` 注释之前添加：

```rust
    // ========================================================================
    // 批量查询（导入优化）
    // ========================================================================

    /// 批量查询 BOM 路线绑定
    pub async fn find_bom_routing_batch(
        pool: &PgPool,
        product_codes: &[String],
    ) -> Result<std::collections::HashMap<String, BomRouting>> {
        if product_codes.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let items: Vec<BomRouting> = sqlx::query_as(
            "SELECT id, product_code, routing_id, created_at, updated_at \
             FROM bom_routing WHERE product_code = ANY($1)",
        )
        .bind(product_codes)
        .fetch_all(pool)
        .await?;
        Ok(items.into_iter().map(|b| (b.product_code.clone(), b)).collect())
    }

    /// 批量按 ID 查询路线
    pub async fn find_routing_by_ids(
        pool: &PgPool,
        ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Routing>> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let items: Vec<Routing> = sqlx::query_as(
            "SELECT id, name, description, created_at, updated_at \
             FROM routing WHERE id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?;
        Ok(items.into_iter().map(|r| (r.id, r)).collect())
    }

    /// 批量查询多个路线的工序列表
    pub async fn find_steps_by_routing_ids_batch(
        pool: &PgPool,
        routing_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<RoutingStep>>> {
        if routing_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let steps: Vec<RoutingStep> = sqlx::query_as(
            "SELECT id, routing_id, process_code, step_order, is_required, remark, created_at, updated_at \
             FROM routing_step WHERE routing_id = ANY($1) \
             ORDER BY step_order ASC, id ASC",
        )
        .bind(routing_ids)
        .fetch_all(pool)
        .await?;
        let mut map: std::collections::HashMap<i64, Vec<RoutingStep>> = std::collections::HashMap::new();
        for step in steps {
            map.entry(step.routing_id).or_default().push(step);
        }
        Ok(map)
    }
```

- [ ] **Step 2: 运行 clippy 验证**

Run: `cargo clippy -p abt`
Expected: No issues found

- [ ] **Step 3: Commit**

```bash
git add abt/src/repositories/routing_repo.rs
git commit -m "feat(routing): add batch query methods for import optimization"
```

---

### Task 2: 重写导入函数的路线校验循环

**Files:**
- Modify: `abt/src/implt/excel/labor_process_import.rs` (路线校验循环，约第 201-281 行)

将逐个查询的路线校验循环替换为批量预加载 + HashMap 查找。

- [ ] **Step 1: 在导入函数中添加批量预加载**

在 `// 工艺路线校验` 注释之前（BOM 校验之后），添加批量数据预加载：

```rust
        // 批量预加载路线数据（消除 N+1 查询）
        let bom_routing_map = RoutingRepo::find_bom_routing_batch(&self.pool, &product_codes).await?;
        let routing_ids: Vec<i64> = bom_routing_map.values().map(|b| b.routing_id).collect();
        let routing_map = RoutingRepo::find_routing_by_ids(&self.pool, &routing_ids).await?;
        let steps_map = RoutingRepo::find_steps_by_routing_ids_batch(&self.pool, &routing_ids).await?;
```

同时在文件顶部的 `use` 块中确保引入了 `RoutingRepo`：

```rust
use crate::repositories::{BomRepo, Executor, LaborProcessDictRepo, LaborProcessRepo, RoutingRepo};
```

- [ ] **Step 2: 重写校验循环为 HashMap 查找**

将原来的 `for pc in &product_codes { match self.routing_service.get_bom_routing(pc).await {` 循环替换为：

```rust
        // 工艺路线校验（使用预加载的批量数据）
        for pc in &product_codes {
            let rows_for_product = grouped.get(pc).unwrap();

            if let Some(binding) = bom_routing_map.get(pc) {
                let routing = match routing_map.get(&binding.routing_id) {
                    Some(r) => r,
                    None => {
                        // 绑定的路线已被删除
                        failure_count += 1;
                        results.push(row_error(0, String::new(), format!("产品 {} 绑定的路线已被删除", pc)));
                        products_to_skip.insert(pc.clone());
                        continue;
                    }
                };
                let routing_steps = steps_map.get(&binding.routing_id).cloned().unwrap_or_default();

                let mut product_has_error = false;

                let imported_codes: HashSet<&str> = rows_for_product
                    .iter()
                    .filter_map(|r| r.process_code.as_deref())
                    .collect();

                let missing_steps: Vec<&RoutingStep> = routing_steps
                    .iter()
                    .filter(|s| !imported_codes.contains(s.process_code.as_str()))
                    .collect();

                if !missing_steps.is_empty() {
                    for step in &missing_steps {
                        failure_count += 1;
                        results.push(row_error(0, format!("{} / {}", pc, step.process_code), format!(
                            "产品 {} 的路线 '{}' 包含工序 '{}' 但导入中缺失，请添加该工序（数量可为0）并在备注中说明原因",
                            pc, routing.name, step.process_code
                        )));
                    }
                    product_has_error = true;
                }

                if !product_has_error {
                    for row in rows_for_product {
                        if row.quantity == Decimal::ZERO {
                            let has_remark = row.remark
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false);
                            if !has_remark {
                                failure_count += 1;
                                results.push(row_error(row.row_number, row.name.clone(), format!(
                                    "产品 {} 的工序 '{}' 数量为 0，需要在备注中说明原因",
                                    pc, row.name
                                )));
                                product_has_error = true;
                            }
                        }
                    }
                }

                if product_has_error {
                    products_to_skip.insert(pc.clone());
                }
            } else {
                // 无绑定路线，尝试匹配
                let codes = unique_sorted_process_codes(rows_for_product);
                if !codes.is_empty() {
                    let matched = match self.routing_service.find_matching_routing(&codes).await {
                        Ok(m) => m,
                        Err(e) => {
                            failure_count += 1;
                            results.push(row_error(0, String::new(), format!("产品 {} 查询匹配路线失败: {}", pc, e)));
                            products_to_skip.insert(pc.clone());
                            continue;
                        }
                    };
                    if matched.is_none() {
                        failure_count += 1;
                        results.push(row_error(0, pc.clone(), format!(
                            "未找到匹配的工艺路线（工序编码: {}），请先在工艺路线管理中创建对应路线后再导入",
                            codes.join(", ")
                        )));
                        products_to_skip.insert(pc.clone());
                    }
                }
            }
        }
```

- [ ] **Step 3: 运行 clippy 验证**

Run: `cargo clippy -p abt`
Expected: No issues found

- [ ] **Step 4: Commit**

```bash
git add abt/src/implt/excel/labor_process_import.rs
git commit -m "perf(import): replace N+1 routing queries with batch preload in labor process import"
```

---

### Task 3: 添加连接池超时配置

**Files:**
- Modify: `abt-grpc/src/server.rs:28-31`

- [ ] **Step 1: 添加超时参数**

在 `server.rs` 的 `PgPoolOptions` 链中添加超时配置，同时在文件顶部添加 `use std::time::Duration;`：

在 `use` 块中添加：
```rust
use std::time::Duration;
```

将原来的：
```rust
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connection)
            .connect(&config.database_url)
            .await?;
```

替换为：
```rust
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connection)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(&config.database_url)
            .await?;
```

- [ ] **Step 2: 运行 clippy 验证**

Run: `cargo clippy -p abt-grpc`
Expected: No issues found

- [ ] **Step 3: Commit**

```bash
git add abt-grpc/src/server.rs
git commit -m "perf(pool): add acquire_timeout, idle_timeout, and max_lifetime to PgPool config"
```
