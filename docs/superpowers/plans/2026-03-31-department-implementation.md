# 部门功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 ABT 系统增加部门功能，实现数据可见性控制：用户只能看到其所属部门的资源数据

**Architecture:** 部门模块独立，新增 departments 表和 user_departments 关联表，资源表增加 department_id 字段。查询时注入部门过滤条件，超级管理员跳过限制。

**Tech Stack:** Rust (sqlx, PostgreSQL), gRPC, Protobuf

---

## 文件结构

```
abt/
├── migrations/
│   └── 011_add_department_tables.sql              # 新增：部门表+关联表
├── src/
│   ├── models/
│   │   ├── department.rs                          # 新增：部门模型
│   │   └── mod.rs                                 # 更新：导出 department
│   ├── repositories/
│   │   ├── department_repo.rs                     # 新增：部门仓储层
│   │   └── mod.rs                                 # 更新：导出 department_repo
│   ├── service/
│   │   ├── department_service.rs                  # 新增：部门服务接口
│   │   └── mod.rs                                 # 更新：导出 department_service
│   └── implt/
│       ├── department_service_impl.rs             # 新增：部门服务实现
│       └── mod.rs                                 # 更新：导出 department_service_impl
│
abt-grpc/
├── proto/abt/v1/
│   └── department.proto                           # 新增：部门 gRPC 定义
└── src/handlers/
    ├── department.rs                              # 新增：部门 handler
    └── mod.rs                                     # 更新：导出 department handler
```

---

## Phase 1: 部门基础

### Task 1: 创建数据库迁移文件

**Files:**
- Create: `abt/migrations/011_add_department_tables.sql`

- [ ] **Step 1: 创建迁移 SQL**

```sql
-- 部门表
CREATE TABLE departments (
    department_id BIGSERIAL PRIMARY KEY,
    department_name VARCHAR(100) NOT NULL,
    department_code VARCHAR(50) UNIQUE NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- 用户-部门关联表
CREATE TABLE user_departments (
    user_id BIGINT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    department_id BIGINT NOT NULL REFERENCES departments(department_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, department_id)
);

-- 索引
CREATE INDEX idx_user_departments_user ON user_departments(user_id);
CREATE INDEX idx_user_departments_department ON user_departments(department_id);
```

- [ ] **Step 2: 创建 resources 表增加 department_id 字段的迁移 SQL**

```sql
-- 在现有 resources 表增加 department_id 字段
ALTER TABLE resources ADD COLUMN department_id BIGINT REFERENCES departments(department_id);
CREATE INDEX idx_resources_department ON resources(department_id);
```

---

### Task 2: 创建 Department Model

**Files:**
- Create: `abt/src/models/department.rs`
- Modify: `abt/src/models/mod.rs`

