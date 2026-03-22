# RBAC 权限系统设计

## 概述

为 ABT 系统设计基于角色的访问控制（RBAC）权限系统，控制用户对系统资源的操作权限。

## 需求总结

| 维度 | 决策 |
|------|------|
| 权限模型 | RBAC（基于角色的访问控制） |
| 权限粒度 | 资源 + 操作（read/write/delete） |
| 用户-角色 | 多对多（一个用户可有多个角色） |
| 角色管理 | 核心角色不可删除 + 可添加自定义角色 |
| 权限冲突 | 就高原则（只要有一个角色允许就允许） |
| 管理方式 | 单一超级管理员 |
| 审计 | 需要审计日志 |
| 批量操作 | 支持批量分配权限和角色 |

## 数据库设计

### ER 图

```
┌─────────┐       ┌─────────┐       ┌─────────────┐
│  users  │───M:N─│  roles  │───M:N─│ permissions │
└─────────┘       └─────────┘       └──────┬──────┘
     │                                     │
     │              ┌──────────────────────┼──────────────────┐
     │              │                      │                  │
     │          ┌───┴────┐           ┌─────┴─────┐      ┌─────┴─────┐
     │          │resources│           │  actions  │      │audit_logs │
     │          └─────────┘           └───────────┘      └───────────┘
     │
     └──────────────────────────────────────────┘
```

### 表结构

#### 用户表 (users)

```sql
CREATE TABLE users (
    user_id BIGSERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_super_admin BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);
```

#### 角色表 (roles)

```sql
CREATE TABLE roles (
    role_id BIGSERIAL PRIMARY KEY,
    role_name VARCHAR(100) NOT NULL,
    role_code VARCHAR(50) UNIQUE NOT NULL,
    is_system_role BOOLEAN NOT NULL DEFAULT false,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);
```

#### 用户-角色关联表 (user_roles)

```sql
CREATE TABLE user_roles (
    user_id BIGINT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);
```

#### 资源表 (resources)

```sql
CREATE TABLE resources (
    resource_id BIGSERIAL PRIMARY KEY,
    resource_name VARCHAR(100) NOT NULL,
    resource_code VARCHAR(50) UNIQUE NOT NULL,
    group_name VARCHAR(100),     -- 分组名：基础数据、库存管理...
    sort_order INT DEFAULT 0,    -- 排序
    description TEXT
);
```

#### 操作表 (actions)

```sql
CREATE TABLE actions (
    action_code VARCHAR(50) PRIMARY KEY,  -- read, write, delete
    action_name VARCHAR(100) NOT NULL,    -- 读取, 编辑, 删除
    sort_order INT DEFAULT 0,
    description TEXT
);
```

#### 权限表 (permissions)

```sql
CREATE TABLE permissions (
    permission_id BIGSERIAL PRIMARY KEY,
    permission_name VARCHAR(100) NOT NULL,
    resource_id BIGINT NOT NULL REFERENCES resources(resource_id) ON DELETE CASCADE,
    action_code VARCHAR(50) NOT NULL REFERENCES actions(action_code) ON DELETE CASCADE,
    sort_order INT DEFAULT 0,
    description TEXT,
    UNIQUE(resource_id, action_code)
);
```

#### 角色权限关联表 (role_permissions)

```sql
CREATE TABLE role_permissions (
    role_id BIGINT NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    permission_id BIGINT NOT NULL REFERENCES permissions(permission_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);
```

#### 权限审计日志表 (permission_audit_logs)

