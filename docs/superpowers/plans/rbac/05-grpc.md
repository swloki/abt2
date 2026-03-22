# Task 5: gRPC 接口层

**Files:**
- Create: `abt-grpc/proto/abt/v1/user.proto`
- Create: `abt-grpc/proto/abt/v1/role.proto`
- Create: `abt-grpc/proto/abt/v1/permission.proto`
- Create: `abt-grpc/src/handlers/user.rs`
- Create: `abt-grpc/src/handlers/role.rs`
- Create: `abt-grpc/src/handlers/permission.rs`
- Modify: `abt-grpc/src/handlers/mod.rs`
- Modify: `abt-grpc/build.rs` (如需要)

**Goal:** 实现 gRPC 接口，暴露 RBAC 功能

---

## Step 1: 创建 user.proto

创建文件 `abt-grpc/proto/abt/v1/user.proto`：

```protobuf
syntax = "proto3";
package abt.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/empty.proto";

service UserService {
    rpc CreateUser(CreateUserRequest) returns (UserResponse);
    rpc UpdateUser(UpdateUserRequest) returns (UserResponse);
    rpc DeleteUser(DeleteUserRequest) returns (google.protobuf.Empty);
    rpc GetUser(GetUserRequest) returns (UserResponse);
    rpc ListUsers(google.protobuf.Empty) returns (UserListResponse);

    // 用户角色管理
    rpc AssignRoles(AssignRolesRequest) returns (google.protobuf.Empty);
    rpc RemoveRoles(RemoveRolesRequest) returns (google.protobuf.Empty);
    rpc BatchAssignRoles(BatchAssignRolesRequest) returns (google.protobuf.Empty);
}

message CreateUserRequest {
    string username = 1;
    string password = 2;
    string display_name = 3;
    bool is_super_admin = 4;
}

message UpdateUserRequest {
    int64 user_id = 1;
    optional string display_name = 2;
    optional bool is_active = 3;
}

message DeleteUserRequest {
    int64 user_id = 1;
}

message GetUserRequest {
    int64 user_id = 1;
}

message UserResponse {
    int64 user_id = 1;
    string username = 2;
    string display_name = 3;
    bool is_active = 4;
    bool is_super_admin = 5;
    repeated RoleInfo roles = 6;
    google.protobuf.Timestamp created_at = 7;
}

message UserListResponse {
    repeated UserResponse users = 1;
}

message AssignRolesRequest {
    int64 user_id = 1;
    repeated int64 role_ids = 2;
}

message RemoveRolesRequest {
    int64 user_id = 1;
    repeated int64 role_ids = 2;
}

message BatchAssignRolesRequest {
    repeated int64 user_ids = 1;
    repeated int64 role_ids = 2;
}

message RoleInfo {
    int64 role_id = 1;
    string role_name = 2;
    string role_code = 3;
}
```

- [ ] **Step 1: 创建 user.proto**

---

## Step 2: 创建 role.proto

创建文件 `abt-grpc/proto/abt/v1/role.proto`：

```protobuf
syntax = "proto3";
package abt.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/empty.proto";
import "user.proto";

service RoleService {
    rpc CreateRole(CreateRoleRequest) returns (RoleResponse);
    rpc UpdateRole(UpdateRoleRequest) returns (RoleResponse);
    rpc DeleteRole(DeleteRoleRequest) returns (google.protobuf.Empty);
    rpc GetRole(GetRoleRequest) returns (RoleResponse);
    rpc ListRoles(google.protobuf.Empty) returns (RoleListResponse);

    // 角色权限管理
    rpc AssignPermissions(AssignPermissionsRequest) returns (google.protobuf.Empty);
    rpc RemovePermissions(RemovePermissionsRequest) returns (google.protobuf.Empty);
}

message CreateRoleRequest {
    string role_name = 1;
    string role_code = 2;
    string description = 3;
}

message UpdateRoleRequest {
    int64 role_id = 1;
    optional string role_name = 2;
    optional string description = 3;
}

message DeleteRoleRequest {
    int64 role_id = 1;
}

message GetRoleRequest {
    int64 role_id = 1;
}

message RoleResponse {
    int64 role_id = 1;
    string role_name = 2;
    string role_code = 3;
    bool is_system_role = 4;
    string description = 5;
    repeated PermissionInfo permissions = 6;
}

message RoleListResponse {
    repeated RoleListItem roles = 1;
}

message RoleListItem {
    int64 role_id = 1;
    string role_name = 2;
    string role_code = 3;
    bool is_system_role = 4;
    string description = 5;
}

message AssignPermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}

message RemovePermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}

message PermissionInfo {
    int64 permission_id = 1;
    string permission_name = 2;
    ResourceInfo resource = 3;
    string action_code = 4;
    string action_name = 5;
}

message ResourceInfo {
    int64 resource_id = 1;
    string resource_name = 2;
    string resource_code = 3;
    string group_name = 4;
}
```

