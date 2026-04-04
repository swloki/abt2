# Role Permission Assignment Proto 重构

## 状态：待实现（编译阻塞）

## 背景

Migration 015 重构了权限系统，从数据库驱动的 resources/permissions/actions 三表模型，改为代码定义 + `role_permissions(resource_code, action_code)` 的简化模型。

服务层（service/repo）已经完成重构，但 **proto 层和 handler 层尚未同步更新**，导致编译失败。

## 编译错误

### 错误 1：`role.rs` handler 类型不匹配

**文件**: `abt-grpc/src/handlers/role.rs:162` 和 `:183`

```rust
// 当前代码（编译失败）
srv.assign_permissions(Some(auth.user_id), req.role_id, req.permission_ids, &mut tx)
srv.remove_permissions(Some(auth.user_id), req.role_id, req.permission_ids, &mut tx)
```

`req.permission_ids` 是 `Vec<i64>`（来自 proto），但 `RoleService` trait 现在期望 `Vec<(String, String)>`（resource_code, action_code 元组）。

### 错误 2：`convert.rs` RoleWithPermissions 转换

**文件**: `abt-grpc/src/handlers/convert.rs:305`

```rust
// 当前代码（编译失败）
permissions: r.permissions.into_iter().map(|p| p.into()).collect(),
```

`RoleWithPermissions.permissions` 现在是 `Vec<String>`（权限代码如 `"product:read"`），不再是 `Vec<PermissionInfo>`。需要转换为 proto 的 `PermissionInfo`。

## 需要修改的文件和具体方案

### 1. `proto/abt/v1/role.proto` — 修改请求消息

**当前**:
```protobuf
message AssignPermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}

message RemovePermissionsRequest {
    int64 role_id = 1;
    repeated int64 permission_ids = 2;
}
```

**改为**:
```protobuf
message ResourceAction {
    string resource_code = 1;
    string action_code = 2;
}

message AssignPermissionsRequest {
    int64 role_id = 1;
    repeated ResourceAction permissions = 2;
}

message RemovePermissionsRequest {
    int64 role_id = 1;
    repeated ResourceAction permissions = 2;
}
```

同时 `RoleResponse.permissions` 也要改，从 `repeated PermissionInfo` 改为 `repeated string permission_codes`（因为后端现在只存代码字符串）。

**当前**:
```protobuf
message RoleResponse {
    ...
    repeated PermissionInfo permissions = 6;
}
```

**改为**:
```protobuf
message RoleResponse {
    ...
    repeated string permission_codes = 6;
}
```

### 2. `abt-grpc/src/handlers/role.rs:162,183` — 转换 proto 到 service 格式

```rust
// assign_permissions:
let resource_actions: Vec<(String, String)> = req.permissions.iter()
    .map(|p| (p.resource_code.clone(), p.action_code.clone()))
    .collect();
srv.assign_permissions(Some(auth.user_id), req.role_id, resource_actions, &mut tx).await...

// remove_permissions:
let resource_actions: Vec<(String, String)> = req.permissions.iter()
    .map(|p| (p.resource_code.clone(), p.action_code.clone()))
    .collect();
srv.remove_permissions(Some(auth.user_id), req.role_id, resource_actions, &mut tx).await...
```

### 3. `abt-grpc/src/handlers/convert.rs:297-308` — 更新 RoleWithPermissions 转换

```rust
impl From<abt::RoleWithPermissions> for ProtoRoleResponse {
    fn from(r: abt::RoleWithPermissions) -> Self {
        ProtoRoleResponse {
            role_id: r.role.role_id,
            role_name: r.role.role_name,
            role_code: r.role.role_code,
            is_system_role: r.role.is_system_role,
            description: r.role.description.unwrap_or_default(),
            permission_codes: r.permissions,  // 直接传 Vec<String>
        }
    }
}
```

### 4. `abt-grpc/src/generated/abt.v1.rs` — 自动生成

运行 `cargo build` 后 `build.rs` 会自动从 proto 生成新代码。不需要手动编辑。

## 已完成的依赖链（供参考）

这些已经改好，不需要再动：

- `abt/src/models/role.rs` — `RoleWithPermissions.permissions: Vec<String>`
- `abt/src/repositories/role_repo.rs` — `assign_permissions`/`remove_permissions` 接受 `&[(String, String)]`
- `abt/src/service/role_service.rs` — trait 签名已更新
- `abt/src/implt/role_service_impl.rs` — impl 已更新

## 注意事项

1. **sqlx 编译时检查**：所有 `sqlx::query!` 宏需要 `DATABASE_URL` 环境变量，并且数据库需要执行完所有 migrations（特别是 015）。如果数据库 schema 不匹配，即使代码逻辑正确也会编译失败。

2. **前端影响**：proto 变更意味着前端 gRPC 客户端也需要更新：
   - `AssignPermissionsRequest` 不再发 `permission_ids: [1, 2, 3]`，改为 `permissions: [{resource_code: "product", action_code: "read"}, ...]`
   - `RoleResponse` 不再返回 `PermissionInfo` 对象，改为 `permission_codes: ["product:read", "product:write", ...]`

3. **PermissionInfo proto 消息清理**：`permission.proto` 中的 `PermissionInfo` 消息仍有 `permission_id`、`resource` 等旧字段。目前 `permission_handler.rs` 中构造 `PermissionInfo` 时用 `0` 填充 `permission_id` 和 `resource_id`。如果后续要清理 proto，可以在编译通过后再处理。
