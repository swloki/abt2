# Task 2: Models 层

**Files:**
- Create: `src/models/user.rs`
- Create: `src/models/role.rs`
- Create: `src/models/permission.rs`
- Modify: `src/models/mod.rs`

**Goal:** 定义 RBAC 相关的数据模型

---

## Step 1: 创建 user.rs

创建文件 `src/models/user.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub user_id: i64,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub is_active: bool,
    pub is_super_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for User {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(User {
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            password_hash: row.try_get("password_hash")?,
            display_name: row.try_get("display_name")?,
            is_active: row.try_get("is_active")?,
            is_super_admin: row.try_get("is_super_admin")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithRoles {
    pub user: User,
    pub roles: Vec<RoleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleInfo {
    pub role_id: i64,
    pub role_name: String,
    pub role_code: String,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for RoleInfo {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(RoleInfo {
            role_id: row.try_get("role_id")?,
            role_name: row.try_get("role_name")?,
            role_code: row.try_get("role_code")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    pub is_super_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
}
```

- [ ] **Step 1: 创建 user.rs**

---

## Step 2: 创建 role.rs

创建文件 `src/models/role.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Role {
    pub role_id: i64,
    pub role_name: String,
    pub role_code: String,
    pub is_system_role: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Role {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Role {
            role_id: row.try_get("role_id")?,
            role_name: row.try_get("role_name")?,
            role_code: row.try_get("role_code")?,
            is_system_role: row.try_get("is_system_role")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleWithPermissions {
    pub role: Role,
    pub permissions: Vec<PermissionInfo>,
}

use super::permission::PermissionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub role_name: String,
    pub role_code: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRoleRequest {
    pub role_name: Option<String>,
    pub description: Option<String>,
}
```

- [ ] **Step 2: 创建 role.rs**

---

## Step 3: 创建 permission.rs

创建文件 `src/models/permission.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resource {
    pub resource_id: i64,
    pub resource_name: String,
    pub resource_code: String,
    pub group_name: Option<String>,
    pub sort_order: i32,
    pub description: Option<String>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Resource {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Resource {
            resource_id: row.try_get("resource_id")?,
            resource_name: row.try_get("resource_name")?,
            resource_code: row.try_get("resource_code")?,
            group_name: row.try_get("group_name")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_code: String,
    pub action_name: String,
    pub sort_order: i32,
    pub description: Option<String>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Action {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Action {
            action_code: row.try_get("action_code")?,
            action_name: row.try_get("action_name")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Permission {
    pub permission_id: i64,
    pub permission_name: String,
    pub resource_id: i64,
    pub action_code: String,
    pub sort_order: i32,
    pub description: Option<String>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for Permission {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Permission {
            permission_id: row.try_get("permission_id")?,
            permission_name: row.try_get("permission_name")?,
            resource_id: row.try_get("resource_id")?,
            action_code: row.try_get("action_code")?,
            sort_order: row.try_get("sort_order")?,
            description: row.try_get("description")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionInfo {
    pub permission_id: i64,
    pub permission_name: String,
    pub resource: Resource,
    pub action_code: String,
    pub action_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub log_id: i64,
    pub operator_id: i64,
    pub operator_name: Option<String>,
    pub target_type: String,
    pub target_id: i64,
    pub action: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for AuditLog {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(AuditLog {
            log_id: row.try_get("log_id")?,
            operator_id: row.try_get("operator_id")?,
            operator_name: row.try_get("operator_name")?,
            target_type: row.try_get("target_type")?,
            target_id: row.try_get("target_id")?,
            action: row.try_get("action")?,
            old_value: row.try_get("old_value")?,
            new_value: row.try_get("new_value")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGroup {
    pub group_name: String,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionInfo>,
}
```

- [ ] **Step 3: 创建 permission.rs**

---

## Step 4: 更新 mod.rs

在 `src/models/mod.rs` 添加：

```rust
pub mod user;
pub mod role;
pub mod permission;

pub use user::*;
pub use role::*;
pub use permission::*;
```

- [ ] **Step 4: 更新 mod.rs 导出新模块**

---

## Step 5: 验证编译

```bash
cargo build
```

预期：编译成功，无错误

- [ ] **Step 5: 运行 cargo build 验证**

---

## Step 6: Commit

```bash
git add src/models/user.rs src/models/role.rs src/models/permission.rs src/models/mod.rs
git commit -m "feat(rbac): add permission models

- Add User, RoleInfo, CreateUserRequest, UpdateUserRequest
- Add Role, RoleWithPermissions, CreateRoleRequest, UpdateRoleRequest
- Add Resource, Action, Permission, PermissionInfo, AuditLog
- Add ResourceGroup, PermissionGroup for grouped responses

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 6: Commit models 文件**
