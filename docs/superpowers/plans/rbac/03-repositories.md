# Task 3: Repositories 层

**Files:**
- Create: `src/repositories/user_repo.rs`
- Create: `src/repositories/role_repo.rs`
- Create: `src/repositories/permission_repo.rs`
- Modify: `src/repositories/mod.rs`

**Goal:** 实现数据访问层，封装所有数据库操作

---

## Step 1: 创建 user_repo.rs

创建文件 `src/repositories/user_repo.rs`：

```rust
use anyhow::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use crate::models::{CreateUserRequest, RoleInfo, UpdateUserRequest, User};

pub struct UserRepo;

impl UserRepo {
    pub async fn insert(
        executor: Executor<'_, Postgres>,
        req: &CreateUserRequest,
        password_hash: &str,
    ) -> Result<i64> {
        let user_id = sqlx::query_scalar!(
            r#"
            INSERT INTO users (username, password_hash, display_name, is_super_admin)
            VALUES ($1, $2, $3, $4)
            RETURNING user_id
            "#,
            req.username,
            password_hash,
            req.display_name,
            req.is_super_admin
        )
        .fetch_one(executor)
        .await?;

        Ok(user_id)
    }

    pub async fn update(
        executor: Executor<'_, Postgres>,
        user_id: i64,
        req: &UpdateUserRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE users SET
                display_name = COALESCE($2, display_name),
                is_active = COALESCE($3, is_active),
                updated_at = NOW()
            WHERE user_id = $1
            "#,
            user_id,
            req.display_name,
            req.is_active
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_, Postgres>, user_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM users WHERE user_id = $1", user_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_id(executor: Executor<'_, Postgres>, user_id: i64) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(user)
    }

    pub async fn find_by_username(executor: Executor<'_, Postgres>, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(executor)
        .await?;

        Ok(user)
    }

    pub async fn list_all(executor: Executor<'_, Postgres>) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            User,
            r#"
            SELECT user_id, username, password_hash, display_name,
                   is_active, is_super_admin, created_at, updated_at
            FROM users
            ORDER BY user_id
            "#
        )
        .fetch_all(executor)
        .await?;

        Ok(users)
    }

    pub async fn get_user_roles(executor: Executor<'_, Postgres>, user_id: i64) -> Result<Vec<RoleInfo>> {
        let roles = sqlx::query_as!(
            RoleInfo,
            r#"
            SELECT r.role_id, r.role_name, r.role_code
            FROM roles r
            JOIN user_roles ur ON r.role_id = ur.role_id
            WHERE ur.user_id = $1
            ORDER BY r.role_id
            "#,
            user_id
        )
        .fetch_all(executor)
        .await?;

        Ok(roles)
    }

    pub async fn assign_roles(
        executor: Executor<'_, Postgres>,
        user_id: i64,
        role_ids: &[i64],
    ) -> Result<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO user_roles (user_id, role_id) "
        );
        builder.push_values(role_ids.iter(), |mut b, role_id| {
            b.push_bind(user_id).push_bind(*role_id);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }

    pub async fn remove_roles(
        executor: Executor<'_, Postgres>,
        user_id: i64,
        role_ids: &[i64],
    ) -> Result<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            "DELETE FROM user_roles WHERE user_id = $1 AND role_id = ANY($2)",
            user_id,
            role_ids
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn batch_assign_roles(
        executor: Executor<'_, Postgres>,
        user_ids: &[i64],
        role_ids: &[i64],
    ) -> Result<()> {
        if user_ids.is_empty() || role_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO user_roles (user_id, role_id) "
        );
        builder.push_values(user_ids.iter().flat_map(|uid| role_ids.iter().map(move |rid| (*uid, *rid))), |mut b, (uid, rid)| {
            b.push_bind(uid).push_bind(rid);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }
}
```

- [ ] **Step 1: 创建 user_repo.rs**

---

## Step 2: 创建 role_repo.rs

创建文件 `src/repositories/role_repo.rs`：

