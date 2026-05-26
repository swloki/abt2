# 工序导入性能修复设计

## 背景

工序 Excel 导入（`labor_process_import.rs`）在工艺路线校验阶段存在 N+1 查询问题：每个产品单独调用 `get_bom_routing`，每次触发 2-3 条 SQL。100 个产品导致 200-300 次数据库查询。

同时，连接池初始化（`abt-grpc/src/server.rs`）只配置了 `max_connections`，缺少超时保护。长时间运行的导入事务可能占满连接池。

## 修复 1: N+1 查询

### 现状

```
labor_process_import.rs 校验循环:
  for pc in &product_codes {
    get_bom_routing(pc)  →  1) SELECT FROM bom_routing WHERE product_code = $1
                           2) SELECT FROM routing WHERE id = $1
                           3) SELECT FROM routing_step WHERE routing_id = $1
  }
```

N 个产品 → 最多 3N 次查询。

### 方案

在 `routing_repo.rs` 新增 3 个批量方法：

1. **`find_bom_routing_batch`** — `SELECT FROM bom_routing WHERE product_code = ANY($1)`，返回 `HashMap<String, BomRouting>`
2. **`find_routing_by_ids`** — `SELECT FROM routing WHERE id = ANY($1)`，返回 `HashMap<i64, Routing>`
3. **`find_steps_by_routing_ids_batch`** — `SELECT FROM routing_step WHERE routing_id = ANY($1)`，返回 `HashMap<i64, Vec<RoutingStep>>`

导入函数在循环前一次性调用这 3 个方法，拿到所有路线数据。校验循环内只做 HashMap 查找，零 SQL。

查询数从 3N 降到 **3**。

### 影响文件

- `abt/src/repositories/routing_repo.rs` — 新增 3 个批量方法
- `abt/src/implt/excel/labor_process_import.rs` — 重写校验循环，用批量数据替代逐个查询

## 修复 2: 连接池超时

### 现状

```rust
// abt-grpc/src/server.rs
PgPoolOptions::new()
    .max_connections(config.max_connection)
    .connect(&config.database_url)
    .await?;
```

无超时配置。

### 方案

添加三个连接池级别超时参数：

```rust
PgPoolOptions::new()
    .max_connections(config.max_connection)
    .acquire_timeout(Duration::from_secs(30))   // 获取连接超时
    .idle_timeout(Duration::from_secs(600))     // 空闲连接回收
    .max_lifetime(Duration::from_secs(1800))    // 连接最大存活
    .connect(&config.database_url)
    .await?;
```

### 影响文件

- `abt-grpc/src/server.rs` — 连接池配置添加超时参数

## 不做的事

- 不加 PostgreSQL 语句级 `statement_timeout`（属于运维配置，不在代码层）
- 不修改 `RoutingService` trait（批量方法只在 repository 层，服务层不需要新接口）
- 不修改非导入路径的调用方（其他使用 `get_bom_routing` 的地方保持不变）