- [ ] **Step 1: 创建 department.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Department {
    pub department_id: i64,
    pub department_name: String,
    pub department_code: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Department {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Department {
            department_id: row.try_get("department_id")?,
            department_name: row.try_get("department_name")?,
            department_code: row.try_get("department_code")?,
            description: row.try_get("description")?,
            is_active: row.try_get("is_active")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    pub department_name: String,
    pub department_code: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateDepartmentRequest {
    pub department_name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}
```

- [ ] **Step 2: 更新 mod.rs 导出 department**

```rust
pub mod department;
pub use department::*;
```

---

### Task 3: 创建 Department Repository

**Files:**
- Create: `abt/src/repositories/department_repo.rs`
- Modify: `abt/src/repositories/mod.rs`

- [ ] **Step 1: 创建 department_repo.rs**

```rust
use crate::models::department::{CreateDepartmentRequest, Department, UpdateDepartmentRequest};
use sqlx::{PgPool, Result};

pub struct DepartmentRepository;

impl DepartmentRepository {
    pub async fn create(
        pool: &PgPool,
        req: &CreateDepartmentRequest,
    ) -> Result<Department> {
        let department = sqlx::query_as!(
            Department,
            r#"
            INSERT INTO departments (department_name, department_code, description)
            VALUES ($1, $2, $3)
            RETURNING department_id, department_name, department_code, description,
                      is_active, created_at, updated_at
            "#,
            req.department_name,
            req.department_code,
            req.description
        )
        .fetch_one(pool)
        .await?;
        Ok(department)
    }

    pub async fn update(
        pool: &PgPool,
        department_id: i64,
        req: &UpdateDepartmentRequest,
    ) -> Result<Option<Department>> {
        sqlx::query_as!(
            Department,
            r#"
            UPDATE departments
            SET department_name = COALESCE($2, department_name),
                description = COALESCE($3, description),
                is_active = COALESCE($4, is_active),
                updated_at = NOW()
            WHERE department_id = $1
            RETURNING department_id, department_name, department_code, description,
                      is_active, created_at, updated_at
            "#,
            department_id,
            req.department_name,
            req.description,
            req.is_active
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, department_id: i64) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM departments WHERE department_id = $1", department_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_by_id(pool: &PgPool, department_id: i64) -> Result<Option<Department>> {
        sqlx::query_as!(
            Department,
            r#"
            SELECT department_id, department_name, department_code, description,
                   is_active, created_at, updated_at
            FROM departments
            WHERE department_id = $1
            "#,
            department_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn list(pool: &PgPool, include_inactive: bool) -> Result<Vec<Department>> {
        if include_inactive {
            sqlx::query_as!(
                Department,
                r#"
                SELECT department_id, department_name, department_code, description,
                       is_active, created_at, updated_at
                FROM departments
                ORDER BY department_id
                "#
            )
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as!(
                Department,
                r#"
                SELECT department_id, department_name, department_code, description,
                       is_active, created_at, updated_at
                FROM departments
                WHERE is_active = true
                ORDER BY department_id
                "#
            )
            .fetch_all(pool)
            .await
        }
    }

    pub async fn get_user_departments(pool: &PgPool, user_id: i64) -> Result<Vec<Department>> {
        sqlx::query_as!(
            Department,
            r#"
            SELECT d.department_id, d.department_name, d.department_code, d.description,
                   d.is_active, d.created_at, d.updated_at
            FROM departments d
            JOIN user_departments ud ON d.department_id = ud.department_id
            WHERE ud.user_id = $1
            ORDER BY d.department_id
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn assign_departments(
        pool: &PgPool,
        user_id: i64,
        department_ids: &[i64],
    ) -> Result<()> {
        for department_id in department_ids {
            sqlx::query!(
                r#"
                INSERT INTO user_departments (user_id, department_id)
                VALUES ($1, $2)
                ON CONFLICT (user_id, department_id) DO NOTHING
                "#,
                user_id,
                department_id
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_departments(
        pool: &PgPool,
        user_id: i64,
        department_ids: &[i64],
    ) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM user_departments
            WHERE user_id = $1 AND department_id = ANY($2)
            "#,
            user_id,
            department_ids
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_department_ids(pool: &PgPool, user_id: i64) -> Result<Vec<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT department_id
            FROM user_departments
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
    }
}
```

- [ ] **Step 2: 更新 mod.rs**

```rust
pub mod department_repo;
pub use department_repo::*;
```

---

### Task 4: 创建 Department Service

**Files:**
- Create: `abt/src/service/department_service.rs`
- Modify: `abt/src/service/mod.rs`

- [ ] **Step 1: 创建 department_service.rs**

```rust
use crate::models::department::{CreateDepartmentRequest, Department, UpdateDepartmentRequest};
use crate::repositories::department_repo::DepartmentRepository;
use sqlx::PgPool;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait DepartmentServiceTrait: Send + Sync {
    async fn create(&self, req: CreateDepartmentRequest) -> Result<Department, String>;
    async fn update(&self, department_id: i64, req: UpdateDepartmentRequest) -> Result<Department, String>;
    async fn delete(&self, department_id: i64) -> Result<bool, String>;
    async fn get_by_id(&self, department_id: i64) -> Result<Option<Department>, String>;
    async fn list(&self, include_inactive: bool) -> Result<Vec<Department>, String>;
    async fn get_user_departments(&self, user_id: i64) -> Result<Vec<Department>, String>;
    async fn assign_departments(&self, user_id: i64, department_ids: Vec<i64>) -> Result<(), String>;
    async fn remove_departments(&self, user_id: i64, department_ids: Vec<i64>) -> Result<(), String>;
}

pub struct DepartmentService {
    pool: Arc<PgPool>,
}

impl DepartmentService {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl DepartmentServiceTrait for DepartmentService {
    async fn create(&self, req: CreateDepartmentRequest) -> Result<Department, String> {
        DepartmentRepository::create(&self.pool, &req)
            .await
            .map_err(|e| e.to_string())
    }

    async fn update(&self, department_id: i64, req: UpdateDepartmentRequest) -> Result<Department, String> {
        DepartmentRepository::update(&self.pool, department_id, &req)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Department not found".to_string())
    }

    async fn delete(&self, department_id: i64) -> Result<bool, String> {
        DepartmentRepository::delete(&self.pool, department_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_by_id(&self, department_id: i64) -> Result<Option<Department>, String> {
        DepartmentRepository::get_by_id(&self.pool, department_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn list(&self, include_inactive: bool) -> Result<Vec<Department>, String> {
        DepartmentRepository::list(&self.pool, include_inactive)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_user_departments(&self, user_id: i64) -> Result<Vec<Department>, String> {
        DepartmentRepository::get_user_departments(&self.pool, user_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn assign_departments(&self, user_id: i64, department_ids: Vec<i64>) -> Result<(), String> {
        DepartmentRepository::assign_departments(&self.pool, user_id, &department_ids)
            .await
            .map_err(|e| e.to_string())
    }

    async fn remove_departments(&self, user_id: i64, department_ids: Vec<i64>) -> Result<(), String> {
        DepartmentRepository::remove_departments(&self.pool, user_id, &department_ids)
            .await
            .map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 2: 更新 mod.rs**

```rust
pub mod department_service;
pub use department_service::*;
```

---

### Task 5: 创建 Department Service Implementation

**Files:**
- Create: `abt/src/implt/department_service_impl.rs`
- Modify: `abt/src/implt/mod.rs`

- [ ] **Step 1: 更新 mod.rs 导出**

```rust
pub mod department_service_impl;
pub use department_service_impl::*;
```

---

### Task 6: 创建 Department Proto

**Files:**
- Create: `abt-grpc/proto/abt/v1/department.proto`

- [ ] **Step 1: 创建 department.proto**

```protobuf
syntax = "proto3";
package abt.v1;

service DepartmentService {
    rpc CreateDepartment(CreateDepartmentRequest) returns (DepartmentResponse);
    rpc UpdateDepartment(UpdateDepartmentRequest) returns (DepartmentResponse);
    rpc DeleteDepartment(DeleteDepartmentRequest) returns (BoolResponse);
    rpc GetDepartment(GetDepartmentRequest) returns (DepartmentResponse);
    rpc ListDepartments(ListDepartmentsRequest) returns (DepartmentListResponse);

    // 用户部门管理
    rpc AssignDepartments(AssignDepartmentsRequest) returns (BoolResponse);
    rpc RemoveDepartments(RemoveDepartmentsRequest) returns (BoolResponse);
    rpc GetUserDepartments(GetUserDepartmentsRequest) returns (DepartmentListResponse);
}

message CreateDepartmentRequest {
    string department_name = 1;
    string department_code = 2;
    string description = 3;
}

message UpdateDepartmentRequest {
    int64 department_id = 1;
    string department_name = 2;
    string description = 3;
    bool is_active = 4;
}

message DeleteDepartmentRequest {
    int64 department_id = 1;
}

message GetDepartmentRequest {
    int64 department_id = 1;
}

message ListDepartmentsRequest {
    bool include_inactive = 1;
}

message DepartmentResponse {
    int64 department_id = 1;
    string department_name = 2;
    string department_code = 3;
    string description = 4;
    bool is_active = 5;
    google.protobuf.Timestamp created_at = 6;
    google.protobuf.Timestamp updated_at = 7;
}

message DepartmentListResponse {
    repeated DepartmentResponse departments = 1;
}

// 用户部门管理
message AssignDepartmentsRequest {
    int64 user_id = 1;
    repeated int64 department_ids = 2;
}

message RemoveDepartmentsRequest {
    int64 user_id = 1;
    repeated int64 department_ids = 2;
}

message GetUserDepartmentsRequest {
    int64 user_id = 1;
}
```

---

### Task 7: 创建 Department Handler

**Files:**
- Create: `abt-grpc/src/handlers/department.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`

- [ ] **Step 1: 创建 department.rs handler**

```rust
use abt_grpc::Abv1DepartmentService as DepartmentGrpcService;
use abt_grpc::{
    AbtService, AssignDepartmentsRequest, BoolResponse, CreateDepartmentRequest,
    DeleteDepartmentRequest, DepartmentListResponse, DepartmentResponse as GrpcDepartmentResponse,
    GetDepartmentRequest, GetUserDepartmentsRequest, ListDepartmentsRequest,
    RemoveDepartmentsRequest, UpdateDepartmentRequest,
};
use crate::state::AppState;
use crate::handlers::shared::*;
use tonic::{Request, Response, Status};

pub struct DepartmentHandler {
    state: AppState,
}

impl DepartmentHandler {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl DepartmentGrpcService for DepartmentHandler {
    async fn create_department(
        &self,
        request: Request<CreateDepartmentRequest>,
    ) -> Result<Response<GrpcDepartmentResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .create(crate::models::department::CreateDepartmentRequest {
                department_name: req.department_name,
                department_code: req.department_code,
                description: if req.description.is_empty() { None } else { Some(req.description) },
            })
            .await
            .map_err(internal_error)?;

        Ok(Response::new(to_grpc_response(result)))
    }

    async fn update_department(
        &self,
        request: Request<UpdateDepartmentRequest>,
    ) -> Result<Response<GrpcDepartmentResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .update(
                req.department_id,
                crate::models::department::UpdateDepartmentRequest {
                    department_name: if req.department_name.is_empty() { None } else { Some(req.department_name) },
                    description: if req.description.is_empty() { None } else { Some(req.description) },
                    is_active: Some(req.is_active),
                },
            )
            .await
            .map_err(internal_error)?;

        Ok(Response::new(to_grpc_response(result)))
    }

    async fn delete_department(
        &self,
        request: Request<DeleteDepartmentRequest>,
    ) -> Result<Response<BoolResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .delete(req.department_id)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(BoolResponse { success: result }))
    }

    async fn get_department(
        &self,
        request: Request<GetDepartmentRequest>,
    ) -> Result<Response<GrpcDepartmentResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .get_by_id(req.department_id)
            .await
            .map_err(internal_error)?;

        match result {
            Some(dept) => Ok(Response::new(to_grpc_response(dept))),
            None => Err(Status::not_found("Department not found")),
        }
    }

    async fn list_departments(
        &self,
        request: Request<ListDepartmentsRequest>,
    ) -> Result<Response<DepartmentListResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .list(req.include_inactive)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(DepartmentListResponse {
            departments: result.into_iter().map(to_grpc_response).collect(),
        }))
    }

    async fn assign_departments(
        &self,
        request: Request<AssignDepartmentsRequest>,
    ) -> Result<Response<BoolResponse>, Status> {
        let req = request.into_inner();
        self.state.department_service
            .assign_departments(req.user_id, req.department_ids)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(BoolResponse { success: true }))
    }

    async fn remove_departments(
        &self,
        request: Request<RemoveDepartmentsRequest>,
    ) -> Result<Response<BoolResponse>, Status> {
        let req = request.into_inner();
        self.state.department_service
            .remove_departments(req.user_id, req.department_ids)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(BoolResponse { success: true }))
    }

    async fn get_user_departments(
        &self,
        request: Request<GetUserDepartmentsRequest>,
    ) -> Result<Response<DepartmentListResponse>, Status> {
        let req = request.into_inner();
        let result = self.state.department_service
            .get_user_departments(req.user_id)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(DepartmentListResponse {
            departments: result.into_iter().map(to_grpc_response).collect(),
        }))
    }
}

fn to_grpc_response(dept: crate::models::department::Department) -> GrpcDepartmentResponse {
    GrpcDepartmentResponse {
        department_id: dept.department_id,
        department_name: dept.department_name,
        department_code: dept.department_code,
        description: dept.description.unwrap_or_default(),
        is_active: dept.is_active,
        created_at: Some(prost_types::Timestamp::from(datetime_to_timestamp(&dept.created_at))),
        updated_at: dept.updated_at.map(|dt| prost_types::Timestamp::from(datetime_to_timestamp(&dt))),
    }
}
```

- [ ] **Step 2: 更新 mod.rs 导出 department handler**

---

## Phase 2: 资源关联

### Task 8: 修改资源查询注入部门过滤

**Files:**
- Modify: `abt/src/repositories/permission_repo.rs`
- Modify: `abt/src/repositories/resource_repo.rs` (如存在)

- [ ] **Step 1: 修改 ListResources 添加部门过滤逻辑**

在 `list_user_resources` 或类似方法中注入：

```rust
// 获取用户所属部门ID列表
let user_dept_ids = DepartmentRepository::get_user_department_ids(pool, user_id).await?;

// 构建查询时添加部门过滤
let resources = if user_dept_ids.is_empty() {
    // 用户没有任何部门，只能看到 department_id 为 NULL 的资源
    sqlx::query_as!(Resource, "SELECT * FROM resources WHERE department_id IS NULL")
        .fetch_all(pool)
        .await?
} else {
    // 用户有部门，看得到所有所属部门的资源
    sqlx::query_as!(
        Resource,
        "SELECT * FROM resources WHERE department_id = ANY($1)",
        &user_dept_ids
    )
    .fetch_all(pool)
    .await?
};
```

---

### Task 9: 超级管理员跳过部门过滤

**Files:**
- Modify: `abt/src/repositories/permission_repo.rs`
- Modify: `abt/src/repositories/resource_repo.rs`

- [ ] **Step 1: 修改资源查询支持超级管理员**

```rust
pub async fn list_user_resources(pool: &PgPool, user_id: i64) -> Result<Vec<Resource>, Error> {
    // 检查是否超级管理员
    let is_super_admin = sqlx::query_scalar!(
        "SELECT is_super_admin FROM users WHERE user_id = $1",
        user_id
    )
    .fetch_optional(pool)
    .await?
    .flatten()
    .unwrap_or(false);

    if is_super_admin {
        // 超级管理员不过滤
        sqlx::query_as!(Resource, "SELECT * FROM resources ORDER BY sort_order")
            .fetch_all(pool)
            .await
    } else {
        // 普通用户按部门过滤
        let user_dept_ids = DepartmentRepository::get_user_department_ids(pool, user_id).await?;
        if user_dept_ids.is_empty() {
            sqlx::query_as!(Resource, "SELECT * FROM resources WHERE department_id IS NULL")
                .fetch_all(pool)
                .await
        } else {
            sqlx::query_as!(
                Resource,
                "SELECT * FROM resources WHERE department_id = ANY($1) ORDER BY sort_order",
                &user_dept_ids
            )
            .fetch_all(pool)
            .await
        }
    }
}
```

---

## Phase 3: 集成与配置

### Task 10: 注册 DepartmentService 到 AppState

**Files:**
- Modify: `abt-grpc/src/state.rs`

- [ ] **Step 1: 在 AppState 中添加 department_service**

```rust
pub struct AppState {
    pub pool: Arc<PgPool>,
    pub user_service: Arc<dyn UserServiceTrait>,
    pub role_service: Arc<dyn RoleServiceTrait>,
    pub permission_service: Arc<dyn PermissionServiceTrait>,
    pub department_service: Arc<dyn DepartmentServiceTrait>,  // 新增
}
```

- [ ] **Step 2: 在 main.rs 或初始化代码中创建并注入**

---

### Task 11: 更新 proto 编译和生成

**Files:**
- Modify: `abt-grpc/build.rs` 或 `Cargo.toml`

- [ ] **Step 1: 确保 department.proto 被编译**

在 `build.rs` 的 tonic-build 配置中添加：

```rust
fn main() -> Result<(), Box<dyn Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .build_transport(true)
        .extern_path(".abt.v1", "abt_grpc")
        .compile(
            &[
                "proto/abt/v1/department.proto",
                // ... 其他 proto 文件
            ],
            &["proto"],
        )?;
    Ok(())
}
```

---

### Task 12: 运行数据库迁移

- [ ] **Step 1: 执行迁移 SQL**

```bash
psql -h localhost -U postgres -d abt -f abt/migrations/011_add_department_tables.sql
```

---

## 实施检查清单

- [ ] Task 1: 迁移 SQL 创建完成
- [ ] Task 2: Department model 创建完成
- [ ] Task 3: Department repository 创建完成
- [ ] Task 4: Department service 创建完成
- [ ] Task 5: Department service impl 更新完成
- [ ] Task 6: department.proto 创建完成
- [ ] Task 7: Department handler 创建完成
- [ ] Task 8: 资源查询添加部门过滤
- [ ] Task 9: 超级管理员跳过过滤
- [ ] Task 10: AppState 注册 department_service
- [ ] Task 11: proto 编译配置更新
- [ ] Task 12: 数据库迁移执行

---

## 相关文档

- 设计文档: `docs/superpowers/specs/2026-03-31-department-design.md`