```rust
use anyhow::Result;
use sqlx::{Executor, Postgres, QueryBuilder};

use crate::models::{CreateRoleRequest, PermissionInfo, Role, UpdateRoleRequest};

pub struct RoleRepo;

impl RoleRepo {
    pub async fn insert(
        executor: Executor<'_, Postgres>,
        req: &CreateRoleRequest,
    ) -> Result<i64> {
        let role_id = sqlx::query_scalar!(
            r#"
            INSERT INTO roles (role_name, role_code, description)
            VALUES ($1, $2, $3)
            RETURNING role_id
            "#,
            req.role_name,
            req.role_code,
            req.description
        )
        .fetch_one(executor)
        .await?;

        Ok(role_id)
    }

    pub async fn update(
        executor: Executor<'_, Postgres>,
        role_id: i64,
        req: &UpdateRoleRequest,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE roles SET
                role_name = COALESCE($2, role_name),
                description = COALESCE($3, description),
                updated_at = NOW()
            WHERE role_id = $1
            "#,
            role_id,
            req.role_name,
            req.description
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn delete(executor: Executor<'_, Postgres>, role_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM roles WHERE role_id = $1", role_id)
            .execute(executor)
            .await?;

        Ok(())
    }

    pub async fn find_by_id(executor: Executor<'_, Postgres>, role_id: i64) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   description, created_at, updated_at
            FROM roles
            WHERE role_id = $1
            "#,
            role_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(role)
    }

    pub async fn find_by_code(executor: Executor<'_, Postgres>, role_code: &str) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   description, created_at, updated_at
            FROM roles
            WHERE role_code = $1
            "#,
            role_code
        )
        .fetch_optional(executor)
        .await?;

        Ok(role)
    }

    pub async fn list_all(executor: Executor<'_, Postgres>) -> Result<Vec<Role>> {
        let roles = sqlx::query_as!(
            Role,
            r#"
            SELECT role_id, role_name, role_code, is_system_role,
                   description, created_at, updated_at
            FROM roles
            ORDER BY role_id
            "#
        )
        .fetch_all(executor)
        .await?;

        Ok(roles)
    }

    pub async fn get_role_permissions(
        executor: Executor<'_, Postgres>,
        role_id: i64,
    ) -> Result<Vec<PermissionInfo>> {
        let permissions = sqlx::query_as!(
            PermissionInfo,
            r#"
            SELECT
                p.permission_id,
                p.permission_name,
                r.resource_id as "resource_id!",
                r.resource_name as "resource_name!",
                r.resource_code as "resource_code!",
                r.group_name as "group_name!",
                r.sort_order as "resource_sort_order!",
                r.description as "resource_description!",
                p.action_code,
                a.action_name
            FROM permissions p
            JOIN role_permissions rp ON p.permission_id = rp.permission_id
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            WHERE rp.role_id = $1
            ORDER BY p.sort_order
            "#,
            role_id
        )
        .fetch_all(executor)
        .await?;

        Ok(permissions)
    }

    pub async fn assign_permissions(
        executor: Executor<'_, Postgres>,
        role_id: i64,
        permission_ids: &[i64],
    ) -> Result<()> {
        if permission_ids.is_empty() {
            return Ok(());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO role_permissions (role_id, permission_id) "
        );
        builder.push_values(permission_ids.iter(), |mut b, pid| {
            b.push_bind(role_id).push_bind(*pid);
        });
        builder.push(" ON CONFLICT DO NOTHING");

        builder.build().execute(executor).await?;

        Ok(())
    }

    pub async fn remove_permissions(
        executor: Executor<'_, Postgres>,
        role_id: i64,
        permission_ids: &[i64],
    ) -> Result<()> {
        if permission_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            "DELETE FROM role_permissions WHERE role_id = $1 AND permission_id = ANY($2)",
            role_id,
            permission_ids
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn is_system_role(executor: Executor<'_, Postgres>, role_id: i64) -> Result<bool> {
        let is_system = sqlx::query_scalar!(
            "SELECT is_system_role FROM roles WHERE role_id = $1",
            role_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(is_system.unwrap_or(false))
    }
}
```

- [ ] **Step 2: 创建 role_repo.rs**

---

## Step 3: 创建 permission_repo.rs

创建文件 `src/repositories/permission_repo.rs`：

