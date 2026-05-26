# abt → abt-core 模块迁移指南

> 将 gRPC handler 的内部实现从调 `abt` 换成调 `abt-core`，Proto 接口和前端不动。
> 分支：`feat/migrate-abt-to-abt-core`

## 核心原则

- **不改 Proto** — 保持现有接口定义和前端代码不变
- **只换 handler 内部** — handler 方法签名不变，内部从 `abt::XxxService` 改为 `abt_core::XxxService`
- **数据从 abt_v2 读** — 事务用 `begin_core_transaction()`，连接 abt_v2 数据库

## 前置条件

- `abt-grpc` 已依赖 `abt-core`
- `.env` 已配置 `ABT_CORE_DATABASE_URL` 指向 `abt_v2`
- `server.rs` 已有 `abt_core_pool`、`begin_core_transaction()`、`category_service()` 方法

## 每个模块的迁移步骤

### Step 1: 确认 abt-core 代码

检查 `abt-core/src/{模块}/` 下是否有完整的 model / repo / service / implt。没有则先补齐。

### Step 2: 建表（abt_v2）

在 `abt-core/migrations/` 新建 SQL，编号接续最大值。表结构对齐 `model.rs`。

```bash
psql -h 127.0.0.1 -U postgres -d abt_v2 -f abt-core/migrations/{NNN}_create_xxx.sql
```

### Step 3: 数据迁移脚本

`scripts/migrate-{module}.ts`，从 abt 读取写入 abt_v2。

```bash
PGPASSWORD=123456 PGUSER=postgres PGHOST=127.0.0.1 ABT_DB=abt ABT_V2_DB=abt_v2 \
  bun run scripts/migrate-{module}.ts
```

注意：PostgreSQL `BIGINT` 在 pg 库中返回 `string`，Map 的 key 需 `Number()` 转换。

### Step 4: 修改 Handler

打开现有 handler（如 `abt-grpc/src/handlers/term.rs`），做以下替换：

**替换前（调 abt）：**
```rust
let srv = state.term_service();                          // abt service
let mut tx = state.begin_transaction().await?;           // abt 数据库事务
srv.xxx(...).await?;                                     // abt 错误类型
```

**替换后（调 abt-core）：**
```rust
let srv = state.xxx_service();                           // abt-core service（需在 server.rs 添加）
let mut tx = state.begin_core_transaction().await?;      // abt_v2 数据库事务
let ctx = ServiceContext::new(&mut tx, operator_id);      // 传 ServiceContext
srv.xxx(ctx, ...).await.map_err(domain_to_status)?;      // DomainError → Status
```

需要额外导入：
```rust
use abt_core::{模块路径}::Service;
use abt_core::shared::types::ServiceContext;
```

### Step 5: 在 server.rs 添加 service 工厂方法

```rust
pub fn xxx_service(&self) -> impl abt_core::xxx::XxxService {
    // 构造 XxxServiceImpl，传入 repo + 依赖的共享服务
}
```

### Step 6: 验证

```bash
cargo clippy -p abt-grpc
```

## handler 改造模式参考

以 `term.rs` 为例，改造前后对比：

| 项 | 改造前（abt） | 改造后（abt-core） |
|----|-------------|-------------------|
| 事务 | `state.begin_transaction()` | `state.begin_core_transaction()` |
| 服务 | `state.term_service()` → `abt::TermService` | `state.category_service()` → `abt_core::CategoryService` |
| 上下文 | 直接传 `&mut tx` | `ServiceContext::new(&mut tx, operator_id)` |
| 错误映射 | `error::err_to_status` | `domain_to_status`（DomainError → tonic::Status） |
| Proto 响应 | handler 中做 model → proto 映射 | 同左，只是 model 来自 abt-core |

## DomainError → tonic::Status 映射

```rust
fn domain_to_status(e: DomainError) -> tonic::Status {
    match e {
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::Duplicate(msg) => tonic::Status::already_exists(msg),
        DomainError::PermissionDenied(msg) => tonic::Status::permission_denied(msg),
        DomainError::BusinessRule(msg) => tonic::Status::failed_precondition(msg),
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::ConcurrentConflict => tonic::Status::aborted("Concurrent conflict"),
        DomainError::Internal(e) => tonic::Status::internal(e.to_string()),
        other => tonic::Status::internal(other.to_string()),
    }
}
```

## 已完成迁移

### 分类（Category / Term）

- **改造 Handler**：`term.rs` — 内部改为调 `abt_core::CategoryService`
- **新增 Handler**：`category.rs` — 新的 `AbtCategoryService`（9 RPC）
- **新 Proto**：`category.proto`（额外新增，不影响旧接口）
- **新表**：`abt_v2.categories`（138 条）+ `abt_v2.product_categories`（9,940 条）
- **迁移脚本**：`scripts/migrate-categories.ts`
- **迁移 SQL**：`abt-core/migrations/008_create_categories.sql`

## 待迁移模块

- [ ] Product（产品）
- [ ] BOM（物料清单）
- [ ] Warehouse / Location（仓库/库位）
- [ ] Inventory（库存）
- [ ] Price（价格）
- [ ] Labor Process（工序）
- [ ] Routing（工艺路线）
