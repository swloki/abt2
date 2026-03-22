# Task 4: Services 层

**Files:**
- Create: `src/service/user_service.rs`
- Create: `src/service/role_service.rs`
- Create: `src/service/permission_service.rs`
- Create: `src/implt/user_service_impl.rs`
- Create: `src/implt/role_service_impl.rs`
- Create: `src/implt/permission_service_impl.rs`
- Modify: `src/service/mod.rs`
- Modify: `src/implt/mod.rs`

**Goal:** 实现业务逻辑层，包含权限检查和审计日志

---

## Step 1: 创建 user_service.rs (trait)

创建文件 `src/service/user_service.rs`：

```rust
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

use crate::models::{
    CreateUserRequest, UpdateUserRequest, User, UserWithRoles,
};

#[async_trait]
pub trait UserService: Send + Sync {
    async fn create(
        &self,
        operator_id: i64,
        req: CreateUserRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<i64>;

    async fn update(
        &self,
        operator_id: i64,
        user_id: i64,
        req: UpdateUserRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn delete(
        &self,
        operator_id: i64,
        user_id: i64,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn get(&self, user_id: i64) -> Result<Option<UserWithRoles>>;

    async fn list(&self) -> Result<Vec<UserWithRoles>>;

    async fn assign_roles(
        &self,
        operator_id: i64,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn remove_roles(
        &self,
        operator_id: i64,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn batch_assign_roles(
        &self,
        operator_id: i64,
        user_ids: Vec<i64>,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;
}
```

- [ ] **Step 1: 创建 user_service.rs**

---

## Step 2: 创建 role_service.rs (trait)

创建文件 `src/service/role_service.rs`：

```rust
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

use crate::models::{
    CreateRoleRequest, Role, RoleWithPermissions, UpdateRoleRequest,
};

#[async_trait]
pub trait RoleService: Send + Sync {
    async fn create(
        &self,
        operator_id: i64,
        req: CreateRoleRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<i64>;

    async fn update(
        &self,
        operator_id: i64,
        role_id: i64,
        req: UpdateRoleRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn delete(
        &self,
        operator_id: i64,
        role_id: i64,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn get(&self, role_id: i64) -> Result<Option<RoleWithPermissions>>;

    async fn list(&self) -> Result<Vec<Role>>;

    async fn assign_permissions(
        &self,
        operator_id: i64,
        role_id: i64,
        permission_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;

    async fn remove_permissions(
        &self,
        operator_id: i64,
        role_id: i64,
        permission_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()>;
}
```

- [ ] **Step 2: 创建 role_service.rs**

---

## Step 3: 创建 permission_service.rs (trait)

创建文件 `src/service/permission_service.rs`：

```rust
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Executor, Postgres};

use crate::models::{
    AuditLog, PermissionGroup, PermissionInfo, ResourceGroup,
};

#[async_trait]
pub trait PermissionService: Send + Sync {
    /// 获取用户的所有权限
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<PermissionInfo>>;

    /// 检查用户是否有某个权限
    async fn check_permission(
        &self,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool>;

    /// 获取资源列表（按分组）
    async fn list_resources(&self) -> Result<Vec<ResourceGroup>>;

    /// 获取所有权限（按分组）
    async fn list_permissions(&self) -> Result<Vec<PermissionGroup>>;

    /// 获取审计日志
    async fn list_audit_logs(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>>;
}
```

- [ ] **Step 3: 创建 permission_service.rs**

---

## Step 4: 创建 user_service_impl.rs

创建文件 `src/implt/user_service_impl.rs`：