- [ ] **Step 2: 创建 role.proto**

---

## Step 3: 创建 permission.proto

创建文件 `abt-grpc/proto/abt/v1/permission.proto`：

```protobuf
syntax = "proto3";
package abt.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/empty.proto";
import "role.proto";

service PermissionService {
    // 获取用户的所有权限
    rpc GetUserPermissions(GetUserPermissionsRequest) returns (UserPermissionsResponse);

    // 检查用户是否有某个权限
    rpc CheckPermission(CheckPermissionRequest) returns (CheckPermissionResponse);

    // 获取资源列表（按分组）
    rpc ListResources(google.protobuf.Empty) returns (ResourceListResponse);

    // 获取所有权限（按分组）
    rpc ListPermissions(google.protobuf.Empty) returns (PermissionListResponse);

    // 获取审计日志
    rpc ListAuditLogs(ListAuditLogsRequest) returns (AuditLogListResponse);
}

message GetUserPermissionsRequest {
    int64 user_id = 1;
}

message UserPermissionsResponse {
    repeated PermissionInfo permissions = 1;
}

message CheckPermissionRequest {
    int64 user_id = 1;
    string resource_code = 2;
    string action_code = 3;
}

message CheckPermissionResponse {
    bool has_permission = 1;
}

message ResourceListResponse {
    repeated ResourceGroup groups = 1;
}

message ResourceGroup {
    string group_name = 1;
    repeated ResourceInfo resources = 2;
}

message PermissionListResponse {
    repeated PermissionGroup groups = 1;
}

message PermissionGroup {
    string group_name = 1;
    repeated PermissionInfo permissions = 2;
}

message ListAuditLogsRequest {
    int64 limit = 1;
    int64 offset = 2;
}

message AuditLogListResponse {
    repeated AuditLogInfo logs = 1;
}

message AuditLogInfo {
    int64 log_id = 1;
    int64 operator_id = 2;
    string operator_name = 3;
    string target_type = 4;
    int64 target_id = 5;
    string action = 6;
    google.protobuf.Timestamp created_at = 7;
}
```

- [ ] **Step 3: 创建 permission.proto**

---

## Step 4: 生成 proto 代码

```bash
cargo build
```

预期：tonic-build 自动生成 gRPC 代码

- [ ] **Step 4: 运行 cargo build 生成 proto 代码**

---

## Step 5: 创建 user.rs handler

创建文件 `abt-grpc/src/handlers/user.rs`：