```sql
CREATE TABLE permission_audit_logs (
    log_id BIGSERIAL PRIMARY KEY,
    operator_id BIGINT NOT NULL REFERENCES users(user_id),
    target_type VARCHAR(20) NOT NULL,    -- user/role/permission
    target_id BIGINT NOT NULL,
    action VARCHAR(50) NOT NULL,         -- create/update/delete/assign/remove
    old_value JSONB,
    new_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

#### 索引

```sql
CREATE INDEX idx_user_roles_user ON user_roles(user_id);
CREATE INDEX idx_user_roles_role ON user_roles(role_id);
CREATE INDEX idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX idx_permission_audit_logs_operator ON permission_audit_logs(operator_id);
CREATE INDEX idx_permission_audit_logs_created ON permission_audit_logs(created_at);
```

## 预置数据

### 系统角色

| role_code | role_name | is_system_role |
|-----------|-----------|----------------|
| super_admin | 超级管理员 | true |
| admin | 管理员 | true |
| user | 普通用户 | true |

### 操作

| action_code | action_name | sort_order |
|-------------|-------------|------------|
| read | 读取 | 1 |
| write | 编辑 | 2 |
| delete | 删除 | 3 |

### 资源（带分组）

| resource_code | resource_name | group_name | sort_order |
|---------------|---------------|------------|------------|
| product | 产品管理 | 基础数据 | 1 |
| term | 术语/分类管理 | 基础数据 | 2 |
| warehouse | 仓库管理 | 库存管理 | 3 |
| location | 库位管理 | 库存管理 | 4 |
| inventory | 库存管理 | 库存管理 | 5 |
| bom | BOM管理 | 生产管理 | 6 |
| labor_process | 工序管理 | 生产管理 | 7 |
| price | 价格管理 | 财务管理 | 8 |
| excel | Excel导入导出 | 系统工具 | 9 |

### 权限

权限自动生成：资源 × 操作，共 27 条权限记录。

示例：
| permission_name | resource_code | action_code |
|-----------------|---------------|-------------|
| 产品-读取 | product | read |
| 产品-编辑 | product | write |
| 产品-删除 | product | delete |
| 库存-读取 | inventory | read |
| ... | ... | ... |

## API 设计

### gRPC 服务定义

```protobuf
syntax = "proto3";
package abt.v1;

// ==================== 用户管理 ====================

service UserService {
    rpc CreateUser(CreateUserRequest) returns (UserResponse);
    rpc UpdateUser(UpdateUserRequest) returns (UserResponse);
    rpc DeleteUser(DeleteUserRequest) returns (BoolResponse);
    rpc GetUser(GetUserRequest) returns (UserResponse);
    rpc ListUsers(ListUsersRequest) returns (UserListResponse);

    // 用户角色管理
    rpc AssignRoles(AssignRolesRequest) returns (BoolResponse);
    rpc RemoveRoles(RemoveRolesRequest) returns (BoolResponse);
    rpc BatchAssignRoles(BatchAssignRolesRequest) returns (BoolResponse);
}

message CreateUserRequest {
    string username = 1;
    string password = 2;
    string display_name = 3;
    bool is_super_admin = 4;
}

message UpdateUserRequest {
    int64 user_id = 1;
    string display_name = 2;
    bool is_active = 3;
}

message UserResponse {
    int64 user_id = 1;
    string username = 2;
    string display_name = 3;
    bool is_active = 4;
    bool is_super_admin = 5;
    repeated RoleInfo roles = 6;
    created_at google.protobuf.Timestamp = 7;
}

message AssignRolesRequest {
    int64 user_id = 1;
    repeated int64 role_ids = 2;
}

message BatchAssignRolesRequest {
    repeated int64 user_ids = 1;
    repeated int64 role_ids = 2;
}

// ==================== 角色管理 ====================

service RoleService {
    rpc CreateRole(CreateRoleRequest) returns (RoleResponse);
    rpc UpdateRole(UpdateRoleRequest) returns (RoleResponse);
    rpc DeleteRole(DeleteRoleRequest) returns (BoolResponse);
    rpc GetRole(GetRoleRequest) returns (RoleResponse);
    rpc ListRoles(ListRolesRequest) returns (RoleListResponse);

    // 角色权限管理
    rpc AssignPermissions(AssignPermissionsRequest) returns (BoolResponse);
    rpc RemovePermissions(RemovePermissionsRequest) returns (BoolResponse);
    rpc BatchAssignPermissions(BatchAssignPermissionsRequest) returns (BoolResponse);
}

message CreateRoleRequest {
    string role_name = 1;
    string role_code = 2;
    string description = 3;
}

message RoleResponse {
    int64 role_id = 1;
    string role_name = 2;
    string role_code = 3;
    bool is_system_role = 4;
    string description = 5;
    repeated PermissionInfo permissions = 6;
}

message AssignPermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}

message BatchAssignPermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}

// ==================== 权限查询 ====================