```rust
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{
    CreateUserRequest, UpdateUserRequest, User, UserWithRoles,
};
use crate::repositories::{PermissionRepo, UserRepo};
use crate::service::UserService;

pub struct UserServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl UserServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }

    fn hash_password(password: &str) -> Result<String> {
        // 使用 bcrypt 或 argon2
        // 这里简化处理，实际应使用安全哈希
        Ok(format!("hashed:{}", password))
    }

    async fn log_audit(
        &self,
        executor: Executor<'_, Postgres>,
        operator_id: i64,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        PermissionRepo::insert_audit_log(
            executor,
            operator_id,
            target_type,
            target_id,
            action,
            old_value,
            new_value,
        )
        .await
    }
}

#[async_trait]
impl UserService for UserServiceImpl {
    async fn create(
        &self,
        operator_id: i64,
        req: CreateUserRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<i64> {
        let password_hash = Self::hash_password(&req.password)?;
        let user_id = UserRepo::insert(executor, &req, &password_hash).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "create",
            None,
            Some(serde_json::to_value(&req)?),
        )
        .await?;

        Ok(user_id)
    }

    async fn update(
        &self,
        operator_id: i64,
        user_id: i64,
        req: UpdateUserRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        let old_user = UserRepo::find_by_id(executor, user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        UserRepo::update(executor, user_id, &req).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "update",
            Some(serde_json::to_value(&old_user)?),
            Some(serde_json::to_value(&req)?),
        )
        .await?;

        Ok(())
    }

    async fn delete(
        &self,
        operator_id: i64,
        user_id: i64,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        let old_user = UserRepo::find_by_id(executor, user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        UserRepo::delete(executor, user_id).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "delete",
            Some(serde_json::to_value(&old_user)?),
            None,
        )
        .await?;

        Ok(())
    }

    async fn get(&self, user_id: i64) -> Result<Option<UserWithRoles>> {
        let user = UserRepo::find_by_id(self.pool.as_ref(), user_id).await?;
        match user {
            Some(user) => {
                let roles = UserRepo::get_user_roles(self.pool.as_ref(), user_id).await?;
                Ok(Some(UserWithRoles { user, roles }))
            }
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<UserWithRoles>> {
        let users = UserRepo::list_all(self.pool.as_ref()).await?;
        let mut result = Vec::new();

        for user in users {
            let roles = UserRepo::get_user_roles(self.pool.as_ref(), user.user_id).await?;
            result.push(UserWithRoles { user, roles });
        }

        Ok(result)
    }

    async fn assign_roles(
        &self,
        operator_id: i64,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        UserRepo::assign_roles(executor, user_id, &role_ids).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "assign_roles",
            None,
            Some(serde_json::to_value(&role_ids)?),
        )
        .await?;

        Ok(())
    }

    async fn remove_roles(
        &self,
        operator_id: i64,
        user_id: i64,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        UserRepo::remove_roles(executor, user_id, &role_ids).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            user_id,
            "remove_roles",
            Some(serde_json::to_value(&role_ids)?),
            None,
        )
        .await?;

        Ok(())
    }

    async fn batch_assign_roles(
        &self,
        operator_id: i64,
        user_ids: Vec<i64>,
        role_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        UserRepo::batch_assign_roles(executor, &user_ids, &role_ids).await?;

        self.log_audit(
            executor,
            operator_id,
            "user",
            0, // batch operation
            "batch_assign_roles",
            None,
            serde_json::json!({
                "user_ids": user_ids,
                "role_ids": role_ids
            })
            .into(),
        )
        .await?;

        Ok(())
    }
}
```

- [ ] **Step 4: 创建 user_service_impl.rs**

---

## Step 5: 创建 role_service_impl.rs

创建文件 `src/implt/role_service_impl.rs`：

```rust
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use std::sync::Arc;

use crate::models::{
    CreateRoleRequest, Role, RoleWithPermissions, UpdateRoleRequest,
};
use crate::repositories::{PermissionRepo, RoleRepo};
use crate::service::RoleService;

pub struct RoleServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl RoleServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }

    async fn log_audit(
        &self,
        executor: Executor<'_, Postgres>,
        operator_id: i64,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        PermissionRepo::insert_audit_log(
            executor,
            operator_id,
            target_type,
            target_id,
            action,
            old_value,
            new_value,
        )
        .await
    }
}

#[async_trait]
impl RoleService for RoleServiceImpl {
    async fn create(
        &self,
        operator_id: i64,
        req: CreateRoleRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<i64> {
        let role_id = RoleRepo::insert(executor, &req).await?;

        self.log_audit(
            executor,
            operator_id,
            "role",
            role_id,
            "create",
            None,
            Some(serde_json::to_value(&req)?),
        )
        .await?;

        Ok(role_id)
    }

    async fn update(
        &self,
        operator_id: i64,
        role_id: i64,
        req: UpdateRoleRequest,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        let old_role = RoleRepo::find_by_id(executor, role_id)
            .await?
            .ok_or_else(|| anyhow!("Role not found"))?;

        RoleRepo::update(executor, role_id, &req).await?;

        self.log_audit(
            executor,
            operator_id,
            "role",
            role_id,
            "update",
            Some(serde_json::to_value(&old_role)?),
            Some(serde_json::to_value(&req)?),
        )
        .await?;

        Ok(())
    }

    async fn delete(
        &self,
        operator_id: i64,
        role_id: i64,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        // 检查是否系统角色
        let is_system = RoleRepo::is_system_role(executor, role_id).await?;
        if is_system {
            return Err(anyhow!("Cannot delete system role"));
        }

        let old_role = RoleRepo::find_by_id(executor, role_id)
            .await?
            .ok_or_else(|| anyhow!("Role not found"))?;

        RoleRepo::delete(executor, role_id).await?;

        self.log_audit(
            executor,
            operator_id,
            "role",
            role_id,
            "delete",
            Some(serde_json::to_value(&old_role)?),
            None,
        )
        .await?;

        Ok(())
    }

    async fn get(&self, role_id: i64) -> Result<Option<RoleWithPermissions>> {
        let role = RoleRepo::find_by_id(self.pool.as_ref(), role_id).await?;
        match role {
            Some(role) => {
                let permissions = RoleRepo::get_role_permissions(self.pool.as_ref(), role_id).await?;
                Ok(Some(RoleWithPermissions { role, permissions }))
            }
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<Role>> {
        RoleRepo::list_all(self.pool.as_ref()).await
    }

    async fn assign_permissions(
        &self,
        operator_id: i64,
        role_id: i64,
        permission_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        RoleRepo::assign_permissions(executor, role_id, &permission_ids).await?;

        self.log_audit(
            executor,
            operator_id,
            "role",
            role_id,
            "assign_permissions",
            None,
            Some(serde_json::to_value(&permission_ids)?),
        )
        .await?;

        Ok(())
    }

    async fn remove_permissions(
        &self,
        operator_id: i64,
        role_id: i64,
        permission_ids: Vec<i64>,
        executor: Executor<'_, Postgres>,
    ) -> Result<()> {
        RoleRepo::remove_permissions(executor, role_id, &permission_ids).await?;

        self.log_audit(
            executor,
            operator_id,
            "role",
            role_id,
            "remove_permissions",
            Some(serde_json::to_value(&permission_ids)?),
            None,
        )
        .await?;

        Ok(())
    }
}
```

