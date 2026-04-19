# BOM 分类功能实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 BOM 增加分类属性，提供 BOM 分类的 CRUD 操作

**Architecture:** 新建 `bom_category` 表，在 `bom` 表添加外键引用，通过 gRPC 服务暴露 CRUD 接口

**Tech Stack:** Rust, PostgreSQL, gRPC (tonic), sqlx

---

## 文件结构

```
abt/src/
├── models/
│   └── bom_category.rs              # [新建] BOM 分类模型
├── repositories/
│   └── bom_category_repo.rs         # [新建] BOM 分类数据访问
├── service/
│   └── bom_category_service.rs      # [新建] BOM 分类业务接口
├── implt/
│   └── bom_category_impl.rs        # [新建] BOM 分类服务实现
│   └── mod.rs                      # [修改] 添加 bom_category_impl 导出

proto/abt/v1/
└── bom_category.proto               # [新建] Proto 定义

abt/migrations/
├── 023_create_bom_category_table.sql # [新建] 创建 bom_category 表
└── 024_add_bom_category_to_bom.sql   # [新建] 为 bom 表添加 bom_category_id 列

abt-grpc/src/
├── handlers/
│   └── bom_category_handler.rs      # [新建] gRPC 处理器
│   └── mod.rs                      # [修改] 添加 bom_category 导出
└── generated/                       # [自动生成] abt/v1/bom_category.rs
└── server.rs                       # [修改] 注册 BomCategoryService

abt/src/
├── lib.rs                          # [修改] 添加工厂函数
├── models/mod.rs                   # [修改] 导出 BomCategory
├── repositories/mod.rs             # [修改] 导出 BomCategoryRepo
└── service/mod.rs                  # [修改] 导出 BomCategoryService
```

---

## Task 1: 创建 Proto 定义

**Files:**
- Create: `proto/abt/v1/bom_category.proto`

- [ ] **Step 1: 创建 proto 文件**

```proto
syntax = "proto3";
package abt.v1;

import "abt/v1/base.proto";

option go_package = "abt/v1";

service AbtBomCategoryService {
  rpc ListBomCategories(ListBomCategoriesRequest) returns (BomCategoryListResponse);
  rpc GetBomCategory(GetBomCategoryRequest) returns (BomCategoryResponse);
  rpc CreateBomCategory(CreateBomCategoryRequest) returns (U64Response);
  rpc UpdateBomCategory(UpdateBomCategoryRequest) returns (BoolResponse);
  rpc DeleteBomCategory(DeleteBomCategoryRequest) returns (BoolResponse);
}

message BomCategoryResponse {
  int64 bom_category_id = 1;
  string bom_category_name = 2;
  int64 created_at = 3;
}

message BomCategoryListResponse {
  repeated BomCategoryResponse items = 1;
  uint64 total = 2;
}

message ListBomCategoriesRequest {
  optional uint32 page = 1;
  optional uint32 page_size = 2;
  optional string keyword = 3;
}

message GetBomCategoryRequest {
  int64 bom_category_id = 1;
}

message CreateBomCategoryRequest {
  string bom_category_name = 1;
}

message UpdateBomCategoryRequest {
  int64 bom_category_id = 1;
  string bom_category_name = 2;
}

message DeleteBomCategoryRequest {
  int64 bom_category_id = 1;
}
```

- [ ] **Step 2: 运行 cargo build 生成代码**

```bash
cargo build
```

Expected: 编译成功，生成 `abt-grpc/src/generated/abt/v1/bom_category.rs`

- [ ] **Step 3: Commit**

```bash
git add proto/abt/v1/bom_category.proto
git commit -m "proto: add bom_category service definition"
```

---

## Task 2: 创建数据库迁移

**Files:**
- Create: `abt/migrations/023_create_bom_category_table.sql`
- Create: `abt/migrations/024_add_bom_category_to_bom.sql`

- [ ] **Step 1: 创建 bom_category 表迁移**

