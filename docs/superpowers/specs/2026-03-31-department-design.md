# 部门功能设计

## 概述

为 ABT 系统增加部门功能，用于数据可见性控制。用户属于部门后，只能看到其所属部门的资源数据。

## 需求总结

| 维度 | 决策 |
|------|------|
| 部门层级 | 平铺结构（不需要树形） |
| 用户-部门 | 多对多（一个用户可属多个部门） |
| 资源-部门 | 多对多（一个资源只属一个部门） |
| 数据可见性 | 部门隔离，用户只能看到所属部门的数据 |
| 权限控制 | 部门管可见性，角色管操作权限（读/写/删） |
| 超级管理员 | 跳过部门限制，可访问所有数据 |

## 设计原则

- **部门 = 可见性**：决定用户能看到哪些数据（行级过滤）
- **角色 = 操作权限**：决定用户能对数据做什么操作（读/写/删）
- 两者正交，组合使用

## 数据库设计

### ER 图

```
┌─────────────┐       ┌──────────────────┐       ┌─────────────┐
│   users     │───M:N──│ user_departments │───M:1──│ departments │
└─────────────┘       └──────────────────┘       └─────────────┘
                                                          │
                                                          │ 1:N
                                                          ▼
                                                   ┌───────────┐
                                                   │ resources │
                                                   └───────────┘
```

### 表结构

#### 部门表 (departments)

```sql
CREATE TABLE departments (
    department_id BIGSERIAL PRIMARY KEY,
    department_name VARCHAR(100) NOT NULL,
    department_code VARCHAR(50) UNIQUE NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);
```

#### 用户-部门关联表 (user_departments)

```sql
CREATE TABLE user_departments (
    user_id BIGINT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    department_id BIGINT NOT NULL REFERENCES departments(department_id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, department_id)
);
```

#### 资源表增加字段 (resources)

```sql
ALTER TABLE resources ADD COLUMN department_id BIGINT REFERENCES departments(department_id);
CREATE INDEX idx_resources_department ON resources(department_id);
```

### 索引

```sql
CREATE INDEX idx_user_departments_user ON user_departments(user_id);
CREATE INDEX idx_user_departments_department ON user_departments(department_id);
```

## 数据查询逻辑

### 部门过滤（核心）

```sql
-- 查询用户可访问的资源（自动过滤部门）
SELECT r.*
FROM resources r
WHERE r.department_id IN (
    SELECT department_id FROM user_departments WHERE user_id = ?
);
```

### 超级管理员例外

```sql
-- 超级管理员不看部门限制
SELECT r.*
FROM resources r
WHERE (
    -- 超级管理员：不过滤
    EXISTS (SELECT 1 FROM users WHERE user_id = ? AND is_super_admin = true)
    OR
    -- 普通用户：按部门过滤
    r.department_id IN (SELECT department_id FROM user_departments WHERE user_id = ?)
);
```

## API 设计

### gRPC 服务定义

```protobuf
// ==================== 部门管理 ====================

service DepartmentService {
    rpc CreateDepartment(CreateDepartmentRequest) returns (DepartmentResponse);
    rpc UpdateDepartment(UpdateDepartmentRequest) returns (DepartmentResponse);
    rpc DeleteDepartment(DeleteDepartmentRequest) returns (BoolResponse);
    rpc GetDepartment(GetDepartmentRequest) returns (DepartmentResponse);
    rpc ListDepartments(ListDepartmentsRequest) returns (DepartmentListResponse);
}

// ==================== 用户部门管理 ====================

service UserService {
    // ... 现有接口 ...

    // 用户部门管理
    rpc AssignDepartments(AssignDepartmentsRequest) returns (BoolResponse);
    rpc RemoveDepartments(RemoveDepartmentsRequest) returns (BoolResponse);
    rpc GetUserDepartments(GetUserDepartmentsRequest) returns (DepartmentListResponse);
}

// ==================== 权限查询扩展 ====================

service PermissionService {
    // ... 现有接口 ...

    // 获取用户可访问的资源（已过滤部门）
    rpc ListUserResources(ListUserResourcesRequest) returns (ResourceListResponse);
}
```

### 消息定义

```protobuf
// 部门相关
message CreateDepartmentRequest {
    string department_name = 1;
    string department_code = 2;
    string description = 3;
}

message DepartmentResponse {
    int64 department_id = 1;
    string department_name = 2;
    string department_code = 3;
    string description = 4;
    bool is_active = 5;
    google.protobuf.Timestamp created_at = 6;
}

message ListDepartmentsRequest {
    bool include_inactive = 1;  // 默认只返回活跃部门
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

// 资源查询扩展
message ListUserResourcesRequest {
    int64 user_id = 1;  // 如果不传，使用当前登录用户
}
```

## 文件结构

```
abt/
├── migrations/
│   └── 011_add_department_tables.sql    # 新增
├── src/
│   ├── models/
│   │   ├── department.rs                # 新增
│   │   └── mod.rs                       # 更新
│   ├── repositories/
│   │   ├── department_repo.rs            # 新增
│   │   └── mod.rs                       # 更新
│   ├── service/
│   │   ├── department_service.rs         # 新增
│   │   └── mod.rs                       # 更新
│   └── implt/
│       ├── department_service_impl.rs    # 新增
│       └── mod.rs                       # 更新

abt-grpc/
├── proto/
│   └── abt/v1/
│       └── department.proto              # 新增
└── src/
    └── handlers/
        ├── department.rs                 # 新增
        └── mod.rs                       # 更新
```

## 迁移计划

### Phase 1: 部门基础
1. 创建 departments 表
2. 创建 user_departments 表
3. 添加 Department model/repo/service/handler

### Phase 2: 资源关联
4. 修改 resources 表添加 department_id
5. 更新资源相关查询，注入部门过滤

### Phase 3: 用户部门管理
6. 扩展 UserService 支持 AssignDepartments/GetUserDepartments
7. 更新 ListResources 返回用户有权限且在所属部门内的资源

### Phase 4: 超级管理员
8. 实现超级管理员跳过部门过滤的查询逻辑

## 业务规则

1. **部门删除**：如果部门被删除，该部门下的资源 department_id 设为 NULL
2. **用户删除**：级联删除 user_departments 关联
3. **部门激活/停用**：停用部门不影响已有数据，只是新增资源时不可选
4. **空部门用户**：如果用户没有任何部门归属，只能看到 department_id 为 NULL 的资源