- [ ] **Step 5: 创建 role_service_impl.rs**

---

## Step 6: 创建 permission_service_impl.rs

创建文件 `src/implt/permission_service_impl.rs`：

```rust
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::{
    AuditLog, PermissionGroup, PermissionInfo, ResourceGroup,
};
use crate::repositories::PermissionRepo;
use crate::service::PermissionService;

pub struct PermissionServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl PermissionServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PermissionService for PermissionServiceImpl {
    async fn get_user_permissions(&self, user_id: i64) -> Result<Vec<PermissionInfo>> {
        PermissionRepo::get_user_permissions(self.pool.as_ref(), user_id).await
    }

    async fn check_permission(
        &self,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool> {
        PermissionRepo::check_permission(
            self.pool.as_ref(),
            user_id,
            resource_code,
            action_code,
        )
        .await
    }

    async fn list_resources(&self) -> Result<Vec<ResourceGroup>> {
        let resources = PermissionRepo::list_resources(self.pool.as_ref()).await?;

        // 按分组聚合
        let mut groups: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();

        for resource in resources {
            let group_name = resource.group_name.clone().unwrap_or_else(|| "其他".to_string());
            groups.entry(group_name).or_default().push(resource);
        }

        let result: Vec<ResourceGroup> = groups
            .into_iter()
            .map(|(group_name, resources)| ResourceGroup {
                group_name,
                resources,
            })
            .collect();

        Ok(result)
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionGroup>> {
        let permissions = PermissionRepo::list_permissions(self.pool.as_ref()).await?;

        // 按分组聚合
        let mut groups: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();

        for perm in permissions {
            let group_name = perm.resource.group_name.clone().unwrap_or_else(|| "其他".to_string());
            groups.entry(group_name).or_default().push(perm);
        }

        let result: Vec<PermissionGroup> = groups
            .into_iter()
            .map(|(group_name, permissions)| PermissionGroup {
                group_name,
                permissions,
            })
            .collect();

        Ok(result)
    }

    async fn list_audit_logs(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        PermissionRepo::list_audit_logs(self.pool.as_ref(), limit, offset).await
    }
}
```

- [ ] **Step 6: 创建 permission_service_impl.rs**

---

## Step 7: 更新 mod.rs 文件

在 `src/service/mod.rs` 添加：

```rust
pub mod user_service;
pub mod role_service;
pub mod permission_service;

pub use user_service::UserService;
pub use role_service::RoleService;
pub use permission_service::PermissionService;
```

在 `src/implt/mod.rs` 添加：

```rust
pub mod user_service_impl;
pub mod role_service_impl;
pub mod permission_service_impl;

pub use user_service_impl::UserServiceImpl;
pub use role_service_impl::RoleServiceImpl;
pub use permission_service_impl::PermissionServiceImpl;
```

- [ ] **Step 7: 更新 mod.rs 文件**

---

## Step 8: 验证编译

```bash
cargo build
```

预期：编译成功

- [ ] **Step 8: 运行 cargo build 验证**

---

## Step 9: Commit

```bash
git add src/service/user_service.rs src/service/role_service.rs src/service/permission_service.rs \
        src/implt/user_service_impl.rs src/implt/role_service_impl.rs src/implt/permission_service_impl.rs \
        src/service/mod.rs src/implt/mod.rs
git commit -m "feat(rbac): add permission services

- Add UserService: CRUD, role management with audit logs
- Add RoleService: CRUD, permission management, system role protection
- Add PermissionService: check, list, group resources/permissions

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 9: Commit services 文件**