```sql
-- 创建 BOM 分类表
CREATE TABLE IF NOT EXISTS bom_category (
    bom_category_id BIGSERIAL PRIMARY KEY,
    bom_category_name VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_bom_category_name ON bom_category(bom_category_name);
```

- [ ] **Step 2: 创建 bom 表添加外键迁移**

```sql
-- 为 bom 表添加 bom_category_id 列
ALTER TABLE bom
ADD COLUMN IF NOT EXISTS bom_category_id BIGINT REFERENCES bom_category(bom_category_id) ON DELETE SET NULL;

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_bom_category_id ON bom(bom_category_id);
```

- [ ] **Step 3: Commit**

```bash
git add abt/migrations/023_create_bom_category_table.sql abt/migrations/024_add_bom_category_to_bom.sql
git commit -m "db: add bom_category table and bom.bom_category_id column"
```

---

## Task 3: 创建 BOM 分类模型

**Files:**
- Create: `abt/src/models/bom_category.rs`
- Modify: `abt/src/models/mod.rs`

- [ ] **Step 1: 创建模型文件**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BomCategory {
    pub bom_category_id: i64,
    pub bom_category_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBomCategoryRequest {
    pub bom_category_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBomCategoryRequest {
    pub bom_category_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomCategoryQuery {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl Default for BomCategoryQuery {
    fn default() -> Self {
        Self {
            keyword: None,
            page: Some(1),
            page_size: Some(20),
        }
    }
}
```

- [ ] **Step 2: 修改 models/mod.rs**

在 `mod bom;` 之后添加：

```rust
mod bom_category;
```

在 `pub use department::*;` 之后添加：

```rust
pub use bom_category::*;
```

- [ ] **Step 3: Commit**

```bash
git add abt/src/models/bom_category.rs abt/src/models/mod.rs
git commit -m "model: add BomCategory"
```

---

## Task 4: 创建 BOM 分类 Repository

**Files:**
- Create: `abt/src/repositories/bom_category_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

- [ ] **Step 1: 创建 repository 文件**

```rust
use anyhow::Result;
use sqlx::PgPool;

use crate::models::{BomCategory, BomCategoryQuery, CreateBomCategoryRequest, UpdateBomCategoryRequest};
use crate::repositories::Executor;

pub struct BomCategoryRepo;

impl BomCategoryRepo {
    pub async fn insert(
        executor: Executor<'_>,
        req: &CreateBomCategoryRequest,
    ) -> Result<i64> {
        let bom_category_id = sqlx::query_scalar!(
            r#"
            INSERT INTO bom_category (bom_category_name)
            VALUES ($1)
            RETURNING bom_category_id
            "#,
            req.bom_category_name
        )
        .fetch_one(executor)
        .await?;

        Ok(bom_category_id)
    }

    pub async fn update(
        executor: Executor<'_>,
        bom_category_id: i64,
        req: &UpdateBomCategoryRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE bom_category
            SET bom_category_name = $2
            WHERE bom_category_id = $1
            "#,
            bom_category_id,
            req.bom_category_name
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_>, bom_category_id: i64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM bom_category WHERE bom_category_id = $1",
            bom_category_id
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, bom_category_id: i64) -> Result<Option<BomCategory>> {
        let category = sqlx::query_as!(
            BomCategory,
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE bom_category_id = $1
            "#,
            bom_category_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(category)
    }

    pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<BomCategory>> {
        let category = sqlx::query_as!(
            BomCategory,
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE bom_category_name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(category)
    }

    pub async fn query(pool: &PgPool, query: &BomCategoryQuery) -> Result<Vec<BomCategory>> {
        let mut sql_query = sqlx::QueryBuilder::new(
            r#"
            SELECT bom_category_id, bom_category_name, created_at
            FROM bom_category
            WHERE 1=1
            "#
        );

        if let Some(keyword) = &query.keyword {
            if !keyword.is_empty() {
                sql_query.push(" AND bom_category_name ILIKE ");
                sql_query.push_bind(format!("%{}%", keyword));
            }
        }

        sql_query.push(" ORDER BY bom_category_id DESC");

        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);

        sql_query.push(" LIMIT ");
        sql_query.push_bind(page_size as i32);
        sql_query.push(" OFFSET ");
        sql_query.push_bind(((page - 1) * page_size) as i32);

        let categories = sql_query.build_query_as::<BomCategory>().fetch_all(pool).await?;

        Ok(categories)
    }

    pub async fn query_count(pool: &PgPool, query: &BomCategoryQuery) -> Result<i64> {
        let mut sql_query = sqlx::QueryBuilder::new(
            "SELECT count(*) FROM bom_category WHERE 1=1"
        );

        if let Some(keyword) = &query.keyword {
            if !keyword.is_empty() {
                sql_query.push(" AND bom_category_name ILIKE ");
                sql_query.push_bind(format!("%{}%", keyword));
            }
        }

        let count: i64 = sql_query.build_query_scalar().fetch_one(pool).await?;

        Ok(count)
    }

    pub async fn is_name_exists(pool: &PgPool, name: &str) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom_category WHERE bom_category_name = $1)",
        )
        .bind(name)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    pub async fn has_boms(pool: &PgPool, bom_category_id: i64) -> Result<bool> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM bom WHERE bom_category_id = $1)",
        )
        .bind(bom_category_id)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }
}
```

- [ ] **Step 2: 修改 repositories/mod.rs**

在 `mod bom_repo;` 之后添加：

```rust
mod bom_category_repo;
```

在 `pub use bom_repo::...` 之后添加：

```rust
pub use bom_category_repo::BomCategoryRepo;
```

- [ ] **Step 3: Commit**

```bash
git add abt/src/repositories/bom_category_repo.rs abt/src/repositories/mod.rs
git commit -m "repo: add BomCategoryRepo"
```

---

## Task 5: 创建 BOM 分类 Service

**Files:**
- Create: `abt/src/service/bom_category_service.rs`
- Modify: `abt/src/service/mod.rs`

- [ ] **Step 1: 创建 service trait 文件**

```rust
use anyhow::Result;
use async_trait::async_trait;

use crate::models::{BomCategory, BomCategoryQuery, CreateBomCategoryRequest, UpdateBomCategoryRequest};
use crate::repositories::Executor;

#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(
        &self,
        req: CreateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<i64>;

    async fn update(
        &self,
        bom_category_id: i64,
        req: UpdateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn delete(
        &self,
        bom_category_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn get(&self, bom_category_id: i64) -> Result<Option<BomCategory>>;

    async fn list(&self, query: BomCategoryQuery) -> Result<(Vec<BomCategory>, i64)>;

    async fn exists_name(&self, name: &str) -> Result<bool>;

    async fn has_boms(&self, bom_category_id: i64) -> Result<bool>;
}
```

- [ ] **Step 2: 修改 service/mod.rs**

在 `mod bom_service;` 之后添加：

```rust
mod bom_category_service;
```

在 `pub use bom_service::BomService;` 之后添加：

```rust
pub use bom_category_service::BomCategoryService;
```

- [ ] **Step 3: Commit**

```bash
git add abt/src/service/bom_category_service.rs abt/src/service/mod.rs
git commit -m "service: add BomCategoryService trait"
```

---

## Task 6: 创建 BOM 分类 Service 实现

**Files:**
- Create: `abt/src/implt/bom_category_impl.rs`
- Modify: `abt/src/implt/mod.rs`

- [ ] **Step 1: 创建 service impl 文件**

```rust
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::{BomCategoryRepo, Executor};
use crate::service::BomCategoryService;

pub struct BomCategoryServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl BomCategoryServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BomCategoryService for BomCategoryServiceImpl {
    async fn create(
        &self,
        req: CreateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<i64> {
        // Check if name already exists
        if BomCategoryRepo::is_name_exists(self.pool.as_ref(), &req.bom_category_name).await? {
            return Err(anyhow!("BOM category name already exists: {}", req.bom_category_name));
        }

        let bom_category_id = BomCategoryRepo::insert(executor, &req).await?;
        Ok(bom_category_id)
    }

    async fn update(
        &self,
        bom_category_id: i64,
        req: UpdateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        // Check if category exists
        let existing = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id)
            .await?
            .ok_or_else(|| anyhow!("BOM category not found"))?;

        // Check if new name conflicts with another category
        if req.bom_category_name != existing.bom_category_name {
            if BomCategoryRepo::is_name_exists(self.pool.as_ref(), &req.bom_category_name).await? {
                return Err(anyhow!("BOM category name already exists: {}", req.bom_category_name));
            }
        }

        BomCategoryRepo::update(executor, bom_category_id, &req).await?;
        Ok(())
    }

    async fn delete(
        &self,
        bom_category_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        // Check if category exists
        let existing = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id)
            .await?
            .ok_or_else(|| anyhow!("BOM category not found"))?;

        // Check if there are BOMs using this category
        if BomCategoryRepo::has_boms(self.pool.as_ref(), bom_category_id).await? {
            return Err(anyhow!("Cannot delete BOM category: there are BOMs using this category"));
        }

        BomCategoryRepo::delete(executor, bom_category_id).await?;
        Ok(())
    }

    async fn get(&self, bom_category_id: i64) -> Result<Option<BomCategory>> {
        let category = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id).await?;
        Ok(category)
    }

    async fn list(&self, query: BomCategoryQuery) -> Result<(Vec<BomCategory>, i64)> {
        let categories = BomCategoryRepo::query(self.pool.as_ref(), &query).await?;
        let total = BomCategoryRepo::query_count(self.pool.as_ref(), &query).await?;
        Ok((categories, total))
    }

    async fn exists_name(&self, name: &str) -> Result<bool> {
        let exists = BomCategoryRepo::is_name_exists(self.pool.as_ref(), name).await?;
        Ok(exists)
    }

    async fn has_boms(&self, bom_category_id: i64) -> Result<bool> {
        let has = BomCategoryRepo::has_boms(self.pool.as_ref(), bom_category_id).await?;
        Ok(has)
    }
}
```

- [ ] **Step 2: 修改 implt/mod.rs**

在 `mod department_service_impl;` 之后添加：

```rust
mod bom_category_impl;
```

在 `pub use department_service_impl::DepartmentServiceImpl;` 之后添加：

```rust
pub use bom_category_impl::BomCategoryServiceImpl;
```

- [ ] **Step 3: Commit**

```bash
git add abt/src/implt/bom_category_impl.rs abt/src/implt/mod.rs
git commit -m "impl: add BomCategoryServiceImpl"
```

---

## Task 7: 在 lib.rs 中添加工厂函数

**Files:**
- Modify: `abt/src/lib.rs`

- [ ] **Step 1: 添加工厂函数**

在 `pub fn get_bom_service` 附近添加：

```rust
/// 获取 BOM 分类服务
pub fn get_bom_category_service(ctx: &AppContext) -> impl crate::service::BomCategoryService {
    crate::implt::BomCategoryServiceImpl::new(Arc::new(ctx.pool().clone()))
}
```

- [ ] **Step 2: Commit**

```bash
git add abt/src/lib.rs
git commit -m "lib: add get_bom_category_service factory function"
```

---

## Task 8: 创建 gRPC Handler

**Files:**
- Create: `abt-grpc/src/handlers/bom_category_handler.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`

- [ ] **Step 1: 创建 handler 文件**

```rust
//! BomCategory gRPC Handler

use crate::generated::abt::v1::{
    bom_category_service_server::BomCategoryService as GrpcBomCategoryService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use abt::{BomCategoryQuery, BomCategoryService};

pub struct BomCategoryHandler;

impl BomCategoryHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BomCategoryHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcBomCategoryService for BomCategoryHandler {
    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_bom_category(
        &self,
        request: Request<CreateBomCategoryRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateBomCategoryRequest {
            bom_category_name: req.bom_category_name,
        };

        let bom_category_id = srv
            .create(create_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: bom_category_id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_bom_category(
        &self,
        request: Request<UpdateBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateBomCategoryRequest {
            bom_category_name: req.bom_category_name,
        };

        srv.update(req.bom_category_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn delete_bom_category(
        &self,
        request: Request<DeleteBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete(req.bom_category_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_bom_category(
        &self,
        request: Request<GetBomCategoryRequest>,
    ) -> GrpcResult<BomCategoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let category = srv
            .get(req.bom_category_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BomCategory", &req.bom_category_id.to_string()))?;

        Ok(Response::new(category.into()))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn list_bom_categories(
        &self,
        request: Request<ListBomCategoriesRequest>,
    ) -> GrpcResult<BomCategoryListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let query = abt::BomCategoryQuery {
            keyword: if req.keyword.is_empty() { None } else { req.keyword },
            page: req.page,
            page_size: req.page_size,
        };

        let (categories, total) = srv
            .list(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomCategoryListResponse {
            items: categories.into_iter().map(|c| c.into()).collect(),
            total: total as u64,
        }))
    }
}
```

- [ ] **Step 2: 修改 handlers/mod.rs**

在 `pub mod department;` 之后添加：

```rust
pub mod bom_category;
```

在 `abt_bom_service_server::AbtBomServiceServer` 之后添加：

```rust
    bom_category_service_server::BomCategoryServiceServer,
```

- [ ] **Step 3: Commit**

```bash
git add abt-grpc/src/handlers/bom_category_handler.rs abt-grpc/src/handlers/mod.rs
git commit -m "handler: add BomCategoryHandler"
```

---

## Task 9: 在 Server 中注册服务

**Files:**
- Modify: `abt-grpc/src/server.rs`

- [ ] **Step 1: 添加 BomCategoryService getter**

在 `department_service` getter 之后添加：

```rust
pub fn bom_category_service(&self) -> impl abt::BomCategoryService {
    abt::get_bom_category_service(self.abt_context)
}
```

- [ ] **Step 2: 在 start_server 中注册服务**

在 `use crate::handlers::{...}` 中添加 `BomCategoryServiceServer`：

```rust
use crate::handlers::{
    ...
    BomCategoryServiceServer,
};
```

在 server builder 中添加服务注册：

```rust
.add_service(AbtBomServiceServer::with_interceptor(
    crate::handlers::bom::BomHandler::new(), auth_interceptor,
))
// ... 其他服务 ...
.add_service(BomCategoryServiceServer::with_interceptor(
    crate::handlers::bom_category::BomCategoryHandler::new(), auth_interceptor,
))
```

- [ ] **Step 3: 运行 cargo build 验证**

```bash
cargo build
```

Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add abt-grpc/src/server.rs
git commit -m "server: register BomCategoryService"
```

---

## Task 10: 为 ListBoms 添加 category_id 过滤（可选，后续）

**Files:**
- Modify: `abt/src/models/bom.rs`
- Modify: `abt/src/repositories/bom_repo.rs`
- Modify: `proto/abt/v1/bom.proto`
- Modify: `abt-grpc/src/handlers/bom.rs`

注：此任务可选，BOM 的 bom_category_id 列已添加，但列表过滤功能可后续迭代实现。

---

## 验证清单

- [ ] `cargo build` 编译通过
- [ ] `cargo test` 所有测试通过
- [ ] 服务启动正常
- [ ] 可以通过 gRPC 创建/查询/删除 BOM 分类
- [ ] 删除有关联 BOM 的分类时返回错误