```rust
use tonic::{Request, Response, Status};
use std::sync::Arc;

use crate::abt::v1::*;
use crate::abt::v1::user_service_server::UserService;
use crate::{GrpcResult, AppState};

pub struct UserHandler {
    pool: Arc<sqlx::PgPool>,
}

impl UserHandler {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl UserService for UserHandler {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> GrpcResult<UserResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64; // TODO: 从 metadata 提取

        let user_id = state
            .user_service()
            .create(operator_id, req.into(), self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let user_with_roles = state
            .user_service()
            .get(user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn update_user(
        &self,
        request: Request<UpdateUserRequest>,
    ) -> GrpcResult<UserResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .user_service()
            .update(operator_id, req.user_id, req.into(), self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let user_with_roles = state
            .user_service()
            .get(req.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn delete_user(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .user_service()
            .delete(operator_id, req.user_id, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> GrpcResult<UserResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();

        let user_with_roles = state
            .user_service()
            .get(req.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn list_users(
        &self,
        _request: Request<()>,
    ) -> GrpcResult<UserListResponse> {
        let state = AppState::get().await;

        let users = state
            .user_service()
            .list()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UserListResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }

    async fn assign_roles(
        &self,
        request: Request<AssignRolesRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .user_service()
            .assign_roles(operator_id, req.user_id, req.role_ids, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn remove_roles(
        &self,
        request: Request<RemoveRolesRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .user_service()
            .remove_roles(operator_id, req.user_id, req.role_ids, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn batch_assign_roles(
        &self,
        request: Request<BatchAssignRolesRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .user_service()
            .batch_assign_roles(operator_id, req.user_ids, req.role_ids, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }
}

// 类型转换实现
impl From<CreateUserRequest> for crate::models::CreateUserRequest {
    fn from(req: CreateUserRequest) -> Self {
        Self {
            username: req.username,
            password: req.password,
            display_name: if req.display_name.is_empty() { None } else { Some(req.display_name) },
            is_super_admin: req.is_super_admin,
        }
    }
}

impl From<UpdateUserRequest> for crate::models::UpdateUserRequest {
    fn from(req: UpdateUserRequest) -> Self {
        Self {
            display_name: req.display_name,
            is_active: req.is_active,
        }
    }
}

impl From<crate::models::UserWithRoles> for UserResponse {
    fn from(u: crate::models::UserWithRoles) -> Self {
        Self {
            user_id: u.user.user_id,
            username: u.user.username,
            display_name: u.user.display_name.unwrap_or_default(),
            is_active: u.user.is_active,
            is_super_admin: u.user.is_super_admin,
            roles: u.roles.into_iter().map(|r| r.into()).collect(),
            created_at: Some(u.user.created_at.into()),
        }
    }
}

impl From<crate::models::RoleInfo> for RoleInfo {
    fn from(r: crate::models::RoleInfo) -> Self {
        Self {
            role_id: r.role_id,
            role_name: r.role_name,
            role_code: r.role_code,
        }
    }
}
```

- [ ] **Step 5: 创建 user.rs handler**

---

## Step 6: 创建 role.rs handler

创建文件 `abt-grpc/src/handlers/role.rs`：

