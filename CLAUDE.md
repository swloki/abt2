# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ABT is a BOM (Bill of Materials) and Inventory Management System built in Rust. It exposes a gRPC API backed by PostgreSQL, and the core library (`abt`) can also be used as a NAPI module for Node.js.

**注意：使用中文进行沟通**

## Build & Verification Commands

```bash
# Build all crates
cargo build

# Lint with clippy (主要验证代码正确性)
cargo clippy

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p abt
cargo test -p abt-grpc

# Run a single test
cargo test -p abt -- test_name
```

**注意：不要使用 `cargo run` 启动服务，服务已经在运行中。验证代码正确性主要使用 `cargo clippy`。**

**Required environment variable:** `DATABASE_URL` (PostgreSQL connection string). Optional: `GRPC_HOST` (default `0.0.0.0`), `GRPC_PORT` (default `8001`), `MAX_CONNECTION` (default `20`). A `.env` file in the `abt-grpc` directory is loaded via `dotenvy`.

## Architecture

### Workspace Structure

```
common/       — Shared type alias (PgExecutor for sqlx)
abt/          — Core business logic library (cdylib + rlib)
abt-grpc/     — gRPC server binary
proto/        — Protobuf service definitions
```

### Layered Design

Each feature follows a consistent four-layer pattern:

1. **Proto definition** (`proto/abt/v1/*.proto`) — gRPC messages and services
2. **Model** (`abt/src/models/`) — Rust structs mapped to/from database rows and proto messages
3. **Repository** (`abt/src/repositories/`) — SQL queries via sqlx (raw SQL, not an ORM)
4. **Service trait** (`abt/src/service/`) — async trait defining the business interface
5. **Service impl** (`abt/src/implt/`) — concrete implementation using repositories
6. **gRPC handler** (`abt-grpc/src/handlers/`) — translates proto requests to service calls, and model responses back to proto

Proto compilation is handled by `abt-grpc/build.rs`, which scans `proto/abt/v1/` and outputs to `abt-grpc/src/generated/`. Running `cargo build` regenerates these files automatically.

### Global State

- `abt::AppContext` holds the PostgreSQL connection pool, initialized once via `init_context_with_pool()`
- Service instances are created via factory functions in `abt/src/lib.rs` (e.g., `get_product_service(ctx)`)
- The Excel service is a global singleton (`OnceLock`) to maintain import progress state
- `abt-grpc::server::AppState` wraps `AppContext` and is accessed via `AppState::get().await`

### Database

PostgreSQL with sqlx (compile-time checked queries via `sqlx::query!` macro). Migrations are in `abt/migrations/` — these are plain SQL files numbered sequentially. Key schema patterns:
- JSONB columns for flexible metadata (e.g., `products.meta`, `boms.bom_detail`)
- Soft deletes via `deleted_at` timestamp columns
- `Decimal(10,6)` for financial/quantity precision
- Audit trails with `operator_id` tracking

### Key Conventions

- Error handling: `anyhow::Result<T>` throughout the service and repository layers
- All service traits use `#[async_trait]` from the `async-trait` crate
- `#![allow(non_snake_case)]` in `abt/src/lib.rs` — proto-generated names use CamelCase
- `abt-grpc` edition is 2021, `abt` edition is 2024
- The `common` crate provides a `PgExecutor` type alias for mutable `PgConnection` references
- gRPC reflection is enabled, so clients can introspect the API

### Documented Solutions

`docs/solutions/` — documented solutions to past problems (bugs, best practices, workflow patterns), organized by category with YAML frontmatter (`module`, `tags`, `problem_type`). Relevant when implementing or debugging in documented areas.

### Adding a New Feature

1. Add `.proto` definitions in `proto/abt/v1/`
2. Create model in `abt/src/models/`
3. Create repository in `abt/src/repositories/`
4. Define service trait in `abt/src/service/`
5. Implement service in `abt/src/implt/`
6. Add factory function in `abt/src/lib.rs`
7. Create handler in `abt-grpc/src/handlers/` (convert between proto and model types)
8. Register handler in `abt-grpc/src/server.rs`
9. Add database migration in `abt/migrations/`