```rust
use anyhow::Result;
use sqlx::{Executor, Postgres};

use crate::models::{Action, AuditLog, Permission, PermissionInfo, Resource};

pub struct PermissionRepo;

impl PermissionRepo {
    pub async fn list_resources(executor: Executor<'_, Postgres>) -> Result<Vec<Resource>> {
        let resources = sqlx::query_as!(
            Resource,
            r#"
            SELECT resource_id, resource_name, resource_code,
                   group_name, sort_order, description
            FROM resources
            ORDER BY sort_order
            "#
        )
        .fetch_all(executor)
        .await?;

        Ok(resources)
    }

    pub async fn list_actions(executor: Executor<'_, Postgres>) -> Result<Vec<Action>> {
        let actions = sqlx::query_as!(
            Action,
            r#"
            SELECT action_code, action_name, sort_order, description
            FROM actions
            ORDER BY sort_order
            "#
        )
        .fetch_all(executor)
        .await?;

        Ok(actions)
    }

    pub async fn list_permissions(executor: Executor<'_, Postgres>) -> Result<Vec<PermissionInfo>> {
        let permissions = sqlx::query_as!(
            PermissionInfo,
            r#"
            SELECT
                p.permission_id,
                p.permission_name,
                r.resource_id as "resource_id!",
                r.resource_name as "resource_name!",
                r.resource_code as "resource_code!",
                r.group_name as "group_name!",
                r.sort_order as "resource_sort_order!",
                r.description as "resource_description!",
                p.action_code,
                a.action_name
            FROM permissions p
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            ORDER BY p.sort_order
            "#
        )
        .fetch_all(executor)
        .await?;

        Ok(permissions)
    }

    pub async fn get_user_permissions(
        executor: Executor<'_, Postgres>,
        user_id: i64,
    ) -> Result<Vec<PermissionInfo>> {
        let permissions = sqlx::query_as!(
            PermissionInfo,
            r#"
            SELECT DISTINCT
                p.permission_id,
                p.permission_name,
                r.resource_id as "resource_id!",
                r.resource_name as "resource_name!",
                r.resource_code as "resource_code!",
                r.group_name as "group_name!",
                r.sort_order as "resource_sort_order!",
                r.description as "resource_description!",
                p.action_code,
                a.action_name
            FROM user_roles ur
            JOIN role_permissions rp ON ur.role_id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.permission_id
            JOIN resources r ON p.resource_id = r.resource_id
            JOIN actions a ON p.action_code = a.action_code
            WHERE ur.user_id = $1
            ORDER BY p.sort_order
            "#,
            user_id
        )
        .fetch_all(executor)
        .await?;

        Ok(permissions)
    }

    pub async fn check_permission(
        executor: Executor<'_, Postgres>,
        user_id: i64,
        resource_code: &str,
        action_code: &str,
    ) -> Result<bool> {
        // 1. 检查是否超级管理员
        let is_super = sqlx::query_scalar!(
            "SELECT is_super_admin FROM users WHERE user_id = $1",
            user_id
        )
        .fetch_optional(executor)
        .await?;

        if is_super.unwrap_or(false) {
            return Ok(true);
        }

        // 2. 检查用户角色是否有此权限
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
        .fetch_one(executor)
        .await?;

        Ok(has_permission.unwrap_or(false))
    }

    pub async fn insert_audit_log(
        executor: Executor<'_, Postgres>,
        operator_id: i64,
        target_type: &str,
        target_id: i64,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO permission_audit_logs
                (operator_id, target_type, target_id, action, old_value, new_value)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            operator_id,
            target_type,
            target_id,
            action,
            old_value,
            new_value
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn list_audit_logs(
        executor: Executor<'_, Postgres>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as!(
            AuditLog,
            r#"
            SELECT
                l.log_id,
                l.operator_id,
                u.display_name as operator_name,
                l.target_type,
                l.target_id,
                l.action,
                l.old_value,
                l.new_value,
                l.created_at
            FROM permission_audit_logs l
            LEFT JOIN users u ON l.operator_id = u.user_id
            ORDER BY l.created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(executor)
        .await?;

        Ok(logs)
    }

    pub async fn find_permission_by_id(
        executor: Executor<'_, Postgres>,
        permission_id: i64,
    ) -> Result<Option<Permission>> {
        let permission = sqlx::query_as!(
            Permission,
            r#"
            SELECT permission_id, permission_name, resource_id,
                   action_code, sort_order, description
            FROM permissions
            WHERE permission_id = $1
            "#,
            permission_id
        )
        .fetch_optional(executor)
        .await?;

        Ok(permission)
    }
}
```

- [ ] **Step 3: 创建 permission_repo.rs**

---

## Step 4: 更新 mod.rs

在 `src/repositories/mod.rs` 添加：

```rust
pub mod user_repo;
pub mod role_repo;
pub mod permission_repo;

pub use user_repo::UserRepo;
pub use role_repo::RoleRepo;
pub use permission_repo::PermissionRepo;
```

- [ ] **Step 4: 更新 mod.rs 导出新模块**

---

## Step 5: 验证编译

```bash
cargo build
```

预期：编译成功

- [ ] **Step 5: 运行 cargo build 验证**

---

## Step 6: Commit

```bash
git add src/repositories/user_repo.rs src/repositories/role_repo.rs src/repositories/permission_repo.rs src/repositories/mod.rs
git commit -m "feat(rbac): add permission repositories

- Add UserRepo: CRUD, role assignment, batch operations
- Add RoleRepo: CRUD, permission assignment, system role check
- Add PermissionRepo: check permission, user permissions, audit logs

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 6: Commit repositories 文件**