```rust
use tonic::{Request, Response, Status};
use std::sync::Arc;

use crate::abt::v1::*;
use crate::abt::v1::role_service_server::RoleService;
use crate::{GrpcResult, AppState};

pub struct RoleHandler {
    pool: Arc<sqlx::PgPool>,
}

impl RoleHandler {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl RoleService for RoleHandler {
    async fn create_role(
        &self,
        request: Request<CreateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        let role_id = state
            .role_service()
            .create(operator_id, req.into(), self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let role_with_perms = state
            .role_service()
            .get(role_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn update_role(
        &self,
        request: Request<UpdateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .role_service()
            .update(operator_id, req.role_id, req.into(), self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let role_with_perms = state
            .role_service()
            .get(req.role_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn delete_role(
        &self,
        request: Request<DeleteRoleRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .role_service()
            .delete(operator_id, req.role_id, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn get_role(
        &self,
        request: Request<GetRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();

        let role_with_perms = state
            .role_service()
            .get(req.role_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn list_roles(
        &self,
        _request: Request<()>,
    ) -> GrpcResult<RoleListResponse> {
        let state = AppState::get().await;

        let roles = state
            .role_service()
            .list()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RoleListResponse {
            roles: roles.into_iter().map(|r| r.into()).collect(),
        }))
    }

    async fn assign_permissions(
        &self,
        request: Request<AssignPermissionsRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .role_service()
            .assign_permissions(operator_id, req.role_id, req.permission_ids, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn remove_permissions(
        &self,
        request: Request<RemovePermissionsRequest>,
    ) -> GrpcResult<()> {
        let state = AppState::get().await;
        let req = request.into_inner();
        let operator_id = 1i64;

        state
            .role_service()
            .remove_permissions(operator_id, req.role_id, req.permission_ids, self.pool.as_ref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }
}

// 类型转换实现
impl From<CreateRoleRequest> for crate::models::CreateRoleRequest {
    fn from(req: CreateRoleRequest) -> Self {
        Self {
            role_name: req.role_name,
            role_code: req.role_code,
            description: if req.description.is_empty() { None } else { Some(req.description) },
        }
    }
}

impl From<UpdateRoleRequest> for crate::models::UpdateRoleRequest {
    fn from(req: UpdateRoleRequest) -> Self {
        Self {
            role_name: req.role_name,
            description: req.description,
        }
    }
}

impl From<crate::models::RoleWithPermissions> for RoleResponse {
    fn from(r: crate::models::RoleWithPermissions) -> Self {
        Self {
            role_id: r.role.role_id,
            role_name: r.role.role_name,
            role_code: r.role.role_code,
            is_system_role: r.role.is_system_role,
            description: r.role.description.unwrap_or_default(),
            permissions: r.permissions.into_iter().map(|p| p.into()).collect(),
        }
    }
}

impl From<crate::models::Role> for RoleListItem {
    fn from(r: crate::models::Role) -> Self {
        Self {
            role_id: r.role_id,
            role_name: r.role_name,
            role_code: r.role_code,
            is_system_role: r.is_system_role,
            description: r.description.unwrap_or_default(),
        }
    }
}

impl From<crate::models::PermissionInfo> for PermissionInfo {
    fn from(p: crate::models::PermissionInfo) -> Self {
        Self {
            permission_id: p.permission_id,
            permission_name: p.permission_name,
            resource: Some(p.resource.into()),
            action_code: p.action_code,
            action_name: p.action_name,
        }
    }
}

impl From<crate::models::Resource> for ResourceInfo {
    fn from(r: crate::models::Resource) -> Self {
        Self {
            resource_id: r.resource_id,
            resource_name: r.resource_name,
            resource_code: r.resource_code,
            group_name: r.group_name.unwrap_or_default(),
        }
    }
}
```

- [ ] **Step 6: 创建 role.rs handler**

---

## Step 7: 创建 permission.rs handler

创建文件 `abt-grpc/src/handlers/permission.rs`：

