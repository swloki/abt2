# CLAUDE.md

## Constraints（必须遵守）

- **使用中文沟通**
- **禁止修改前端代码**（`E:\work\front\abt_front` 目录下所有文件），只改后端
- **不要用 `cargo run` 启动服务**，服务已在运行中。验证代码正确性主要用 `cargo clippy`

## Project Overview

ABT 是 BOM（物料清单）和库存管理系统，基于 Rust 构建。对外暴露 gRPC API，底层数据库为 PostgreSQL。核心库 `abt` 也可作为 NAPI 模块供 Node.js 使用。

**环境变量：** `DATABASE_URL`（必须，PostgreSQL 连接串）。可选：`GRPC_HOST`（默认 `0.0.0.0`）、`GRPC_PORT`（默认 `8001`）、`MAX_CONNECTION`（默认 `20`）。`abt-grpc` 目录下的 `.env` 文件通过 `dotenvy` 加载。

## Design Principle: Interface & Model First（接口与模型先行）

新功能开发强制执行以下顺序，不可跳步：

**接口 + 模型设计 → 评审确认 → 交互设计 → 实现**

1. **接口先行** — 先定义清晰稳定的 API 契约（Proto 定义），确认后不可随意变更
2. **模型先行** — 同步设计领域模型（请求/响应结构、实体、值对象），语义清晰、边界明确、职责单一
3. **基于接口设计交互** — 禁止在接口未定义时设计前端交互或 UI
4. **文档化** — 接口和模型以 Proto 定义作为文档，是系统的骨架和共享语言

## Build & Verification

```bash
cargo build          # 构建所有 crate（同时重新生成 proto 代码）
cargo clippy         # 主要验证手段
cargo test           # 运行所有测试
cargo test -p abt    # 运行 abt crate 测试
cargo test -p abt-grpc # 运行 abt-grpc crate 测试
cargo test -p abt -- test_name  # 运行单个测试
```

## Architecture

### Workspace Structure

```
common/       — 共享类型别名（PgExecutor for sqlx）
abt/          — 核心业务逻辑库（cdylib + rlib）
abt-grpc/     — gRPC 服务端
proto/        — Protobuf 服务定义
```

### Layered Design

每个功能遵循以下分层模式：

1. **Proto definition** (`proto/abt/v1/*.proto`) — gRPC 消息和服务定义
2. **Model** (`abt/src/models/`) — Rust 结构体，映射数据库行和 Proto 消息
3. **Repository** (`abt/src/repositories/`) — 通过 sqlx 执行原始 SQL
4. **Service trait** (`abt/src/service/`) — 定义业务接口的 async trait
5. **Service impl** (`abt/src/implt/`) — 基于 repository 的具体实现
6. **gRPC handler** (`abt-grpc/src/handlers/`) — Proto 请求与 Service 调用之间的转换

Proto 编译由 `abt-grpc/build.rs` 处理，扫描 `proto/abt/v1/` 并输出到 `abt-grpc/src/generated/`。`cargo build` 会自动重新生成。

### Global State

- `abt::AppContext` 持有 PostgreSQL 连接池，通过 `init_context_with_pool()` 一次性初始化
- Service 实例通过 `abt/src/lib.rs` 中的工厂函数创建（如 `get_product_service(ctx)`）
- Excel service 是全局单例（`OnceLock`），用于维护导入进度状态
- `abt-grpc::server::AppState` 包装 `AppContext`，通过 `AppState::get().await` 访问

### Database

PostgreSQL + sqlx（通过 `sqlx::query!` 宏实现编译期检查）。Migration 在 `abt/migrations/` 中，按序编号的纯 SQL 文件。

- JSONB 列用于灵活元数据（如 `products.meta`、`boms.bom_detail`）
- 通过 `deleted_at` 时间戳实现软删除
- `Decimal(10,6)` 用于财务/数量精度
- 通过 `operator_id` 追踪操作审计

### Key Conventions

- 错误处理：service 和 repository 层统一使用 `anyhow::Result<T>`
- 所有 service trait 使用 `async-trait` crate 的 `#[async_trait]`
- `abt/src/lib.rs` 中 `#![allow(non_snake_case)]` — Proto 生成的名称使用 CamelCase
- `abt-grpc` edition 为 2021，`abt` edition 为 2024
- `common` crate 提供 `PgExecutor` 类型别名，即可变 `PgConnection` 引用
- 已启用 gRPC reflection，客户端可内省 API

### Documented Solutions

`docs/solutions/` — 记录历史问题的解决方案（bug、最佳实践、工作流模式），按类别组织，使用 YAML frontmatter（`module`、`tags`、`problem_type`）。在已记录的领域实现或调试时参考。

### Adding a New Feature

1. 在 `proto/abt/v1/` 添加 `.proto` 定义
2. 在 `abt/src/models/` 创建 model
3. 在 `abt/src/repositories/` 创建 repository
4. 在 `abt/src/service/` 定义 service trait
5. 在 `abt/src/implt/` 实现 service
6. 在 `abt/src/lib.rs` 添加工厂函数
7. 在 `abt-grpc/src/handlers/` 创建 handler（Proto 与 Model 类型互转）
8. 在 `abt-grpc/src/server.rs` 注册 handler
9. 在 `abt/migrations/` 添加数据库迁移