service PermissionService {
    // 获取用户的所有权限
    rpc GetUserPermissions(GetUserPermissionsRequest) returns (UserPermissionsResponse);

    // 检查用户是否有某个权限
    rpc CheckPermission(CheckPermissionRequest) returns (CheckPermissionResponse);

    // 获取资源列表（按分组）
    rpc ListResources(ListResourcesRequest) returns (ResourceListResponse);

    // 获取所有权限（按分组）
    rpc ListPermissions(ListPermissionsRequest) returns (PermissionListResponse);

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

// ==================== 通用消息 ====================

message RoleInfo {
    int64 role_id = 1;
    string role_name = 2;
    string role_code = 3;
}

message ResourceInfo {
    int64 resource_id = 1;
    string resource_name = 2;
    string resource_code = 3;
    string group_name = 4;
}

message PermissionInfo {
    int64 permission_id = 1;
    string permission_name = 2;
    ResourceInfo resource = 3;
    string action_code = 4;
    string action_name = 5;
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

## 权限检查逻辑

### 核心算法

```rust
/// 检查用户是否有指定资源的操作权限
pub async fn check_permission(
    pool: &PgPool,
    user_id: i64,
    resource_code: &str,
    action_code: &str,
) -> Result<bool, Error> {
    // 1. 获取用户信息
    let user = sqlx::query!("SELECT is_super_admin FROM users WHERE user_id = $1", user_id)
        .fetch_optional(pool)
        .await?;

    // 2. 超级管理员直接通过
    if user.map(|u| u.is_super_admin).unwrap_or(false) {
        return Ok(true);
    }

    // 3. 查询用户所有角色的权限（就高原则）
    let has_permission = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM user_roles ur
            JOIN role_permissions rp ON ur.role_id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.permission_id
            JOIN resources r ON p.resource_id = r.resource_id
            WHERE ur.user_id = $1
              AND r.resource_code = $2
              AND p.action_code = $3
        )
        "#,
        user_id,
        resource_code,
        action_code
    )
    .fetch_one(pool)
    .await?;

    Ok(has_permission.unwrap_or(false))
}
```

### gRPC Interceptor

```rust
/// 权限拦截器，在 gRPC 调用前检查权限
pub async fn permission_interceptor(
    req: Request<()>,
    resource_code: &str,
    action_code: &str,
) -> Result<Request<()>, Status> {
    let user_id = extract_user_id_from_metadata(&req)?;

    let pool = AppState::get().pool();
    let has_permission = check_permission(pool, user_id, resource_code, action_code)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    if !has_permission {
        return Err(Status::permission_denied("No permission"));
    }

    Ok(req)
}
```

## 前端集成

### 权限控制方式

1. **菜单隐藏**：根据 `read` 权限控制菜单显示
2. **按钮隐藏**：根据 `write`/`delete` 权限控制操作按钮显示
3. **API 拦截**：后端统一拦截，无权限返回 403

### 前端获取权限示例

```typescript
// 获取当前用户权限
const permissions = await permissionService.getUserPermissions(userId);

// 检查权限
function hasPermission(resource: string, action: string): boolean {
    return permissions.some(
        p => p.resource.code === resource && p.actionCode === action
    );
}

// 使用示例
if (hasPermission('product', 'write')) {
    // 显示编辑按钮
}
```

## 迁移计划

1. 创建权限相关数据库表
2. 插入预置数据（角色、操作、资源、权限）
3. 创建用户、角色、权限相关的 models
4. 创建 repositories 层
5. 创建 service 层
6. 创建 gRPC handlers
7. 添加权限拦截器
8. 创建数据库迁移文件

## 文件结构

```
abt/
├── migrations/
│   └── 010_create_permission_tables.sql
├── src/
│   ├── models/
│   │   ├── mod.rs          # 添加 permission 模块导出
│   │   ├── user.rs         # 新增
│   │   ├── role.rs         # 新增
│   │   ├── permission.rs   # 新增
│   │   └── ...
│   ├── repositories/
│   │   ├── mod.rs          # 添加 permission 模块导出
│   │   ├── user_repo.rs    # 新增
│   │   ├── role_repo.rs    # 新增
│   │   └── permission_repo.rs # 新增
│   ├── service/
│   │   ├── mod.rs          # 添加 permission 模块导出
│   │   ├── user_service.rs # 新增
│   │   ├── role_service.rs # 新增
│   │   └── permission_service.rs # 新增
│   └── implt/
│       ├── mod.rs          # 添加 permission 模块导出
│       ├── user_service_impl.rs   # 新增
│       ├── role_service_impl.rs   # 新增
│       └── permission_service_impl.rs # 新增

abt-grpc/
├── proto/
│   └── abt/v1/
│       ├── user.proto       # 新增
│       ├── role.proto       # 新增
│       └── permission.proto # 新增
└── src/
    └── handlers/
        ├── mod.rs          # 添加 handler 导出
        ├── user.rs         # 新增
        ├── role.rs         # 新增
        └── permission.rs   # 新增
```