```rust
use tonic::{Request, Response, Status};
use std::sync::Arc;

use crate::abt::v1::*;
use crate::abt::v1::permission_service_server::PermissionService;
use crate::{GrpcResult, AppState};

pub struct PermissionHandler {
    pool: Arc<sqlx::PgPool>,
}

impl PermissionHandler {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl PermissionService for PermissionHandler {
    async fn get_user_permissions(
        &self,
        request: Request<GetUserPermissionsRequest>,
    ) -> GrpcResult<UserPermissionsResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();

        let permissions = state
            .permission_service()
            .get_user_permissions(req.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UserPermissionsResponse {
            permissions: permissions.into_iter().map(|p| p.into()).collect(),
        }))
    }

    async fn check_permission(
        &self,
        request: Request<CheckPermissionRequest>,
    ) -> GrpcResult<CheckPermissionResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();

        let has_permission = state
            .permission_service()
            .check_permission(req.user_id, &req.resource_code, &req.action_code)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CheckPermissionResponse { has_permission }))
    }

    async fn list_resources(
        &self,
        _request: Request<()>,
    ) -> GrpcResult<ResourceListResponse> {
        let state = AppState::get().await;

        let groups = state
            .permission_service()
            .list_resources()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ResourceListResponse {
            groups: groups.into_iter().map(|g| g.into()).collect(),
        }))
    }

    async fn list_permissions(
        &self,
        _request: Request<()>,
    ) -> GrpcResult<PermissionListResponse> {
        let state = AppState::get().await;

        let groups = state
            .permission_service()
            .list_permissions()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PermissionListResponse {
            groups: groups.into_iter().map(|g| g.into()).collect(),
        }))
    }

    async fn list_audit_logs(
        &self,
        request: Request<ListAuditLogsRequest>,
    ) -> GrpcResult<AuditLogListResponse> {
        let state = AppState::get().await;
        let req = request.into_inner();

        let logs = state
            .permission_service()
            .list_audit_logs(req.limit, req.offset)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AuditLogListResponse {
            logs: logs.into_iter().map(|l| l.into()).collect(),
        }))
    }
}

// 类型转换实现
impl From<crate::models::ResourceGroup> for ResourceGroup {
    fn from(g: crate::models::ResourceGroup) -> Self {
        Self {
            group_name: g.group_name,
            resources: g.resources.into_iter().map(|r| r.into()).collect(),
        }
    }
}

impl From<crate::models::PermissionGroup> for PermissionGroup {
    fn from(g: crate::models::PermissionGroup) -> Self {
        Self {
            group_name: g.group_name,
            permissions: g.permissions.into_iter().map(|p| p.into()).collect(),
        }
    }
}

impl From<crate::models::AuditLog> for AuditLogInfo {
    fn from(l: crate::models::AuditLog) -> Self {
        Self {
            log_id: l.log_id,
            operator_id: l.operator_id,
            operator_name: l.operator_name.unwrap_or_default(),
            target_type: l.target_type,
            target_id: l.target_id,
            action: l.action,
            created_at: Some(l.created_at.into()),
        }
    }
}
```

- [ ] **Step 7: 创建 permission.rs handler**

---

## Step 8: 更新 handlers/mod.rs

在 `abt-grpc/src/handlers/mod.rs` 添加：

```rust
pub mod user;
pub mod role;
pub mod permission;

pub use user::UserHandler;
pub use role::RoleHandler;
pub use permission::PermissionHandler;
```

- [ ] **Step 8: 更新 handlers/mod.rs**

---

## Step 9: 注册服务到 gRPC Server

在 gRPC server 启动代码中添加：

```rust
use abt_grpc::handlers::{UserHandler, RoleHandler, PermissionHandler};

// 在 server builder 中添加
server
    .add_service(user_service_server::UserServiceServer::new(UserHandler::new(pool.clone())))
    .add_service(role_service_server::RoleServiceServer::new(RoleHandler::new(pool.clone())))
    .add_service(permission_service_server::PermissionServiceServer::new(PermissionHandler::new(pool.clone())));
```

- [ ] **Step 9: 注册服务到 gRPC Server**

---

## Step 10: 验证编译

```bash
cargo build
```

预期：编译成功

- [ ] **Step 10: 运行 cargo build 验证**

---

## Step 11: 测试 gRPC 接口

```bash
# 启动服务后测试
grpcurl -plaintext localhost:50051 list

# 测试列出角色
grpcurl -plaintext localhost:50051 abt.v1.RoleService/ListRoles

# 测试检查权限
grpcurl -plaintext -d '{"user_id": 1, "resource_code": "product", "action_code": "read"}' \
    localhost:50051 abt.v1.PermissionService/CheckPermission
```

- [ ] **Step 11: 使用 grpcurl 测试接口**

---

## Step 12: Commit

```bash
git add abt-grpc/proto/abt/v1/user.proto \
        abt-grpc/proto/abt/v1/role.proto \
        abt-grpc/proto/abt/v1/permission.proto \
        abt-grpc/src/handlers/user.rs \
        abt-grpc/src/handlers/role.rs \
        abt-grpc/src/handlers/permission.rs \
        abt-grpc/src/handlers/mod.rs
git commit -m "feat(rbac): add gRPC handlers for permission system

- Add UserService: CRUD, role assignment
- Add RoleService: CRUD, permission assignment
- Add PermissionService: check, list, audit logs

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 12: Commit gRPC 文件**
